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

type R<T> = Result<T, String>;

/// Ejecuta trabajo bloqueante (HTTP con reqwest::blocking, git) en un hilo aparte.
/// Los comandos `async` corren en el runtime de Tauri y un cliente bloqueante ahí
/// aborta con "Cannot start a runtime from within a runtime".
async fn blocking<T: Send + 'static>(app: tauri::AppHandle, f: impl FnOnce(&AppState) -> R<T> + Send + 'static) -> R<T> {
    tauri::async_runtime::spawn_blocking(move || {
        let st = app.state::<AppState>();
        f(&st)
    })
    .await
    .map_err(|e| format!("la tarea se interrumpió: {e}"))?
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn open_project(st: &AppState, id: &str) -> R<Project> {
    Project::open_by_id(&st.paths.translations(), id).map_err(err)
}

fn now() -> u64 {
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
    Ok(st.settings.lock().unwrap().clone())
}

#[tauri::command]
fn save_settings(st: State<AppState>, settings: Settings) -> R<Settings> {
    let mut s = settings;
    let changed_server = st.settings.lock().unwrap().content_server != s.content_server;
    if changed_server {
        s.apply_server_defaults();
    }
    *st.settings.lock().unwrap() = s.clone();
    st.save_settings().map_err(err)?;
    Ok(s)
}

#[tauri::command]
fn get_profile(st: State<AppState>) -> R<Profile> {
    Ok(st.profile.lock().unwrap().clone())
}

#[tauri::command]
fn save_profile(st: State<AppState>, profile: Profile) -> R<Profile> {
    *st.profile.lock().unwrap() = profile.clone();
    st.save_profile().map_err(err)?;
    Ok(profile)
}

#[tauri::command]
fn logout(st: State<AppState>) -> R<()> {
    let session = st.profile.lock().unwrap().session.clone();
    if let Some(s) = session {
        let api = st.settings.lock().unwrap().git_api_url.clone();
        if let Ok(c) = WacsClient::new(&api) {
            let _ = c.logout(&s);
        }
    }
    *st.profile.lock().unwrap() = Profile::default();
    st.save_profile().map_err(err)
}

#[tauri::command]
async fn wacs_login(app: tauri::AppHandle, username: String, password: String) -> R<Profile> {
    blocking(app, move |st| wacs_login_inner(st, &username, &password)).await
}

fn wacs_login_inner(st: &AppState, username: &str, password: &str) -> R<Profile> {
    let api = st.settings.lock().unwrap().git_api_url.clone();
    let client = WacsClient::new(&api).map_err(err)?;
    let session = client
        .login(&username, &password, &btt_core::wacs::token_name(&st.device_name))
        .map_err(|e| match e {
            btt_core::Error::Unauthorized => "Usuario o contraseña incorrectos".to_string(),
            other => other.to_string(),
        })?;
    let mut p = st.profile.lock().unwrap();
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

fn total_chunks(st: &AppState, project: &Project) -> usize {
    let m = &project.manifest;
    // Fuente seleccionada → chunks del contenedor; si no, versificación.
    for s in &m.source_translations {
        if let Ok(rc) = st.container(&s.language_id, &m.project.id, &s.resource_id) {
            let n: usize = rc.chapters().iter().map(|c| rc.chunks(c).len()).sum();
            if n > 0 {
                return n;
            }
        }
    }
    if m.type_.id == "tw" {
        return project.chunks("01").map(|c| c.len()).unwrap_or(0);
    }
    let lib = st.library.lock().unwrap();
    let markers = lib.chunk_markers(&m.project.id).unwrap_or_default();
    let n: usize = markers.values().map(|v| v.len()).sum();
    if n > 0 {
        n
    } else {
        project.chapters().unwrap_or_default().iter().map(|c| project.chunks(c).map(|x| x.len()).unwrap_or(0)).sum()
    }
}

fn list_item(st: &AppState, p: &Project) -> ProjectListItem {
    let mut summary = p.summary();
    if summary.project_name.is_empty() {
        let ui_lang = st.settings.lock().unwrap().ui_language.clone();
        let lang = if ui_lang == "es" { "es-419" } else { "en" };
        summary.project_name = st.library.lock().unwrap().project_name(&summary.project_id, lang);
    }
    let total = total_chunks(st, p);
    let progress = if total == 0 { 0.0 } else { (summary.finished_chunks as f32 / total as f32).min(1.0) };
    let has_conflicts = gitrepo::conflicted_files(&p.root).map(|f| !f.is_empty()).unwrap_or(false);
    let last_backup = st.backup_state.lock().unwrap().last_time.get(&summary.id).copied();
    ProjectListItem { summary, total_chunks: total, progress, has_conflicts, last_backup }
}

#[tauri::command]
fn list_projects(st: State<AppState>) -> R<Vec<ProjectListItem>> {
    let projects = Project::list(&st.paths.translations()).map_err(err)?;
    Ok(projects.iter().map(|p| list_item(&st, p)).collect())
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

/// Estructura de capítulos/chunks: unión de la fuente principal y lo que hay en disco.
fn chapter_structure(st: &AppState, project: &Project) -> R<Vec<ChapterInfo>> {
    let m = &project.manifest;
    let mut chapters: Vec<ChapterInfo> = vec![];
    let push = |ch: &str, chunks: Vec<String>, chapters: &mut Vec<ChapterInfo>| {
        if let Some(existing) = chapters.iter_mut().find(|c| c.id == ch) {
            for c in chunks {
                if !existing.chunks.iter().any(|x| x.id == c) {
                    existing.chunks.push(ChunkInfo { id: c, finished: false, has_text: false, has_conflict: false });
                }
            }
        } else {
            chapters.push(ChapterInfo {
                id: ch.to_string(),
                title: String::new(),
                chunks: chunks.into_iter().map(|c| ChunkInfo { id: c, finished: false, has_text: false, has_conflict: false }).collect(),
            });
        }
    };
    // front/title siempre primero.
    push("front", vec!["title".into()], &mut chapters);
    let source_rc = m
        .source_translations
        .iter()
        .find_map(|s| st.container(&s.language_id, &m.project.id, &s.resource_id).ok());
    if m.type_.id == "tw" {
        // Palabras: chunks = slugs del diccionario fuente + los existentes.
        let mut words: Vec<String> = source_rc.as_ref().map(|rc| rc.word_slugs()).unwrap_or_default();
        for c in project.chunks("01").map_err(err)? {
            if !words.contains(&c) {
                words.push(c);
            }
        }
        words.sort();
        push("01", words, &mut chapters);
    } else {
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
            let lib = st.library.lock().unwrap();
            for (ch, verses) in lib.chunk_markers(&m.project.id).unwrap_or_default() {
                let mut chunks = vec!["title".to_string()];
                chunks.extend(verses.iter().map(|v| format!("{v:02}")));
                push(&ch, chunks, &mut chapters);
            }
        }
        for ch in project.chapters().map_err(err)? {
            let chunks = project.chunks(&ch).map_err(err)?;
            push(&ch, chunks, &mut chapters);
        }
    }
    // Ordenar y rellenar estado.
    chapters.sort_by(|a, b| btt_core::project::natural_cmp(&a.id, &b.id));
    for ch in &mut chapters {
        ch.chunks.sort_by(|a, b| btt_core::project::chunk_cmp(&a.id, &b.id));
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
fn get_project(st: State<AppState>, id: String) -> R<ProjectDetail> {
    let p = open_project(&st, &id)?;
    let chapters = chapter_structure(&st, &p)?;
    let lib = st.library.lock().unwrap();
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
        item: list_item(&st, &p),
        book_title: p.read_book_title().unwrap_or_default(),
        manifest: p.manifest.clone(),
        chapters,
        sources,
    })
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
fn create_project(st: State<AppState>, spec: NewProject) -> R<String> {
    let lib = st.library.lock().unwrap();
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
    let translator = st.profile.lock().unwrap().display_name();
    manifest.add_translator(&translator);
    // Fuente por defecto: la primera descargada para este libro (en/ulb preferida).
    {
        let lib = st.library.lock().unwrap();
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
    st.settings.lock().unwrap().last_project = id.clone();
    let _ = st.save_settings();
    Ok(id)
}

#[tauri::command]
fn delete_project(st: State<AppState>, id: String) -> R<String> {
    let p = open_project(&st, &id)?;
    // Respaldo previo con fecha.
    let stamp = chrono_like_stamp();
    let dest = st.paths.backups().join(format!("{id}.{stamp}.tstudio"));
    let backup = btt_core::tstudio::export(&[&p], &dest, &st.author(), btt_core::VERSION).map_err(err)?;
    p.delete().map_err(err)?;
    let mut s = st.settings.lock().unwrap();
    if s.last_project == id {
        s.last_project.clear();
    }
    drop(s);
    let _ = st.save_settings();
    Ok(backup.display().to_string())
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
fn change_resource(st: State<AppState>, id: String, resource: String) -> R<String> {
    let p = open_project(&st, &id)?;
    let p = p.change_resource(IdName::new(&resource, resource_name(&resource))).map_err(err)?;
    gitrepo::commit_all(&p.root, &st.author(), "change resource").map_err(err)?;
    Ok(p.id())
}

#[tauri::command]
fn set_translators(st: State<AppState>, id: String, translators: Vec<String>) -> R<Vec<String>> {
    let mut p = open_project(&st, &id)?;
    p.manifest.translators.clear();
    for t in translators {
        p.manifest.add_translator(&t);
    }
    p.save_manifest().map_err(err)?;
    Ok(p.manifest.translators)
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
fn set_project_sources(st: State<AppState>, id: String, sources: Vec<SourceRef>) -> R<Manifest> {
    let mut p = open_project(&st, &id)?;
    let lib = st.library.lock().unwrap();
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
    helps: Vec<HelpItem>,
}

fn expected_verses(st: &AppState, p: &Project, chapter: &str, chunk: &str) -> Vec<u32> {
    if chapter == "front" || chunk == "title" || chunk == "reference" || p.manifest.type_.id != "text" || p.manifest.resource.id == "obs" {
        return vec![];
    }
    for s in &p.manifest.source_translations {
        if let Ok(rc) = st.container(&s.language_id, &p.manifest.project.id, &s.resource_id) {
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
fn read_chunk(st: State<AppState>, id: String, chapter: String, chunk: String) -> R<ChunkData> {
    let p = open_project(&st, &id)?;
    let mut text = p.read_chunk(&chapter, &chunk).map_err(err)?;
    let expected = expected_verses(&st, &p, &chapter, &chunk);
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
    Ok(ChunkData {
        issues: if is_help { vec![] } else { validation::validate_chunk(&text, &expected) },
        conflict_options: if has_conflict { btt_core::conflicts::parse(&text) } else { vec![] },
        helps: if is_help && !has_conflict { validation::parse_helps(&text) } else { vec![] },
        finished: p.manifest.is_chunk_finished(&chapter, &chunk),
        expected_verses: expected,
        has_conflict,
        text,
    })
}

#[tauri::command]
fn write_chunk(st: State<AppState>, id: String, chapter: String, chunk: String, text: String) -> R<()> {
    let p = open_project(&st, &id)?;
    if text.trim().is_empty() && chunk != "title" && chunk != "reference" {
        // Igual que la app original: texto vacío borra el archivo.
        p.delete_chunk(&chapter, &chunk).map_err(err)
    } else {
        p.write_chunk(&chapter, &chunk, &text).map_err(err)
    }
}

#[tauri::command]
fn write_helps(st: State<AppState>, id: String, chapter: String, chunk: String, items: Vec<HelpItem>) -> R<String> {
    let p = open_project(&st, &id)?;
    let text = validation::serialize_helps(&items);
    if text.is_empty() {
        p.delete_chunk(&chapter, &chunk).map_err(err)?;
    } else {
        p.write_chunk(&chapter, &chunk, &text).map_err(err)?;
    }
    Ok(text)
}

#[tauri::command]
fn set_chunk_finished(st: State<AppState>, id: String, chapter: String, chunk: String, finished: bool) -> R<Vec<Issue>> {
    let mut p = open_project(&st, &id)?;
    if finished {
        let text = p.read_chunk(&chapter, &chunk).map_err(err)?;
        let issues = if p.manifest.type_.id == "text" {
            let expected = expected_verses(&st, &p, &chapter, &chunk);
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
}

#[derive(Serialize)]
pub struct MarkAllResult {
    marked: usize,
    total: usize,
    invalid: Vec<ChunkValidation>,
}

#[tauri::command]
fn mark_all_chunks(st: State<AppState>, id: String) -> R<MarkAllResult> {
    let mut p = open_project(&st, &id)?;
    let m = p.manifest.clone();
    let rc = m.source_translations.iter().find_map(|s| st.container(&s.language_id, &m.project.id, &s.resource_id).ok());
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
}

#[tauri::command]
fn validate_project(st: State<AppState>, id: String) -> R<Vec<ChunkValidation>> {
    let p = open_project(&st, &id)?;
    let m = &p.manifest;
    let rc = m.source_translations.iter().find_map(|s| st.container(&s.language_id, &m.project.id, &s.resource_id).ok());
    validation::validate_project(&p, rc.as_deref()).map_err(err)
}

#[tauri::command]
fn commit_project(st: State<AppState>, id: String) -> R<Option<String>> {
    let p = open_project(&st, &id)?;
    gitrepo::commit_all(&p.root, &st.author(), gitrepo::AUTOSAVE_MESSAGE).map_err(err)
}

#[tauri::command]
fn chunk_history(st: State<AppState>, id: String, chapter: String, chunk: String) -> R<Vec<gitrepo::Commit>> {
    let p = open_project(&st, &id)?;
    gitrepo::commit_all(&p.root, &st.author(), gitrepo::AUTOSAVE_MESSAGE).map_err(err)?;
    gitrepo::file_history(&p.root, &format!("{chapter}/{chunk}.txt")).map_err(err)
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
fn search_project(st: State<AppState>, id: String, query: String, in_source: bool) -> R<Vec<SearchHit>> {
    let p = open_project(&st, &id)?;
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Ok(vec![]);
    }
    let mut hits = vec![];
    if in_source {
        let m = &p.manifest;
        if let Some(rc) = m.source_translations.iter().find_map(|s| st.container(&s.language_id, &m.project.id, &s.resource_id).ok()) {
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
}

fn snippet_for(text: &str, q: &str) -> Option<String> {
    let lower = text.to_lowercase();
    let i = lower.find(q)?;
    let start = lower[..i].char_indices().rev().nth(40).map(|(j, _)| j).unwrap_or(0);
    let end = lower[i..].char_indices().nth(q.chars().count() + 60).map(|(j, _)| i + j).unwrap_or(text.len());
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
        let lib = st.library.lock().unwrap();
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
    st.library.lock().unwrap().target_languages(&query, limit).map_err(err)
}

#[tauri::command]
fn list_books(st: State<AppState>, language: String) -> R<Vec<ProjectRow>> {
    let lib = st.library.lock().unwrap();
    let mut rows = lib.projects(&language).map_err(err)?;
    rows.sort_by_key(|p| (btt_core::bible_sort_index(&p.slug), p.sort));
    Ok(rows)
}

#[tauri::command]
fn list_categories(st: State<AppState>, language: String) -> R<Vec<CategoryRow>> {
    st.library.lock().unwrap().categories(&language).map_err(err)
}

#[tauri::command]
fn available_sources(st: State<AppState>, project: String, gl_mode: bool) -> R<Vec<ResourceRow>> {
    let lib = st.library.lock().unwrap();
    let mut rows = lib.source_texts(&project, gl_mode).map_err(err)?;
    rows.sort_by_key(|r| (!r.downloaded, r.language_slug.clone(), r.slug.clone()));
    Ok(rows)
}

#[tauri::command]
fn all_resources(st: State<AppState>, project: String) -> R<Vec<ResourceRow>> {
    st.library.lock().unwrap().resources(&project).map_err(err)
}

#[tauri::command]
fn source_languages(st: State<AppState>) -> R<Vec<btt_core::library::SourceLanguageRow>> {
    st.library.lock().unwrap().source_languages().map_err(err)
}

#[tauri::command]
async fn download_source(app: tauri::AppHandle, language: String, project: String, resource: String) -> R<Vec<String>> {
    blocking(app, move |st| {
    let row = st
        .library
        .lock()
        .unwrap()
        .resource(&language, &project, &resource)
        .map_err(err)?
        .ok_or_else(|| "recurso no encontrado en el catálogo".to_string())?;
    let client = btt_core::sources::http_client().map_err(err)?;
    let lib = st.library.lock().unwrap();
    let r = btt_core::sources::download_resource(&lib, &row, &client, true).map_err(err);
    drop(lib);
    st.clear_rc_cache();
    r
}).await
}

#[tauri::command]
fn delete_source(st: State<AppState>, language: String, project: String, resource: String) -> R<()> {
    st.library.lock().unwrap().delete_container(&language, &project, &resource).map_err(err)?;
    st.clear_rc_cache();
    Ok(())
}

#[tauri::command]
async fn update_catalog(app: tauri::AppHandle) -> R<Vec<String>> {
    blocking(app, move |st| {
    let url = st.settings.lock().unwrap().root_catalog_url();
    let client = btt_core::sources::http_client().map_err(err)?;
    let lib = st.library.lock().unwrap();
    btt_core::sources::update_catalog(&lib, &client, &url).map_err(err)
}).await
}

#[tauri::command]
async fn update_languages(app: tauri::AppHandle) -> R<usize> {
    blocking(app, move |st| {
    let url = st.settings.lock().unwrap().langnames_url.clone();
    let client = btt_core::sources::http_client().map_err(err)?;
    let lib = st.library.lock().unwrap();
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
    let url = st.settings.lock().unwrap().index_sqlite_url.clone();
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
fn import_index_file(st: State<AppState>, path: String) -> R<()> {
    let bytes = std::fs::read(&path).map_err(err)?;
    if !bytes.starts_with(b"SQLite format 3") {
        return Err("el archivo no es una base SQLite".into());
    }
    replace_index(&st, &bytes)
}

fn replace_index(st: &AppState, bytes: &[u8]) -> R<()> {
    let tmp = st.paths.index().with_extension("sqlite.new");
    std::fs::write(&tmp, bytes).map_err(err)?;
    // Verificar que abre y tiene las tablas esperadas.
    {
        let lib = btt_core::library::Library::open(&tmp, &st.paths.containers()).map_err(err)?;
        lib.target_languages("", 1).map_err(|e| format!("índice inválido: {e}"))?;
    }
    let mut guard = st.library.lock().unwrap();
    // Cerrar la conexión actual reemplazándola por una en memoria y renombrar.
    *guard = btt_core::library::Library::open_in_memory(&st.paths.containers()).map_err(err)?;
    let backup = st.paths.index().with_extension("sqlite.bak");
    let _ = std::fs::remove_file(&backup);
    let _ = std::fs::rename(st.paths.index(), &backup);
    let _ = std::fs::remove_file(st.paths.index().with_extension("sqlite-wal"));
    let _ = std::fs::remove_file(st.paths.index().with_extension("sqlite-shm"));
    std::fs::rename(&tmp, st.paths.index()).map_err(err)?;
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
fn import_source_container(st: State<AppState>, path: String) -> R<String> {
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
    st.library.lock().unwrap().import_container(&rc).map_err(err)?;
    st.clear_rc_cache();
    Ok(rc.slug())
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
fn import_tstudio(st: State<AppState>, path: String, mode: ImportMode) -> R<Vec<ImportOutcome>> {
    let out = btt_core::tstudio::import(Path::new(&path), &st.paths.translations(), mode, &st.author()).map_err(err)?;
    Ok(out)
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
fn import_usfm(st: State<AppState>, path: String, language: String, book: String, mode: ImportMode) -> R<UsfmImportSummary> {
    let raw = std::fs::read(&path).map_err(err)?;
    let text = String::from_utf8_lossy(&raw).into_owned();
    let parsed = btt_core::usfm::parse_book(&text).map_err(err)?;
    let mut warnings = vec![];
    let book_id = if !book.is_empty() { book.to_lowercase() } else { parsed.id.to_lowercase() };
    if book_id.is_empty() {
        warnings.push("El archivo no tiene marcador \\id; elige el libro.".into());
    }
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
    let lib = st.library.lock().unwrap();
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
    manifest.add_translator(&st.profile.lock().unwrap().display_name());
    let p = Project::create(&staging, manifest).map_err(err)?;
    btt_core::usfm::import_book(&p, &parsed, &markers).map_err(err)?;
    let outcome = btt_core::tstudio::import_dir(&p.root, &st.paths.translations(), mode, &st.author());
    let _ = btt_core::project::remove_dir_all_robust(&staging);
    summary.project_id = outcome.id.clone();
    summary.outcome = Some(outcome);
    Ok(summary)
}

fn uuid_simple() -> String {
    format!("{:x}{:x}", now(), std::process::id())
}

#[tauri::command]
fn export_tstudio(st: State<AppState>, id: String, dest: String) -> R<String> {
    let p = open_project(&st, &id)?;
    let dest = if dest.is_empty() { st.paths.exports().join(format!("{id}.tstudio")) } else { PathBuf::from(dest) };
    let out = btt_core::tstudio::export(&[&p], &dest, &st.author(), btt_core::VERSION).map_err(err)?;
    st.backup_state.lock().unwrap().last_time.insert(id, now());
    let _ = st.save_backup_state();
    Ok(out.display().to_string())
}

#[tauri::command]
fn export_usfm(st: State<AppState>, id: String, dest: String) -> R<String> {
    let p = open_project(&st, &id)?;
    if p.manifest.type_.id != "text" || p.manifest.resource.id == "obs" {
        return Err("Solo los proyectos de texto bíblico se exportan a USFM".into());
    }
    let usfm = btt_core::usfm::export_book(&p).map_err(err)?;
    let dest = if dest.is_empty() {
        st.paths.exports().join(format!("{}_{}.usfm", p.manifest.target_language.id, p.manifest.project.id.to_uppercase()))
    } else {
        PathBuf::from(dest)
    };
    btt_core::project::atomic_write(&dest, &usfm).map_err(err)?;
    Ok(dest.display().to_string())
}

#[tauri::command]
fn export_print_html(st: State<AppState>, id: String, include_incomplete: bool, include_images: bool) -> R<String> {
    let p = open_project(&st, &id)?;
    let m = &p.manifest;
    let rc = m.source_translations.iter().find_map(|s| st.container(&s.language_id, &m.project.id, &s.resource_id).ok());
    let settings = st.settings.lock().unwrap().clone();
    let opts = btt_core::print::PrintOptions {
        include_incomplete,
        include_images,
        font_family: format!("'{}', 'Noto Sans', serif", settings.translation_font),
        font_size_pt: (settings.translation_font_size as f32 / 2.3).round().max(8.0) as u32,
        images_base: String::new(),
    };
    let html = btt_core::print::render_html(&p, rc.as_deref(), &opts).map_err(err)?;
    let dest = st.paths.exports().join(format!("{id}.html"));
    btt_core::project::atomic_write(&dest, &html).map_err(err)?;
    Ok(dest.display().to_string())
}

// ------------------------------------------------------------------ WACS / git

#[derive(Serialize)]
pub struct UploadResult {
    push: PushStatus,
    merge: Option<MergeStatus>,
    url: String,
    repo_url: String,
}

fn credentials(st: &AppState) -> R<(Credentials, String)> {
    let p = st.profile.lock().unwrap();
    let s = p.session.as_ref().ok_or_else(|| "Inicia sesión en el servidor para subir proyectos".to_string())?;
    Ok((Credentials { username: s.user.username.clone(), token: s.token.sha1.clone() }, s.user.username.clone()))
}

#[tauri::command]
async fn upload_project(app: tauri::AppHandle, id: String) -> R<UploadResult> {
    blocking(app, move |st| {
    let p = open_project(st, &id)?;
    let (creds, username) = credentials(st)?;
    let settings = st.settings.lock().unwrap().clone();
    let client = WacsClient::new(&settings.git_api_url).map_err(err)?;
    let repo = client.create_repo(&creds.token, &username, &id).map_err(|e| match e {
        btt_core::Error::Unauthorized => "La sesión expiró. Inicia sesión de nuevo.".to_string(),
        other => other.to_string(),
    })?;
    let author = st.author();
    let merge = gitrepo::pull(&p.root, &repo.clone_url, Some(&creds), &author).map_err(err)?;
    if let MergeStatus::Conflicts { .. } = &merge {
        return Ok(UploadResult { push: PushStatus::Rejected { reason: "conflictos".into() }, merge: Some(merge), url: String::new(), repo_url: repo.html_url });
    }
    if matches!(merge, MergeStatus::AuthFailure) {
        return Err("El servidor rechazó las credenciales. Inicia sesión de nuevo.".into());
    }
    let push = gitrepo::push(&p.root, &repo.clone_url, &creds).map_err(err)?;
    if matches!(push, PushStatus::Ok | PushStatus::UpToDate) {
        st.backup_state.lock().unwrap().last_time.insert(format!("upload:{id}"), now());
        let _ = st.save_backup_state();
    }
    Ok(UploadResult {
        url: btt_core::wacs::reader_url(&settings.reader_url, &username, &id),
        repo_url: repo.html_url,
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
    let api = st.settings.lock().unwrap().git_api_url.clone();
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
    let api = st.settings.lock().unwrap().git_api_url.clone();
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
    let creds = st.profile.lock().unwrap().session.as_ref().map(|s| Credentials { username: s.user.username.clone(), token: s.token.sha1.clone() });
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
fn run_backup_now(st: State<AppState>) -> R<btt_core::backup::BackupReport> {
    let author = st.author();
    let mut bs = st.backup_state.lock().unwrap();
    let report = btt_core::backup::run(&st.paths.translations(), &st.paths.backups(), &mut bs, &author, btt_core::VERSION).map_err(err)?;
    drop(bs);
    st.save_backup_state().map_err(err)?;
    Ok(report)
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
        let mut s = st.settings.lock().unwrap();
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
        chunk_at_commit,
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
        upload_project,
        pull_project,
        restore_backup_branch,
        search_wacs,
        import_from_wacs,
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
    /// El cliente HTTP bloqueante no puede crearse dentro del runtime async
    /// (era la causa de que descargar fuentes no hiciera nada); en `spawn_blocking` sí.
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
