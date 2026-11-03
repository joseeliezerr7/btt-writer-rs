//! Respaldos automáticos: cada proyecto se exporta a `backups/{id}.tstudio`
//! cuando su commit cambió desde el último respaldo.

use crate::gitrepo::{self, Author};
use crate::project::Project;
use crate::{tstudio, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Estado persistente: último commit respaldado por proyecto.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BackupState {
    pub last_hash: BTreeMap<String, String>,
    pub last_time: BTreeMap<String, u64>,
}

impl BackupState {
    pub fn load(path: &Path) -> BackupState {
        std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        crate::project::atomic_write(path, &serde_json::to_string_pretty(self)?)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupReport {
    pub backed_up: Vec<String>,
    pub skipped: Vec<String>,
    pub failed: Vec<(String, String)>,
}

/// Ejecuta una pasada de respaldo sobre todos los proyectos.
pub fn run(translations_dir: &Path, backups_dir: &Path, state: &mut BackupState, author: &Author, build: &str) -> Result<BackupReport> {
    std::fs::create_dir_all(backups_dir)?;
    let mut report = BackupReport { backed_up: vec![], skipped: vec![], failed: vec![] };
    for project in Project::list(translations_dir)? {
        let id = project.id();
        match backup_project(&project, backups_dir, state, author, build) {
            Ok(true) => report.backed_up.push(id),
            Ok(false) => report.skipped.push(id),
            Err(e) => report.failed.push((id, e.to_string())),
        }
    }
    Ok(report)
}

/// Respalda un proyecto si cambió. Devuelve `true` si se escribió un archivo.
pub fn backup_project(project: &Project, backups_dir: &Path, state: &mut BackupState, author: &Author, build: &str) -> Result<bool> {
    let id = project.id();
    gitrepo::commit_all(&project.root, author, gitrepo::AUTOSAVE_MESSAGE)?;
    let hash = gitrepo::head_hash(&project.root).unwrap_or_default();
    let dest = backups_dir.join(format!("{id}.tstudio"));
    if dest.is_file() && state.last_hash.get(&id) == Some(&hash) {
        return Ok(false);
    }
    tstudio::export(&[project], &dest, author, build)?;
    state.last_hash.insert(id.clone(), hash);
    state.last_time.insert(
        id,
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
    );
    Ok(true)
}

/// Lista de respaldos disponibles (`.tstudio` y `.zip`) con su fecha.
pub fn list(backups_dir: &Path) -> Result<Vec<(String, u64)>> {
    let mut out = vec![];
    if !backups_dir.is_dir() {
        return Ok(out);
    }
    for e in std::fs::read_dir(backups_dir)? {
        let e = e?;
        let name = e.file_name().to_string_lossy().to_string();
        if name.ends_with(".tstudio") || name.ends_with(".zip") {
            let t = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
            out.push((name, t));
        }
    }
    out.sort_by_key(|e| std::cmp::Reverse(e.1));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{IdName, Manifest, TargetLanguage};

    #[test]
    fn backs_up_only_when_changed() {
        let tmp = tempfile::tempdir().unwrap();
        let tr = tmp.path().join("translations");
        let bk = tmp.path().join("backups");
        std::fs::create_dir_all(&tr).unwrap();
        let m = Manifest::new(
            TargetLanguage { id: "zz".into(), name: "Zz".into(), ..Default::default() },
            IdName::new("jud", "Jude"),
            IdName::new("text", "Text"),
            IdName::new("reg", "Regular"),
            "1",
        );
        let p = Project::create(&tr, m).unwrap();
        p.write_chunk("01", "01", "a").unwrap();
        let author = Author::new("T", "t@t");
        let mut state = BackupState::default();
        let r = run(&tr, &bk, &mut state, &author, "3").unwrap();
        assert_eq!(r.backed_up, vec!["zz_jud_text_reg"]);
        assert!(bk.join("zz_jud_text_reg.tstudio").is_file());
        let r = run(&tr, &bk, &mut state, &author, "3").unwrap();
        assert_eq!(r.skipped, vec!["zz_jud_text_reg"]);
        p.write_chunk("01", "01", "b").unwrap();
        let r = run(&tr, &bk, &mut state, &author, "3").unwrap();
        assert_eq!(r.backed_up, vec!["zz_jud_text_reg"]);
        let state_path = tmp.path().join("state.json");
        state.save(&state_path).unwrap();
        assert_eq!(BackupState::load(&state_path).last_hash.len(), 1);
        assert_eq!(list(&bk).unwrap()[0].0, "zz_jud_text_reg.tstudio");
    }
}
