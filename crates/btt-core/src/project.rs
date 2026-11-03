//! Proyecto (traducción objetivo) almacenado en disco.
//!
//! Estructura, idéntica a la de BTT Writer 1.6 / 2.0:
//!
//! ```text
//! translations/
//!   cbt_3jn_text_reg/
//!     manifest.json
//!     LICENSE.md
//!     front/title.txt         ← título del libro
//!     01/title.txt            ← título del capítulo
//!     01/reference.txt        ← referencia (OBS)
//!     01/01.txt               ← chunk (USFM parcial: \v 1 ... \v 4 ...)
//!     01/05.txt
//!     .git/                   ← historial automático
//! ```

use crate::manifest::Manifest;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const MANIFEST_FILE: &str = "manifest.json";
pub const FRONT_DIR: &str = "front";
pub const TITLE_FILE: &str = "title.txt";
pub const REFERENCE_FILE: &str = "reference.txt";

pub const LICENSE_MD: &str = include_str!("../assets/LICENSE.md");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkId {
    pub chapter: String,
    pub chunk: String,
}

impl ChunkId {
    pub fn new(chapter: &str, chunk: &str) -> Self {
        ChunkId { chapter: chapter.into(), chunk: chunk.into() }
    }
    pub fn key(&self) -> String {
        format!("{}-{}", self.chapter, self.chunk)
    }
    pub fn is_title(&self) -> bool {
        self.chunk == "title"
    }
    pub fn is_reference(&self) -> bool {
        self.chunk == "reference"
    }
}

/// Resumen ligero para listados.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub id: String,
    pub path: String,
    pub language_id: String,
    pub language_name: String,
    pub language_direction: String,
    pub project_id: String,
    pub project_name: String,
    pub type_id: String,
    pub resource_id: String,
    pub resource_name: String,
    pub translators: Vec<String>,
    pub finished_chunks: usize,
    pub translated_chunks: usize,
    pub modified_at: u64,
}

#[derive(Debug, Clone)]
pub struct Project {
    pub root: PathBuf,
    pub manifest: Manifest,
}

impl Project {
    /// Crea un proyecto nuevo dentro de `translations_dir`.
    pub fn create(translations_dir: &Path, manifest: Manifest) -> Result<Project> {
        let id = manifest.unique_id();
        let root = translations_dir.join(&id);
        if root.exists() {
            return Err(Error::ProjectExists(id));
        }
        fs::create_dir_all(root.join(FRONT_DIR))?;
        let project = Project { root: root.clone(), manifest };
        project.save_manifest()?;
        atomic_write(&root.join("LICENSE.md"), LICENSE_MD)?;
        atomic_write(&root.join(FRONT_DIR).join(TITLE_FILE), "")?;
        Ok(project)
    }

    pub fn open(root: &Path) -> Result<Project> {
        let manifest_path = root.join(MANIFEST_FILE);
        if !manifest_path.is_file() {
            return Err(Error::ProjectNotFound(root.display().to_string()));
        }
        let json = fs::read_to_string(&manifest_path)?;
        let manifest = Manifest::parse(&json)?;
        Ok(Project { root: root.to_path_buf(), manifest })
    }

    pub fn open_by_id(translations_dir: &Path, id: &str) -> Result<Project> {
        Project::open(&translations_dir.join(id))
    }

    /// Lista todos los proyectos válidos (ignora carpetas sin manifest).
    pub fn list(translations_dir: &Path) -> Result<Vec<Project>> {
        let mut out = vec![];
        if !translations_dir.is_dir() {
            return Ok(out);
        }
        for entry in fs::read_dir(translations_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            if let Ok(p) = Project::open(&entry.path()) {
                out.push(p);
            }
        }
        out.sort_by_key(|p| std::cmp::Reverse(p.modified_at()));
        Ok(out)
    }

    pub fn id(&self) -> String {
        self.manifest.unique_id()
    }

    pub fn save_manifest(&self) -> Result<()> {
        atomic_write(&self.root.join(MANIFEST_FILE), &self.manifest.to_json())
    }

    /// Momento de la última modificación (segundos UNIX) considerando los chunks.
    pub fn modified_at(&self) -> u64 {
        let mut latest = mtime(&self.root.join(MANIFEST_FILE));
        if let Ok(rd) = fs::read_dir(&self.root) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() && p.file_name().map(|n| n != ".git").unwrap_or(false) {
                    if let Ok(files) = fs::read_dir(&p) {
                        for f in files.flatten() {
                            latest = latest.max(mtime(&f.path()));
                        }
                    }
                }
            }
        }
        latest
    }

    /// Capítulos existentes en disco (ordenados), sin `front`.
    pub fn chapters(&self) -> Result<Vec<String>> {
        let mut out = vec![];
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.file_type()?.is_dir() && name != ".git" && name != FRONT_DIR {
                out.push(name);
            }
        }
        out.sort_by(|a, b| natural_cmp(a, b));
        Ok(out)
    }

    /// Chunks de un capítulo existentes en disco, ordenados; `title` y
    /// `reference` van primero.
    pub fn chunks(&self, chapter: &str) -> Result<Vec<String>> {
        let dir = self.root.join(chapter);
        let mut out = vec![];
        if !dir.is_dir() {
            return Ok(out);
        }
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(stem) = name.strip_suffix(".txt") {
                out.push(stem.to_string());
            }
        }
        out.sort_by(|a, b| chunk_cmp(a, b));
        Ok(out)
    }

    /// Todos los chunks del proyecto en orden canónico.
    pub fn all_chunks(&self) -> Result<Vec<ChunkId>> {
        let mut out = vec![];
        if self.root.join(FRONT_DIR).is_dir() {
            for c in self.chunks(FRONT_DIR)? {
                out.push(ChunkId::new(FRONT_DIR, &c));
            }
        }
        for ch in self.chapters()? {
            for c in self.chunks(&ch)? {
                out.push(ChunkId::new(&ch, &c));
            }
        }
        Ok(out)
    }

    fn chunk_path(&self, chapter: &str, chunk: &str) -> Result<PathBuf> {
        validate_segment(chapter)?;
        validate_segment(chunk)?;
        Ok(self.root.join(chapter).join(format!("{chunk}.txt")))
    }

    pub fn read_chunk(&self, chapter: &str, chunk: &str) -> Result<String> {
        let path = self.chunk_path(chapter, chunk)?;
        match fs::read_to_string(&path) {
            Ok(s) => Ok(s),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn chunk_exists(&self, chapter: &str, chunk: &str) -> bool {
        self.chunk_path(chapter, chunk).map(|p| p.is_file()).unwrap_or(false)
    }

    /// Escribe un chunk. Crea el capítulo si no existe. Un texto vacío
    /// mantiene el archivo (vacío) para conservar la estructura.
    pub fn write_chunk(&self, chapter: &str, chunk: &str, text: &str) -> Result<()> {
        let path = self.chunk_path(chapter, chunk)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        atomic_write(&path, text)
    }

    pub fn delete_chunk(&self, chapter: &str, chunk: &str) -> Result<()> {
        let path = self.chunk_path(chapter, chunk)?;
        if path.is_file() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    pub fn read_book_title(&self) -> Result<String> {
        self.read_chunk(FRONT_DIR, "title")
    }
    pub fn write_book_title(&self, title: &str) -> Result<()> {
        self.write_chunk(FRONT_DIR, "title", title)
    }
    pub fn read_chapter_title(&self, chapter: &str) -> Result<String> {
        self.read_chunk(chapter, "title")
    }
    pub fn read_chapter_reference(&self, chapter: &str) -> Result<String> {
        self.read_chunk(chapter, "reference")
    }

    pub fn set_chunk_finished(&mut self, chapter: &str, chunk: &str, finished: bool) -> Result<()> {
        self.manifest.set_chunk_finished(chapter, chunk, finished);
        self.save_manifest()
    }

    /// Número de chunks con texto no vacío (excluye `front`).
    pub fn translated_chunk_count(&self) -> Result<usize> {
        let mut n = 0;
        for ch in self.chapters()? {
            for c in self.chunks(&ch)? {
                if !self.read_chunk(&ch, &c)?.trim().is_empty() {
                    n += 1;
                }
            }
        }
        Ok(n)
    }

    pub fn summary(&self) -> ProjectSummary {
        let m = &self.manifest;
        ProjectSummary {
            id: self.id(),
            path: self.root.display().to_string(),
            language_id: m.target_language.id.clone(),
            language_name: m.target_language.name.clone(),
            language_direction: m.target_language.direction.clone(),
            project_id: m.project.id.clone(),
            project_name: m.project.name.clone(),
            type_id: m.type_.id.clone(),
            resource_id: m.resource.id.clone(),
            resource_name: m.resource.name.clone(),
            translators: m.translators.clone(),
            finished_chunks: m.finished_chunks.len(),
            translated_chunks: self.translated_chunk_count().unwrap_or(0),
            modified_at: self.modified_at(),
        }
    }

    /// Elimina el proyecto del disco por completo.
    pub fn delete(self) -> Result<()> {
        remove_dir_all_robust(&self.root)
    }

    /// Cambia el tipo de recurso (p. ej. de `reg` a `ulb`). Renombra la carpeta.
    pub fn change_resource(mut self, resource: crate::manifest::IdName) -> Result<Project> {
        let old_root = self.root.clone();
        self.manifest.resource = resource;
        let new_root = old_root.with_file_name(self.manifest.unique_id());
        if new_root != old_root {
            if new_root.exists() {
                return Err(Error::ProjectExists(self.manifest.unique_id()));
            }
            fs::rename(&old_root, &new_root)?;
            self.root = new_root;
        }
        self.save_manifest()?;
        Ok(self)
    }
}

/// Orden natural de capítulos/chunks: numéricos por valor, `front` primero.
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    match (a.parse::<u32>(), b.parse::<u32>()) {
        (Ok(x), Ok(y)) => x.cmp(&y),
        (Ok(_), Err(_)) => std::cmp::Ordering::Greater,
        (Err(_), Ok(_)) => std::cmp::Ordering::Less,
        _ => a.cmp(b),
    }
}

/// `title` < `reference` < numéricos.
pub fn chunk_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    fn rank(s: &str) -> u8 {
        match s {
            "title" => 0,
            "reference" => 1,
            _ => 2,
        }
    }
    rank(a).cmp(&rank(b)).then_with(|| natural_cmp(a, b))
}

fn validate_segment(s: &str) -> Result<()> {
    if s.is_empty()
        || s.contains(['/', '\\', ':'])
        || s == "."
        || s == ".."
        || s.starts_with('.')
    {
        return Err(Error::UnsafePath(s.to_string()));
    }
    Ok(())
}

fn mtime(p: &Path) -> u64 {
    fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Escritura atómica: temp + rename. Evita archivos a medio escribir si la
/// app se cierra o OneDrive sincroniza en ese instante.
pub fn atomic_write(path: &Path, content: &str) -> Result<()> {
    let dir = path.parent().ok_or_else(|| Error::Other("ruta sin directorio".into()))?;
    fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(
        ".{}.tmp-{}",
        path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        uuid::Uuid::new_v4().simple()
    ));
    fs::write(&tmp, content)?;
    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(_) => {
            // Windows: rename falla si el destino existe y está bloqueado; se
            // intenta sobrescribir directamente.
            let r = fs::write(path, content);
            let _ = fs::remove_file(&tmp);
            r.map_err(Into::into)
        }
    }
}

/// `remove_dir_all` con reintentos (Windows/OneDrive liberan bloqueos tarde).
pub fn remove_dir_all_robust(path: &Path) -> Result<()> {
    let mut last = None;
    for _ in 0..5 {
        match fs::remove_dir_all(path) {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => {
                last = Some(e);
                // Quitar atributos de solo lectura (objetos de .git) y reintentar.
                for entry in walkdir::WalkDir::new(path).into_iter().flatten() {
                    if let Ok(meta) = entry.metadata() {
                        let mut perms = meta.permissions();
                        #[allow(clippy::permissions_set_readonly_false)]
                        perms.set_readonly(false);
                        let _ = fs::set_permissions(entry.path(), perms);
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    }
    Err(last.unwrap().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{IdName, TargetLanguage};

    fn manifest() -> Manifest {
        Manifest::new(
            TargetLanguage { id: "cbt".into(), name: "Chayahuita".into(), ..Default::default() },
            IdName::new("3jn", "3 John"),
            IdName::new("text", "Text"),
            IdName::new("reg", "Regular"),
            "1",
        )
    }

    #[test]
    fn create_write_read_list() {
        let tmp = tempfile::tempdir().unwrap();
        let mut p = Project::create(tmp.path(), manifest()).unwrap();
        assert_eq!(p.id(), "cbt_3jn_text_reg");
        assert!(tmp.path().join("cbt_3jn_text_reg/LICENSE.md").is_file());
        assert!(tmp.path().join("cbt_3jn_text_reg/front/title.txt").is_file());

        p.write_chunk("01", "05", "\\v 5 Amado").unwrap();
        p.write_chunk("01", "01", "\\v 1 El anciano").unwrap();
        p.write_chunk("01", "title", "Capítulo 1").unwrap();
        p.write_book_title("3 Juan").unwrap();
        assert_eq!(p.read_chunk("01", "01").unwrap(), "\\v 1 El anciano");
        assert_eq!(p.read_chunk("01", "99").unwrap(), "");
        assert_eq!(p.chapters().unwrap(), vec!["01"]);
        assert_eq!(p.chunks("01").unwrap(), vec!["title", "01", "05"]);
        assert_eq!(p.translated_chunk_count().unwrap(), 3);
        assert_eq!(p.read_book_title().unwrap(), "3 Juan");

        p.set_chunk_finished("01", "01", true).unwrap();
        let reopened = Project::open_by_id(tmp.path(), "cbt_3jn_text_reg").unwrap();
        assert!(reopened.manifest.is_chunk_finished("01", "01"));

        let list = Project::list(tmp.path()).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].summary().finished_chunks, 1);

        assert!(matches!(Project::create(tmp.path(), manifest()), Err(Error::ProjectExists(_))));
    }

    #[test]
    fn rejects_unsafe_segments() {
        let tmp = tempfile::tempdir().unwrap();
        let p = Project::create(tmp.path(), manifest()).unwrap();
        assert!(p.write_chunk("../x", "01", "a").is_err());
        assert!(p.write_chunk("01", "..\\01", "a").is_err());
        assert!(p.write_chunk(".git", "config", "a").is_err());
    }

    #[test]
    fn natural_order() {
        let mut v = vec!["10", "2", "front", "01"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(v, vec!["front", "01", "2", "10"]);
        let mut c = vec!["10", "reference", "02", "title"];
        c.sort_by(|a, b| chunk_cmp(a, b));
        assert_eq!(c, vec!["title", "reference", "02", "10"]);
    }

    #[test]
    fn change_resource_renames_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let p = Project::create(tmp.path(), manifest()).unwrap();
        p.write_chunk("01", "01", "x").unwrap();
        let p = p.change_resource(IdName::new("ulb", "Unlocked Literal Bible")).unwrap();
        assert_eq!(p.id(), "cbt_3jn_text_ulb");
        assert!(tmp.path().join("cbt_3jn_text_ulb/01/01.txt").is_file());
        assert!(!tmp.path().join("cbt_3jn_text_reg").exists());
    }

    #[test]
    fn delete_removes_everything() {
        let tmp = tempfile::tempdir().unwrap();
        let p = Project::create(tmp.path(), manifest()).unwrap();
        let root = p.root.clone();
        p.delete().unwrap();
        assert!(!root.exists());
    }
}
