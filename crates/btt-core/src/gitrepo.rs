//! Historial y sincronización con git (libgit2 vía `git2`).
//!
//! Cada proyecto es un repositorio git. Se hace commit automático
//! ("auto save") y se sincroniza con WACS/DCS por HTTPS usando el token del
//! usuario como contraseña (no requiere claves SSH ni binario `git`).

use crate::manifest::Manifest;
use crate::{Error, Result};
use git2::{
    build::RepoBuilder, BranchType, Cred, FetchOptions, IndexAddOption, Oid, PushOptions,
    RemoteCallbacks, Repository, RepositoryInitOptions, ResetType, Signature,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const DEFAULT_BRANCH: &str = "master";
pub const BACKUP_BRANCH: &str = "backup-master";
pub const AUTOSAVE_MESSAGE: &str = "auto save";

#[derive(Debug, Clone)]
pub struct Author {
    pub name: String,
    pub email: String,
}

impl Author {
    pub fn new(name: &str, email: &str) -> Self {
        let name = if name.trim().is_empty() { "BTT Writer" } else { name.trim() };
        let email = if email.trim().is_empty() { "btt-writer@localhost" } else { email.trim() };
        Author { name: name.into(), email: email.into() }
    }
    fn signature(&self) -> Result<Signature<'static>> {
        Ok(Signature::now(&self.name, &self.email)?)
    }
}

#[derive(Debug, Clone)]
pub struct Credentials {
    pub username: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Commit {
    pub id: String,
    pub time: i64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PushStatus {
    Ok,
    UpToDate,
    RejectedNonFastForward,
    Rejected { reason: String },
    AuthFailure,
    NoRemoteRepo,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum MergeStatus {
    UpToDate,
    FastForward,
    Merged,
    /// Archivos con marcadores de conflicto (rutas relativas).
    Conflicts { files: Vec<String> },
    RemoteEmpty,
    AuthFailure,
    NoRemoteRepo,
}

/// Abre el repo del proyecto o lo inicializa con rama `master`.
pub fn open_or_init(path: &Path) -> Result<Repository> {
    match Repository::open(path) {
        Ok(r) => Ok(r),
        Err(_) => {
            let mut opts = RepositoryInitOptions::new();
            opts.initial_head(DEFAULT_BRANCH);
            Ok(Repository::init_opts(path, &opts)?)
        }
    }
}

/// Borra archivos `.lock` que quedan tras un cierre abrupto.
pub fn unlock(path: &Path) {
    let git = path.join(".git");
    let _ = std::fs::remove_file(git.join("index.lock"));
    let _ = std::fs::remove_file(git.join("HEAD.lock"));
    if let Ok(rd) = std::fs::read_dir(git.join("refs/heads")) {
        for e in rd.flatten() {
            if e.path().extension().map(|x| x == "lock").unwrap_or(false) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

fn head_commit(repo: &Repository) -> Option<git2::Commit<'_>> {
    repo.head().ok().and_then(|h| h.peel_to_commit().ok())
}

/// Añade todos los cambios y hace commit. Devuelve `None` si no había cambios.
pub fn commit_all(path: &Path, author: &Author, message: &str) -> Result<Option<String>> {
    unlock(path);
    let repo = open_or_init(path)?;
    let mut index = repo.index()?;
    index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)?;
    index.update_all(["*"].iter(), None)?;
    index.write()?;
    let tree_id = index.write_tree()?;
    let head = head_commit(&repo);
    if let Some(h) = &head {
        if h.tree_id() == tree_id && repo.state() == git2::RepositoryState::Clean {
            return Ok(None);
        }
    }
    let sig = author.signature()?;
    let tree = repo.find_tree(tree_id)?;
    let parents: Vec<&git2::Commit> = head.iter().collect();
    let oid = repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)?;
    repo.cleanup_state()?;
    Ok(Some(oid.to_string()))
}

pub fn head_hash(path: &Path) -> Option<String> {
    let repo = Repository::open(path).ok()?;
    let oid = repo.head().ok()?.target()?;
    Some(oid.to_string())
}

/// Commits que modificaron un archivo (más reciente primero).
pub fn file_history(path: &Path, file: &str) -> Result<Vec<Commit>> {
    let repo = open_or_init(path)?;
    let mut out = vec![];
    if head_commit(&repo).is_none() {
        return Ok(out);
    }
    let mut walk = repo.revwalk()?;
    walk.push_head()?;
    walk.set_sorting(git2::Sort::TIME)?;
    let blob_of = |c: &git2::Commit| -> Option<Oid> {
        c.tree().ok().and_then(|t| t.get_path(Path::new(file)).ok()).map(|e| e.id())
    };
    for oid in walk {
        let commit = repo.find_commit(oid?)?;
        let mine = blob_of(&commit);
        let parent_blob = commit.parent(0).ok().and_then(|p| blob_of(&p));
        if mine.is_some() && mine != parent_blob {
            out.push(Commit {
                id: commit.id().to_string(),
                time: commit.time().seconds(),
                message: commit.summary().unwrap_or("").to_string(),
            });
        }
    }
    Ok(out)
}

/// Contenido de un archivo en un commit concreto.
pub fn file_at(path: &Path, commit: &str, file: &str) -> Result<String> {
    let repo = Repository::open(path)?;
    let commit = repo.find_commit(Oid::from_str(commit)?)?;
    let entry = commit.tree()?.get_path(Path::new(file))?;
    let blob = repo.find_blob(entry.id())?;
    Ok(String::from_utf8_lossy(blob.content()).into_owned())
}

fn callbacks(creds: Option<&Credentials>) -> RemoteCallbacks<'static> {
    let creds = creds.cloned();
    let mut cb = RemoteCallbacks::new();
    let attempts = std::cell::Cell::new(0u32);
    cb.credentials(move |_url, username_from_url, _allowed| {
        attempts.set(attempts.get() + 1);
        if attempts.get() > 2 {
            return Err(git2::Error::from_str("credenciales rechazadas"));
        }
        match &creds {
            Some(c) => Cred::userpass_plaintext(
                if c.username.is_empty() { username_from_url.unwrap_or("git") } else { &c.username },
                &c.token,
            ),
            None => Err(git2::Error::from_str("sin credenciales")),
        }
    });
    cb
}

fn classify(e: &git2::Error) -> Option<MergeStatus> {
    let msg = e.message().to_lowercase();
    if e.code() == git2::ErrorCode::Auth
        || msg.contains("401")
        || msg.contains("authentication")
        || msg.contains("credenciales")
    {
        return Some(MergeStatus::AuthFailure);
    }
    if msg.contains("404") || msg.contains("not found") || msg.contains("repository not found") {
        return Some(MergeStatus::NoRemoteRepo);
    }
    None
}

fn set_remote(repo: &Repository, name: &str, url: &str) -> Result<()> {
    match repo.find_remote(name) {
        Ok(_) => repo.remote_set_url(name, url)?,
        Err(_) => {
            repo.remote(name, url)?;
        }
    }
    Ok(())
}

/// Crea/actualiza la rama de respaldo apuntando a HEAD.
pub fn create_backup_branch(path: &Path) -> Result<()> {
    let repo = open_or_init(path)?;
    if let Some(c) = head_commit(&repo) {
        repo.branch(BACKUP_BRANCH, &c, true)?;
    }
    Ok(())
}

/// `git reset --hard <branch>`.
pub fn reset_hard_to_branch(path: &Path, branch: &str) -> Result<()> {
    let repo = open_or_init(path)?;
    let b = repo.find_branch(branch, BranchType::Local)?;
    let commit = b.get().peel_to_commit()?;
    repo.reset(commit.as_object(), ResetType::Hard, None)?;
    repo.cleanup_state()?;
    Ok(())
}

/// Sube `master` a `url`. El repo remoto debe existir.
pub fn push(path: &Path, url: &str, creds: &Credentials) -> Result<PushStatus> {
    let repo = open_or_init(path)?;
    if head_commit(&repo).is_none() {
        return Ok(PushStatus::UpToDate);
    }
    set_remote(&repo, "origin", url)?;
    let mut remote = repo.find_remote("origin")?;
    let rejection = std::rc::Rc::new(std::cell::RefCell::new(None::<String>));
    let mut cb = callbacks(Some(creds));
    let rej = rejection.clone();
    cb.push_update_reference(move |_ref, status| {
        if let Some(s) = status {
            *rej.borrow_mut() = Some(s.to_string());
        }
        Ok(())
    });
    let mut opts = PushOptions::new();
    opts.remote_callbacks(cb);
    let refspec = format!("refs/heads/{DEFAULT_BRANCH}:refs/heads/{DEFAULT_BRANCH}");
    match remote.push(&[refspec.as_str()], Some(&mut opts)) {
        Ok(()) => {}
        Err(e) => {
            return Ok(match classify(&e) {
                Some(MergeStatus::AuthFailure) => PushStatus::AuthFailure,
                Some(MergeStatus::NoRemoteRepo) => PushStatus::NoRemoteRepo,
                _ => {
                    let m = e.message().to_lowercase();
                    if m.contains("non-fast-forward") || m.contains("fetch first") || m.contains("rejected") {
                        PushStatus::RejectedNonFastForward
                    } else {
                        return Err(e.into());
                    }
                }
            });
        }
    }
    let rejection = rejection.borrow().clone();
    Ok(match rejection {
        None => PushStatus::Ok,
        Some(r) if r.to_lowercase().contains("non-fast-forward") || r.to_lowercase().contains("fetch first") => {
            PushStatus::RejectedNonFastForward
        }
        Some(r) => PushStatus::Rejected { reason: r },
    })
}

/// Descarga `master` del remoto y lo fusiona en el proyecto.
/// Los conflictos en `manifest.json` se resuelven uniendo ambos; `LICENSE.md`
/// conserva el local; el resto queda con marcadores para resolver en la app.
pub fn pull(path: &Path, url: &str, creds: Option<&Credentials>, author: &Author) -> Result<MergeStatus> {
    commit_all(path, author, AUTOSAVE_MESSAGE)?;
    create_backup_branch(path)?;
    let repo = open_or_init(path)?;
    set_remote(&repo, "origin", url)?;
    fetch_and_merge(&repo, "origin", creds, author)
}

/// Fusiona otro proyecto local (p. ej. uno importado) en este.
pub fn merge_local(path: &Path, other: &Path, author: &Author) -> Result<MergeStatus> {
    commit_all(path, author, AUTOSAVE_MESSAGE)?;
    commit_all(other, author, AUTOSAVE_MESSAGE)?;
    create_backup_branch(path)?;
    let repo = open_or_init(path)?;
    let url = other.to_string_lossy().replace('\\', "/");
    set_remote(&repo, "new", &url)?;
    let r = fetch_and_merge(&repo, "new", None, author);
    let _ = repo.remote_delete("new");
    r
}

fn fetch_and_merge(repo: &Repository, remote_name: &str, creds: Option<&Credentials>, author: &Author) -> Result<MergeStatus> {
    let mut remote = repo.find_remote(remote_name)?;
    let mut fo = FetchOptions::new();
    fo.remote_callbacks(callbacks(creds));
    let refspec = format!("+refs/heads/{DEFAULT_BRANCH}:refs/remotes/{remote_name}/{DEFAULT_BRANCH}");
    if let Err(e) = remote.fetch(&[refspec.as_str()], Some(&mut fo), None) {
        if let Some(s) = classify(&e) {
            return Ok(s);
        }
        let m = e.message().to_lowercase();
        if m.contains("couldn't find remote ref") || m.contains("empty") {
            return Ok(MergeStatus::RemoteEmpty);
        }
        return Err(e.into());
    }
    let remote_ref = match repo.find_reference(&format!("refs/remotes/{remote_name}/{DEFAULT_BRANCH}")) {
        Ok(r) => r,
        Err(_) => return Ok(MergeStatus::RemoteEmpty),
    };
    let remote_commit = remote_ref.peel_to_commit()?;
    let annotated = repo.reference_to_annotated_commit(&remote_ref)?;

    let Some(local) = head_commit(repo) else {
        // Repo local sin commits: adoptar el remoto.
        repo.reference(&format!("refs/heads/{DEFAULT_BRANCH}"), remote_commit.id(), true, "fast-forward")?;
        repo.set_head(&format!("refs/heads/{DEFAULT_BRANCH}"))?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))?;
        return Ok(MergeStatus::FastForward);
    };

    let (analysis, _) = repo.merge_analysis(&[&annotated])?;
    if analysis.is_up_to_date() {
        return Ok(MergeStatus::UpToDate);
    }
    if analysis.is_fast_forward() {
        let mut r = repo.find_reference(&format!("refs/heads/{DEFAULT_BRANCH}"))?;
        r.set_target(remote_commit.id(), "fast-forward")?;
        repo.set_head(&format!("refs/heads/{DEFAULT_BRANCH}"))?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))?;
        return Ok(MergeStatus::FastForward);
    }

    // Merge real.
    let mut checkout = git2::build::CheckoutBuilder::new();
    checkout.allow_conflicts(true).conflict_style_merge(true).force();
    repo.merge(&[&annotated], None, Some(&mut checkout))?;
    let mut index = repo.index()?;
    let mut conflicted: Vec<String> = vec![];
    if index.has_conflicts() {
        let paths: Vec<String> = index
            .conflicts()?
            .flatten()
            .filter_map(|c| c.our.or(c.their).or(c.ancestor))
            .map(|e| String::from_utf8_lossy(&e.path).into_owned())
            .collect();
        let workdir = repo.workdir().ok_or_else(|| Error::Other("repo sin directorio de trabajo".into()))?;
        for p in paths {
            let full = workdir.join(&p);
            if p == "manifest.json" {
                let ours = blob_text(repo, &local, &p).and_then(|t| Manifest::parse(&t).ok());
                let theirs = blob_text(repo, &remote_commit, &p).and_then(|t| Manifest::parse(&t).ok());
                let merged = match (ours, theirs) {
                    (Some(mut o), Some(t)) => {
                        o.merge_from(&t);
                        Some(o.to_json())
                    }
                    (Some(o), None) => Some(o.to_json()),
                    (None, Some(t)) => Some(t.to_json()),
                    _ => None,
                };
                if let Some(m) = merged {
                    std::fs::write(&full, m)?;
                }
            } else if p == "LICENSE.md" {
                if let Some(ours) = blob_text(repo, &local, &p) {
                    std::fs::write(&full, ours)?;
                }
            } else {
                conflicted.push(p.clone());
            }
            index.remove_path(Path::new(&p))?;
            if full.is_file() {
                index.add_path(Path::new(&p))?;
            }
        }
        index.write()?;
    }
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let sig = author.signature()?;
    repo.commit(Some("HEAD"), &sig, &sig, "merge", &tree, &[&local, &remote_commit])?;
    repo.cleanup_state()?;
    if conflicted.is_empty() {
        Ok(MergeStatus::Merged)
    } else {
        conflicted.sort();
        Ok(MergeStatus::Conflicts { files: conflicted })
    }
}

fn blob_text(repo: &Repository, commit: &git2::Commit, path: &str) -> Option<String> {
    let entry = commit.tree().ok()?.get_path(Path::new(path)).ok()?;
    let blob = repo.find_blob(entry.id()).ok()?;
    Some(String::from_utf8_lossy(blob.content()).into_owned())
}

/// Clona un repositorio remoto en `dest`.
pub fn clone(url: &str, creds: Option<&Credentials>, dest: &Path) -> Result<()> {
    let mut fo = FetchOptions::new();
    fo.remote_callbacks(callbacks(creds));
    let mut builder = RepoBuilder::new();
    builder.fetch_options(fo);
    match builder.clone(url, dest) {
        Ok(_) => Ok(()),
        Err(e) => match classify(&e) {
            Some(MergeStatus::AuthFailure) => Err(Error::Unauthorized),
            Some(MergeStatus::NoRemoteRepo) => Err(Error::Other(format!("el repositorio {url} no existe"))),
            _ => Err(e.into()),
        },
    }
}

/// Archivos del árbol de trabajo que contienen marcadores de conflicto.
pub fn conflicted_files(path: &Path) -> Result<Vec<String>> {
    let mut out = vec![];
    for entry in walkdir::WalkDir::new(path).into_iter().filter_entry(|e| e.file_name() != ".git") {
        let entry = entry.map_err(|e| Error::Other(e.to_string()))?;
        if entry.file_type().is_file() && entry.path().extension().map(|x| x == "txt").unwrap_or(false) {
            if let Ok(t) = std::fs::read_to_string(entry.path()) {
                if crate::conflicts::has_conflicts(&t) {
                    let rel = entry.path().strip_prefix(path).unwrap_or(entry.path());
                    out.push(rel.to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }
    out.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{IdName, TargetLanguage};
    use crate::project::Project;

    fn author() -> Author {
        Author::new("Tester", "t@example.com")
    }

    fn new_project(dir: &Path, lang: &str) -> Project {
        let m = Manifest::new(
            TargetLanguage { id: lang.into(), name: "L".into(), ..Default::default() },
            IdName::new("jud", "Jude"),
            IdName::new("text", "Text"),
            IdName::new("reg", "Regular"),
            "1",
        );
        Project::create(dir, m).unwrap()
    }

    #[test]
    fn commit_history_and_file_at() {
        let tmp = tempfile::tempdir().unwrap();
        let p = new_project(tmp.path(), "xa");
        let first = commit_all(&p.root, &author(), "init").unwrap();
        assert!(first.is_some());
        assert!(commit_all(&p.root, &author(), "nada").unwrap().is_none());
        p.write_chunk("01", "01", "v1").unwrap();
        let c1 = commit_all(&p.root, &author(), AUTOSAVE_MESSAGE).unwrap().unwrap();
        p.write_chunk("01", "01", "v2").unwrap();
        let c2 = commit_all(&p.root, &author(), AUTOSAVE_MESSAGE).unwrap().unwrap();
        assert_eq!(head_hash(&p.root).unwrap(), c2);
        let hist = file_history(&p.root, "01/01.txt").unwrap();
        assert_eq!(hist.iter().map(|c| c.id.clone()).collect::<Vec<_>>(), vec![c2.clone(), c1.clone()]);
        assert_eq!(file_at(&p.root, &c1, "01/01.txt").unwrap(), "v1");
        assert_eq!(file_at(&p.root, &c2, "01/01.txt").unwrap(), "v2");
    }

    #[test]
    fn merge_local_resolves_manifest_and_flags_content_conflicts() {
        let tmp = tempfile::tempdir().unwrap();
        let a_dir = tmp.path().join("a");
        let b_dir = tmp.path().join("b");
        std::fs::create_dir_all(&a_dir).unwrap();
        std::fs::create_dir_all(&b_dir).unwrap();
        let a = new_project(&a_dir, "xa");
        a.write_chunk("01", "01", "\\v 1 base").unwrap();
        a.write_chunk("01", "05", "\\v 5 cinco").unwrap();
        commit_all(&a.root, &author(), "init").unwrap();
        // b = copia de a (clon local) con cambios divergentes.
        clone(&a.root.to_string_lossy().replace('\\', "/"), None, &b_dir.join("xa_jud_text_reg")).unwrap();
        let mut b = Project::open(&b_dir.join("xa_jud_text_reg")).unwrap();
        b.write_chunk("01", "01", "\\v 1 remoto").unwrap();
        b.write_chunk("01", "09", "\\v 9 nueve").unwrap();
        b.manifest.add_translator("Remoto");
        b.set_chunk_finished("01", "09", true).unwrap();
        commit_all(&b.root, &author(), "remote work").unwrap();
        let mut a = Project::open(&a.root).unwrap();
        a.write_chunk("01", "01", "\\v 1 local").unwrap();
        a.manifest.add_translator("Local");
        a.set_chunk_finished("01", "05", true).unwrap();

        let status = merge_local(&a.root, &b.root, &author()).unwrap();
        assert_eq!(status, MergeStatus::Conflicts { files: vec!["01/01.txt".into()] });
        let merged = Project::open(&a.root).unwrap();
        assert_eq!(merged.manifest.translators, vec!["Local", "Remoto"]);
        assert_eq!(merged.manifest.finished_chunks, vec!["01-05", "01-09"]);
        assert_eq!(merged.read_chunk("01", "09").unwrap(), "\\v 9 nueve");
        let text = merged.read_chunk("01", "01").unwrap();
        assert!(crate::conflicts::has_conflicts(&text), "{text}");
        let options: Vec<String> = crate::conflicts::parse(&text).into_iter().map(|s| s.trim().to_string()).collect();
        assert!(
            options.contains(&"\\v 1 local".to_string()) && options.contains(&"\\v 1 remoto".to_string()),
            "{text}\n{options:?}"
        );
        assert_eq!(conflicted_files(&a.root).unwrap(), vec!["01/01.txt"]);
        // El estado del repo quedó limpio y la rama de respaldo existe.
        let repo = Repository::open(&a.root).unwrap();
        assert_eq!(repo.state(), git2::RepositoryState::Clean);
        assert!(repo.find_branch(BACKUP_BRANCH, BranchType::Local).is_ok());
        // Deshacer con la rama de respaldo.
        reset_hard_to_branch(&a.root, BACKUP_BRANCH).unwrap();
        assert_eq!(Project::open(&a.root).unwrap().read_chunk("01", "01").unwrap(), "\\v 1 local");
    }

    #[test]
    fn merge_fast_forward_when_no_local_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let a_dir = tmp.path().join("a");
        std::fs::create_dir_all(&a_dir).unwrap();
        let a = new_project(&a_dir, "xb");
        commit_all(&a.root, &author(), "init").unwrap();
        let b_root = tmp.path().join("b");
        clone(&a.root.to_string_lossy().replace('\\', "/"), None, &b_root).unwrap();
        let b = Project::open(&b_root).unwrap();
        b.write_chunk("01", "01", "x").unwrap();
        commit_all(&b.root, &author(), "w").unwrap();
        assert_eq!(merge_local(&a.root, &b.root, &author()).unwrap(), MergeStatus::FastForward);
        assert_eq!(Project::open(&a.root).unwrap().read_chunk("01", "01").unwrap(), "x");
        assert_eq!(merge_local(&a.root, &b.root, &author()).unwrap(), MergeStatus::UpToDate);
    }

    #[test]
    fn push_to_local_bare_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let bare = tmp.path().join("remote.git");
        Repository::init_bare(&bare).unwrap();
        let p = new_project(tmp.path(), "xc");
        commit_all(&p.root, &author(), "init").unwrap();
        let url = bare.to_string_lossy().replace('\\', "/");
        let creds = Credentials { username: "u".into(), token: "t".into() };
        assert_eq!(push(&p.root, &url, &creds).unwrap(), PushStatus::Ok);
        let remote = Repository::open_bare(&bare).unwrap();
        assert!(remote.find_reference("refs/heads/master").is_ok());
    }
}
