//! Carpetas de datos, ajustes, perfil y arranque de la biblioteca.
//!
//! Estructura (por defecto `~/BTT-Writer-3` en escritorio; en móvil la app
//! pasa su carpeta de documentos):
//! ```text
//! translations/            proyectos
//! backups/                 respaldos automáticos .tstudio
//! database/index.sqlite    catálogo
//! resource_containers/     fuentes .tsrc
//! exports/  imported/  fonts/  logs/  temp/
//! settings.json  profile.json  backup-state.json
//! ```

use crate::gitrepo::Author;
use crate::library::Library;
use crate::project::atomic_write;
use crate::tstudio::{self, ImportMode, ImportOutcome};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    pub fn new(root: &Path) -> Paths {
        Paths { root: root.to_path_buf() }
    }

    /// `~/BTT-Writer-3` (o `BTT_WRITER_HOME` si está definida).
    pub fn default_desktop() -> Paths {
        if let Ok(h) = std::env::var("BTT_WRITER_HOME") {
            return Paths::new(Path::new(&h));
        }
        let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).unwrap_or_else(|_| ".".into());
        Paths::new(&Path::new(&home).join("BTT-Writer-3"))
    }

    pub fn translations(&self) -> PathBuf { self.root.join("translations") }
    pub fn backups(&self) -> PathBuf { self.root.join("backups") }
    pub fn database(&self) -> PathBuf { self.root.join("database") }
    pub fn index(&self) -> PathBuf { self.database().join("index.sqlite") }
    pub fn containers(&self) -> PathBuf { self.root.join("resource_containers") }
    pub fn exports(&self) -> PathBuf { self.root.join("exports") }
    pub fn imported(&self) -> PathBuf { self.root.join("imported") }
    pub fn fonts(&self) -> PathBuf { self.root.join("fonts") }
    pub fn logs(&self) -> PathBuf { self.root.join("logs") }
    pub fn temp(&self) -> PathBuf { self.root.join("temp") }
    /// Caché del listado de proyectos (chunks traducidos, conflictos) por fecha de modificación.
    pub fn list_cache_file(&self) -> PathBuf { self.root.join("list-cache.json") }
    pub fn settings_file(&self) -> PathBuf { self.root.join("settings.json") }
    pub fn profile_file(&self) -> PathBuf { self.root.join("profile.json") }
    pub fn backup_state_file(&self) -> PathBuf { self.root.join("backup-state.json") }

    pub fn ensure(&self) -> Result<()> {
        for d in [self.translations(), self.backups(), self.database(), self.containers(), self.exports(), self.imported(), self.fonts(), self.logs(), self.temp()] {
            std::fs::create_dir_all(d)?;
        }
        Ok(())
    }

    /// Carpetas de datos de versiones anteriores que existan en este equipo.
    pub fn legacy_roots() -> Vec<PathBuf> {
        let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).unwrap_or_else(|_| ".".into());
        let home = Path::new(&home);
        [home.join("BTT-Writer-2.0"), home.join("BTT-Writer"), home.join("translationStudio")]
            .into_iter()
            .filter(|p| p.join("translations").is_dir() || p.join("targetTranslations").is_dir() || p.join("backups").is_dir() || p.join("automatic_backups").is_dir())
            .collect()
    }

    /// Copia `index.sqlite` y descomprime `containers.zip` si aún no existen.
    pub fn bootstrap_library(&self, bundled_index: &Path, bundled_containers_zip: Option<&Path>) -> Result<bool> {
        self.ensure()?;
        let mut changed = false;
        if !self.index().is_file() && bundled_index.is_file() {
            std::fs::copy(bundled_index, self.index())?;
            changed = true;
        }
        if let Some(zip_path) = bundled_containers_zip {
            if zip_path.is_file() {
                let file = std::fs::File::open(zip_path)?;
                let mut zip = zip::ZipArchive::new(file)?;
                for i in 0..zip.len() {
                    let mut f = zip.by_index(i)?;
                    let name = f.name().replace('\\', "/");
                    let Some(base) = name.rsplit('/').next() else { continue };
                    if !base.ends_with(".tsrc") || f.is_dir() {
                        continue;
                    }
                    let dest = self.containers().join(base);
                    if dest.is_file() {
                        continue;
                    }
                    let mut out = std::fs::File::create(&dest)?;
                    std::io::copy(&mut f, &mut out)?;
                    changed = true;
                }
            }
        }
        Ok(changed)
    }

    pub fn open_library(&self) -> Result<Library> {
        let lib = Library::open(&self.index(), &self.containers())?;
        lib.ensure_schema()?;
        Ok(lib)
    }

    /// Importa proyectos y respaldos de una instalación anterior.
    pub fn migrate_from(&self, old_root: &Path, author: &Author) -> Result<Vec<ImportOutcome>> {
        self.migrate_from_with_progress(old_root, author, |_| {})
    }

    /// Cuenta lo que migraría `migrate_from` (proyectos en carpeta + respaldos automáticos).
    pub fn legacy_project_count(old_root: &Path) -> usize {
        Self::legacy_items(old_root).len()
    }

    fn legacy_items(old_root: &Path) -> Vec<LegacyItem> {
        let mut items = vec![];
        for dir_name in ["translations", "targetTranslations"] {
            let dir = old_root.join(dir_name);
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            for entry in rd.flatten() {
                if entry.path().is_dir() && entry.path().join("manifest.json").is_file() {
                    items.push(LegacyItem::Dir(entry.path()));
                }
            }
        }
        // 1.6 guarda cada proyecto como `automatic_backups/{id}.tstudio` (último estado).
        let dir = old_root.join("automatic_backups");
        if let Ok(rd) = std::fs::read_dir(&dir) {
            let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "tstudio")).collect();
            files.sort();
            items.extend(files.into_iter().map(LegacyItem::Archive));
        }
        items
    }

    /// Como `migrate_from`, informando el avance (`hechos`, `total`, nombre del elemento actual).
    pub fn migrate_from_with_progress(&self, old_root: &Path, author: &Author, mut progress: impl FnMut(MigrateProgress)) -> Result<Vec<ImportOutcome>> {
        self.ensure()?;
        let items = Self::legacy_items(old_root);
        let total = items.len();
        let mut outcomes = vec![];
        for (i, item) in items.into_iter().enumerate() {
            let label = item.path().file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            progress(MigrateProgress { done: i, total, current: label.clone() });
            match item {
                LegacyItem::Dir(path) => {
                    // Copiar a temp y luego importar (no se toca la carpeta antigua).
                    let staging = self.temp().join(format!("migrate-{}", uuid::Uuid::new_v4().simple()));
                    copy_dir(&path, &staging)?;
                    outcomes.push(tstudio::import_dir(&staging, &self.translations(), ImportMode::Merge, author));
                    let _ = crate::project::remove_dir_all_robust(&staging);
                }
                LegacyItem::Archive(path) => match tstudio::import(&path, &self.translations(), ImportMode::Merge, author) {
                    Ok(out) => outcomes.extend(out),
                    Err(e) => outcomes.push(ImportOutcome { id: label, status: tstudio::ImportStatus::Failed { error: e.to_string() } }),
                },
            }
        }
        progress(MigrateProgress { done: total, total, current: String::new() });
        for dir_name in ["backups", "automatic_backups"] {
            let dir = old_root.join(dir_name);
            if dir.is_dir() {
                for entry in std::fs::read_dir(&dir)? {
                    let entry = entry?;
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.ends_with(".tstudio") || name.ends_with(".zip") {
                        let dest = self.backups().join(format!("legacy-{name}"));
                        if !dest.exists() {
                            let _ = std::fs::copy(entry.path(), dest);
                        }
                    }
                }
            }
        }
        let old_rc = old_root.join("resource_containers");
        if old_rc.is_dir() {
            for entry in std::fs::read_dir(&old_rc)? {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".tsrc") && !self.containers().join(&name).exists() {
                    let _ = std::fs::copy(entry.path(), self.containers().join(&name));
                }
            }
        }
        Ok(outcomes)
    }
}

enum LegacyItem {
    Dir(PathBuf),
    Archive(PathBuf),
}

impl LegacyItem {
    fn path(&self) -> &Path {
        match self {
            LegacyItem::Dir(p) | LegacyItem::Archive(p) => p,
        }
    }
}

/// Avance de una migración: elementos procesados de un total y el actual.
#[derive(Debug, Clone, Serialize)]
pub struct MigrateProgress {
    pub done: usize,
    pub total: usize,
    pub current: String,
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
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
    Ok(())
}

// ------------------------------------------------------------------ ajustes

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Settings {
    /// `light` | `dark` | `system`
    pub color_theme: String,
    /// Idioma de la interfaz (`es`, `en`...).
    pub ui_language: String,
    pub gateway_language_mode: bool,
    pub translation_font: String,
    pub translation_font_size: u32,
    pub source_font: String,
    pub source_font_size: u32,
    /// `wacs` | `wacs_dev` | `dcs`
    pub content_server: String,
    pub git_api_url: String,
    pub data_server_url: String,
    pub media_server_url: String,
    pub reader_url: String,
    pub create_account_url: String,
    pub langnames_url: String,
    pub index_sqlite_url: String,
    pub translation_manual_url: String,
    pub enable_tm_links: bool,
    /// Minutos; -1 desactiva.
    pub backup_interval_minutes: i32,
    pub logging_level: String,
    pub sort_by: String,
    pub book_order: String,
    pub blind_draft: bool,
    pub check_hardware: bool,
    pub last_project: String,
    pub migrated_from: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        let mut s = Settings {
            color_theme: "system".into(),
            ui_language: "es".into(),
            gateway_language_mode: false,
            translation_font: "Noto Sans".into(),
            translation_font_size: 25,
            source_font: "Noto Sans".into(),
            source_font_size: 25,
            content_server: "wacs".into(),
            git_api_url: String::new(),
            data_server_url: String::new(),
            media_server_url: String::new(),
            reader_url: String::new(),
            create_account_url: String::new(),
            langnames_url: String::new(),
            index_sqlite_url: "https://writer-resources.bibletranslationtools.org/index.sqlite".into(),
            translation_manual_url: "https://read.bibleineverylanguage.org/WA-Catalog/en_tm".into(),
            enable_tm_links: true,
            backup_interval_minutes: 5,
            logging_level: "info".into(),
            sort_by: "project".into(),
            book_order: "bible".into(),
            blind_draft: false,
            check_hardware: true,
            last_project: String::new(),
            migrated_from: vec![],
        };
        s.apply_server_defaults();
        s
    }
}

impl Settings {
    pub fn load(path: &Path) -> Settings {
        std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        atomic_write(path, &serde_json::to_string_pretty(self)?)
    }

    /// Rellena las URLs según `content_server`.
    pub fn apply_server_defaults(&mut self) {
        let (api, data, media, reader, lang) = match self.content_server.as_str() {
            "wacs_dev" => (
                crate::wacs::WACS_DEV_API,
                "https://content.wacsdev.org",
                "https://api-dev.bibletranslationtools.org",
                "https://read-dev.bibleineverylanguage.org",
                "https://langnames.bibleineverylanguage.org/langnames.json",
            ),
            "dcs" => (
                crate::wacs::DCS_API,
                "https://git.door43.org",
                "https://api.unfoldingword.org",
                "https://door43.org/u",
                "https://td.unfoldingword.org/exports/langnames.json",
            ),
            _ => (
                crate::wacs::WACS_API,
                "https://content.bibletranslationtools.org",
                "https://api.bibletranslationtools.org",
                "https://read.bibleineverylanguage.org",
                "https://langnames.bibleineverylanguage.org/langnames.json",
            ),
        };
        self.git_api_url = api.into();
        self.data_server_url = data.into();
        self.media_server_url = media.into();
        self.reader_url = reader.into();
        self.create_account_url = format!("{data}/user/sign_up");
        self.langnames_url = lang.into();
    }

    pub fn root_catalog_url(&self) -> String {
        format!("{}/ts/txt/2/catalog.json", self.media_server_url.trim_end_matches('/'))
    }
}

// ------------------------------------------------------------------ perfil

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct Profile {
    pub full_name: String,
    pub email: String,
    pub session: Option<crate::wacs::Session>,
    pub terms_accepted_version: u32,
    pub privacy_accepted: bool,
}

pub const TERMS_VERSION: u32 = 3;

impl Profile {
    pub fn load(path: &Path) -> Profile {
        std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        atomic_write(path, &serde_json::to_string_pretty(self)?)
    }
    pub fn is_logged_in(&self) -> bool {
        !self.full_name.trim().is_empty() || self.session.is_some()
    }
    pub fn display_name(&self) -> String {
        if !self.full_name.trim().is_empty() {
            return self.full_name.trim().to_string();
        }
        self.session.as_ref().map(|s| if s.user.full_name.is_empty() { s.user.username.clone() } else { s.user.full_name.clone() }).unwrap_or_default()
    }
    pub fn author(&self) -> Author {
        let email = if self.email.is_empty() {
            self.session.as_ref().map(|s| s.user.email.clone()).unwrap_or_default()
        } else {
            self.email.clone()
        };
        Author::new(&self.display_name(), &email)
    }
    pub fn needs_terms(&self) -> bool {
        self.terms_accepted_version < TERMS_VERSION
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_and_bootstrap_from_fixtures() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(&tmp.path().join("data"));
        let fx = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let changed = paths.bootstrap_library(&fx.join("index.sqlite"), Some(&fx.join("containers.zip"))).unwrap();
        assert!(changed);
        assert!(paths.index().is_file());
        let lib = paths.open_library().unwrap();
        let downloaded = lib.downloaded_containers().unwrap();
        assert!(downloaded.contains(&"en_mrk_ulb".to_string()), "{downloaded:?}");
        let rc = lib.open_container("en", "mrk", "ulb").unwrap();
        assert_eq!(rc.chapters().len(), 16);
        assert!(lib.resource("en", "mrk", "ulb").unwrap().unwrap().downloaded);
        // Segunda vez no cambia nada.
        assert!(!paths.bootstrap_library(&fx.join("index.sqlite"), Some(&fx.join("containers.zip"))).unwrap());
    }

    #[test]
    fn settings_and_profile_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let mut s = Settings::default();
        assert_eq!(s.git_api_url, crate::wacs::WACS_API);
        assert!(s.root_catalog_url().ends_with("/ts/txt/2/catalog.json"));
        s.content_server = "dcs".into();
        s.apply_server_defaults();
        assert_eq!(s.git_api_url, crate::wacs::DCS_API);
        s.save(&tmp.path().join("settings.json")).unwrap();
        assert_eq!(Settings::load(&tmp.path().join("settings.json")), s);
        assert_eq!(Settings::load(&tmp.path().join("nope.json")).backup_interval_minutes, 5);

        let mut p = Profile::default();
        assert!(!p.is_logged_in());
        assert!(p.needs_terms());
        p.full_name = "Ana".into();
        p.terms_accepted_version = TERMS_VERSION;
        p.save(&tmp.path().join("profile.json")).unwrap();
        let q = Profile::load(&tmp.path().join("profile.json"));
        assert!(q.is_logged_in() && !q.needs_terms());
        assert_eq!(q.author().name, "Ana");
    }

    #[test]
    fn migrate_from_legacy_layout() {
        let tmp = tempfile::tempdir().unwrap();
        let old = tmp.path().join("BTT-Writer-2.0");
        let fx = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/exports");
        // Simular una instalación antigua extrayendo un .tstudio.
        tstudio::extract(&fx.join("aaa_mrk_text_reg.tstudio"), &old.join("translations")).unwrap();
        std::fs::remove_file(old.join("translations/manifest.json")).ok();
        std::fs::create_dir_all(old.join("backups")).unwrap();
        std::fs::copy(fx.join("aa_jud_text_reg.tstudio"), old.join("backups/aa_jud_text_reg.tstudio")).unwrap();
        let paths = Paths::new(&tmp.path().join("new"));
        let out = paths.migrate_from(&old, &Author::new("T", "t@t")).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "aaa_mrk_text_reg");
        assert!(paths.translations().join("aaa_mrk_text_reg/manifest.json").is_file());
        assert!(paths.backups().join("legacy-aa_jud_text_reg.tstudio").is_file());
        // La carpeta antigua queda intacta.
        assert!(old.join("translations/aaa_mrk_text_reg/manifest.json").is_file());
    }
}
