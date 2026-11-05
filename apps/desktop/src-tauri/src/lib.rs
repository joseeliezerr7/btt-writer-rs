//! App Tauri de BTT Writer: estado compartido, arranque y registro de comandos.

mod commands;

use btt_core::appdata::{Paths, Profile, Settings};
use btt_core::backup::BackupState;
use btt_core::library::Library;
use btt_core::project::ProjectScan;
use btt_core::rc::ResourceContainer;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

/// Estado global de la aplicación.
pub struct AppState {
    pub paths: Paths,
    pub settings: Mutex<Settings>,
    pub profile: Mutex<Profile>,
    pub library: Mutex<Library>,
    pub backup_state: Mutex<BackupState>,
    /// Contenedores abiertos (slug → RC). Se vacía al descargar/borrar fuentes.
    pub rc_cache: Mutex<HashMap<String, Arc<ResourceContainer>>>,
    /// Recorrido de chunks por proyecto (id → scan), válido mientras no cambie `modified_at_ms`.
    pub list_cache: Mutex<HashMap<String, ProjectScan>>,
    /// Solo una pasada de respaldo a la vez (temporizador vs. "respaldar ahora").
    pub backup_running: Mutex<()>,
    pub device_name: String,
}

impl AppState {
    pub fn container(&self, lang: &str, project: &str, resource: &str) -> btt_core::Result<Arc<ResourceContainer>> {
        let slug = btt_core::rc::container_slug(lang, project, resource);
        if let Some(rc) = self.rc_cache.lock().unwrap_or_else(|e| e.into_inner()).get(&slug) {
            return Ok(rc.clone());
        }
        let rc = Arc::new(self.library.lock().unwrap_or_else(|e| e.into_inner()).open_container(lang, project, resource)?);
        self.rc_cache.lock().unwrap_or_else(|e| e.into_inner()).insert(slug, rc.clone());
        Ok(rc)
    }

    pub fn clear_rc_cache(&self) {
        self.rc_cache.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    pub fn author(&self) -> btt_core::gitrepo::Author {
        self.profile.lock().unwrap_or_else(|e| e.into_inner()).author()
    }

    pub fn save_settings(&self) -> btt_core::Result<()> {
        self.settings.lock().unwrap_or_else(|e| e.into_inner()).save(&self.paths.settings_file())
    }

    pub fn save_profile(&self) -> btt_core::Result<()> {
        self.profile.lock().unwrap_or_else(|e| e.into_inner()).save(&self.paths.profile_file())
    }

    pub fn save_backup_state(&self) -> btt_core::Result<()> {
        self.backup_state.lock().unwrap_or_else(|e| e.into_inner()).save(&self.paths.backup_state_file())
    }

    /// Scan de un proyecto, reutilizando la caché si los archivos no cambiaron.
    pub fn project_scan(&self, project: &btt_core::project::Project) -> ProjectScan {
        let id = project.id();
        let mtime = project.modified_at_ms();
        if let Some(c) = self.list_cache.lock().unwrap_or_else(|e| e.into_inner()).get(&id) {
            if c.modified_at_ms == mtime {
                return c.clone();
            }
        }
        let scan = project.scan();
        self.list_cache.lock().unwrap_or_else(|e| e.into_inner()).insert(id, scan.clone());
        scan
    }

    pub fn save_list_cache(&self) {
        let cache = self.list_cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Ok(json) = serde_json::to_string(&*cache) {
            let _ = btt_core::project::atomic_write(&self.paths.list_cache_file(), &json);
        }
    }
}

fn load_list_cache(path: &std::path::Path) -> HashMap<String, ProjectScan> {
    std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

/// Carpeta raíz de datos según plataforma.
fn data_root(app: &tauri::AppHandle) -> PathBuf {
    if cfg!(any(target_os = "android", target_os = "ios")) {
        app.path().app_data_dir().map(|p| p.join("BTT-Writer")).unwrap_or_else(|_| PathBuf::from("BTT-Writer"))
    } else {
        Paths::default_desktop().root
    }
}

/// Localiza un recurso empaquetado (index.sqlite / containers.zip).
fn bundled(app: &tauri::AppHandle, name: &str) -> Option<PathBuf> {
    let candidates = [
        app.path().resource_dir().ok().map(|d| d.join("resources").join(name)),
        app.path().resource_dir().ok().map(|d| d.join(name)),
        Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources").join(name)),
    ];
    candidates.into_iter().flatten().find(|p| p.is_file())
}

fn device_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| std::env::consts::OS.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build()).plugin(tauri_plugin_process::init());
    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .setup(|app| {
            let handle = app.handle().clone();
            let paths = Paths::new(&data_root(&handle));
            paths.ensure()?;
            let index = bundled(&handle, "index.sqlite");
            let containers = bundled(&handle, "containers.zip");
            match (&index, &containers) {
                (Some(i), c) => {
                    paths.bootstrap_library(i, c.as_deref())?;
                }
                (None, _) => log::warn!("no se encontró index.sqlite empaquetado; se creará un índice vacío"),
            }
            if !paths.index().is_file() {
                let lib = Library::open(&paths.index(), &paths.containers())?;
                lib.ensure_schema()?;
            }
            let library = paths.open_library()?;
            let settings = Settings::load(&paths.settings_file());
            let profile = Profile::load(&paths.profile_file());
            let backup_state = BackupState::load(&paths.backup_state_file());
            let list_cache = load_list_cache(&paths.list_cache_file());
            let state = AppState {
                paths,
                settings: Mutex::new(settings),
                profile: Mutex::new(profile),
                library: Mutex::new(library),
                backup_state: Mutex::new(backup_state),
                rc_cache: Mutex::new(HashMap::new()),
                list_cache: Mutex::new(list_cache),
                backup_running: Mutex::new(()),
                device_name: device_name(),
            };
            app.manage(state);
            start_backup_timer(handle);
            Ok(())
        })
        .invoke_handler(commands::handler())
        .run(tauri::generate_context!())
        .expect("error al iniciar BTT Writer");
}

/// Hilo de respaldo automático según el intervalo configurado.
fn start_backup_timer(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        let minutes: i32 = {
            let st = app.state::<AppState>();
            let m = st.settings.lock().unwrap_or_else(|e| e.into_inner()).backup_interval_minutes;
            m
        };
        let wait = if minutes <= 0 { 60 } else { (minutes as u64) * 60 };
        std::thread::sleep(std::time::Duration::from_secs(wait));
        if minutes <= 0 {
            continue;
        }
        let st = app.state::<AppState>();
        let Ok(_running) = st.backup_running.try_lock() else {
            continue; // ya hay un respaldo manual en curso
        };
        let author = st.author();
        // Trabajar sobre una copia: el candado no se retiene durante el recorrido
        // de todos los proyectos (el listado también lo necesita).
        let mut bs = st.backup_state.lock().unwrap_or_else(|e| e.into_inner()).clone();
        match btt_core::backup::run(&st.paths.translations(), &st.paths.backups(), &mut bs, &author, btt_core::VERSION) {
            Ok(report) => {
                {
                    let mut cur = st.backup_state.lock().unwrap_or_else(|e| e.into_inner());
                    cur.last_hash.extend(bs.last_hash);
                    cur.last_time.extend(bs.last_time);
                }
                let _ = st.save_backup_state();
                if !report.backed_up.is_empty() {
                    let _ = app.emit("backup-done", &report.backed_up);
                }
                // Un respaldo que falla no puede quedarse en el registro: el usuario debe verlo.
                if !report.failed.is_empty() {
                    log::warn!("respaldos fallidos: {:?}", report.failed);
                    let _ = app.emit("backup-failed", &report.failed);
                }
            }
            Err(e) => {
                log::warn!("respaldo automático: {e}");
                let _ = app.emit("backup-failed", &vec![("*".to_string(), e.to_string())]);
            }
        }
    });
}
