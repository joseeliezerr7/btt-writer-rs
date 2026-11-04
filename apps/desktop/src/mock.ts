// Backend simulado para abrir la interfaz en un navegador normal (sin Tauri).
// Solo se usa cuando no existe el puente IPC; permite probar las pantallas.
import type { ChunkData, HelpItem, Issue } from "./api";

const languages = [
  { slug: "es-419", name: "Español Latinoamericano", anglicized_name: "Spanish", direction: "ltr", region: "Americas", is_gateway_language: true, temporary: false },
  { slug: "cbt", name: "Chayahuita", anglicized_name: "Chayahuita", direction: "ltr", region: "Americas", is_gateway_language: false, temporary: false },
  { slug: "miq", name: "Mískito", anglicized_name: "Miskito", direction: "ltr", region: "Americas", is_gateway_language: false, temporary: false },
  { slug: "arb", name: "العربية", anglicized_name: "Arabic", direction: "rtl", region: "Asia", is_gateway_language: true, temporary: false },
  { slug: "qaa-x-demo", name: "Lengua demo", anglicized_name: "Demo", direction: "ltr", region: "", is_gateway_language: false, temporary: true },
];
const books = [
  { slug: "gen", name: "Génesis", desc: "", sort: 1, chunks_url: "", source_language: "es-419", category: "bible-ot" },
  { slug: "jon", name: "Jonás", desc: "", sort: 32, chunks_url: "", source_language: "es-419", category: "bible-ot" },
  { slug: "mrk", name: "Marcos", desc: "", sort: 41, chunks_url: "", source_language: "es-419", category: "bible-nt" },
  { slug: "3jn", name: "3 Juan", desc: "", sort: 64, chunks_url: "", source_language: "es-419", category: "bible-nt" },
  { slug: "jud", name: "Judas", desc: "", sort: 65, chunks_url: "", source_language: "es-419", category: "bible-nt" },
  { slug: "obs", name: "Open Bible Stories", desc: "", sort: 100, chunks_url: "", source_language: "es-419", category: "other" },
  { slug: "bible", name: "translationWords", desc: "", sort: 101, chunks_url: "", source_language: "es-419", category: "other" },
];
const resource = (lang: string, project: string, slug: string, name: string, downloaded: boolean, type = "book") => ({
  language_slug: lang, language_name: lang === "en" ? "English" : "Español", language_direction: "ltr", project_slug: project, project_name: project,
  slug, name, type, translate_mode: "all", checking_level: "3", version: "7", pub_date: "", mime_type: "", url: "x", modified_at: 0, downloaded,
});
const resources: Record<string, ReturnType<typeof resource>[]> = {};
for (const b of books) {
  resources[b.slug] = [resource("en", b.slug, "ulb", "Unlocked Literal Bible", true), resource("en", b.slug, "udb", "Unlocked Dynamic Bible", false), resource("es-419", b.slug, "ulb", "ULB Español", b.slug === "3jn")];
}
resources["bible"] = [resource("en", "bible", "tw", "translationWords", true, "dict")];

const SOURCE: Record<string, Record<string, { number: string; text: string }[]>> = {
  "01": {
    "01": [
      { number: "1", text: "El anciano a Gayo, el amado, a quien amo en la verdad." },
      { number: "2", text: "Amado, oro para que te vaya bien en todo y tengas salud, así como le va bien a tu alma." },
      { number: "3", text: "Pues me alegré mucho cuando vinieron hermanos y dieron testimonio de tu verdad." },
      { number: "4", text: "No tengo mayor gozo que oír que mis hijos andan en la verdad." },
    ],
    "05": [
      { number: "5", text: "Amado, fielmente te conduces cuando sirves a los hermanos, especialmente a los extranjeros." },
      { number: "6", text: "Ellos han dado testimonio de tu amor ante la iglesia." },
      { number: "7", text: "Porque por amor del Nombre salieron sin aceptar nada de los gentiles." },
      { number: "8", text: "Nosotros, pues, debemos acoger a tales personas." },
    ],
    "09": [
      { number: "9", text: "Yo escribí algo a la iglesia, pero Diótrefes no nos recibe." },
      { number: "10", text: "Por esta causa, si yo fuere, recordaré las obras que hace." },
    ],
    "11": [
      { number: "11", text: "Amado, no imites lo malo, sino lo bueno." },
      { number: "12", text: "Todos dan testimonio de Demetrio." },
    ],
    "13": [
      { number: "13", text: "Yo tenía muchas cosas que escribirte." },
      { number: "14", text: "Espero verte en breve. La paz sea contigo. Los amigos te saludan." },
    ],
  },
};

interface MockProject {
  id: string; language: typeof languages[number]; project: string; type: string; resource: string;
  chunks: Record<string, string>; finished: string[]; translators: string[]; sources: { language_id: string; resource_id: string }[]; title: string;
}
const projects: MockProject[] = [
  {
    id: "cbt_3jn_text_reg", language: languages[1], project: "3jn", type: "text", resource: "reg",
    chunks: { "01/01": "\\v 1 El anciano a Gayo el amado, a quien amo en verdad.\n\\v 2 Amado, oro para que seas prosperado.\n\\v 3 Pues mucho me regocijé.\n\\v 4 No tengo yo mayor gozo.", "01/05": "\\v 5 Amado \\f + \\ft nota de ejemplo\\f* fielmente te conduces.\n\\v 6 \\v 7 Porque por amor del Nombre.", "01/09": "<<<<<<< HEAD\n\\v 9 Yo escribí algo (local).\n\\v 10 Por esta causa.\n=======\n\\v 9 Yo escribí algo (servidor).\n\\v 10 Por esta causa.\n>>>>>>> refs/heads/master" },
    finished: ["01-01"], translators: ["Joseeliezerr123"], sources: [{ language_id: "en", resource_id: "ulb" }, { language_id: "es-419", resource_id: "ulb" }], title: "3 Juan",
  },
  { id: "miq_jon_text_reg", language: languages[2], project: "jon", type: "text", resource: "reg", chunks: {}, finished: [], translators: ["Ana"], sources: [{ language_id: "en", resource_id: "ulb" }], title: "" },
  { id: "es-419_3jn_tn", language: languages[0], project: "3jn", type: "tn", resource: "", chunks: {}, finished: [], translators: ["Ana"], sources: [{ language_id: "en", resource_id: "ulb" }], title: "" },
];
let settings = {
  color_theme: "system", ui_language: "es", gateway_language_mode: true, translation_font: "Noto Sans", translation_font_size: 25, source_font: "Noto Sans", source_font_size: 25,
  content_server: "wacs", git_api_url: "https://content.bibletranslationtools.org/api/v1", data_server_url: "https://content.bibletranslationtools.org", media_server_url: "https://api.bibletranslationtools.org",
  reader_url: "https://read.bibleineverylanguage.org", create_account_url: "https://content.bibletranslationtools.org/user/sign_up", langnames_url: "", index_sqlite_url: "", translation_manual_url: "",
  enable_tm_links: true, backup_interval_minutes: 5, logging_level: "info", sort_by: "project", book_order: "bible", blind_draft: false, check_hardware: true, last_project: "", migrated_from: [] as string[],
};
let profile = { full_name: "", email: "", session: null as null | { user: { id: number; username: string; full_name: string; email: string; avatar_url: string }; token: { id: number; name: string; sha1: string } }, terms_accepted_version: 0, privacy_accepted: false };

const expectedVerses = (chapter: string, chunk: string) => (SOURCE[chapter]?.[chunk] ?? []).map((v) => Number(v.number));
function validate(text: string, expected: number[]): Issue[] {
  if (!text.trim()) return [{ kind: "empty" }];
  if (text.includes("<<<<<<< ")) return [{ kind: "conflict" }];
  const found = [...text.matchAll(/\\v\s+(\d+)\s*([^\\]*)/g)].map((m) => ({ n: Number(m[1]), t: m[2].trim() }));
  const issues: Issue[] = [];
  for (const f of found) if (!f.t) issues.push({ kind: "consecutive_markers", verse: f.n });
  for (const e of expected) if (!found.some((f) => f.n === e)) issues.push({ kind: "missing", verse: e });
  return issues;
}
const summary = (p: MockProject) => {
  const total = p.type === "tw" ? 20 : Object.values(SOURCE).reduce((n, c) => n + Object.keys(c).length, 0) + 2;
  return {
    id: p.id, path: "C:/mock/" + p.id, language_id: p.language.slug, language_name: p.language.name, language_direction: p.language.direction, project_id: p.project,
    project_name: "", type_id: p.type, resource_id: p.resource, resource_name: "", translators: p.translators, finished_chunks: p.finished.length,
    translated_chunks: Object.values(p.chunks).filter(Boolean).length, modified_at: Math.floor(Date.now() / 1000) - 3600, total_chunks: total, progress: p.finished.length / total,
    has_conflicts: Object.values(p.chunks).some((c) => c.includes("<<<<<<< ")), last_backup: null,
  };
};
const find = (id: string) => {
  const p = projects.find((x) => x.id === id);
  if (!p) throw new Error("Proyecto no encontrado: " + id);
  return p;
};
const delay = <T,>(v: T) => new Promise<T>((r) => setTimeout(() => r(v), 60));

export async function mockInvoke<T>(cmd: string, a: Record<string, unknown> = {}): Promise<T> {
  const s = (k: string) => String(a[k] ?? "");
  switch (cmd) {
    case "app_info": return delay({ version: "3.0.0-mock", data_dir: "C:/Users/demo/BTT-Writer-3", legacy_roots: ["C:/Users/demo/BTT-Writer-2.0"], platform: "browser", device: "navegador" }) as T;
    case "get_settings": return delay(settings) as T;
    case "save_settings": settings = { ...settings, ...(a.settings as typeof settings) }; return delay(settings) as T;
    case "get_profile": return delay(profile) as T;
    case "save_profile": profile = { ...profile, ...(a.profile as typeof profile) }; return delay(profile) as T;
    case "logout": profile = { full_name: "", email: "", session: null, terms_accepted_version: 0, privacy_accepted: false }; return delay(undefined) as T;
    case "wacs_login":
      if (s("password") !== "demo") throw new Error("Usuario o contraseña incorrectos");
      profile = { ...profile, full_name: profile.full_name || s("username"), session: { user: { id: 1, username: s("username"), full_name: "", email: "", avatar_url: "" }, token: { id: 1, name: "t", sha1: "x" } } };
      return delay(profile) as T;
    case "list_projects": return delay(projects.map(summary)) as T;
    case "get_project": {
      const p = find(s("id"));
      const chapters = [{ id: "front", title: "", chunks: [{ id: "title", finished: p.finished.includes("front-title"), has_text: !!p.title, has_conflict: false }] }];
      if (p.type === "tw") chapters.push({ id: "01", title: "", chunks: ["god", "love", "elder"].map((w) => ({ id: w, finished: p.finished.includes("01-" + w), has_text: !!p.chunks["01/" + w], has_conflict: false })) });
      else for (const ch of Object.keys(SOURCE)) chapters.push({ id: ch, title: p.chunks[ch + "/title"] ?? "", chunks: ["title", ...Object.keys(SOURCE[ch])].map((c) => ({ id: c, finished: p.finished.includes(`${ch}-${c}`), has_text: !!p.chunks[`${ch}/${c}`], has_conflict: (p.chunks[`${ch}/${c}`] ?? "").includes("<<<<<<< ") })) });
      return delay({
        item: summary(p), book_title: p.title, chapters,
        manifest: { package_version: 8, format: "usfm", generator: { name: "mock", build: "1" }, target_language: { id: p.language.slug, name: p.language.name, direction: p.language.direction, anglicized_name: "", region: "", is_gateway_language: false }, project: { id: p.project, name: "" }, type: { id: p.type, name: "" }, resource: { id: p.resource, name: "" }, source_translations: p.sources.map((x) => ({ ...x, checking_level: "3", date_modified: 0, version: "" })), translators: p.translators, finished_chunks: p.finished },
        sources: p.sources.map((x) => resources[p.type === "tw" ? "bible" : p.project]?.find((r) => r.language_slug === x.language_id && r.slug === x.resource_id) ?? resource(x.language_id, p.project, x.resource_id, x.resource_id, false)),
      }) as T;
    }
    case "create_project": {
      const spec = a.spec as { language: string; project: string; type: string; resource: string };
      const id = spec.type === "text" ? `${spec.language}_${spec.project}_text_${spec.resource || "reg"}` : `${spec.language}_${spec.project}_${spec.type}`;
      if (projects.some((p) => p.id === id)) throw new Error("Ya estás traduciendo este libro a este idioma");
      projects.push({ id, language: languages.find((l) => l.slug === spec.language)!, project: spec.project, type: spec.type, resource: spec.type === "text" ? spec.resource || "reg" : "", chunks: {}, finished: [], translators: [profile.full_name], sources: [{ language_id: "en", resource_id: spec.type === "tw" ? "tw" : "ulb" }], title: "" });
      return delay(id) as T;
    }
    case "delete_project": projects.splice(projects.findIndex((p) => p.id === s("id")), 1); return delay("C:/mock/backups/" + s("id") + ".tstudio") as T;
    case "change_resource": { const p = find(s("id")); p.resource = s("resource"); p.id = `${p.language.slug}_${p.project}_text_${p.resource}`; return delay(p.id) as T; }
    case "set_translators": { const p = find(s("id")); p.translators = a.translators as string[]; return delay(p.translators) as T; }
    case "set_book_title": find(s("id")).title = s("title"); return delay(undefined) as T;
    case "set_project_sources": { const p = find(s("id")); p.sources = (a.sources as { language: string; resource: string }[]).map((x) => ({ language_id: x.language, resource_id: x.resource })); return mockInvoke("get_project", { id: p.id }).then((d: any) => d.manifest) as T; }
    case "read_chunk": {
      const p = find(s("id"));
      const key = `${s("chapter")}/${s("chunk")}`;
      const text = s("chapter") === "front" ? p.title : p.chunks[key] ?? "";
      const expected = p.type === "text" && s("chunk") !== "title" ? expectedVerses(s("chapter"), s("chunk")) : [];
      const has_conflict = text.includes("<<<<<<< ");
      const data: ChunkData = { text, finished: p.finished.includes(`${s("chapter")}-${s("chunk")}`), expected_verses: expected, issues: p.type === "text" ? validate(text, expected) : [], has_conflict, conflict_options: has_conflict ? [text.split("=======")[0].split("\n").slice(1).join("\n").trim(), text.split("=======")[1].split(">>>>>>>")[0].trim()] : [], conflict_origins: has_conflict ? [{ id: "a1", time: Math.floor(Date.now() / 1000) - 3600, message: "autosave", author: profile.full_name || "Jose" }, { id: "b2", time: Math.floor(Date.now() / 1000) - 86400 * 40, message: "autosave", author: "Juan_HRomeroC" }] : [], helps: p.type !== "text" && text ? (JSON.parse(text) as HelpItem[]) : [] };
      return delay(data) as T;
    }
    case "write_chunk": { const p = find(s("id")); if (s("chapter") === "front") p.title = s("text"); else p.chunks[`${s("chapter")}/${s("chunk")}`] = s("text"); return delay(undefined) as T; }
    case "write_helps": { const p = find(s("id")); const txt = JSON.stringify(a.items); p.chunks[`${s("chapter")}/${s("chunk")}`] = txt; return delay(txt) as T; }
    case "set_chunk_finished": {
      const p = find(s("id")); const key = `${s("chapter")}-${s("chunk")}`;
      if (a.finished) { const issues = validate(p.chunks[`${s("chapter")}/${s("chunk")}`] ?? (s("chapter") === "front" ? p.title : ""), p.type === "text" && s("chunk") !== "title" ? expectedVerses(s("chapter"), s("chunk")) : []); if (issues.length) return delay(issues) as T; if (!p.finished.includes(key)) p.finished.push(key); }
      else p.finished = p.finished.filter((k) => k !== key);
      return delay([]) as T;
    }
    case "mark_all_chunks": { const p = find(s("id")); let marked = 0; for (const [k, v] of Object.entries(p.chunks)) { const [ch, c] = k.split("/"); if (validate(v, expectedVerses(ch, c)).length === 0) { if (!p.finished.includes(`${ch}-${c}`)) p.finished.push(`${ch}-${c}`); marked++; } } return delay({ marked, total: 12, invalid: [] }) as T; }
    case "validate_project": { const p = find(s("id")); return delay(Object.entries(SOURCE).flatMap(([ch, chunks]) => Object.keys(chunks).map((c) => ({ chapter: ch, chunk: c, finished: p.finished.includes(`${ch}-${c}`), issues: validate(p.chunks[`${ch}/${c}`] ?? "", expectedVerses(ch, c)) })))) as T; }
    case "commit_project": return delay(null) as T;
    case "chunk_history": return delay([{ id: "abc123", time: Math.floor(Date.now() / 1000) - 600, message: "auto save" }, { id: "def456", time: Math.floor(Date.now() / 1000) - 7200, message: "auto save" }]) as T;
    case "chunk_at_commit": return delay("\\v 1 Versión anterior del texto.") as T;
    case "search_project": return delay(Object.entries(find(s("id")).chunks).filter(([, v]) => v.toLowerCase().includes(s("query").toLowerCase())).map(([k, v]) => ({ chapter: k.split("/")[0], chunk: k.split("/")[1], snippet: v.slice(0, 80) }))) as T;
    case "get_source_chapter": return delay(Object.entries(SOURCE[s("chapter")] ?? {}).map(([chunk, verses]) => ({ chapter: s("chapter"), chunk, chunk_title: `${verses[0].number}-${verses[verses.length - 1].number}`, preamble: "", verses: verses.map((v) => ({ ...v, footnotes: v.number === "5" ? ["Nota al pie de la fuente"] : [], paragraphs_before: v.number === "1" ? ["p"] : [] })), raw: "" }))) as T;
    case "get_source_info": return delay({ book_title: "3 John", chapters: ["01"], chapter_titles: ["Chapter 1"], direction: "ltr", language_name: "English", resource_name: "ULB" }) as T;
    case "get_helps": return delay({ has_notes: true, has_words: true, has_questions: true, notes: [{ title: "El anciano", body: "Se refiere al apóstol Juan. Ver [[:en:ta:vol2:translate:figs_metonymy|metonimia]]." }, { title: "a quien amo en la verdad", body: "**Posibles significados**: 1) «a quien amo de verdad» 2) «a quien amo porque ambos conocemos la verdad»." }], questions: [{ title: "¿Quién escribe la carta?", body: "El anciano." }], words: [{ slug: "love", title: "amor, amar", body: "## Definición\n\nAmar a otra persona es cuidarla y hacer cosas que la beneficien.\n\n## Ver también\n\n* [Dios](../kt/god.md)", aliases: [], see_also: ["god"] }, { slug: "truth", title: "verdad", body: "# Tiempo Bíblico: día\n\n## Definición\n\n<p>El término &quot;día&quot; literalmente se refiere a un periodo de tiempo de 24 horas.  Tambien es utilizado figurativamente.</p>\n<ul>\n<li>Para los Israelitas y Judíos, un día comienza a la puesta del sol.</li>\n<li>A veces el término &quot;día&quot; se usa para un periodo <b>mas largo</b>, como el &quot;día de Yave&quot;.</li>\n</ul>\n<h2>Ver también</h2>\n<ul><li><a href=\"../kt/god.md\">Dios</a></li></ul>", aliases: [], see_also: [] }] }) as T;
    case "get_word": return delay({ slug: s("slug"), title: s("slug"), body: "## Definición\n\nArtículo de ejemplo para " + s("slug") + ".", aliases: [], see_also: [] }) as T;
    case "word_index": return delay([{ slug: "god", title: "Dios" }, { slug: "love", title: "amor" }, { slug: "truth", title: "verdad" }].filter((w) => w.title.includes(s("query")))) as T;
    case "search_languages": { const q = s("query").toLowerCase(); return delay(languages.filter((l) => !q || l.slug.includes(q) || l.name.toLowerCase().includes(q) || l.anglicized_name.toLowerCase().includes(q))) as T; }
    case "list_books": return delay(books) as T;
    case "list_categories": return delay([{ slug: "bible-ot", name: "Antiguo Testamento" }, { slug: "bible-nt", name: "Nuevo Testamento" }, { slug: "other", name: "Otros" }]) as T;
    case "available_sources": return delay(resources[s("project")] ?? []) as T;
    case "all_resources": return delay(s("project") ? resources[s("project")] ?? [] : Object.values(resources).flat()) as T;
    case "source_languages": return delay([{ slug: "en", name: "English", direction: "ltr" }, { slug: "es-419", name: "Español", direction: "ltr" }]) as T;
    case "download_source": { const r = resources[s("project")]?.find((x) => x.language_slug === s("language") && x.slug === s("resource")); if (r) r.downloaded = true; return new Promise((res) => setTimeout(() => res([`${s("language")}_${s("project")}_${s("resource")}`] as T), 800)); }
    case "delete_source": { const r = resources[s("project")]?.find((x) => x.language_slug === s("language") && x.slug === s("resource")); if (r) r.downloaded = false; return delay(undefined) as T; }
    case "update_catalog": return new Promise((res) => setTimeout(() => res(["en_jud_ulb", "es-419_jud_ulb"] as T), 700));
    case "update_languages": return delay(9166) as T;
    case "download_index": throw new Error("Error del servidor (403): el servidor devolvió una página web en lugar de datos (posible bloqueo anti-bot)");
    case "inspect_tstudio": return delay({ package_version: 3, timestamp: 0, generator: { name: "mock", build: "1" }, target_translations: [{ id: "aa_jud_text_reg", path: "aa_jud_text_reg", commit_hash: "", direction: "ltr", target_language_name: "Afar" }] }) as T;
    case "import_tstudio": return delay([{ id: "aa_jud_text_reg", status: { status: a.mode === "skip" && projects.some((p) => p.id === "cbt_3jn_text_reg") ? "imported" : "merged" } }]) as T;
    case "import_usfm": return delay({ book_id: "jud", title: "Judas", chapters: 1, verses: 25, project_id: s("language") ? `${s("language")}_jud_text_reg` : "", outcome: s("language") ? { id: `${s("language")}_jud_text_reg`, status: { status: "imported" } } : null, warnings: [] }) as T;
    case "export_tstudio": return delay("C:/Users/demo/BTT-Writer-3/exports/" + s("id") + ".tstudio") as T;
    case "export_usfm": return delay("C:/Users/demo/BTT-Writer-3/exports/" + s("id") + ".usfm") as T;
    case "export_print_html": return delay("C:/Users/demo/BTT-Writer-3/exports/" + s("id") + ".html") as T;
    case "render_print_html": {
      const o = a.opts as { columns: number; font_size_pt: number; include_incomplete: boolean; cover: boolean; toc: boolean; license: boolean; notes: boolean; words: boolean; page_numbers: boolean; verse_color: boolean; paragraphs: string; heading: string; margin_cm: number; line_height: number; font_family: string };
      const cols = o.columns >= 2 ? "columns: 2; column-gap: 2em;" : "";
      const heading = o.heading === "centered" ? ".chapter h2{font-size:1.7em;text-align:center;color:#0b4f70;border-bottom:1px solid #cfd6e0}" : ".chapter h2{font-size:1em;text-align:start;color:#2f6fb5;margin:0 0 1.2em}";
      const verse = o.verse_color ? "sup.v{color:#0e6f9a;font-weight:700}" : "sup.v{color:inherit;font-weight:400}";
      const para = o.paragraphs === "indent" ? "p{margin:0}p.p{text-indent:1.4em}" : "p{margin:0 0 1em}";
      const footer = o.page_numbers ? '@bottom-center{content:"Página " counter(page) " de " counter(pages);font:10pt Cambria,serif}' : "";
      const m = o.margin_cm || 2.5;
      const html = `<!doctype html><html><head><meta charset="utf-8"><style>@page{size:Letter;margin:${m}cm;${footer}}html{background:#e9ecf1}body{font-family:'${o.font_family || "Arial"}',Arial,sans-serif;font-size:${o.font_size_pt || 12}pt;line-height:${o.line_height || 1.5};margin:0;background:#fff}@media screen{body{max-width:8.5in;margin:1.2rem auto;padding:${m}cm;box-shadow:0 2px 20px rgba(0,0,0,.18);min-height:11in}}.cover{text-align:center;page-break-after:always;padding-top:30%}.cover h1{font-size:2.6em}.rule{width:4em;height:3px;background:#2f6fb5;margin:1.2em auto}.license,.toc{page-break-after:always}.license h3,.toc h3{border-bottom:2px solid #2f6fb5}${heading}.text{${cols}text-align:justify}sup.v{font-size:.62em;line-height:0}${verse}${para}.helps{page-break-before:always;font-size:.92em}.helps-body{columns:2;column-gap:2.2em}.helps .ref{font-style:italic;color:#2f6fb5;margin:0 0 .6em 2em}.helps .note{break-inside:avoid;margin:0 0 .9em}.helps .note-title{color:#2f6fb5}.helps .note p{margin:0 0 .3em}.words{page-break-before:always}.words h2{font-size:1.15em;color:#2f6fb5}.word{border-bottom:1px solid #999;padding-bottom:.6em;margin-bottom:1em}.word h3{font-size:1em;color:#2f6fb5;margin:0}.word h4{font-size:1em;font-weight:400;font-style:italic;color:#2f6fb5;margin:.2em 0 0 1.5em}.word p{margin:0 0 .4em}</style></head><body>${o.cover ? '<section class="cover"><h1>3 Juan</h1><div class="rule"></div><p>Chayahuita</p></section>' : ""}${o.license ? '<section class="license"><h3>Licencia</h3><p>Creative Commons Attribution-ShareAlike 4.0</p></section>' : ""}${o.toc ? '<section class="toc"><h3>Contenido</h3><ol><li>3 Juan Capítulo 1</li></ol></section>' : ""}<section class="chapter"><h2>3 Juan Capítulo 1</h2><div class="text"><p class="p"><sup class="v">1</sup>El anciano a Gayo el amado, a quien amo en verdad. <sup class="v">2</sup>Amado, oro para que seas prosperado en todas las cosas y que tengas salud, así como prospera tu alma.</p><p class="p"><sup class="v">3</sup>Pues mucho me regocijé cuando vinieron los hermanos y dieron testimonio de tu verdad.${o.include_incomplete ? " <sup class='v'>4</sup>No tengo yo mayor gozo que este, el oír que mis hijos andan en la verdad." : ""}</p></div></section>${o.notes ? '<section class="helps"><div class="helps-body"><div class="ref">3 Juan 1:1</div><div class="note"><div class="note-title">El anciano</div><p>Esto se refiere a Juan, el apóstol y discípulo de Jesús.</p></div><div class="note"><div class="note-title">a quien amo en verdad</div><p>Traducción Alterna: "a quien amo de verdad" (Ver: figs-explicit)</p></div><div class="ref">3 Juan 1:3</div><div class="note"><div class="note-title">andan en la verdad</div><p>Andar es una metáfora de cómo vive una persona.</p></div></div></section>' : ""}${o.words ? '<section class="words"><h2>Palabras de traducción</h2><article class="word"><h3>verdad</h3><h4>Definición:</h4><p>El término "verdad" se refiere a hechos, eventos y declaraciones que son reales.</p></article></section>' : ""}</body></html>`;
      return delay(html) as T;
    }
    case "upload_project": return new Promise((res) => setTimeout(() => res({ push: { status: "ok" }, merge: { status: "up_to_date" }, url: "https://read.bibleineverylanguage.org/demo/" + s("id"), repo_url: "https://content.bibletranslationtools.org/demo/" + s("id") } as T), 900));
    case "pull_project": return delay({ status: "up_to_date" }) as T;
    case "restore_backup_branch": return delay(undefined) as T;
    case "search_wacs": return delay([{ id: 1, name: "cbt_3jn_text_reg", full_name: `${s("username") || "demo"}/cbt_3jn_text_reg`, html_url: "", clone_url: "https://x/cbt_3jn_text_reg.git", ssh_url: "", private: false, empty: false, description: "", owner: { id: 1, username: s("username") || "demo", full_name: "", email: "", avatar_url: "" }, updated_at: "" }, { id: 2, name: "cbt_gen_tw_x_y", full_name: "demo/cbt_gen_tw_x_y", html_url: "", clone_url: "", ssh_url: "", private: true, empty: false, description: "", owner: { id: 1, username: "demo", full_name: "", email: "", avatar_url: "" }, updated_at: "" }]) as T;
    case "import_from_wacs": return delay({ id: "cbt_3jn_text_reg", status: { status: "exists" } }) as T;
    case "list_backups": return delay([{ name: "cbt_3jn_text_reg.tstudio", path: "C:/mock/backups/cbt_3jn_text_reg.tstudio", modified: Math.floor(Date.now() / 1000) - 300 }]) as T;
    case "run_backup_now": return delay({ backed_up: projects.map((p) => p.id), skipped: [], failed: [] }) as T;
    case "inspect_legacy": return delay({ path: s("root"), projects: 3 }) as T;
    case "migrate_legacy": return delay([{ id: "aa_mrk_text_reg", status: { status: "imported" } }]) as T;
    case "send_feedback": return delay("C:/Users/demo/BTT-Writer-3/logs/feedback.txt") as T;
    case "open_data_dir": return delay("C:/Users/demo/BTT-Writer-3") as T;
    default: throw new Error("Comando no simulado: " + cmd);
  }
}
