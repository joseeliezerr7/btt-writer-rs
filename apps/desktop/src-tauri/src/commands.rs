//! Comandos expuestos a la interfaz. Todos devuelven `Result<T, String>` para
//! que el frontend muestre el mensaje de error tal cual.

use crate::AppState;
use tauri::{Emitter, Manager};
use btt_core::appdata::{Profile, Settings};
use btt_core::gitrepo::{self, Credentials, MergeStatus, PushStatus};
use btt_core::library::{CategoryRow, ProjectRow, ResourceRow, TargetLanguageRow};
use btt_core::manifest::{resource_name, type_name, IdName, Manifest, SourceTranslation, TargetLanguage};
use btt_core::project::{Project, ProjectSummary};
use btt_core::rc::{HelpEntry, WordArticle};
use btt_core::tstudio::{ImportMode, ImportOutcome};
use btt_core::usx::SourceChunk;
use btt_core::validation::{self, ChunkValidation, HelpItem, Issue};
use btt_core::wacs::{Repo, WacsClient};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::State;

pub(crate) type R<T> = Result<T, String>;

/// Ejecuta trabajo bloqueante (HTTP con reqwest::blocking, git) en un hilo aparte.
/// Los comandos `async` corren en el runtime de Tauri y un cliente bloqueante ahí
/// aborta con "Cannot start a runtime from within a runtime".
pub(crate) async fn blocking<T: Send + 'static>(app: tauri::AppHandle, f: impl FnOnce(&AppState) -> R<T> + Send + 'static) -> R<T> {
    tauri::async_runtime::spawn_blocking(move || {
        let st = app.state::<AppState>();
        f(&st)
    })
    .await
    .map_err(|e| format!("la tarea se interrumpió: {e}"))?
}

pub(crate) fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// Proyecto fuente con el que se buscan los contenedores: los diccionarios tW se
/// publican como `{lang}_bible_tw`, no bajo el id del proyecto.
fn source_project_slug(m: &Manifest) -> &str {
    if m.type_.id == "tw" { "bible" } else { &m.project.id }
}

pub(crate) fn open_project(st: &AppState, id: &str) -> R<Project> {
    Project::open_by_id(&st.paths.translations(), id).map_err(err)
}

/// Un identificador (idioma, libro, recurso, tipo) que acaba formando nombres de
/// carpeta o de archivo: se rechaza todo lo que no sea un único componente de ruta.
fn check_id(what: &str, s: &str) -> R<()> {
    if btt_core::manifest::safe_id(s) {
        Ok(())
    } else {
        Err(format!("{what} no válido: {s:?}"))
    }
}

/// Host de una URL (`https://host/...` → `host`), sin puerto ni credenciales.
fn url_host(url: &str) -> Option<&str> {
    let rest = url.strip_prefix("https://")?;
    let host = rest.split(['/', '?', '#']).next()?;
    let host = host.rsplit('@').next()?;
    let host = host.split(':').next()?;
    (!host.is_empty()).then_some(host)
}

/// Solo se entrega el token de sesión a repositorios HTTPS del servidor configurado.
pub(crate) fn check_clone_url(st: &AppState, url: &str) -> R<()> {
    let api = st.settings.lock().unwrap_or_else(|e| e.into_inner()).git_api_url.clone();
    match (url_host(url), url_host(&api)) {
        (Some(h), Some(api_host)) if h.eq_ignore_ascii_case(api_host) => Ok(()),
        _ => Err(format!("Solo se pueden clonar repositorios HTTPS del servidor configurado ({url})")),
    }
}

pub(crate) fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

// ------------------------------------------------------------------ app / ajustes

#[derive(Serialize)]
pub struct AppInfo {
    version: String,
    data_dir: String,
    legacy_roots: Vec<String>,
    platform: String,
    device: String,
}

#[tauri::command]
fn app_info(st: State<AppState>) -> R<AppInfo> {
    Ok(AppInfo {
        version: btt_core::VERSION.into(),
        data_dir: st.paths.root.display().to_string(),
        legacy_roots: btt_core::appdata::Paths::legacy_roots().iter().map(|p| p.display().to_string()).collect(),
        platform: std::env::consts::OS.into(),
        device: st.device_name.clone(),
    })
}

#[tauri::command]
fn get_settings(st: State<AppState>) -> R<Settings> {
    Ok(st.settings.lock().unwrap_or_else(|e| e.into_inner()).clone())
}

#[tauri::command]
fn save_settings(st: State<AppState>, settings: Settings) -> R<Settings> {
    let mut s = settings;
    let changed_server = st.settings.lock().unwrap_or_else(|e| e.into_inner()).content_server != s.content_server;
    if changed_server {
        s.apply_server_defaults();
    }
    *st.settings.lock().unwrap_or_else(|e| e.into_inner()) = s.clone();
    st.save_settings().map_err(err)?;
    Ok(s)
}

#[tauri::command]
fn get_profile(st: State<AppState>) -> R<Profile> {
    // La interfaz solo necesita saber si hay sesión y de quién: el token se queda en Rust.
    let mut p = st.profile.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if let Some(s) = p.session.as_mut() {
        s.token.sha1.clear();
    }
    Ok(p)
}

#[tauri::command]
fn save_profile(st: State<AppState>, profile: Profile) -> R<Profile> {
    *st.profile.lock().unwrap_or_else(|e| e.into_inner()) = profile.clone();
    st.save_profile().map_err(err)?;
    Ok(profile)
}

#[tauri::command]
fn logout(st: State<AppState>) -> R<()> {
    let session = st.profile.lock().unwrap_or_else(|e| e.into_inner()).session.clone();
    if let Some(s) = session {
        let api = st.settings.lock().unwrap_or_else(|e| e.into_inner()).git_api_url.clone();
        if let Ok(c) = WacsClient::new(&api) {
            let _ = c.logout(&s);
        }
    }
    *st.profile.lock().unwrap_or_else(|e| e.into_inner()) = Profile::default();
    st.save_profile().map_err(err)
}

#[tauri::command]
async fn wacs_login(app: tauri::AppHandle, username: String, password: String) -> R<Profile> {
    blocking(app, move |st| wacs_login_inner(st, &username, &password)).await
}

fn wacs_login_inner(st: &AppState, username: &str, password: &str) -> R<Profile> {
    let api = st.settings.lock().unwrap_or_else(|e| e.into_inner()).git_api_url.clone();
    let client = WacsClient::new(&api).map_err(err)?;
    let session = client
        .login(&username, &password, &btt_core::wacs::token_name(&st.device_name))
        .map_err(|e| match e {
            btt_core::Error::Unauthorized => "Usuario o contraseña incorrectos".to_string(),
            other => other.to_string(),
        })?;
    let mut p = st.profile.lock().unwrap_or_else(|e| e.into_inner());
    if p.full_name.is_empty() {
        p.full_name = if session.user.full_name.is_empty() { session.user.username.clone() } else { session.user.full_name.clone() };
    }
    if p.email.is_empty() {
        p.email = session.user.email.clone();
    }
    p.session = Some(session);
    let out = p.clone();
    drop(p);
    st.save_profile().map_err(err)?;
    Ok(out)
}

// ------------------------------------------------------------------ proyectos

#[derive(Serialize)]
pub struct ProjectListItem {
    #[serde(flatten)]
    summary: ProjectSummary,
    total_chunks: usize,
    progress: f32,
    has_conflicts: bool,
    last_backup: Option<u64>,
}

/// Total de tarjetas del proyecto y cuántas están terminadas. Cuenta exactamente
/// las tarjetas que el usuario ve (título del libro, títulos de capítulo y chunks),
/// así el "x / y" coincide con lo que hay en pantalla; los ids terminados que ya
/// no existen (otra división de chunks) no se cuentan.
fn chunk_counts(st: &AppState, project: &Project) -> (usize, usize) {
    let m = &project.manifest;
    let mut total = 0;
    let mut finished = 0;
    for (ch, chunks) in card_ids(st, project) {
        total += chunks.len();
        finished += chunks.iter().filter(|c| m.is_chunk_finished(&ch, c)).count();
    }
    (total, finished)
}

fn list_item(st: &AppState, p: &Project) -> ProjectListItem {
    let scan = st.project_scan(p);
    let mut summary = p.summary_from_scan(&scan);
    if summary.project_name.is_empty() {
        let ui_lang = st.settings.lock().unwrap_or_else(|e| e.into_inner()).ui_language.clone();
        let lang = if ui_lang == "es" { "es-419" } else { "en" };
        summary.project_name = st.library.lock().unwrap_or_else(|e| e.into_inner()).project_name(&summary.project_id, lang);
    }
    let (total, finished) = chunk_counts(st, p);
    summary.finished_chunks = finished;
    let progress = if total == 0 { 0.0 } else { (finished as f32 / total as f32).min(1.0) };
    let last_backup = st.backup_state.lock().unwrap_or_else(|e| e.into_inner()).last_time.get(&summary.id).copied();
    ProjectListItem { summary, total_chunks: total, progress, has_conflicts: scan.has_conflicts, last_backup }
}

#[tauri::command]
async fn list_projects(app: tauri::AppHandle) -> R<Vec<ProjectListItem>> {
    blocking(app, move |st| {
        let projects = Project::list(&st.paths.translations()).map_err(err)?;
        // El recorrido de chunks de los proyectos sin caché es E/S pura (y el
        // antivirus la encarece): repartirlo entre varios hilos.
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(2, 8);
        let per = projects.len().div_ceil(threads).max(1);
        let items: Vec<ProjectListItem> = std::thread::scope(|scope| {
            let handles: Vec<_> = projects.chunks(per).map(|group| scope.spawn(move || group.iter().map(|p| list_item(st, p)).collect::<Vec<_>>())).collect();
            handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
        });
        st.save_list_cache();
        Ok(items)
    })
    .await
}

#[derive(Serialize)]
pub struct ChunkInfo {
    id: String,
    finished: bool,
    has_text: bool,
    has_conflict: bool,
}

#[derive(Serialize)]
pub struct ChapterInfo {
    id: String,
    title: String,
    chunks: Vec<ChunkInfo>,
}

#[derive(Serialize)]
pub struct ProjectDetail {
    item: ProjectListItem,
    manifest: Manifest,
    chapters: Vec<ChapterInfo>,
    /// Fuentes abiertas con su estado de descarga.
    sources: Vec<ResourceRow>,
    book_title: String,
}

/// Ids de las tarjetas del proyecto por capítulo (ordenados): unión de la fuente
/// principal (o la versificación del índice) y lo que hay en disco. `front/title`
/// va siempre primero; en los proyectos de palabras (tw) solo hay el capítulo "01".
fn card_ids(st: &AppState, project: &Project) -> Vec<(String, Vec<String>)> {
    let m = &project.manifest;
    let mut chapters: Vec<(String, Vec<String>)> = vec![];
    let push = |ch: &str, chunks: Vec<String>, chapters: &mut Vec<(String, Vec<String>)>| {
        if let Some((_, existing)) = chapters.iter_mut().find(|(c, _)| c == ch) {
            for c in chunks {
                if !existing.contains(&c) {
                    existing.push(c);
                }
            }
        } else {
            chapters.push((ch.to_string(), chunks));
        }
    };
    let source_rc = m
        .source_translations
        .iter()
        .find_map(|s| st.container(&s.language_id, source_project_slug(m), &s.resource_id).ok());
    if m.type_.id == "tw" {
        // Palabras: chunks = slugs del diccionario fuente + los existentes.
        let mut words: Vec<String> = source_rc.as_ref().map(|rc| rc.word_slugs()).unwrap_or_default();
        for c in project.chunks("01").unwrap_or_default() {
            if !words.contains(&c) {
                words.push(c);
            }
        }
        words.sort();
        push("01", words, &mut chapters);
    } else {
        push("front", vec!["title".into()], &mut chapters);
        if let Some(rc) = &source_rc {
            for ch in rc.chapters() {
                let mut chunks = vec!["title".to_string()];
                if m.resource.id == "obs" {
                    chunks.push("reference".into());
                }
                chunks.extend(rc.chunks(&ch));
                push(&ch, chunks, &mut chapters);
            }
        } else if m.type_.id == "text" {
            let lib = st.library.lock().unwrap_or_else(|e| e.into_inner());
            for (ch, verses) in lib.chunk_markers(&m.project.id).unwrap_or_default() {
                let mut chunks = vec!["title".to_string()];
                chunks.extend(verses.iter().map(|v| format!("{v:02}")));
                push(&ch, chunks, &mut chapters);
            }
        }
        for ch in project.chapters().unwrap_or_default() {
            let mut chunks = vec!["title".to_string()];
            chunks.extend(project.chunks(&ch).unwrap_or_default());
            push(&ch, chunks, &mut chapters);
        }
    }
    chapters.sort_by(|a, b| btt_core::project::natural_cmp(&a.0, &b.0));
    for (_, chunks) in &mut chapters {
        chunks.sort_by(|a, b| btt_core::project::chunk_cmp(a, b));
    }
    chapters
}

/// Estructura de capítulos/chunks con su estado (texto, conflicto, terminado).
fn chapter_structure(st: &AppState, project: &Project) -> R<Vec<ChapterInfo>> {
    let m = &project.manifest;
    let source_rc = m
        .source_translations
        .iter()
        .find_map(|s| st.container(&s.language_id, source_project_slug(m), &s.resource_id).ok());
    let mut chapters: Vec<ChapterInfo> = card_ids(st, project)
        .into_iter()
        .map(|(id, chunks)| ChapterInfo {
            id,
            title: String::new(),
            chunks: chunks.into_iter().map(|c| ChunkInfo { id: c, finished: false, has_text: false, has_conflict: false }).collect(),
        })
        .collect();
    for ch in &mut chapters {
        if ch.id != "front" {
            ch.title = project.read_chapter_title(&ch.id).unwrap_or_default();
            if ch.title.is_empty() {
                if let Some(rc) = &source_rc {
                    ch.title = rc.chapter_title(&ch.id);
                }
            }
        }
        for c in &mut ch.chunks {
            let text = project.read_chunk(&ch.id, &c.id).unwrap_or_default();
            c.has_text = !text.trim().is_empty();
            c.has_conflict = btt_core::conflicts::has_conflicts(&text);
            c.finished = m.is_chunk_finished(&ch.id, &c.id);
        }
    }
    Ok(chapters)
}

#[tauri::command]
async fn get_project(app: tauri::AppHandle, id: String) -> R<ProjectDetail> {
    blocking(app, move |st| {
    let p = open_project(st, &id)?;
    let chapters = chapter_structure(st, &p)?;
    let lib = st.library.lock().unwrap_or_else(|e| e.into_inner());
    let mut sources = vec![];
    for s in &p.manifest.source_translations {
        let project_slug = if p.manifest.type_.id == "tw" { "bible" } else { &p.manifest.project.id };
        if let Ok(Some(r)) = lib.resource(&s.language_id, project_slug, &s.resource_id) {
            sources.push(r);
        } else {
            sources.push(ResourceRow {
                language_slug: s.language_id.clone(),
                language_name: s.language_id.clone(),
                language_direction: "ltr".into(),
                project_slug: project_slug.into(),
                project_name: p.manifest.project.name.clone(),
                slug: s.resource_id.clone(),
                name: resource_name(&s.resource_id).into(),
                r#type: "book".into(),
                translate_mode: String::new(),
                checking_level: s.checking_level.clone(),
                version: s.version.clone(),
                pub_date: String::new(),
                mime_type: String::new(),
                url: String::new(),
                modified_at: 0,
                downloaded: lib.has_container(&s.language_id, project_slug, &s.resource_id),
            });
        }
    }
    drop(lib);
    Ok(ProjectDetail {
        item: list_item(st, &p),
        book_title: p.read_book_title().unwrap_or_default(),
        manifest: p.manifest.clone(),
        chapters,
        sources,
    })
}).await
}

#[derive(Deserialize)]
pub struct NewProject {
    language: String,
    project: String,
    #[serde(rename = "type")]
    type_id: String,
    resource: String,
}

#[tauri::command]
async fn create_project(app: tauri::AppHandle, spec: NewProject) -> R<String> {
    blocking(app, move |st| {
    check_id("idioma", &spec.language)?;
    check_id("libro", &spec.project)?;
    if !spec.type_id.is_empty() { check_id("tipo", &spec.type_id)?; }
    if !spec.resource.is_empty() { check_id("recurso", &spec.resource)?; }
    let lib = st.library.lock().unwrap_or_else(|e| e.into_inner());
    let lang = lib
        .target_language(&spec.language)
        .map_err(err)?
        .ok_or_else(|| format!("idioma {} no encontrado", spec.language))?;
    let project_name = lib.project_name(&spec.project, "en");
    drop(lib);
    let type_id = if spec.type_id.is_empty() { "text" } else { &spec.type_id };
    let resource_id = if type_id == "text" {
        if spec.resource.is_empty() { "reg".to_string() } else { spec.resource.clone() }
    } else {
        String::new()
    };
    let mut manifest = Manifest::new(
        TargetLanguage::from(lang),
        IdName::new(&spec.project, &project_name),
        IdName::new(type_id, type_name(type_id)),
        IdName::new(&resource_id, resource_name(&resource_id)),
        btt_core::VERSION,
    );
    // Nombre del proyecto se guarda vacío como hace la app original (se calcula al mostrar).
    manifest.project.name = String::new();
    let translator = st.profile.lock().unwrap_or_else(|e| e.into_inner()).display_name();
    manifest.add_translator(&translator);
    // Fuente por defecto: la primera descargada para este libro (en/ulb preferida).
    {
        let lib = st.library.lock().unwrap_or_else(|e| e.into_inner());
        let project_slug = if type_id == "tw" { "bible" } else { spec.project.as_str() };
        let mut sources = lib.resources(project_slug).unwrap_or_default();
        sources.retain(|r| r.downloaded && (if type_id == "tw" { r.r#type == "dict" } else { r.r#type == "book" }));
        sources.sort_by_key(|r| (r.language_slug != "en", r.slug != "ulb"));
        if let Some(r) = sources.first() {
            manifest.add_source_translation(SourceTranslation {
                language_id: r.language_slug.clone(),
                resource_id: r.slug.clone(),
                checking_level: r.checking_level.clone(),
                date_modified: r.modified_at.max(0) as u64,
                version: r.version.clone(),
            });
        }
    }
    let p = Project::create(&st.paths.translations(), manifest).map_err(|e| match e {
        btt_core::Error::ProjectExists(id) => format!("Ya estás traduciendo este libro a este idioma ({id})"),
        other => other.to_string(),
    })?;
    gitrepo::commit_all(&p.root, &st.author(), "create project").map_err(err)?;
    let id = p.id();
    st.settings.lock().unwrap_or_else(|e| e.into_inner()).last_project = id.clone();
    let _ = st.save_settings();
    Ok(id)
}).await
}

#[tauri::command]
async fn delete_project(app: tauri::AppHandle, id: String) -> R<String> {
    blocking(app, move |st| {
    let p = open_project(st, &id)?;
    // Respaldo previo con fecha.
    let stamp = chrono_like_stamp();
    let dest = st.paths.backups().join(format!("{id}.{stamp}.tstudio"));
    let backup = btt_core::tstudio::export(&[&p], &dest, &st.author(), btt_core::VERSION).map_err(err)?;
    p.delete().map_err(err)?;
    let mut s = st.settings.lock().unwrap_or_else(|e| e.into_inner());
    if s.last_project == id {
        s.last_project.clear();
    }
    drop(s);
    let _ = st.save_settings();
    Ok(backup.display().to_string())
}).await
}

fn chrono_like_stamp() -> String {
    // yyyyMMdd-HHmmss sin dependencia de zona horaria (UTC).
    let secs = now() as i64;
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}{m:02}{d:02}-{:02}{:02}{:02}", rem / 3600, (rem % 3600) / 60, rem % 60)
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[tauri::command]
async fn change_resource(app: tauri::AppHandle, id: String, resource: String) -> R<String> {
    blocking(app, move |st| {
    check_id("recurso", &resource)?;
    let p = open_project(st, &id)?;
    let p = p.change_resource(IdName::new(&resource, resource_name(&resource))).map_err(err)?;
    gitrepo::commit_all(&p.root, &st.author(), "change resource").map_err(err)?;
    Ok(p.id())
}).await
}

#[tauri::command]
async fn set_translators(app: tauri::AppHandle, id: String, translators: Vec<String>) -> R<Vec<String>> {
    blocking(app, move |st| {
    let mut p = open_project(st, &id)?;
    p.manifest.translators.clear();
    for t in translators {
        p.manifest.add_translator(&t);
    }
    p.save_manifest().map_err(err)?;
    Ok(p.manifest.translators)
}).await
}

#[tauri::command]
fn set_book_title(st: State<AppState>, id: String, title: String) -> R<()> {
    let p = open_project(&st, &id)?;
    p.write_book_title(&title).map_err(err)
}

#[derive(Deserialize)]
pub struct SourceRef {
    language: String,
    resource: String,
}

#[tauri::command]
async fn set_project_sources(app: tauri::AppHandle, id: String, sources: Vec<SourceRef>) -> R<Manifest> {
    blocking(app, move |st| {
    let mut p = open_project(st, &id)?;
    let lib = st.library.lock().unwrap_or_else(|e| e.into_inner());
    let project_slug = if p.manifest.type_.id == "tw" { "bible".to_string() } else { p.manifest.project.id.clone() };
    p.manifest.source_translations.clear();
    for s in sources.into_iter().take(3) {
        let row = lib.resource(&s.language, &project_slug, &s.resource).unwrap_or(None);
        p.manifest.add_source_translation(SourceTranslation {
            language_id: s.language,
            resource_id: s.resource,
            checking_level: row.as_ref().map(|r| r.checking_level.clone()).unwrap_or_else(|| "3".into()),
            date_modified: row.as_ref().map(|r| r.modified_at.max(0) as u64).unwrap_or(0),
            version: row.as_ref().map(|r| r.version.clone()).unwrap_or_default(),
        });
    }
    drop(lib);
    p.save_manifest().map_err(err)?;
    Ok(p.manifest)
}).await
}

// ------------------------------------------------------------------ chunks

#[derive(Serialize)]
pub struct ChunkData {
    text: String,
    finished: bool,
    /// Versos que debe contener según la fuente principal.
    expected_verses: Vec<u32>,
    issues: Vec<Issue>,
    has_conflict: bool,
    conflict_options: Vec<String>,
    /// Commit de origen (autor, fecha) de cada versión, o `None` si no consta.
    conflict_origins: Vec<Option<gitrepo::Commit>>,
    helps: Vec<HelpItem>,
}

fn expected_verses(st: &AppState, p: &Project, chapter: &str, chunk: &str) -> Vec<u32> {
    if chapter == "front" || chunk == "title" || chunk == "reference" || p.manifest.type_.id != "text" || p.manifest.resource.id == "obs" {
        return vec![];
    }
    for s in &p.manifest.source_translations {
        if let Ok(rc) = st.container(&s.language_id, source_project_slug(&p.manifest), &s.resource_id) {
            if let Some(sc) = rc.source_chunk(chapter, chunk) {
                let mut v: Vec<u32> = sc
                    .verses
                    .iter()
                    .flat_map(|x| {
                        let a = btt_core::usfm::first_number(&x.number);
                        let b = btt_core::usfm::last_number(&x.number).max(a);
                        a..=b
                    })
                    .filter(|&n| n > 0)
                    .collect();
                v.dedup();
                return v;
            }
        }
    }
    vec![]
}

#[tauri::command]
async fn read_chunk(app: tauri::AppHandle, id: String, chapter: String, chunk: String) -> R<ChunkData> {
    blocking(app, move |st| {
    let p = open_project(st, &id)?;
    let mut text = p.read_chunk(&chapter, &chunk).map_err(err)?;
    let expected = expected_verses(st, &p, &chapter, &chunk);
    // Como la app original: añadir marcadores de versos faltantes al abrir.
    if !expected.is_empty() && !text.trim().is_empty() {
        let fixed = validation::add_missing_verses(&text, &expected);
        if fixed != text {
            p.write_chunk(&chapter, &chunk, &fixed).map_err(err)?;
            text = fixed;
        }
    }
    let has_conflict = btt_core::conflicts::has_conflicts(&text);
    let is_help = p.manifest.type_.id != "text";
    let conflict_options = if has_conflict { btt_core::conflicts::parse(&text) } else { vec![] };
    // Autor y fecha de cada versión según el historial git del chunk.
    let conflict_origins = if has_conflict {
        gitrepo::version_origins(&p.root, &format!("{chapter}/{chunk}.txt"), &conflict_options).unwrap_or_else(|_| vec![None; conflict_options.len()])
    } else {
        vec![]
    };
    Ok(ChunkData {
        issues: if is_help { vec![] } else { validation::validate_chunk(&text, &expected) },
        conflict_options,
        conflict_origins,
        helps: if is_help && !has_conflict { validation::parse_helps(&text) } else { vec![] },
        finished: p.manifest.is_chunk_finished(&chapter, &chunk),
        expected_verses: expected,
        has_conflict,
        text,
    })
}).await
}

#[tauri::command]
async fn write_chunk(app: tauri::AppHandle, id: String, chapter: String, chunk: String, text: String) -> R<()> {
    blocking(app, move |st| {
    let p = open_project(st, &id)?;
    if text.trim().is_empty() && chunk != "title" && chunk != "reference" {
        // Igual que la app original: texto vacío borra el archivo.
        p.delete_chunk(&chapter, &chunk).map_err(err)
    } else {
        p.write_chunk(&chapter, &chunk, &text).map_err(err)
    }
}).await
}

#[tauri::command]
async fn write_helps(app: tauri::AppHandle, id: String, chapter: String, chunk: String, items: Vec<HelpItem>) -> R<String> {
    blocking(app, move |st| {
    let p = open_project(st, &id)?;
    let text = validation::serialize_helps(&items);
    if text.is_empty() {
        p.delete_chunk(&chapter, &chunk).map_err(err)?;
    } else {
        p.write_chunk(&chapter, &chunk, &text).map_err(err)?;
    }
    Ok(text)
}).await
}

#[tauri::command]
async fn set_chunk_finished(app: tauri::AppHandle, id: String, chapter: String, chunk: String, finished: bool) -> R<Vec<Issue>> {
    blocking(app, move |st| {
    let mut p = open_project(st, &id)?;
    if finished {
        let text = p.read_chunk(&chapter, &chunk).map_err(err)?;
        let issues = if p.manifest.type_.id == "text" {
            let expected = expected_verses(st, &p, &chapter, &chunk);
            validation::validate_chunk(&text, &expected)
        } else if validation::helps_complete(&validation::parse_helps(&text)) {
            vec![]
        } else {
            vec![Issue::Empty]
        };
        if !issues.is_empty() {
            return Ok(issues);
        }
    }
    p.set_chunk_finished(&chapter, &chunk, finished).map_err(err)?;
    gitrepo::commit_all(&p.root, &st.author(), gitrepo::AUTOSAVE_MESSAGE).map_err(err)?;
    Ok(vec![])
}).await
}

#[derive(Serialize)]
pub struct MarkAllResult {
    marked: usize,
    total: usize,
    invalid: Vec<ChunkValidation>,
}

#[tauri::command]
async fn mark_all_chunks(app: tauri::AppHandle, id: String) -> R<MarkAllResult> {
    blocking(app, move |st| {
    let mut p = open_project(st, &id)?;
    let m = p.manifest.clone();
    let rc = m.source_translations.iter().find_map(|s| st.container(&s.language_id, source_project_slug(&m), &s.resource_id).ok());
    let results = validation::validate_project(&p, rc.as_deref()).map_err(err)?;
    let mut marked = 0;
    let mut invalid = vec![];
    for r in &results {
        if r.issues.is_empty() {
            p.manifest.set_chunk_finished(&r.chapter, &r.chunk, true);
            marked += 1;
        } else if !r.issues.contains(&Issue::Empty) {
            invalid.push(r.clone());
        }
    }
    p.save_manifest().map_err(err)?;
    gitrepo::commit_all(&p.root, &st.author(), gitrepo::AUTOSAVE_MESSAGE).map_err(err)?;
    Ok(MarkAllResult { marked, total: results.len(), invalid })
}).await
}

#[tauri::command]
async fn validate_project(app: tauri::AppHandle, id: String) -> R<Vec<ChunkValidation>> {
    blocking(app, move |st| {
    let p = open_project(st, &id)?;
    let m = &p.manifest;
    let rc = m.source_translations.iter().find_map(|s| st.container(&s.language_id, source_project_slug(m), &s.resource_id).ok());
    validation::validate_project(&p, rc.as_deref()).map_err(err)
}).await
}

#[tauri::command]
async fn commit_project(app: tauri::AppHandle, id: String) -> R<Option<String>> {
    blocking(app, move |st| {
    let p = open_project(st, &id)?;
    gitrepo::commit_all(&p.root, &st.author(), gitrepo::AUTOSAVE_MESSAGE).map_err(err)
}).await
}

/// Una versión guardada de un chunk, con un fragmento del texto para distinguirla.
#[derive(Serialize)]
pub struct HistoryEntry {
    id: String,
    time: i64,
    message: String,
    author: String,
    preview: String,
    /// El texto de esta versión es el que está ahora en el proyecto.
    current: bool,
}

/// Versiones guardadas de un chunk (la más reciente primero). Guarda antes los cambios pendientes.
#[tauri::command]
async fn chunk_history(app: tauri::AppHandle, id: String, chapter: String, chunk: String) -> R<Vec<HistoryEntry>> {
    blocking(app, move |st| {
    let p = open_project(st, &id)?;
    gitrepo::commit_all(&p.root, &st.author(), gitrepo::AUTOSAVE_MESSAGE).map_err(err)?;
    history_entries(&p, &chapter, &chunk)
}).await
}

/// Quién escribió cada versículo del chunk, según el historial git (ver `btt_core::authorship`).
#[tauri::command]
async fn chunk_authors(app: tauri::AppHandle, id: String, chapter: String, chunk: String) -> R<Vec<btt_core::authorship::VerseAuthor>> {
    blocking(app, move |st| {
    let p = open_project(st, &id)?;
    let current = p.read_chunk(&chapter, &chunk).unwrap_or_default();
    let me = st.author().name;
    Ok(btt_core::authorship::verse_authors(&p.root, &format!("{chapter}/{chunk}.txt"), &current, &me))
}).await
}

/// Versiones distintas de un chunk, la más reciente primero (ver `chunk_history`).
fn history_entries(p: &Project, chapter: &str, chunk: &str) -> R<Vec<HistoryEntry>> {
    let file = format!("{chapter}/{chunk}.txt");
    let norm = |s: &str| s.replace("\r\n", "\n").trim().to_string();
    let current = norm(&p.read_chunk(chapter, chunk).unwrap_or_default());
    // Cada texto distinto aparece una sola vez, con la fecha y el autor de cuando se escribió
    // por primera vez: restaurar no añade versiones, solo cambia cuál es la actual.
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut out: Vec<HistoryEntry> = vec![];
    for c in gitrepo::file_history(&p.root, &file).map_err(err)? {
        let text = gitrepo::file_at(&p.root, &c.id, &file).unwrap_or_default();
        let key = norm(&text);
        if let Some(&i) = seen.get(&key) {
            // Aparición más antigua del mismo texto: es su origen real.
            let e = &mut out[i];
            if c.time < e.time && !c.message.starts_with("restore ") {
                e.time = c.time;
                e.author = c.author;
                e.message = c.message;
            }
            continue;
        }
        let body = text.trim_start();
        let body = body.strip_prefix("\\c ").map(|r| r.trim_start_matches(|ch: char| ch.is_ascii_digit()).trim_start()).unwrap_or(body);
        let plain = btt_core::usfm::to_plain_text(body).split_whitespace().collect::<Vec<_>>().join(" ");
        let preview: String = plain.chars().take(110).collect();
        seen.insert(key.clone(), out.len());
        out.push(HistoryEntry { current: key == current, id: c.id, time: c.time, message: c.message, author: c.author, preview });
    }
    out.sort_by(|a, b| b.time.cmp(&a.time));
    Ok(out)
}

/// Vuelve a una versión anterior de un chunk y la registra en el historial como
/// "restore <commit>" (la versión que había se conserva y se puede recuperar).
/// Si el texto ya era el mismo, no crea ninguna entrada.
#[tauri::command]
async fn restore_chunk(app: tauri::AppHandle, id: String, commit: String, chapter: String, chunk: String) -> R<String> {
    blocking(app, move |st| {
    let p = open_project(st, &id)?;
    let file = format!("{chapter}/{chunk}.txt");
    let text = gitrepo::file_at(&p.root, &commit, &file).map_err(err)?;
    let norm = |s: &str| s.replace("\r\n", "\n").trim().to_string();
    if norm(&text) == norm(&p.read_chunk(&chapter, &chunk).unwrap_or_default()) {
        return Ok(text);
    }
    if text.trim().is_empty() && chunk != "title" && chunk != "reference" {
        p.delete_chunk(&chapter, &chunk).map_err(err)?;
    } else {
        p.write_chunk(&chapter, &chunk, &text).map_err(err)?;
    }
    let short: String = commit.chars().take(7).collect();
    gitrepo::commit_all(&p.root, &st.author(), &format!("restore {short}")).map_err(err)?;
    Ok(text)
}).await
}

#[tauri::command]
fn chunk_at_commit(st: State<AppState>, id: String, commit: String, chapter: String, chunk: String) -> R<String> {
    let p = open_project(&st, &id)?;
    gitrepo::file_at(&p.root, &commit, &format!("{chapter}/{chunk}.txt")).map_err(err)
}

#[derive(Serialize)]
pub struct SearchHit {
    chapter: String,
    chunk: String,
    snippet: String,
}

#[tauri::command]
async fn search_project(app: tauri::AppHandle, id: String, query: String, in_source: bool) -> R<Vec<SearchHit>> {
    blocking(app, move |st| {
    let p = open_project(st, &id)?;
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Ok(vec![]);
    }
    let mut hits = vec![];
    if in_source {
        let m = &p.manifest;
        if let Some(rc) = m.source_translations.iter().find_map(|s| st.container(&s.language_id, source_project_slug(m), &s.resource_id).ok()) {
            for ch in rc.chapters() {
                for c in rc.chunks(&ch) {
                    let text = rc.source_chunk(&ch, &c).map(|s| s.plain_text()).unwrap_or_default();
                    if let Some(snippet) = snippet_for(&text, &q) {
                        hits.push(SearchHit { chapter: ch.clone(), chunk: c, snippet });
                    }
                }
            }
        }
    } else {
        for ch in p.chapters().map_err(err)? {
            for c in p.chunks(&ch).map_err(err)? {
                let text = btt_core::usfm::to_plain_text(&p.read_chunk(&ch, &c).map_err(err)?);
                if let Some(snippet) = snippet_for(&text, &q) {
                    hits.push(SearchHit { chapter: ch.clone(), chunk: c, snippet });
                }
            }
        }
    }
    Ok(hits)
}).await
}

fn snippet_for(text: &str, q: &str) -> Option<String> {
    // `to_lowercase` puede cambiar la longitud en bytes (İ, K de Kelvin…): se busca
    // carácter a carácter y se corta por límites de carácter del texto original.
    let q: Vec<char> = q.to_lowercase().chars().collect();
    if q.is_empty() {
        return None;
    }
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let lower: Vec<char> = chars.iter().map(|(_, c)| c.to_lowercase().next().unwrap_or(*c)).collect();
    let hit = (0..lower.len()).find(|&i| lower[i..].starts_with(&q))?;
    let start = chars[hit.saturating_sub(40)].0;
    let end = chars.get(hit + q.len() + 60).map(|(j, _)| *j).unwrap_or(text.len());
    Some(text[start..end].replace('\n', " "))
}

// ------------------------------------------------------------------ fuentes y ayudas

#[derive(Serialize)]
pub struct SourceChunkView {
    chapter: String,
    chunk: String,
    chunk_title: String,
    #[serde(flatten)]
    content: SourceChunk,
    /// Para OBS/markdown: texto crudo.
    raw: String,
}

#[tauri::command]
fn get_source_chapter(st: State<AppState>, language: String, project: String, resource: String, chapter: String) -> R<Vec<SourceChunkView>> {
    let rc = st.container(&language, &project, &resource).map_err(err)?;
    let mut out = vec![];
    for c in rc.chunks(&chapter) {
        let raw = rc.read_chunk(&chapter, &c).unwrap_or_default();
        let content = if rc.package.content_mime_type.contains("usx") { btt_core::usx::parse_chunk(&raw) } else { SourceChunk::default() };
        let title = content.verses.first().map(|v| v.number.clone()).unwrap_or_default();
        let last = content.verses.last().map(|v| v.number.clone()).unwrap_or_default();
        out.push(SourceChunkView {
            chapter: chapter.clone(),
            chunk: c,
            chunk_title: if title.is_empty() || title == last { title } else { format!("{title}-{}", btt_core::usfm::last_number(&last)) },
            content,
            raw,
        });
    }
    Ok(out)
}

#[derive(Serialize)]
pub struct SourceInfo {
    book_title: String,
    chapters: Vec<String>,
    chapter_titles: Vec<String>,
    direction: String,
    language_name: String,
    resource_name: String,
}

#[tauri::command]
fn get_source_info(st: State<AppState>, language: String, project: String, resource: String) -> R<SourceInfo> {
    let rc = st.container(&language, &project, &resource).map_err(err)?;
    let chapters = rc.chapters();
    Ok(SourceInfo {
        book_title: rc.book_title(),
        chapter_titles: chapters.iter().map(|c| rc.chapter_title(c)).collect(),
        chapters,
        direction: rc.package.language.direction.clone(),
        language_name: rc.package.language.name.clone(),
        resource_name: rc.package.resource.name.clone(),
    })
}

#[derive(Serialize)]
pub struct Helps {
    notes: Vec<HelpEntry>,
    questions: Vec<HelpEntry>,
    words: Vec<WordArticle>,
    has_notes: bool,
    has_questions: bool,
    has_words: bool,
}

#[tauri::command]
fn get_helps(st: State<AppState>, language: String, project: String, resource: String, chapter: String, chunk: String) -> R<Helps> {
    let (has_notes, has_questions, has_words) = {
        let lib = st.library.lock().unwrap_or_else(|e| e.into_inner());
        (lib.has_container(&language, &project, "tn"), lib.has_container(&language, &project, "tq"), lib.has_container(&language, "bible", "tw"))
    };
    let notes = if has_notes { st.container(&language, &project, "tn").map(|rc| rc.help_entries(&chapter, &chunk)).unwrap_or_default() } else { vec![] };
    let questions = if has_questions { st.container(&language, &project, "tq").map(|rc| rc.help_entries(&chapter, &chunk)).unwrap_or_default() } else { vec![] };
    let mut words = vec![];
    if has_words {
        if let (Ok(book), Ok(dict)) = (st.container(&language, &project, &resource), st.container(&language, "bible", "tw")) {
            for slug in book.words_for_chunk(&chapter, &chunk) {
                if let Some(w) = dict.word(&slug) {
                    words.push(w);
                }
            }
        }
    }
    Ok(Helps { notes, questions, words, has_notes, has_questions, has_words })
}

#[tauri::command]
fn get_word(st: State<AppState>, language: String, slug: String) -> R<Option<WordArticle>> {
    Ok(st.container(&language, "bible", "tw").ok().and_then(|rc| rc.word(&slug)))
}

#[derive(Serialize)]
pub struct WordIndexEntry {
    slug: String,
    title: String,
}

#[tauri::command]
fn word_index(st: State<AppState>, language: String, query: String) -> R<Vec<WordIndexEntry>> {
    let rc = st.container(&language, "bible", "tw").map_err(err)?;
    let q = query.trim().to_lowercase();
    let mut out: Vec<WordIndexEntry> = rc
        .word_slugs()
        .into_iter()
        .filter_map(|s| rc.word(&s).map(|w| WordIndexEntry { slug: s, title: w.title }))
        .filter(|w| q.is_empty() || w.title.to_lowercase().contains(&q) || w.slug.contains(&q))
        .collect();
    out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    Ok(out)
}

// ------------------------------------------------------------------ biblioteca

#[tauri::command]
fn search_languages(st: State<AppState>, query: String, limit: usize) -> R<Vec<TargetLanguageRow>> {
    st.library.lock().unwrap_or_else(|e| e.into_inner()).target_languages(&query, limit).map_err(err)
}

#[tauri::command]
fn list_books(st: State<AppState>, language: String) -> R<Vec<ProjectRow>> {
    let lib = st.library.lock().unwrap_or_else(|e| e.into_inner());
    let mut rows = lib.projects(&language).map_err(err)?;
    rows.sort_by_key(|p| (btt_core::bible_sort_index(&p.slug), p.sort));
    Ok(rows)
}

#[tauri::command]
fn list_categories(st: State<AppState>, language: String) -> R<Vec<CategoryRow>> {
    st.library.lock().unwrap_or_else(|e| e.into_inner()).categories(&language).map_err(err)
}

#[tauri::command]
fn available_sources(st: State<AppState>, project: String, gl_mode: bool) -> R<Vec<ResourceRow>> {
    let lib = st.library.lock().unwrap_or_else(|e| e.into_inner());
    let mut rows = lib.source_texts(&project, gl_mode).map_err(err)?;
    rows.sort_by_key(|r| (!r.downloaded, r.language_slug.clone(), r.slug.clone()));
    Ok(rows)
}

#[tauri::command]
fn all_resources(st: State<AppState>, project: String) -> R<Vec<ResourceRow>> {
    st.library.lock().unwrap_or_else(|e| e.into_inner()).resources(&project).map_err(err)
}

#[tauri::command]
fn source_languages(st: State<AppState>) -> R<Vec<btt_core::library::SourceLanguageRow>> {
    st.library.lock().unwrap_or_else(|e| e.into_inner()).source_languages().map_err(err)
}

#[tauri::command]
async fn download_source(app: tauri::AppHandle, language: String, project: String, resource: String) -> R<Vec<String>> {
    blocking(app, move |st| {
    check_id("idioma", &language)?;
    check_id("libro", &project)?;
    check_id("recurso", &resource)?;
    let row = st
        .library
        .lock()
        .unwrap()
        .resource(&language, &project, &resource)
        .map_err(err)?
        .ok_or_else(|| "recurso no encontrado en el catálogo".to_string())?;
    let client = btt_core::sources::http_client().map_err(err)?;
    let r = btt_core::sources::download_resource(&st.library, &row, &client, true).map_err(err);
    st.clear_rc_cache();
    r
}).await
}

#[derive(Serialize, Clone)]
pub struct DownloadProgress {
    done: usize,
    total: usize,
    current: String,
}

/// Descarga todos los libros de un idioma y recurso (p. ej. es-419 / ulb) que aún no
/// estén en disco, con sus ayudas. Emite `download-progress` por cada libro.
#[tauri::command]
async fn download_language_sources(app: tauri::AppHandle, language: String, resource: String) -> R<Vec<String>> {
    let handle = app.clone();
    blocking(app, move |st| {
        let rows: Vec<ResourceRow> = st
            .library
            .lock()
            .unwrap()
            .resources("")
            .map_err(err)?
            .into_iter()
            .filter(|r| r.r#type == "book" && r.language_slug == language && r.slug == resource && !r.downloaded && !r.url.is_empty())
            .collect();
        let client = btt_core::sources::http_client().map_err(err)?;
        let total = rows.len();
        let mut done = vec![];
        let mut failed = vec![];
        for (i, row) in rows.iter().enumerate() {
            let _ = handle.emit("download-progress", DownloadProgress { done: i, total, current: row.project_name.clone() });
            let result = btt_core::sources::download_resource(&st.library, row, &client, true);
            st.clear_rc_cache();
            match result {
                Ok(mut d) => done.append(&mut d),
                Err(e) => failed.push(format!("{}: {e}", row.project_slug)),
            }
        }
        let _ = handle.emit("download-progress", DownloadProgress { done: total, total, current: String::new() });
        if !failed.is_empty() && done.is_empty() {
            return Err(failed.join("\n"));
        }
        if !failed.is_empty() {
            log::warn!("libros no descargados: {}", failed.join("; "));
        }
        Ok(done)
    })
    .await
}

#[tauri::command]
fn delete_source(st: State<AppState>, language: String, project: String, resource: String) -> R<()> {
    check_id("idioma", &language)?;
    check_id("libro", &project)?;
    check_id("recurso", &resource)?;
    st.library.lock().unwrap_or_else(|e| e.into_inner()).delete_container(&language, &project, &resource).map_err(err)?;
    st.clear_rc_cache();
    Ok(())
}

#[tauri::command]
async fn update_catalog(app: tauri::AppHandle) -> R<Vec<String>> {
    blocking(app, move |st| {
    let url = st.settings.lock().unwrap_or_else(|e| e.into_inner()).root_catalog_url();
    let client = btt_core::sources::http_client().map_err(err)?;
    let lib = st.library.lock().unwrap_or_else(|e| e.into_inner());
    btt_core::sources::update_catalog(&lib, &client, &url).map_err(err)
}).await
}

#[tauri::command]
async fn update_languages(app: tauri::AppHandle) -> R<usize> {
    blocking(app, move |st| {
    let url = st.settings.lock().unwrap_or_else(|e| e.into_inner()).langnames_url.clone();
    let client = btt_core::sources::http_client().map_err(err)?;
    let lib = st.library.lock().unwrap_or_else(|e| e.into_inner());
    btt_core::sources::update_languages(
        &lib,
        &client,
        &url,
        Some("https://td.unfoldingword.org/api/templanguages/"),
        Some("https://td.unfoldingword.org/api/templanguages/assignment/changed/"),
    )
    .map_err(err)
}).await
}

#[tauri::command]
async fn download_index(app: tauri::AppHandle) -> R<()> {
    blocking(app, move |st| {
    let url = st.settings.lock().unwrap_or_else(|e| e.into_inner()).index_sqlite_url.clone();
    let client = btt_core::sources::http_client().map_err(err)?;
    let resp = client.get(&url).send().map_err(err)?;
    if !resp.status().is_success() {
        return Err(format!("el servidor respondió {}", resp.status()));
    }
    let bytes = resp.bytes().map_err(err)?;
    if bytes.len() < 1024 || !bytes.starts_with(b"SQLite format 3") {
        return Err("el archivo descargado no es un index.sqlite válido".into());
    }
    replace_index(st, &bytes)
}).await
}

#[tauri::command]
async fn import_index_file(app: tauri::AppHandle, path: String) -> R<()> {
    blocking(app, move |st| {
    let bytes = std::fs::read(&path).map_err(err)?;
    if !bytes.starts_with(b"SQLite format 3") {
        return Err("el archivo no es una base SQLite".into());
    }
    replace_index(st, &bytes)
}).await
}

fn replace_index(st: &AppState, bytes: &[u8]) -> R<()> {
    let tmp = st.paths.index().with_extension("sqlite.new");
    std::fs::write(&tmp, bytes).map_err(err)?;
    // Verificar que abre y tiene las tablas esperadas.
    {
        let lib = btt_core::library::Library::open(&tmp, &st.paths.containers()).map_err(err)?;
        lib.target_languages("", 1).map_err(|e| format!("índice inválido: {e}"))?;
    }
    let mut guard = st.library.lock().unwrap_or_else(|e| e.into_inner());
    // Cerrar la conexión actual reemplazándola por una en memoria y renombrar.
    *guard = btt_core::library::Library::open_in_memory(&st.paths.containers()).map_err(err)?;
    let backup = st.paths.index().with_extension("sqlite.bak");
    let _ = std::fs::remove_file(&backup);
    let _ = std::fs::rename(st.paths.index(), &backup);
    let _ = std::fs::remove_file(st.paths.index().with_extension("sqlite-wal"));
    let _ = std::fs::remove_file(st.paths.index().with_extension("sqlite-shm"));
    if let Err(e) = std::fs::rename(&tmp, st.paths.index()) {
        // Volver al índice anterior: la app no puede quedarse con un catálogo vacío.
        let _ = std::fs::remove_file(&tmp);
        let _ = std::fs::rename(&backup, st.paths.index());
        *guard = st.paths.open_library().map_err(err)?;
        return Err(format!("no se pudo reemplazar el índice: {e}"));
    }
    *guard = st.paths.open_library().map_err(err)?;
    // Re-registrar contenedores descargados que no estén en el índice nuevo.
    let slugs = guard.downloaded_containers().unwrap_or_default();
    for s in slugs {
        if let Ok(rc) = btt_core::rc::ResourceContainer::open(&st.paths.containers().join(format!("{s}.tsrc"))) {
            let _ = guard.import_container(&rc);
        }
    }
    drop(guard);
    st.clear_rc_cache();
    Ok(())
}

#[tauri::command]
async fn import_source_container(app: tauri::AppHandle, path: String) -> R<String> {
    blocking(app, move |st| {
    let p = Path::new(&path);
    let rc = if p.is_dir() {
        // Carpeta RC abierta: empaquetar.
        let mut files = std::collections::BTreeMap::new();
        for e in walkdir_files(p) {
            let rel = e.strip_prefix(p).unwrap().to_string_lossy().replace('\\', "/");
            files.insert(rel, std::fs::read(&e).map_err(err)?);
        }
        btt_core::rc::ResourceContainer::from_files(files).map_err(err)?
    } else {
        btt_core::rc::ResourceContainer::open(p).map_err(err)?
    };
    st.library.lock().unwrap_or_else(|e| e.into_inner()).import_container(&rc).map_err(err)?;
    st.clear_rc_cache();
    Ok(rc.slug())
}).await
}

fn walkdir_files(root: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        if let Ok(rd) = std::fs::read_dir(&d) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else {
                    out.push(p);
                }
            }
        }
    }
    out
}

// ------------------------------------------------------------------ importar / exportar

#[tauri::command]
fn inspect_tstudio(path: String) -> R<btt_core::tstudio::ArchiveManifest> {
    btt_core::tstudio::inspect(Path::new(&path)).map_err(err)
}

#[tauri::command]
async fn import_tstudio(app: tauri::AppHandle, path: String, mode: ImportMode) -> R<Vec<ImportOutcome>> {
    blocking(app, move |st| {
    let out = btt_core::tstudio::import(Path::new(&path), &st.paths.translations(), mode, &st.author()).map_err(err)?;
    Ok(out)
}).await
}

#[derive(Serialize)]
pub struct UsfmImportSummary {
    book_id: String,
    title: String,
    chapters: usize,
    verses: usize,
    project_id: String,
    outcome: Option<ImportOutcome>,
    warnings: Vec<String>,
}

/// Analiza un USFM; si `language` no está vacío, lo importa como proyecto.
#[tauri::command]
async fn import_usfm(app: tauri::AppHandle, path: String, language: String, book: String, mode: ImportMode) -> R<UsfmImportSummary> {
    blocking(app, move |st| {
    let raw = std::fs::read(&path).map_err(err)?;
    let text = String::from_utf8_lossy(&raw).into_owned();
    let parsed = btt_core::usfm::parse_book(&text).map_err(err)?;
    let mut warnings = vec![];
    let book_id = if !book.is_empty() { book.to_lowercase() } else { parsed.id.to_lowercase() };
    // El código de libro viene del marcador \id del archivo: solo letras y dígitos (gen, 1jn, obs).
    if !book_id.is_empty() && (book_id.len() > 8 || !book_id.chars().all(|c| c.is_ascii_alphanumeric())) {
        return Err(format!("Código de libro no válido en \\id: {book_id:?}"));
    }
    if book_id.is_empty() {
        warnings.push("El archivo no tiene marcador \\id; elige el libro.".into());
    }
    check_id("idioma", &language).or_else(|e| if language.is_empty() { Ok(()) } else { Err(e) })?;
    let verses: usize = parsed.chapters.iter().map(|c| c.verses.len()).sum();
    for c in &parsed.chapters {
        if c.verses.is_empty() {
            warnings.push(format!("El capítulo {} no tiene versos.", c.number));
        }
    }
    let mut summary = UsfmImportSummary {
        book_id: book_id.clone(),
        title: parsed.header_title.clone(),
        chapters: parsed.chapters.len(),
        verses,
        project_id: String::new(),
        outcome: None,
        warnings,
    };
    if language.is_empty() || book_id.is_empty() {
        return Ok(summary);
    }
    let lib = st.library.lock().unwrap_or_else(|e| e.into_inner());
    let lang = lib.target_language(&language).map_err(err)?.ok_or_else(|| format!("idioma {language} no encontrado"))?;
    let markers = lib.chunk_markers(&book_id).map_err(err)?;
    let project_name = lib.project_name(&book_id, "en");
    drop(lib);
    // Crear en staging e importar con el modo elegido (merge/overwrite).
    let staging = st.paths.temp().join(format!("usfm-{}", uuid_simple()));
    std::fs::create_dir_all(&staging).map_err(err)?;
    let mut manifest = Manifest::new(
        TargetLanguage::from(lang),
        IdName::new(&book_id, &project_name),
        IdName::new("text", "Text"),
        IdName::new("reg", "Regular"),
        btt_core::VERSION,
    );
    manifest.project.name = String::new();
    manifest.add_translator(&st.profile.lock().unwrap_or_else(|e| e.into_inner()).display_name());
    let p = Project::create(&staging, manifest).map_err(err)?;
    btt_core::usfm::import_book(&p, &parsed, &markers).map_err(err)?;
    let outcome = btt_core::tstudio::import_dir(&p.root, &st.paths.translations(), mode, &st.author());
    let _ = btt_core::project::remove_dir_all_robust(&staging);
    summary.project_id = outcome.id.clone();
    summary.outcome = Some(outcome);
    Ok(summary)
}).await
}

pub(crate) fn uuid_simple() -> String {
    // Con contador: dos importaciones en el mismo segundo no chocan.
    static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("{:x}{:x}{n:x}", now(), std::process::id())
}

#[tauri::command]
async fn export_tstudio(app: tauri::AppHandle, id: String, dest: String) -> R<String> {
    blocking(app, move |st| {
    let p = open_project(st, &id)?;
    let dest = if dest.is_empty() { st.paths.exports().join(format!("{id}.tstudio")) } else { PathBuf::from(dest) };
    if dest.extension().map(|e| e != "tstudio").unwrap_or(true) {
        return Err("El destino debe terminar en .tstudio".into());
    }
    let out = btt_core::tstudio::export(&[&p], &dest, &st.author(), btt_core::VERSION).map_err(err)?;
    st.backup_state.lock().unwrap_or_else(|e| e.into_inner()).last_time.insert(id, now());
    let _ = st.save_backup_state();
    Ok(out.display().to_string())
}).await
}

#[tauri::command]
async fn export_usfm(app: tauri::AppHandle, id: String, dest: String) -> R<String> {
    blocking(app, move |st| {
    let p = open_project(st, &id)?;
    if p.manifest.type_.id != "text" || p.manifest.resource.id == "obs" {
        return Err("Solo los proyectos de texto bíblico se exportan a USFM".into());
    }
    let usfm = btt_core::usfm::export_book(&p).map_err(err)?;
    let dest = if dest.is_empty() {
        st.paths.exports().join(format!("{}_{}.usfm", p.manifest.target_language.id, p.manifest.project.id.to_uppercase()))
    } else {
        PathBuf::from(dest)
    };
    if dest.extension().map(|e| !(e == "usfm" || e == "txt" || e == "sfm")).unwrap_or(true) {
        return Err("El destino debe terminar en .usfm".into());
    }
    btt_core::project::atomic_write(&dest, &usfm).map_err(err)?;
    Ok(dest.display().to_string())
}).await
}

#[tauri::command]
async fn export_print_html(app: tauri::AppHandle, id: String, opts: PrintRequest) -> R<String> {
    blocking(app, move |st| {
        let html = print_html(st, &id, &opts)?;
        let dest = st.paths.exports().join(format!("{id}.html"));
        btt_core::project::atomic_write(&dest, &html).map_err(err)?;
        Ok(dest.display().to_string())
    })
    .await
}

/// Opciones de impresión elegidas en el diálogo.
#[derive(Deserialize, Clone)]
pub struct PrintRequest {
    include_incomplete: bool,
    include_images: bool,
    #[serde(default)]
    columns: u8,
    #[serde(default)]
    font_size_pt: u32,
    #[serde(default)]
    font_family: String,
    #[serde(default)]
    cover: bool,
    #[serde(default)]
    license: bool,
    #[serde(default)]
    toc: bool,
    #[serde(default = "yes")]
    notes: bool,
    #[serde(default = "yes")]
    words: bool,
    #[serde(default = "yes")]
    page_numbers: bool,
    #[serde(default)]
    verse_color: bool,
    #[serde(default)]
    paragraphs: String,
    #[serde(default)]
    heading: String,
    #[serde(default)]
    margin_cm: f32,
    #[serde(default)]
    line_height: f32,
}

fn yes() -> bool {
    true
}

fn print_html(st: &AppState, id: &str, req: &PrintRequest) -> R<String> {
    use btt_core::print::{PrintOptions, PrintSources};
    let p = open_project(st, id)?;
    let m = &p.manifest;
    // Fuente: la primera traducción fuente descargada. Las notas y palabras van en su idioma.
    let src = m.source_translations.iter().find_map(|s| st.container(&s.language_id, source_project_slug(m), &s.resource_id).ok().map(|rc| (s.language_id.clone(), rc)));
    let (src_lang, rc) = match src {
        Some((l, rc)) => (Some(l), Some(rc)),
        None => (m.source_translations.first().map(|s| s.language_id.clone()), None),
    };
    let notes = match (&src_lang, req.notes) {
        (Some(l), true) => st.container(l, &m.project.id, "tn").ok(),
        _ => None,
    };
    let words = match (&src_lang, req.words) {
        (Some(l), true) => st.container(l, "bible", "tw").ok(),
        _ => None,
    };
    let settings = st.settings.lock().unwrap_or_else(|e| e.into_inner()).clone();
    // El nombre de la fuente va dentro del CSS: solo letras, dígitos, espacios y guiones.
    let font = req.font_family.trim();
    let font = if font.is_empty() || !font.chars().all(|c| c.is_alphanumeric() || c == ' ' || c == '-') { "Arial".to_string() } else { font.to_string() };
    let defaults = PrintOptions::default();
    let opts = PrintOptions {
        include_incomplete: req.include_incomplete,
        include_images: req.include_images,
        font_family: format!("'{font}', Arial, Helvetica, sans-serif"),
        font_size_pt: if req.font_size_pt == 0 { defaults.font_size_pt } else { req.font_size_pt.clamp(7, 24) },
        images_base: String::new(),
        columns: if req.columns >= 2 { 2 } else { 1 },
        justify: true,
        cover: req.cover,
        license: req.license,
        toc: req.toc,
        notes: req.notes,
        words: req.words,
        page_numbers: req.page_numbers,
        verse_color: req.verse_color,
        paragraphs: if req.paragraphs.is_empty() { defaults.paragraphs } else { req.paragraphs.clone() },
        heading: if req.heading.is_empty() { defaults.heading } else { req.heading.clone() },
        margin_cm: if req.margin_cm <= 0.0 { defaults.margin_cm } else { req.margin_cm },
        line_height: if req.line_height <= 0.0 { defaults.line_height } else { req.line_height },
        lang: settings.ui_language.clone(),
    };
    let sources = PrintSources { source: rc.as_deref(), notes: notes.as_deref(), words: words.as_deref() };
    btt_core::print::render_document(&p, sources, &opts).map_err(err)
}

/// HTML del documento para la vista previa dentro de la app.
#[tauri::command]
async fn render_print_html(app: tauri::AppHandle, id: String, opts: PrintRequest) -> R<String> {
    blocking(app, move |st| print_html(st, &id, &opts)).await
}

// ------------------------------------------------------------------ WACS / git

#[derive(Serialize)]
pub struct UploadResult {
    push: PushStatus,
    merge: Option<MergeStatus>,
    url: String,
    repo_url: String,
}

pub(crate) fn credentials(st: &AppState) -> R<(Credentials, String)> {
    let p = st.profile.lock().unwrap_or_else(|e| e.into_inner());
    let s = p.session.as_ref().ok_or_else(|| "Inicia sesión en el servidor para subir proyectos".to_string())?;
    Ok((Credentials { username: s.user.username.clone(), token: s.token.sha1.clone() }, s.user.username.clone()))
}

#[tauri::command]
async fn upload_project(app: tauri::AppHandle, id: String) -> R<UploadResult> {
    blocking(app, move |st| {
    let p = open_project(st, &id)?;
    let (creds, username) = credentials(st)?;
    let settings = st.settings.lock().unwrap_or_else(|e| e.into_inner()).clone();
    // Un proyecto en equipo se sube al repositorio compartido, no al propio del usuario.
    let (clone_url, html_url, reader_user) = match crate::collab::active_remote(st, &id) {
        Some(c) => (c.clone_url, c.html_url, c.owner),
        None => {
            let client = WacsClient::new(&settings.git_api_url).map_err(err)?;
            let repo = client.create_repo(&creds.token, &username, &id).map_err(|e| match e {
                btt_core::Error::Unauthorized => "La sesión expiró. Inicia sesión de nuevo.".to_string(),
                other => other.to_string(),
            })?;
            (repo.clone_url, repo.html_url, username.clone())
        }
    };
    let author = st.author();
    let merge = gitrepo::pull(&p.root, &clone_url, Some(&creds), &author).map_err(err)?;
    if let MergeStatus::Conflicts { .. } = &merge {
        return Ok(UploadResult { push: PushStatus::Rejected { reason: "conflictos".into() }, merge: Some(merge), url: String::new(), repo_url: html_url });
    }
    if matches!(merge, MergeStatus::AuthFailure) {
        return Err("El servidor rechazó las credenciales. Inicia sesión de nuevo.".into());
    }
    let push = gitrepo::push(&p.root, &clone_url, &creds).map_err(err)?;
    if matches!(push, PushStatus::Ok | PushStatus::UpToDate) {
        st.backup_state.lock().unwrap_or_else(|e| e.into_inner()).last_time.insert(format!("upload:{id}"), now());
        let _ = st.save_backup_state();
    }
    Ok(UploadResult {
        url: btt_core::wacs::reader_url(&settings.reader_url, &reader_user, &id),
        repo_url: html_url,
        push,
        merge: Some(merge),
    })
}).await
}

#[tauri::command]
async fn pull_project(app: tauri::AppHandle, id: String) -> R<MergeStatus> {
    blocking(app, move |st| {
    let p = open_project(st, &id)?;
    let (creds, username) = credentials(st)?;
    if let Some(c) = crate::collab::active_remote(st, &id) {
        return gitrepo::pull(&p.root, &c.clone_url, Some(&creds), &st.author()).map_err(err);
    }
    let api = st.settings.lock().unwrap_or_else(|e| e.into_inner()).git_api_url.clone();
    let client = WacsClient::new(&api).map_err(err)?;
    let repo = client.get_repo(Some(&creds.token), &username, &id).map_err(|e| match e {
        btt_core::Error::Server { status: 404, .. } => "El proyecto no existe en el servidor".to_string(),
        other => other.to_string(),
    })?;
    gitrepo::pull(&p.root, &repo.clone_url, Some(&creds), &st.author()).map_err(err)
}).await
}

#[tauri::command]
fn restore_backup_branch(st: State<AppState>, id: String) -> R<()> {
    let p = open_project(&st, &id)?;
    gitrepo::reset_hard_to_branch(&p.root, gitrepo::BACKUP_BRANCH).map_err(err)
}

#[tauri::command]
async fn search_wacs(app: tauri::AppHandle, username: String, query: String) -> R<Vec<Repo>> {
    blocking(app, move |st| {
    let api = st.settings.lock().unwrap_or_else(|e| e.into_inner()).git_api_url.clone();
    let client = WacsClient::new(&api).map_err(err)?;
    if username.trim().is_empty() {
        client.search_repos(&query, None, 50).map_err(err)
    } else {
        client.user_repos(username.trim(), &query).map_err(err)
    }
}).await
}

#[tauri::command]
async fn import_from_wacs(app: tauri::AppHandle, clone_url: String, mode: ImportMode) -> R<ImportOutcome> {
    blocking(app, move |st| {
    check_clone_url(st, &clone_url)?;
    let creds = st.profile.lock().unwrap_or_else(|e| e.into_inner()).session.as_ref().map(|s| Credentials { username: s.user.username.clone(), token: s.token.sha1.clone() });
    let staging = st.paths.temp().join(format!("clone-{}", uuid_simple()));
    gitrepo::clone(&clone_url, creds.as_ref(), &staging).map_err(err)?;
    let outcome = btt_core::tstudio::import_dir(&staging, &st.paths.translations(), mode, &st.author());
    let _ = btt_core::project::remove_dir_all_robust(&staging);
    Ok(outcome)
}).await
}

// ------------------------------------------------------------------ respaldos / migración

#[derive(Serialize)]
pub struct BackupEntry {
    name: String,
    path: String,
    modified: u64,
}

#[tauri::command]
fn list_backups(st: State<AppState>) -> R<Vec<BackupEntry>> {
    let dir = st.paths.backups();
    Ok(btt_core::backup::list(&dir)
        .map_err(err)?
        .into_iter()
        .map(|(name, modified)| BackupEntry { path: dir.join(&name).display().to_string(), name, modified })
        .collect())
}

#[tauri::command]
async fn run_backup_now(app: tauri::AppHandle) -> R<btt_core::backup::BackupReport> {
    blocking(app, move |st| {
    let Ok(_running) = st.backup_running.try_lock() else {
        return Err("Ya hay un respaldo en curso".into());
    };
    let author = st.author();
    let mut bs = st.backup_state.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let report = btt_core::backup::run(&st.paths.translations(), &st.paths.backups(), &mut bs, &author, btt_core::VERSION).map_err(err)?;
    {
        let mut cur = st.backup_state.lock().unwrap_or_else(|e| e.into_inner());
        cur.last_hash.extend(bs.last_hash);
        cur.last_time.extend(bs.last_time);
    }
    st.save_backup_state().map_err(err)?;
    Ok(report)
}).await
}

#[tauri::command]
async fn migrate_legacy(app: tauri::AppHandle, root: String) -> R<Vec<ImportOutcome>> {
    let handle = app.clone();
    blocking(app, move |st| {
        let author = st.author();
        let out = st
            .paths
            .migrate_from_with_progress(Path::new(&root), &author, |p| {
                let _ = handle.emit("migrate-progress", &p);
            })
            .map_err(err)?;
        let mut s = st.settings.lock().unwrap_or_else(|e| e.into_inner());
        if !s.migrated_from.contains(&root) {
            s.migrated_from.push(root);
        }
        drop(s);
        let _ = st.save_settings();
        st.clear_rc_cache();
        Ok(out)
    })
    .await
}

#[derive(Serialize)]
pub struct LegacyRootInfo {
    path: String,
    projects: usize,
}

#[tauri::command]
fn inspect_legacy(root: String) -> LegacyRootInfo {
    LegacyRootInfo { projects: btt_core::appdata::Paths::legacy_project_count(Path::new(&root)), path: root }
}

#[tauri::command]
fn send_feedback(st: State<AppState>, email: String, message: String) -> R<String> {
    let dest = st.paths.logs().join(format!("feedback-{}.txt", chrono_like_stamp()));
    let info = format!(
        "BTT Writer {} ({})\nDispositivo: {}\nCorreo: {}\n\n{}\n",
        btt_core::VERSION,
        std::env::consts::OS,
        st.device_name,
        email,
        message
    );
    btt_core::project::atomic_write(&dest, &info).map_err(err)?;
    Ok(dest.display().to_string())
}

#[tauri::command]
fn open_data_dir(st: State<AppState>) -> R<String> {
    Ok(st.paths.root.display().to_string())
}

pub fn handler() -> impl Fn(tauri::ipc::Invoke) -> bool {
    tauri::generate_handler![
        app_info,
        get_settings,
        save_settings,
        get_profile,
        save_profile,
        logout,
        wacs_login,
        list_projects,
        get_project,
        create_project,
        delete_project,
        change_resource,
        set_translators,
        set_book_title,
        set_project_sources,
        read_chunk,
        write_chunk,
        write_helps,
        set_chunk_finished,
        mark_all_chunks,
        validate_project,
        commit_project,
        chunk_history,
        chunk_authors,
        chunk_at_commit,
        restore_chunk,
        search_project,
        get_source_chapter,
        get_source_info,
        get_helps,
        get_word,
        word_index,
        search_languages,
        list_books,
        list_categories,
        available_sources,
        all_resources,
        source_languages,
        download_source,
        download_language_sources,
        delete_source,
        update_catalog,
        update_languages,
        download_index,
        import_index_file,
        import_source_container,
        inspect_tstudio,
        import_tstudio,
        import_usfm,
        export_tstudio,
        export_usfm,
        export_print_html,
        render_print_html,
        upload_project,
        pull_project,
        restore_backup_branch,
        search_wacs,
        import_from_wacs,
        crate::collab::collab_status,
        crate::collab::collab_projects,
        crate::collab::collab_enable,
        crate::collab::collab_disable,
        crate::collab::collab_members,
        crate::collab::collab_add_member,
        crate::collab::collab_remove_member,
        crate::collab::collab_shared_repos,
        crate::collab::collab_join,
        crate::collab::collab_sync,
        crate::collab::collab_release,
        crate::collab::collab_claim,
        crate::collab::search_users,
        list_backups,
        run_backup_now,
        migrate_legacy,
        inspect_legacy,
        send_feedback,
        open_data_dir,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// El cliente HTTP bloqueante no puede crearse dentro del runtime async
    /// (era la causa de que descargar fuentes no hiciera nada); en `spawn_blocking` sí.
    #[test]
    /// Con un proyecto real: las versiones repetidas se agrupan y solo una es la actual.
    #[test]
    fn historial_agrupa_versiones_repetidas() {
        let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).unwrap_or_default();
        let dir = std::path::PathBuf::from(home).join("BTT-Writer-3/translations/pay_gal_text_reg");
        if !dir.join("01/01.txt").is_file() {
            eprintln!("omitida: no existe {}", dir.display());
            return;
        }
        let p = Project::open(&dir).unwrap();
        let entries = history_entries(&p, "01", "01").unwrap();
        let commits = gitrepo::file_history(&p.root, "01/01.txt").unwrap();
        assert!(!entries.is_empty());
        assert!(entries.len() <= commits.len());
        assert_eq!(entries.iter().filter(|e| e.current).count(), 1, "exactamente una versión actual");
        let mut texts: Vec<String> = entries.iter().map(|e| gitrepo::file_at(&p.root, &e.id, "01/01.txt").unwrap().replace("\r\n", "\n").trim().to_string()).collect();
        let n = texts.len();
        texts.sort();
        texts.dedup();
        assert_eq!(texts.len(), n, "sin textos repetidos");
        assert!(entries.windows(2).all(|w| w[0].time >= w[1].time), "más reciente primero");
    }

    #[test]
    fn cliente_bloqueante_fuera_del_runtime() {
        let dentro = std::panic::catch_unwind(|| {
            tauri::async_runtime::block_on(async { btt_core::sources::http_client().map(|_| ()).is_ok() })
        });
        assert!(!matches!(dentro, Ok(true)), "se esperaba que fallara o abortara dentro del runtime");
        let fuera = tauri::async_runtime::block_on(async {
            tauri::async_runtime::spawn_blocking(|| btt_core::sources::http_client().map(|_| ())).await
        });
        assert!(matches!(fuera, Ok(Ok(()))), "{fuera:?}");
    }
}
