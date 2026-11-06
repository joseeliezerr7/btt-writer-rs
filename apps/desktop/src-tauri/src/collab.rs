//! Comandos del trabajo en equipo: un repositorio compartido en WACS por
//! proyecto, sincronización automática y reservas de chunk (ver `btt_core::collab`).

use crate::commands::{blocking, check_clone_url, credentials, err, now, open_project, uuid_simple, R};
use crate::AppState;
use btt_core::collab::{self, ClaimView, SyncOptions, SyncReport};
use btt_core::gitrepo::MergeStatus;
use btt_core::tstudio::{ImportMode, ImportOutcome, ImportStatus};
use btt_core::wacs::{Repo, User, WacsClient};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuración de un proyecto en equipo en este dispositivo (no viaja por git).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct CollabConfig {
    pub enabled: bool,
    pub clone_url: String,
    pub html_url: String,
    pub owner: String,
    pub repo: String,
    #[serde(default)]
    pub last_sync: u64,
    #[serde(default)]
    pub last_error: String,
}

pub fn load_configs(path: &std::path::Path) -> HashMap<String, CollabConfig> {
    std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn config(st: &AppState, id: &str) -> Option<CollabConfig> {
    st.collab.lock().unwrap_or_else(|e| e.into_inner()).get(id).cloned()
}

/// Repositorio compartido del proyecto, si el trabajo en equipo está activo.
pub(crate) fn active_remote(st: &AppState, id: &str) -> Option<CollabConfig> {
    config(st, id).filter(|c| c.enabled)
}

fn set_config(st: &AppState, id: &str, cfg: CollabConfig) -> R<()> {
    st.collab.lock().unwrap_or_else(|e| e.into_inner()).insert(id.to_string(), cfg);
    st.save_collab().map_err(err)
}

fn client(st: &AppState) -> R<WacsClient> {
    let api = st.settings.lock().unwrap_or_else(|e| e.into_inner()).git_api_url.clone();
    WacsClient::new(&api).map_err(err)
}

fn session_error(e: btt_core::Error) -> String {
    match e {
        btt_core::Error::Unauthorized => "La sesión expiró. Inicia sesión de nuevo.".to_string(),
        other => other.to_string(),
    }
}

#[derive(Serialize)]
pub struct CollabStatus {
    pub config: Option<CollabConfig>,
    pub claims: Vec<ClaimView>,
    pub conflicts: Vec<String>,
    /// Usuario de este dispositivo (para distinguir reservas propias).
    pub me: String,
    pub syncing: bool,
}

fn status(st: &AppState, id: &str) -> R<CollabStatus> {
    let p = open_project(st, id)?;
    if active_remote(st, id).is_none() {
        // Sin equipo no hay chunks retenidos: si quedó alguno (equipo desactivado a
        // medias), se cierra para que vuelva a entrar en los commits.
        btt_core::hold::close_all(&p.root, None);
    }
    let me = st.profile.lock().unwrap_or_else(|e| e.into_inner()).session.as_ref().map(|s| s.user.username.clone()).unwrap_or_default();
    let syncing = st.collab_running.lock().unwrap_or_else(|e| e.into_inner()).contains(id);
    Ok(CollabStatus {
        config: config(st, id),
        claims: collab::active(&p.root, now()),
        conflicts: btt_core::gitrepo::conflicted_files(&p.root).unwrap_or_default(),
        me,
        syncing,
    })
}

#[tauri::command]
pub async fn collab_status(app: tauri::AppHandle, id: String) -> R<CollabStatus> {
    blocking(app, move |st| status(st, &id)).await
}

/// Proyectos de este dispositivo con trabajo en equipo activo.
#[tauri::command]
pub fn collab_projects(st: tauri::State<AppState>) -> R<Vec<String>> {
    Ok(st.collab.lock().unwrap_or_else(|e| e.into_inner()).iter().filter(|(_, c)| c.enabled).map(|(id, _)| id.clone()).collect())
}

/// Activa el equipo: crea (o reutiliza) el repositorio del usuario en WACS, da acceso
/// a los compañeros y hace la primera sincronización.
#[tauri::command]
pub async fn collab_enable(app: tauri::AppHandle, id: String, members: Vec<String>) -> R<CollabStatus> {
    blocking(app, move |st| {
        let p = open_project(st, &id)?;
        let (creds, username) = credentials(st)?;
        let client = client(st)?;
        let mut cfg = config(st, &id).unwrap_or_default();
        if cfg.clone_url.is_empty() {
            let repo = client.create_repo(&creds.token, &username, &id).map_err(session_error)?;
            cfg.clone_url = repo.clone_url;
            cfg.html_url = repo.html_url;
            cfg.owner = repo.owner.username;
            cfg.repo = repo.name;
        }
        cfg.enabled = true;
        cfg.last_error.clear();
        for m in members.iter().map(|m| m.trim()).filter(|m| !m.is_empty() && !m.eq_ignore_ascii_case(&username)) {
            client.add_collaborator(&creds.token, &cfg.owner, &cfg.repo, m).map_err(|e| format!("No se pudo dar acceso a {m}: {}", session_error(e)))?;
        }
        set_config(st, &id, cfg.clone())?;
        let report = run_sync(st, &id, &p.root, &cfg, None, true);
        if let Some(e) = report.error {
            return Err(e);
        }
        if report.offline {
            return Err("No se pudo contactar con el servidor. Revisa la conexión e inténtalo de nuevo.".into());
        }
        status(st, &id)
    })
    .await
}

/// Desactiva el equipo en este dispositivo: libera las reservas propias y sube lo pendiente.
#[tauri::command]
pub async fn collab_disable(app: tauri::AppHandle, id: String) -> R<CollabStatus> {
    blocking(app, move |st| {
        let p = open_project(st, &id)?;
        if let Some(cfg) = active_remote(st, &id) {
            let me = credentials(st).map(|(_, u)| u).unwrap_or_default();
            let _ = collab::release_all(&p.root, &me, now());
            // La pasada sin chunk en edición cierra las retenciones (y sube lo cerrado).
            let _ = run_sync(st, &id, &p.root, &cfg, None, true);
        }
        btt_core::hold::close_all(&p.root, None);
        let _ = btt_core::hold::clear(&p.root);
        let mut cfg = config(st, &id).unwrap_or_default();
        cfg.enabled = false;
        set_config(st, &id, cfg)?;
        status(st, &id)
    })
    .await
}

#[derive(Serialize)]
pub struct Members {
    pub owner: String,
    pub collaborators: Vec<User>,
    /// Este usuario puede añadir o quitar compañeros (es el dueño del repositorio).
    pub can_manage: bool,
}

#[tauri::command]
pub async fn collab_members(app: tauri::AppHandle, id: String) -> R<Members> {
    blocking(app, move |st| {
        let cfg = config(st, &id).ok_or("El proyecto no está en equipo")?;
        let (creds, username) = credentials(st)?;
        let collaborators = client(st)?.collaborators(&creds.token, &cfg.owner, &cfg.repo).map_err(session_error)?;
        Ok(Members { can_manage: cfg.owner.eq_ignore_ascii_case(&username), owner: cfg.owner, collaborators })
    })
    .await
}

#[tauri::command]
pub async fn collab_add_member(app: tauri::AppHandle, id: String, username: String) -> R<Members> {
    let id2 = id.clone();
    blocking(app.clone(), move |st| {
        let cfg = config(st, &id2).ok_or("El proyecto no está en equipo")?;
        let (creds, _) = credentials(st)?;
        client(st)?.add_collaborator(&creds.token, &cfg.owner, &cfg.repo, username.trim()).map_err(session_error)
    })
    .await?;
    collab_members(app, id).await
}

#[tauri::command]
pub async fn collab_remove_member(app: tauri::AppHandle, id: String, username: String) -> R<Members> {
    let id2 = id.clone();
    blocking(app.clone(), move |st| {
        let cfg = config(st, &id2).ok_or("El proyecto no está en equipo")?;
        let (creds, _) = credentials(st)?;
        client(st)?.remove_collaborator(&creds.token, &cfg.owner, &cfg.repo, username.trim()).map_err(session_error)
    })
    .await?;
    collab_members(app, id).await
}

#[tauri::command]
pub async fn search_users(app: tauri::AppHandle, query: String) -> R<Vec<User>> {
    blocking(app, move |st| {
        if query.trim().len() < 2 {
            return Ok(vec![]);
        }
        client(st)?.search_users(&query, 10).map_err(err)
    })
    .await
}

/// Repositorios de WACS que otros compartieron con este usuario (y los propios), con
/// nombre de proyecto reconocible.
#[tauri::command]
pub async fn collab_shared_repos(app: tauri::AppHandle) -> R<Vec<Repo>> {
    blocking(app, move |st| {
        let (creds, _) = credentials(st)?;
        let mut repos = client(st)?.my_repos(&creds.token).map_err(session_error)?;
        repos.retain(|r| r.parsed_id().is_some());
        repos.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(repos)
    })
    .await
}

/// Se une a un proyecto compartido: lo clona (o lo fusiona con la copia local) y
/// deja el equipo activo apuntando a ese repositorio.
#[tauri::command]
pub async fn collab_join(app: tauri::AppHandle, clone_url: String, mode: ImportMode) -> R<ImportOutcome> {
    blocking(app, move |st| {
        check_clone_url(st, &clone_url)?;
        let (creds, _) = credentials(st)?;
        let staging = st.paths.temp().join(format!("clone-{}", uuid_simple()));
        btt_core::gitrepo::clone(&clone_url, Some(&creds), &staging).map_err(err)?;
        let outcome = btt_core::tstudio::import_dir(&staging, &st.paths.translations(), mode, &st.author());
        let _ = btt_core::project::remove_dir_all_robust(&staging);
        if matches!(outcome.status, ImportStatus::Failed { .. } | ImportStatus::Exists) {
            return Ok(outcome);
        }
        // Dueño y nombre del repositorio a partir de la URL `https://host/owner/repo.git`.
        let mut parts = clone_url.trim_end_matches('/').rsplit('/');
        let repo = parts.next().unwrap_or("").trim_end_matches(".git").to_string();
        let owner = parts.next().unwrap_or("").to_string();
        let html_url = clone_url.trim_end_matches(".git").to_string();
        let mut cfg = config(st, &outcome.id).unwrap_or_default();
        cfg.enabled = true;
        cfg.clone_url = clone_url.clone();
        cfg.html_url = html_url;
        cfg.owner = owner;
        cfg.repo = repo;
        cfg.last_error.clear();
        set_config(st, &outcome.id, cfg)?;
        st.list_cache.lock().unwrap_or_else(|e| e.into_inner()).remove(&outcome.id);
        Ok(outcome)
    })
    .await
}

fn run_sync(st: &AppState, id: &str, root: &std::path::Path, cfg: &CollabConfig, editing: Option<String>, push: bool) -> SyncReport {
    let (creds, user) = match credentials(st) {
        Ok(c) => c,
        Err(e) => return SyncReport { error: Some(e), at: now(), ..Default::default() },
    };
    let author = st.author();
    let report = collab::sync(root, &cfg.clone_url, Some(&creds), &author, &SyncOptions { editing, user, push });
    let mut updated = cfg.clone();
    if report.error.is_none() && !report.offline {
        updated.last_sync = report.at;
        updated.last_error.clear();
    } else if let Some(e) = &report.error {
        updated.last_error = e.clone();
    }
    if updated != *cfg {
        let _ = set_config(st, id, updated);
    }
    if !report.changed.is_empty() {
        st.list_cache.lock().unwrap_or_else(|e| e.into_inner()).remove(id);
    }
    report
}

/// Una pasada de sincronización. `editing` renueva la reserva del chunk que se está
/// escribiendo; `push` sube lo local (si no hay conflictos). Solo corre una por proyecto.
#[tauri::command]
pub async fn collab_sync(app: tauri::AppHandle, id: String, editing: Option<String>, push: bool) -> R<SyncReport> {
    blocking(app, move |st| {
        let cfg = active_remote(st, &id).ok_or("El proyecto no está en equipo")?;
        let p = open_project(st, &id)?;
        {
            let mut running = st.collab_running.lock().unwrap_or_else(|e| e.into_inner());
            if !running.insert(id.clone()) {
                return Ok(SyncReport { error: Some("busy".into()), at: now(), ..Default::default() });
            }
        }
        let report = run_sync(st, &id, &p.root, &cfg, editing, push);
        st.collab_running.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
        Ok(report)
    })
    .await
}

/// Libera la reserva de un chunk (al terminar de editarlo). La siguiente pasada lo sube.
#[tauri::command]
pub async fn collab_release(app: tauri::AppHandle, id: String, key: String) -> R<()> {
    blocking(app, move |st| {
        let p = open_project(st, &id)?;
        let (_, user) = credentials(st)?;
        collab::release(&p.root, &key, &user, now()).map_err(err)
    })
    .await
}

/// Reserva un chunk antes de editarlo y lo retiene: no se sube hasta cerrarlo.
/// `false` si otra persona lo tiene reservado (se retiene igual: el usuario decide).
#[tauri::command]
pub async fn collab_claim(app: tauri::AppHandle, id: String, key: String) -> R<bool> {
    blocking(app, move |st| {
        let p = open_project(st, &id)?;
        let (_, user) = credentials(st)?;
        let t = now();
        // Volver a entrar en un chunk ya reservado no reescribe la reserva (ni genera commit).
        let free = collab::renew(&p.root, &key, &user, t).map_err(err)? != collab::Renewed::Taken;
        btt_core::hold::start(&p.root, &btt_core::hold::chunk_path(&key), t).map_err(err)?;
        Ok(free)
    })
    .await
}

/// Resultado de una fusión con el servidor, para mostrarlo en la interfaz.
#[allow(dead_code)]
pub(crate) fn merge_label(m: &MergeStatus) -> &'static str {
    match m {
        MergeStatus::UpToDate => "al día",
        MergeStatus::FastForward | MergeStatus::Merged => "fusionado",
        MergeStatus::Conflicts { .. } => "conflictos",
        MergeStatus::RemoteEmpty => "servidor vacío",
        MergeStatus::AuthFailure => "sesión rechazada",
        MergeStatus::NoRemoteRepo => "repositorio inexistente",
    }
}
