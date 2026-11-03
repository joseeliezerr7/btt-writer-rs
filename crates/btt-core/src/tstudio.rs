//! Archivos `.tstudio` (zip) para respaldo e intercambio de proyectos.
//!
//! ```text
//! manifest.json                 ← manifest del archivo (package_version 3)
//! {id}/manifest.json            ← proyecto completo, incluido .git/
//! {id}/LICENSE.md
//! {id}/front/title.txt, {id}/01/01.txt …
//! ```
//! Se aceptan al importar las versiones 1 (`projects`), 2 y 3
//! (`target_translations`), nombres con `\` y entradas mal ubicadas, como
//! las que generaba BTT Writer 2.0.

use crate::gitrepo::{self, Author, MergeStatus};
use crate::manifest::Manifest;
use crate::project::Project;
use crate::rc::normalize_path;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;

pub const ARCHIVE_VERSION: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArchiveTranslation {
    pub id: String,
    pub path: String,
    pub commit_hash: String,
    pub direction: String,
    #[serde(default)]
    pub target_language_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveManifest {
    pub package_version: u32,
    pub timestamp: u64,
    pub generator: crate::manifest::Generator,
    pub target_translations: Vec<ArchiveTranslation>,
}

impl ArchiveManifest {
    /// Lee cualquier versión (1, 2 o 3) del manifest de archivo.
    pub fn parse(json: &str) -> Result<ArchiveManifest> {
        let v: Value = serde_json::from_str(json).map_err(|e| Error::InvalidTstudio(e.to_string()))?;
        let list = v
            .get("target_translations")
            .or_else(|| v.get("projects"))
            .and_then(|l| l.as_array())
            .cloned()
            .unwrap_or_default();
        let mut tts = vec![];
        for t in list {
            let commit_hash = match t.get("commit_hash") {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Object(o)) => o.get("stdout").and_then(|s| s.as_str()).unwrap_or("").trim().to_string(),
                _ => String::new(),
            };
            let id = t.get("id").and_then(|s| s.as_str()).unwrap_or("").to_string();
            if id.is_empty() {
                continue;
            }
            tts.push(ArchiveTranslation {
                path: t.get("path").and_then(|s| s.as_str()).unwrap_or(&id).to_string(),
                id,
                commit_hash,
                direction: t.get("direction").and_then(|s| s.as_str()).unwrap_or("ltr").to_string(),
                target_language_name: t.get("target_language_name").and_then(|s| s.as_str()).unwrap_or("").to_string(),
            });
        }
        let generator = v
            .get("generator")
            .cloned()
            .and_then(|g| serde_json::from_value(g).ok())
            .unwrap_or_default();
        Ok(ArchiveManifest {
            package_version: v.get("package_version").and_then(|n| n.as_u64()).unwrap_or(1) as u32,
            timestamp: v.get("timestamp").and_then(|n| n.as_u64()).unwrap_or(0),
            generator,
            target_translations: tts,
        })
    }
}

/// Exporta uno o varios proyectos a `dest` (archivo `.tstudio`).
/// Hace commit previo para que el historial viaje completo.
pub fn export(projects: &[&Project], dest: &Path, author: &Author, build: &str) -> Result<PathBuf> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = dest.with_extension("tstudio.part");
    {
        let file = File::create(&tmp)?;
        let mut zip = zip::ZipWriter::new(file);
        let opts = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);
        let mut tts = vec![];
        for p in projects {
            gitrepo::commit_all(&p.root, author, gitrepo::AUTOSAVE_MESSAGE)?;
            let id = p.id();
            tts.push(ArchiveTranslation {
                id: id.clone(),
                path: id.clone(),
                commit_hash: gitrepo::head_hash(&p.root).unwrap_or_default(),
                direction: p.manifest.target_language.direction.clone(),
                target_language_name: p.manifest.target_language.name.clone(),
            });
            for entry in walkdir::WalkDir::new(&p.root).into_iter() {
                let entry = entry.map_err(|e| Error::Other(e.to_string()))?;
                if !entry.file_type().is_file() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy();
                if name.starts_with('.') && name.contains(".tmp-") {
                    continue; // temporales de escritura atómica
                }
                let rel = entry.path().strip_prefix(&p.root).unwrap();
                let zip_name = format!("{id}/{}", rel.to_string_lossy().replace('\\', "/"));
                zip.start_file(&zip_name, opts)?;
                let mut f = File::open(entry.path())?;
                std::io::copy(&mut f, &mut zip)?;
            }
        }
        let manifest = ArchiveManifest {
            package_version: ARCHIVE_VERSION,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            generator: crate::manifest::Generator { name: "btt-writer-desktop".into(), build: build.into() },
            target_translations: tts,
        };
        zip.start_file("manifest.json", opts)?;
        zip.write_all(serde_json::to_string_pretty(&manifest)?.as_bytes())?;
        zip.finish()?;
    }
    std::fs::rename(&tmp, dest).or_else(|_| std::fs::copy(&tmp, dest).map(|_| ()).and_then(|_| std::fs::remove_file(&tmp)))?;
    Ok(dest.to_path_buf())
}

/// Resumen de lo que contiene un `.tstudio` sin importarlo.
pub fn inspect(archive: &Path) -> Result<ArchiveManifest> {
    let file = File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file)?;
    // Manifest raíz: la entrada cuyo nombre normalizado es exactamente
    // `manifest.json` o (2.0) una ruta absoluta que termina en manifest.json
    // sin pertenecer a una carpeta de proyecto.
    let mut root_manifest = None;
    let mut project_ids = vec![];
    for i in 0..zip.len() {
        let mut f = zip.by_index(i)?;
        let raw = f.name().replace('\\', "/");
        let Ok(name) = normalize_path(&raw) else {
            // ruta absoluta (bug de 2.0): si es manifest.json, es el raíz
            if raw.ends_with("manifest.json") {
                let mut s = String::new();
                f.read_to_string(&mut s)?;
                root_manifest = Some(s);
            }
            continue;
        };
        if name == "manifest.json" {
            let mut s = String::new();
            f.read_to_string(&mut s)?;
            root_manifest = Some(s);
        } else if let Some((dir, rest)) = name.split_once('/') {
            if rest == "manifest.json" {
                project_ids.push(dir.to_string());
            }
        }
    }
    let mut m = match root_manifest {
        Some(s) => ArchiveManifest::parse(&s)?,
        None => ArchiveManifest {
            package_version: 0,
            timestamp: 0,
            generator: Default::default(),
            target_translations: vec![],
        },
    };
    for id in project_ids {
        if !m.target_translations.iter().any(|t| t.path == id) {
            m.target_translations.push(ArchiveTranslation {
                id: id.clone(),
                path: id,
                commit_hash: String::new(),
                direction: "ltr".into(),
                target_language_name: String::new(),
            });
        }
    }
    if m.target_translations.is_empty() {
        return Err(Error::InvalidTstudio("el archivo no contiene proyectos".into()));
    }
    Ok(m)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportMode {
    /// Si el proyecto existe, fusionar historiales (git).
    Merge,
    /// Si existe, reemplazarlo por el importado.
    Overwrite,
    /// Si existe, no tocarlo y reportar `Exists`.
    Skip,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ImportStatus {
    Imported,
    Merged,
    Overwritten,
    Exists,
    Conflicts { files: Vec<String> },
    Failed { error: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportOutcome {
    pub id: String,
    pub status: ImportStatus,
}

/// Importa todos los proyectos de un `.tstudio` (o `.zip`) en `translations_dir`.
pub fn import(archive: &Path, translations_dir: &Path, mode: ImportMode, author: &Author) -> Result<Vec<ImportOutcome>> {
    std::fs::create_dir_all(translations_dir)?;
    let staging = translations_dir.join(format!(".import-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&staging)?;
    let result = (|| {
        extract(archive, &staging)?;
        let mut outcomes = vec![];
        for dir in find_project_dirs(&staging)? {
            outcomes.push(import_dir(&dir, translations_dir, mode, author));
        }
        if outcomes.is_empty() {
            return Err(Error::InvalidTstudio("el archivo no contiene proyectos".into()));
        }
        Ok(outcomes)
    })();
    let _ = crate::project::remove_dir_all_robust(&staging);
    result
}

/// Importa una carpeta de proyecto ya extraída (también usada por "clonar del servidor").
pub fn import_dir(dir: &Path, translations_dir: &Path, mode: ImportMode, author: &Author) -> ImportOutcome {
    let fallback_id = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    match import_dir_inner(dir, translations_dir, mode, author) {
        Ok(o) => o,
        Err(e) => ImportOutcome { id: fallback_id, status: ImportStatus::Failed { error: e.to_string() } },
    }
}

fn import_dir_inner(dir: &Path, translations_dir: &Path, mode: ImportMode, author: &Author) -> Result<ImportOutcome> {
    let json = std::fs::read_to_string(dir.join("manifest.json"))?;
    let mut manifest = Manifest::parse(&json)?;
    manifest.generator = crate::manifest::Generator {
        name: crate::manifest::GENERATOR_NAME.into(),
        build: crate::VERSION.into(),
    };
    let id = manifest.unique_id();
    crate::project::atomic_write(&dir.join("manifest.json"), &manifest.to_json())?;
    if !dir.join("LICENSE.md").is_file() {
        crate::project::atomic_write(&dir.join("LICENSE.md"), crate::project::LICENSE_MD)?;
    }
    gitrepo::unlock(dir);
    gitrepo::commit_all(dir, author, gitrepo::AUTOSAVE_MESSAGE)?;

    let dest = translations_dir.join(&id);
    if dest.exists() {
        return Ok(match mode {
            ImportMode::Skip => ImportOutcome { id, status: ImportStatus::Exists },
            ImportMode::Overwrite => {
                crate::project::remove_dir_all_robust(&dest)?;
                move_dir(dir, &dest)?;
                ImportOutcome { id, status: ImportStatus::Overwritten }
            }
            ImportMode::Merge => match gitrepo::merge_local(&dest, dir, author)? {
                MergeStatus::Conflicts { files } => ImportOutcome { id, status: ImportStatus::Conflicts { files } },
                MergeStatus::AuthFailure | MergeStatus::NoRemoteRepo => {
                    ImportOutcome { id, status: ImportStatus::Failed { error: "no se pudo fusionar".into() } }
                }
                _ => ImportOutcome { id, status: ImportStatus::Merged },
            },
        });
    }
    move_dir(dir, &dest)?;
    Ok(ImportOutcome { id, status: ImportStatus::Imported })
}

fn move_dir(from: &Path, to: &Path) -> Result<()> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    // Distinto volumen: copiar y borrar.
    for entry in walkdir::WalkDir::new(from) {
        let entry = entry.map_err(|e| Error::Other(e.to_string()))?;
        let rel = entry.path().strip_prefix(from).unwrap();
        let target = to.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else {
            if let Some(p) = target.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::fs::copy(entry.path(), &target)?;
        }
    }
    crate::project::remove_dir_all_robust(from)
}

/// Extrae un zip de forma segura (sin zip slip; `\` → `/`).
pub fn extract(archive: &Path, dest: &Path) -> Result<()> {
    let file = File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file)?;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i)?;
        let raw = f.name().replace('\\', "/");
        let Ok(name) = normalize_path(&raw) else {
            // Entradas con rutas absolutas (bug de 2.0): guardarlas por su nombre base.
            if let Some(base) = raw.rsplit('/').next() {
                if base == "manifest.json" && !f.is_dir() {
                    let mut buf = vec![];
                    f.read_to_end(&mut buf)?;
                    std::fs::write(dest.join("manifest.json"), buf)?;
                }
            }
            continue;
        };
        let out = dest.join(&name);
        if f.is_dir() || raw.ends_with('/') {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut o = File::create(&out)?;
        std::io::copy(&mut f, &mut o)?;
    }
    Ok(())
}

/// Carpetas de proyecto (contienen `manifest.json` con `target_language`), hasta 2 niveles.
pub fn find_project_dirs(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = vec![];
    for entry in walkdir::WalkDir::new(root).min_depth(1).max_depth(3) {
        let entry = entry.map_err(|e| Error::Other(e.to_string()))?;
        if entry.file_type().is_file() && entry.file_name() == "manifest.json" {
            if let Ok(s) = std::fs::read_to_string(entry.path()) {
                if s.contains("\"target_language\"") {
                    out.push(entry.path().parent().unwrap().to_path_buf());
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

    fn author() -> Author {
        Author::new("Tester", "t@example.com")
    }

    fn fixtures() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/exports")
    }

    #[test]
    fn imports_v1_v2_fixtures_from_original_app() {
        let tmp = tempfile::tempdir().unwrap();
        let tr = tmp.path().join("translations");
        for name in ["aa_jud_text_reg.tstudio", "aa_mrk_text_reg.tstudio", "aaa_mrk_text_reg.tstudio"] {
            let info = inspect(&fixtures().join(name)).unwrap();
            assert_eq!(info.target_translations.len(), 1, "{name}");
            let out = import(&fixtures().join(name), &tr, ImportMode::Skip, &author()).unwrap();
            assert_eq!(out.len(), 1);
            assert_eq!(out[0].status, ImportStatus::Imported, "{name}: {:?}", out[0]);
        }
        let p = Project::open_by_id(&tr, "aaa_mrk_text_reg").unwrap();
        assert_eq!(p.manifest.package_version, 8);
        assert_eq!(p.manifest.resource.name, "Unlocked Literal Bible".replace("Unlocked Literal Bible", "Regular"));
        assert_eq!(p.manifest.source_translations.len(), 2, "duplicados eliminados");
        assert!(p.read_chunk("01", "01").unwrap().contains("\\v"));
        assert!(gitrepo::head_hash(&p.root).is_some());
        // Importar otra vez: existe.
        let out = import(&fixtures().join("aaa_mrk_text_reg.tstudio"), &tr, ImportMode::Skip, &author()).unwrap();
        assert_eq!(out[0].status, ImportStatus::Exists);
        // Fusionar sin cambios: Merged/UpToDate.
        let out = import(&fixtures().join("aaa_mrk_text_reg.tstudio"), &tr, ImportMode::Merge, &author()).unwrap();
        assert!(matches!(out[0].status, ImportStatus::Merged), "{:?}", out[0]);
    }

    #[test]
    fn export_then_import_round_trip_and_overwrite() {
        let tmp = tempfile::tempdir().unwrap();
        let tr = tmp.path().join("translations");
        std::fs::create_dir_all(&tr).unwrap();
        let m = Manifest::new(
            TargetLanguage { id: "xy".into(), name: "Xy".into(), ..Default::default() },
            IdName::new("jud", "Jude"),
            IdName::new("text", "Text"),
            IdName::new("reg", "Regular"),
            "1",
        );
        let p = Project::create(&tr, m).unwrap();
        p.write_chunk("01", "01", "\\v 1 hola").unwrap();
        let dest = tmp.path().join("backups/xy_jud_text_reg.tstudio");
        export(&[&p], &dest, &author(), "3.0.0").unwrap();
        let info = inspect(&dest).unwrap();
        assert_eq!(info.package_version, 3);
        assert_eq!(info.target_translations[0].id, "xy_jud_text_reg");
        assert_eq!(info.target_translations[0].commit_hash, gitrepo::head_hash(&p.root).unwrap());

        // Importar en otra carpeta.
        let tr2 = tmp.path().join("t2");
        let out = import(&dest, &tr2, ImportMode::Skip, &author()).unwrap();
        assert_eq!(out[0].status, ImportStatus::Imported);
        let q = Project::open_by_id(&tr2, "xy_jud_text_reg").unwrap();
        assert_eq!(q.read_chunk("01", "01").unwrap(), "\\v 1 hola");
        // El historial original viaja dentro del archivo (el import añade un
        // commit más al actualizar el generator del manifest).
        let hist = gitrepo::file_history(&q.root, "01/01.txt").unwrap();
        assert_eq!(hist.last().unwrap().id, gitrepo::head_hash(&p.root).unwrap());

        // Overwrite reemplaza contenido.
        q.write_chunk("01", "01", "cambiado").unwrap();
        let out = import(&dest, &tr2, ImportMode::Overwrite, &author()).unwrap();
        assert_eq!(out[0].status, ImportStatus::Overwritten);
        assert_eq!(Project::open_by_id(&tr2, "xy_jud_text_reg").unwrap().read_chunk("01", "01").unwrap(), "\\v 1 hola");
    }

    #[test]
    fn rejects_archive_without_projects_and_zip_slip() {
        let tmp = tempfile::tempdir().unwrap();
        let bad = tmp.path().join("bad.tstudio");
        {
            let mut zip = zip::ZipWriter::new(File::create(&bad).unwrap());
            let o = SimpleFileOptions::default();
            zip.start_file("../evil.txt", o).unwrap();
            zip.write_all(b"x").unwrap();
            zip.start_file("readme.txt", o).unwrap();
            zip.write_all(b"x").unwrap();
            zip.finish().unwrap();
        }
        assert!(matches!(import(&bad, &tmp.path().join("t"), ImportMode::Skip, &author()), Err(Error::InvalidTstudio(_))));
        assert!(!tmp.path().join("evil.txt").exists());
    }
}
