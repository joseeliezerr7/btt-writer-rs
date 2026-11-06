// Capa de acceso a los comandos Rust (Tauri). Tipos espejo de btt-core.
import { invoke } from "@tauri-apps/api/core";

export interface AppInfo {
  version: string;
  data_dir: string;
  legacy_roots: string[];
  platform: string;
  device: string;
}

export interface Settings {
  color_theme: "light" | "dark" | "system";
  ui_language: string;
  gateway_language_mode: boolean;
  translation_font: string;
  translation_font_size: number;
  source_font: string;
  source_font_size: number;
  content_server: "wacs" | "wacs_dev" | "dcs";
  git_api_url: string;
  data_server_url: string;
  media_server_url: string;
  reader_url: string;
  create_account_url: string;
  langnames_url: string;
  index_sqlite_url: string;
  translation_manual_url: string;
  enable_tm_links: boolean;
  backup_interval_minutes: number;
  logging_level: string;
  sort_by: string;
  book_order: string;
  blind_draft: boolean;
  check_hardware: boolean;
  last_project: string;
  migrated_from: string[];
}

export interface User {
  id: number;
  username: string;
  full_name: string;
  email: string;
  avatar_url: string;
}

export interface Profile {
  full_name: string;
  email: string;
  session: { user: User; token: { id: number; name: string; sha1: string } } | null;
  terms_accepted_version: number;
  privacy_accepted: boolean;
}

export interface ProjectItem {
  id: string;
  path: string;
  language_id: string;
  language_name: string;
  language_direction: string;
  project_id: string;
  project_name: string;
  type_id: string;
  resource_id: string;
  resource_name: string;
  translators: string[];
  finished_chunks: number;
  translated_chunks: number;
  modified_at: number;
  total_chunks: number;
  progress: number;
  has_conflicts: boolean;
  last_backup: number | null;
}

export interface ChunkInfo {
  id: string;
  finished: boolean;
  has_text: boolean;
  has_conflict: boolean;
}
export interface ChapterInfo {
  id: string;
  title: string;
  chunks: ChunkInfo[];
}

export interface SourceTranslation {
  language_id: string;
  resource_id: string;
  checking_level: string;
  date_modified: number;
  version: string;
}

export interface Manifest {
  package_version: number;
  format: string;
  generator: { name: string; build: string };
  target_language: { id: string; name: string; direction: string; anglicized_name: string; region: string; is_gateway_language: boolean };
  project: { id: string; name: string };
  type: { id: string; name: string };
  resource: { id: string; name: string };
  source_translations: SourceTranslation[];
  translators: string[];
  finished_chunks: string[];
}

export interface ResourceRow {
  language_slug: string;
  language_name: string;
  language_direction: string;
  project_slug: string;
  project_name: string;
  slug: string;
  name: string;
  type: string;
  translate_mode: string;
  checking_level: string;
  version: string;
  pub_date: string;
  mime_type: string;
  url: string;
  modified_at: number;
  downloaded: boolean;
}

export interface ProjectDetail {
  item: ProjectItem;
  manifest: Manifest;
  chapters: ChapterInfo[];
  sources: ResourceRow[];
  book_title: string;
}

export type Issue =
  | { kind: "empty" }
  | { kind: "consecutive_markers"; verse: number }
  | { kind: "out_of_range"; verse: number }
  | { kind: "duplicate"; verse: number }
  | { kind: "out_of_order"; verse: number }
  | { kind: "missing"; verse: number }
  | { kind: "conflict" };

export interface HelpItem {
  title: string;
  body: string;
}

export interface PrintRequest {
  include_incomplete: boolean;
  include_images: boolean;
  columns: 1 | 2;
  font_size_pt: number;
  font_family: string;
  /** Portada, licencia e índice (apagados en el formato clásico). */
  cover: boolean;
  license: boolean;
  toc: boolean;
  /** Notas de traducción después de cada capítulo y glosario de palabras al final. */
  notes: boolean;
  words: boolean;
  page_numbers: boolean;
  /** Números de versículo en azul (si no, del color del texto). */
  verse_color: boolean;
  paragraphs: "spaced" | "indent";
  heading: "left" | "centered";
  margin_cm: number;
  line_height: number;
}

/** Formato predeterminado: el PDF clásico de los equipos (Arial 12, notas a dos columnas, pie con número de página) con la portada y la licencia del BTT Writer original. */
export const DEFAULT_PRINT: PrintRequest = {
  include_incomplete: true,
  include_images: false,
  columns: 1,
  font_size_pt: 12,
  font_family: "Arial",
  cover: true,
  license: true,
  toc: false,
  notes: true,
  words: true,
  page_numbers: true,
  verse_color: false,
  paragraphs: "spaced",
  heading: "left",
  margin_cm: 2.5,
  line_height: 1.5,
};

export interface CommitInfo {
  id: string;
  time: number;
  message: string;
  author: string;
}

export interface ChunkData {
  text: string;
  finished: boolean;
  expected_verses: number[];
  issues: Issue[];
  has_conflict: boolean;
  conflict_options: string[];
  /** Commit de origen de cada versión del conflicto (autor y fecha), o null. */
  conflict_origins: (CommitInfo | null)[];
  helps: HelpItem[];
}

export interface SourceVerse {
  number: string;
  text: string;
  footnotes: string[];
  paragraphs_before: string[];
}
export interface SourceChunkView {
  chapter: string;
  chunk: string;
  chunk_title: string;
  preamble: string;
  verses: SourceVerse[];
  raw: string;
}
export interface SourceInfo {
  book_title: string;
  chapters: string[];
  chapter_titles: string[];
  direction: string;
  language_name: string;
  resource_name: string;
}

export interface HelpEntry {
  title: string;
  body: string;
}
export interface WordArticle {
  slug: string;
  title: string;
  body: string;
  aliases: string[];
  see_also: string[];
}
export interface Helps {
  notes: HelpEntry[];
  questions: HelpEntry[];
  words: WordArticle[];
  has_notes: boolean;
  has_questions: boolean;
  has_words: boolean;
}

export interface TargetLanguageRow {
  slug: string;
  name: string;
  anglicized_name: string;
  direction: string;
  region: string;
  is_gateway_language: boolean;
  temporary: boolean;
}
export interface ProjectRow {
  slug: string;
  name: string;
  desc: string;
  sort: number;
  chunks_url: string;
  source_language: string;
  category: string;
}
export interface CategoryRow {
  slug: string;
  name: string;
}

export type ImportMode = "merge" | "overwrite" | "skip";
export type ImportStatus =
  | { status: "imported" }
  | { status: "merged" }
  | { status: "overwritten" }
  | { status: "exists" }
  | { status: "conflicts"; files: string[] }
  | { status: "failed"; error: string };
export interface ImportOutcome {
  id: string;
  status: ImportStatus;
}

export interface ArchiveManifest {
  package_version: number;
  timestamp: number;
  generator: { name: string; build: string };
  target_translations: { id: string; path: string; commit_hash: string; direction: string; target_language_name: string }[];
}

export interface UsfmImportSummary {
  book_id: string;
  title: string;
  chapters: number;
  verses: number;
  project_id: string;
  outcome: ImportOutcome | null;
  warnings: string[];
}

export type PushStatus =
  | { status: "ok" }
  | { status: "up_to_date" }
  | { status: "rejected_non_fast_forward" }
  | { status: "rejected"; reason: string }
  | { status: "auth_failure" }
  | { status: "no_remote_repo" };
export type MergeStatus =
  | { status: "up_to_date" }
  | { status: "fast_forward" }
  | { status: "merged" }
  | { status: "conflicts"; files: string[] }
  | { status: "remote_empty" }
  | { status: "auth_failure" }
  | { status: "no_remote_repo" };
export interface UploadResult {
  push: PushStatus;
  merge: MergeStatus | null;
  url: string;
  repo_url: string;
}

// Trabajo en equipo (repositorio compartido + reservas de chunk)
export interface CollabConfig {
  enabled: boolean;
  clone_url: string;
  html_url: string;
  owner: string;
  repo: string;
  last_sync: number;
  last_error: string;
}
export interface ClaimView {
  key: string;
  user: string;
  since: number;
  until: number;
}
export interface CollabStatus {
  config: CollabConfig | null;
  claims: ClaimView[];
  conflicts: string[];
  me: string;
  syncing: boolean;
}
export interface SyncReport {
  merge: MergeStatus | null;
  push: PushStatus | null;
  changed: string[];
  conflicts: string[];
  claims: ClaimView[];
  offline: boolean;
  error: string | null;
  at: number;
}
export interface CollabMembers {
  owner: string;
  collaborators: User[];
  can_manage: boolean;
}

export interface Repo {
  id: number;
  name: string;
  full_name: string;
  html_url: string;
  clone_url: string;
  ssh_url: string;
  private: boolean;
  empty: boolean;
  description: string;
  owner: User;
  updated_at: string;
}

export interface ChunkValidation {
  chapter: string;
  chunk: string;
  finished: boolean;
  issues: Issue[];
}
export interface MarkAllResult {
  marked: number;
  total: number;
  invalid: ChunkValidation[];
}
export interface Commit {
  id: string;
  time: number;
  message: string;
}
/** Versión guardada de un chunk (historial). */
export interface HistoryEntry extends Commit {
  author: string;
  /** Fragmento del texto de esa versión. */
  preview: string;
  /** Es el texto que está ahora en el proyecto. */
  current: boolean;
}
export interface SearchHit {
  chapter: string;
  chunk: string;
  snippet: string;
}
export interface BackupEntry {
  name: string;
  path: string;
  modified: number;
}
export interface BackupReport {
  backed_up: string[];
  skipped: string[];
  failed: [string, string][];
}
export interface WordIndexEntry {
  slug: string;
  title: string;
}

/** Dentro de Tauri usa IPC; en un navegador normal usa el backend simulado (solo para probar la interfaz). */
export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
/** Android o iOS: sin diálogo nativo de "guardar como" ni actualizador. */
export const isMobile = typeof navigator !== "undefined" && /Android|iPhone|iPad/i.test(navigator.userAgent);

const call = async <T,>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
  if (isTauri) return invoke<T>(cmd, args);
  const { mockInvoke } = await import("./mock");
  return mockInvoke<T>(cmd, args);
};

export const api = {
  appInfo: () => call<AppInfo>("app_info"),
  getSettings: () => call<Settings>("get_settings"),
  saveSettings: (settings: Settings) => call<Settings>("save_settings", { settings }),
  getProfile: () => call<Profile>("get_profile"),
  saveProfile: (profile: Profile) => call<Profile>("save_profile", { profile }),
  logout: () => call<void>("logout"),
  wacsLogin: (username: string, password: string) => call<Profile>("wacs_login", { username, password }),

  listProjects: () => call<ProjectItem[]>("list_projects"),
  getProject: (id: string) => call<ProjectDetail>("get_project", { id }),
  createProject: (spec: { language: string; project: string; type: string; resource: string }) => call<string>("create_project", { spec }),
  deleteProject: (id: string) => call<string>("delete_project", { id }),
  changeResource: (id: string, resource: string) => call<string>("change_resource", { id, resource }),
  setTranslators: (id: string, translators: string[]) => call<string[]>("set_translators", { id, translators }),
  setBookTitle: (id: string, title: string) => call<void>("set_book_title", { id, title }),
  setProjectSources: (id: string, sources: { language: string; resource: string }[]) => call<Manifest>("set_project_sources", { id, sources }),

  readChunk: (id: string, chapter: string, chunk: string) => call<ChunkData>("read_chunk", { id, chapter, chunk }),
  writeChunk: (id: string, chapter: string, chunk: string, text: string) => call<void>("write_chunk", { id, chapter, chunk, text }),
  writeHelps: (id: string, chapter: string, chunk: string, items: HelpItem[]) => call<string>("write_helps", { id, chapter, chunk, items }),
  setChunkFinished: (id: string, chapter: string, chunk: string, finished: boolean) => call<Issue[]>("set_chunk_finished", { id, chapter, chunk, finished }),
  markAllChunks: (id: string) => call<MarkAllResult>("mark_all_chunks", { id }),
  validateProject: (id: string) => call<ChunkValidation[]>("validate_project", { id }),
  commitProject: (id: string) => call<string | null>("commit_project", { id }),
  chunkHistory: (id: string, chapter: string, chunk: string) => call<HistoryEntry[]>("chunk_history", { id, chapter, chunk }),
  chunkAtCommit: (id: string, commit: string, chapter: string, chunk: string) => call<string>("chunk_at_commit", { id, commit, chapter, chunk }),
  /** Vuelve a una versión anterior (queda registrada como restauración) y devuelve el texto. */
  restoreChunk: (id: string, commit: string, chapter: string, chunk: string) => call<string>("restore_chunk", { id, commit, chapter, chunk }),
  searchProject: (id: string, query: string, inSource: boolean) => call<SearchHit[]>("search_project", { id, query, inSource }),

  getSourceChapter: (language: string, project: string, resource: string, chapter: string) =>
    call<SourceChunkView[]>("get_source_chapter", { language, project, resource, chapter }),
  getSourceInfo: (language: string, project: string, resource: string) => call<SourceInfo>("get_source_info", { language, project, resource }),
  getHelps: (language: string, project: string, resource: string, chapter: string, chunk: string) =>
    call<Helps>("get_helps", { language, project, resource, chapter, chunk }),
  getWord: (language: string, slug: string) => call<WordArticle | null>("get_word", { language, slug }),
  wordIndex: (language: string, query: string) => call<WordIndexEntry[]>("word_index", { language, query }),

  searchLanguages: (query: string, limit = 50) => call<TargetLanguageRow[]>("search_languages", { query, limit }),
  listBooks: (language: string) => call<ProjectRow[]>("list_books", { language }),
  listCategories: (language: string) => call<CategoryRow[]>("list_categories", { language }),
  availableSources: (project: string, glMode: boolean) => call<ResourceRow[]>("available_sources", { project, glMode }),
  allResources: (project: string) => call<ResourceRow[]>("all_resources", { project }),
  downloadSource: (language: string, project: string, resource: string) => call<string[]>("download_source", { language, project, resource }),
  /** Descarga todos los libros de un idioma/recurso que falten (emite `download-progress`). */
  downloadLanguageSources: (language: string, resource: string) => call<string[]>("download_language_sources", { language, resource }),
  deleteSource: (language: string, project: string, resource: string) => call<void>("delete_source", { language, project, resource }),
  updateCatalog: () => call<string[]>("update_catalog"),
  updateLanguages: () => call<number>("update_languages"),
  downloadIndex: () => call<void>("download_index"),
  importIndexFile: (path: string) => call<void>("import_index_file", { path }),
  importSourceContainer: (path: string) => call<string>("import_source_container", { path }),

  inspectTstudio: (path: string) => call<ArchiveManifest>("inspect_tstudio", { path }),
  importTstudio: (path: string, mode: ImportMode) => call<ImportOutcome[]>("import_tstudio", { path, mode }),
  importUsfm: (path: string, language: string, book: string, mode: ImportMode) => call<UsfmImportSummary>("import_usfm", { path, language, book, mode }),
  exportTstudio: (id: string, dest = "") => call<string>("export_tstudio", { id, dest }),
  exportUsfm: (id: string, dest = "") => call<string>("export_usfm", { id, dest }),
  exportPrintHtml: (id: string, opts: PrintRequest) => call<string>("export_print_html", { id, opts }),
  renderPrintHtml: (id: string, opts: PrintRequest) => call<string>("render_print_html", { id, opts }),

  uploadProject: (id: string) => call<UploadResult>("upload_project", { id }),
  pullProject: (id: string) => call<MergeStatus>("pull_project", { id }),
  restoreBackupBranch: (id: string) => call<void>("restore_backup_branch", { id }),
  searchWacs: (username: string, query: string) => call<Repo[]>("search_wacs", { username, query }),
  importFromWacs: (cloneUrl: string, mode: ImportMode) => call<ImportOutcome>("import_from_wacs", { cloneUrl, mode }),
  // Equipo
  collabStatus: (id: string) => call<CollabStatus>("collab_status", { id }),
  collabProjects: () => call<string[]>("collab_projects"),
  collabEnable: (id: string, members: string[]) => call<CollabStatus>("collab_enable", { id, members }),
  collabDisable: (id: string) => call<CollabStatus>("collab_disable", { id }),
  collabMembers: (id: string) => call<CollabMembers>("collab_members", { id }),
  collabAddMember: (id: string, username: string) => call<CollabMembers>("collab_add_member", { id, username }),
  collabRemoveMember: (id: string, username: string) => call<CollabMembers>("collab_remove_member", { id, username }),
  collabSharedRepos: () => call<Repo[]>("collab_shared_repos"),
  collabJoin: (cloneUrl: string, mode: ImportMode) => call<ImportOutcome>("collab_join", { cloneUrl, mode }),
  collabSync: (id: string, editing: string | null, push: boolean) => call<SyncReport>("collab_sync", { id, editing, push }),
  collabRelease: (id: string, key: string) => call<void>("collab_release", { id, key }),
  collabClaim: (id: string, key: string) => call<boolean>("collab_claim", { id, key }),
  searchUsers: (query: string) => call<User[]>("search_users", { query }),

  listBackups: () => call<BackupEntry[]>("list_backups"),
  runBackupNow: () => call<BackupReport>("run_backup_now"),
  migrateLegacy: (root: string) => call<ImportOutcome[]>("migrate_legacy", { root }),
  inspectLegacy: (root: string) => call<{ path: string; projects: number }>("inspect_legacy", { root }),
  sendFeedback: (email: string, message: string) => call<string>("send_feedback", { email, message }),
  openDataDir: () => call<string>("open_data_dir"),
};

/** Nombre legible del tipo de recurso de un proyecto (como la columna "Resource Type"). */
export function resourceLabel(p: { type_id: string; resource_id: string }, t: (k: string) => string): string {
  if (p.type_id === "text") return p.resource_id === "reg" ? t("type.text") : `${t("type.text")} ${p.resource_id}`;
  if (p.type_id === "tn") return t("type.notes");
  if (p.type_id === "tq") return t("type.questions");
  if (p.type_id === "tw") return t("type.words");
  return p.type_id;
}
