//! Descarga de textos fuente y ayudas desde el catálogo (formato legado
//! "tS v2": `source.json`, `notes.json`, `questions.json`, `words.json`,
//! `tw_cat.json`) y su conversión a contenedores de recursos `.tsrc`.

use crate::library::{Library, ResourceRow};
use crate::rc::{RcLanguage, RcPackage, RcProject, RcResource, RcStatus, ResourceContainer};
use crate::{Error, Result};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::Duration;

pub const USER_AGENT: &str = "btt-writer-rs/3.0 (+https://github.com/joseeliezerr7/BTT-Writerv2)";

/// Cliente HTTP bloqueante compartido por catálogo, fuentes y WACS.
pub fn http_client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(90))
        .connect_timeout(Duration::from_secs(20))
        .build()?)
}

/// Descarga texto; convierte respuestas no 2xx en `Error::Server`.
pub fn fetch_text(client: &reqwest::blocking::Client, url: &str) -> Result<String> {
    let resp = client.get(url).send()?;
    let status = resp.status();
    let body = resp.text()?;
    if !status.is_success() {
        let snippet: String = body.chars().take(200).collect();
        let body = if snippet.contains("<html") {
            "el servidor devolvió una página web en lugar de datos (posible bloqueo anti-bot)".to_string()
        } else {
            snippet
        };
        return Err(Error::Server { status: status.as_u16(), body });
    }
    Ok(body)
}

// ------------------------------------------------------------------ formatos legados

#[derive(Debug, Deserialize)]
struct LegacySource {
    #[serde(default)]
    chapters: Vec<LegacyChapter>,
    #[serde(default)]
    date_modified: Value,
}

#[derive(Debug, Deserialize)]
struct LegacyChapter {
    number: String,
    #[serde(default)]
    title: String,
    #[serde(default, rename = "ref")]
    reference: String,
    #[serde(default)]
    frames: Vec<LegacyFrame>,
}

#[derive(Debug, Deserialize)]
struct LegacyFrame {
    id: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    img: String,
}

#[derive(Debug, Deserialize)]
struct LegacyNotesChunk {
    id: String,
    #[serde(default)]
    tn: Vec<LegacyNote>,
}
#[derive(Debug, Deserialize)]
struct LegacyNote {
    #[serde(default, rename = "ref")]
    reference: String,
    #[serde(default)]
    text: String,
}

#[derive(Debug, Deserialize)]
struct LegacyQuestionsChapter {
    id: String,
    #[serde(default)]
    cq: Vec<LegacyQuestion>,
}
#[derive(Debug, Deserialize)]
struct LegacyQuestion {
    #[serde(default)]
    q: String,
    #[serde(default)]
    a: String,
    #[serde(default, rename = "ref")]
    refs: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct LegacyWord {
    id: String,
    #[serde(default)]
    term: String,
    #[serde(default)]
    def: String,
    #[serde(default)]
    def_title: String,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    cf: Vec<String>,
    #[serde(default)]
    ex: Vec<LegacyExample>,
    #[serde(default)]
    sub: String,
}
#[derive(Debug, Deserialize)]
struct LegacyExample {
    #[serde(default, rename = "ref")]
    reference: String,
    #[serde(default)]
    text: String,
}

#[derive(Debug, Deserialize)]
struct LegacyTwCat {
    #[serde(default)]
    chapters: Vec<LegacyTwCatChapter>,
}
#[derive(Debug, Deserialize)]
struct LegacyTwCatChapter {
    id: String,
    #[serde(default)]
    frames: Vec<LegacyTwCatFrame>,
}
#[derive(Debug, Deserialize)]
struct LegacyTwCatFrame {
    id: String,
    #[serde(default)]
    items: Vec<LegacyTwCatItem>,
}
#[derive(Debug, Deserialize)]
struct LegacyTwCatItem {
    id: String,
}

fn split_frame_id(id: &str, chapter_hint: Option<&str>) -> (String, String) {
    match id.split_once('-') {
        Some((c, f)) => (pad(c), pad(f)),
        None => (pad(chapter_hint.unwrap_or("01")), pad(id)),
    }
}

fn pad(s: &str) -> String {
    let s = s.trim();
    if s.chars().all(|c| c.is_ascii_digit()) && s.len() < 2 {
        format!("{s:0>2}")
    } else {
        s.to_string()
    }
}

fn base_package(row: &ResourceRow, mime: &str, modified_at: i64) -> RcPackage {
    RcPackage {
        package_version: "0.1".into(),
        modified_at,
        content_mime_type: mime.into(),
        language: RcLanguage {
            slug: row.language_slug.clone(),
            name: row.language_name.clone(),
            direction: row.language_direction.clone(),
        },
        project: RcProject {
            slug: row.project_slug.clone(),
            name: row.project_name.clone(),
            sort: 0,
            category_slug: if row.project_slug == "obs" { "bible-obs".into() } else { String::new() },
            ..Default::default()
        },
        resource: RcResource {
            slug: row.slug.clone(),
            name: row.name.clone(),
            r#type: row.r#type.clone(),
            translation_words_assignments_url: None,
            status: RcStatus {
                translate_mode: row.translate_mode.clone(),
                checking_level: row.checking_level.clone(),
                version: row.version.clone(),
                pub_date: row.pub_date.clone(),
                ..Default::default()
            },
            imported: false,
        },
        chunk_status: vec![],
    }
}

fn toc_yaml(chapters: &BTreeMap<String, Vec<String>>) -> String {
    let mut s = String::new();
    for (ch, chunks) in chapters {
        s.push_str(&format!("-\n  chapter: '{ch}'\n  chunks:\n"));
        for c in chunks {
            s.push_str(&format!("    - '{c}'\n"));
        }
    }
    s
}

fn modified_value(v: &Value) -> i64 {
    match v {
        Value::Number(n) => n.as_i64().unwrap_or(0),
        Value::String(s) => s.trim().parse().unwrap_or(0),
        _ => 0,
    }
}

/// Convierte `source.json` (+ `tw_cat.json` opcional) en un contenedor `book`.
pub fn book_container(row: &ResourceRow, source_json: &str, tw_cat_json: Option<&str>) -> Result<ResourceContainer> {
    let src: LegacySource = serde_json::from_str(source_json)
        .map_err(|e| Error::InvalidContainer(format!("source.json: {e}")))?;
    if src.chapters.is_empty() {
        return Err(Error::InvalidContainer("source.json sin capítulos".into()));
    }
    let is_obs = row.project_slug == "obs" || row.slug == "obs";
    let mime = if is_obs { "text/markdown" } else { "text/usx" };
    let ext = if is_obs { "md" } else { "usx" };
    let mut pkg = base_package(row, mime, modified_value(&src.date_modified));
    pkg.resource.r#type = "book".into();
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut toc: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut config: BTreeMap<String, BTreeMap<String, Vec<String>>> = BTreeMap::new();

    files.insert(format!("content/front/title.{ext}"), row.project_name.clone().into_bytes());
    toc.insert("front".into(), vec!["title".into()]);

    for ch in &src.chapters {
        let ch_id = pad(&ch.number);
        let mut chunks = vec![];
        if !ch.title.trim().is_empty() {
            files.insert(format!("content/{ch_id}/title.{ext}"), ch.title.trim().as_bytes().to_vec());
            chunks.push("title".to_string());
        }
        if !ch.reference.trim().is_empty() {
            files.insert(format!("content/{ch_id}/reference.{ext}"), ch.reference.trim().as_bytes().to_vec());
            chunks.push("reference".to_string());
        }
        for f in &ch.frames {
            let (_, chunk) = split_frame_id(&f.id, Some(&ch_id));
            let mut text = f.text.clone();
            if is_obs && !f.img.is_empty() && !text.contains(&f.img) {
                text = format!("![OBS Image]({})\n\n{}", f.img, text);
            }
            if f.format == "usfm" && !is_obs {
                // Algunas fuentes antiguas vienen en USFM: se convierte a USX mínimo.
                text = usfm_chunk_to_usx(&text);
            }
            files.insert(format!("content/{ch_id}/{chunk}.{ext}"), text.into_bytes());
            chunks.push(chunk);
        }
        chunks.sort_by(|a, b| crate::project::chunk_cmp(a, b));
        toc.insert(ch_id, chunks);
    }

    if let Some(cat) = tw_cat_json {
        if let Ok(cat) = serde_json::from_str::<LegacyTwCat>(cat) {
            for ch in cat.chapters {
                let ch_id = pad(&ch.id);
                for fr in ch.frames {
                    let words: Vec<String> = fr.items.iter().map(|i| format!("//bible/tw/{}", i.id)).collect();
                    if !words.is_empty() {
                        config.entry(ch_id.clone()).or_default().insert(pad(&fr.id), words);
                    }
                }
            }
        }
    }
    let mut cfg = String::from("content:\n");
    for (ch, chunks) in &config {
        cfg.push_str(&format!("  '{ch}':\n"));
        for (chunk, words) in chunks {
            cfg.push_str(&format!("    '{chunk}':\n      words:\n"));
            for w in words {
                cfg.push_str(&format!("        - {w}\n"));
            }
        }
    }
    if config.is_empty() {
        cfg = "{}".into();
    }
    files.insert("content/config.yml".into(), cfg.into_bytes());
    files.insert("content/toc.yml".into(), toc_yaml(&toc).into_bytes());
    files.insert("LICENSE.md".into(), crate::project::LICENSE_MD.as_bytes().to_vec());
    files.insert("package.json".into(), serde_json::to_vec_pretty(&pkg)?);
    ResourceContainer::from_files(files)
}

/// Conversión mínima USFM → USX para chunks fuente antiguos.
fn usfm_chunk_to_usx(usfm: &str) -> String {
    let mut out = String::new();
    for v in crate::usfm::verses_of_chunk(usfm) {
        out.push_str(&format!("<verse number=\"{}\" style=\"v\" />{}\n", v.number, crate::usfm::to_plain_text(&v.text)));
    }
    if out.is_empty() {
        out = crate::usfm::to_plain_text(usfm);
    }
    out
}

/// Convierte `notes.json` en un contenedor `help` (tN).
pub fn notes_container(row: &ResourceRow, notes_json: &str) -> Result<ResourceContainer> {
    let notes: Vec<LegacyNotesChunk> = serde_json::from_str(notes_json)
        .map_err(|e| Error::InvalidContainer(format!("notes.json: {e}")))?;
    let mut pkg = base_package(row, "text/markdown", 0);
    pkg.resource.r#type = "help".into();
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut toc: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for n in notes {
        if n.tn.is_empty() {
            continue;
        }
        let (ch, chunk) = split_frame_id(&n.id, None);
        let mut md = String::new();
        for t in &n.tn {
            md.push_str(&format!("#{}\n\n{}\n\n", t.reference.trim(), t.text.trim()));
        }
        files.insert(format!("content/{ch}/{chunk}.md"), md.trim_end().as_bytes().to_vec());
        toc.entry(ch).or_default().push(chunk);
    }
    if files.is_empty() {
        return Err(Error::InvalidContainer("notes.json sin notas".into()));
    }
    for v in toc.values_mut() {
        v.sort_by(|a, b| crate::project::chunk_cmp(a, b));
    }
    files.insert("content/config.yml".into(), b"{}".to_vec());
    files.insert("content/toc.yml".into(), toc_yaml(&toc).into_bytes());
    files.insert("LICENSE.md".into(), crate::project::LICENSE_MD.as_bytes().to_vec());
    files.insert("package.json".into(), serde_json::to_vec_pretty(&pkg)?);
    ResourceContainer::from_files(files)
}

/// Convierte `questions.json` en un contenedor `help` (tQ).
pub fn questions_container(row: &ResourceRow, questions_json: &str) -> Result<ResourceContainer> {
    let chapters: Vec<LegacyQuestionsChapter> = serde_json::from_str(questions_json)
        .map_err(|e| Error::InvalidContainer(format!("questions.json: {e}")))?;
    let mut pkg = base_package(row, "text/markdown", 0);
    pkg.resource.r#type = "help".into();
    let mut by_chunk: BTreeMap<(String, String), String> = BTreeMap::new();
    for ch in chapters {
        let ch_id = pad(&ch.id);
        for q in ch.cq {
            for r in &q.refs {
                let (c, chunk) = split_frame_id(r, Some(&ch_id));
                let entry = by_chunk.entry((c, chunk)).or_default();
                entry.push_str(&format!("#{}\n\n{}\n\n", q.q.trim(), q.a.trim()));
            }
        }
    }
    if by_chunk.is_empty() {
        return Err(Error::InvalidContainer("questions.json sin preguntas".into()));
    }
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut toc: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for ((ch, chunk), md) in by_chunk {
        files.insert(format!("content/{ch}/{chunk}.md"), md.trim_end().as_bytes().to_vec());
        toc.entry(ch).or_default().push(chunk);
    }
    for v in toc.values_mut() {
        v.sort_by(|a, b| crate::project::chunk_cmp(a, b));
    }
    files.insert("content/config.yml".into(), b"{}".to_vec());
    files.insert("content/toc.yml".into(), toc_yaml(&toc).into_bytes());
    files.insert("LICENSE.md".into(), crate::project::LICENSE_MD.as_bytes().to_vec());
    files.insert("package.json".into(), serde_json::to_vec_pretty(&pkg)?);
    ResourceContainer::from_files(files)
}

/// Convierte `words.json` en un contenedor `dict` (tW).
pub fn words_container(row: &ResourceRow, words_json: &str) -> Result<ResourceContainer> {
    let words: Vec<LegacyWord> = serde_json::from_str(words_json)
        .map_err(|e| Error::InvalidContainer(format!("words.json: {e}")))?;
    if words.is_empty() {
        return Err(Error::InvalidContainer("words.json vacío".into()));
    }
    let mut pkg = base_package(row, "text/markdown", 0);
    pkg.resource.r#type = "dict".into();
    pkg.project.slug = "bible".into();
    pkg.project.name = "translationWords".into();
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut cfg = String::new();
    let mut toc: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for w in words {
        let slug = w.id.trim().to_lowercase();
        if slug.is_empty() {
            continue;
        }
        let mut md = format!("# {}\n\n", if w.term.trim().is_empty() { slug.clone() } else { w.term.trim().to_string() });
        if !w.sub.trim().is_empty() {
            md.push_str(&format!("{}\n\n", w.sub.trim()));
        }
        md.push_str(&format!("## {}\n\n{}\n", if w.def_title.trim().is_empty() { "Definition" } else { w.def_title.trim() }, w.def.trim()));
        if !w.ex.is_empty() {
            md.push_str("\n## Examples\n\n");
            for e in &w.ex {
                md.push_str(&format!("* **{}**: {}\n", e.reference, e.text.trim()));
            }
        }
        if !w.cf.is_empty() {
            md.push_str("\n## See also\n\n");
            for c in &w.cf {
                md.push_str(&format!("* [{c}](../{c}/01.md)\n"));
            }
        }
        files.insert(format!("content/{slug}/01.md"), md.into_bytes());
        toc.insert(slug.clone(), vec!["01".into()]);
        cfg.push_str(&format!("{slug}:\n  def_title: {}\n", yaml_str(if w.def_title.is_empty() { "Definition" } else { &w.def_title })));
        if !w.cf.is_empty() {
            cfg.push_str("  see_also:\n");
            for c in &w.cf {
                cfg.push_str(&format!("    - {}\n", yaml_str(c)));
            }
        }
        if !w.aliases.is_empty() {
            cfg.push_str("  aliases:\n");
            for a in &w.aliases {
                cfg.push_str(&format!("    - {}\n", yaml_str(a)));
            }
        }
    }
    files.insert("content/config.yml".into(), cfg.into_bytes());
    files.insert("content/toc.yml".into(), toc_yaml(&toc).into_bytes());
    files.insert("LICENSE.md".into(), crate::project::LICENSE_MD.as_bytes().to_vec());
    files.insert("package.json".into(), serde_json::to_vec_pretty(&pkg)?);
    ResourceContainer::from_files(files)
}

fn yaml_str(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

// ------------------------------------------------------------------ descarga

/// Descarga un recurso del catálogo y lo guarda como contenedor.
/// Devuelve los slugs de contenedores creados (el libro y, si existen, tN/tQ/tW).
pub fn download_resource(lib: &Library, row: &ResourceRow, client: &reqwest::blocking::Client, with_helps: bool) -> Result<Vec<String>> {
    let mut done = vec![];
    if row.url.is_empty() {
        return Err(Error::Other(format!("el recurso {} no tiene URL de descarga", row.container_slug())));
    }
    match row.r#type.as_str() {
        "book" => {
            let src = fetch_text(client, &row.url)?;
            let tw_cat = lib
                .tw_assignments_url(&row.language_slug, &row.project_slug, &row.slug)?
                .and_then(|u| fetch_text(client, &u).ok());
            let rc = book_container(row, &src, tw_cat.as_deref())?;
            lib.import_container(&rc)?;
            done.push(rc.slug());
            if with_helps {
                for r in lib.resources(&row.project_slug)? {
                    if r.language_slug == row.language_slug && (r.slug == "tn" || r.slug == "tq") && !r.url.is_empty() {
                        match download_resource(lib, &r, client, false) {
                            Ok(mut d) => done.append(&mut d),
                            Err(e) => log::warn!("ayuda {} no descargada: {e}", r.container_slug()),
                        }
                    }
                }
                for r in lib.resources("bible")? {
                    if r.language_slug == row.language_slug && r.slug == "tw" && !r.url.is_empty() && !r.downloaded {
                        match download_resource(lib, &r, client, false) {
                            Ok(mut d) => done.append(&mut d),
                            Err(e) => log::warn!("tW {} no descargado: {e}", r.container_slug()),
                        }
                    }
                }
            }
        }
        "help" => {
            let json = fetch_text(client, &row.url)?;
            let rc = if row.slug == "tq" { questions_container(row, &json)? } else { notes_container(row, &json)? };
            lib.import_container(&rc)?;
            done.push(rc.slug());
        }
        "dict" => {
            let json = fetch_text(client, &row.url)?;
            let rc = words_container(row, &json)?;
            lib.import_container(&rc)?;
            done.push(rc.slug());
        }
        other => return Err(Error::Other(format!("tipo de recurso no soportado: {other}"))),
    }
    Ok(done)
}

/// Actualiza la lista de fuentes disponibles desde el catálogo raíz.
pub fn update_catalog(lib: &Library, client: &reqwest::blocking::Client, root_catalog_url: &str) -> Result<Vec<String>> {
    let catalog = fetch_text(client, root_catalog_url)?;
    lib.update_sources_from_catalog(&catalog, |url| fetch_text(client, url))
}

/// Actualiza idiomas destino (langnames) e idiomas temporales.
pub fn update_languages(lib: &Library, client: &reqwest::blocking::Client, langnames_url: &str, temp_url: Option<&str>, approved_url: Option<&str>) -> Result<usize> {
    let names = fetch_text(client, langnames_url)?;
    let n = lib.update_target_languages(&names)?;
    if let Some(t) = temp_url {
        if let Ok(temp) = fetch_text(client, t) {
            let approved = approved_url.and_then(|a| fetch_text(client, a).ok());
            let _ = lib.update_temp_languages(&temp, approved.as_deref());
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(slug: &str, type_: &str) -> ResourceRow {
        ResourceRow {
            language_slug: "es-419".into(),
            language_name: "Español".into(),
            language_direction: "ltr".into(),
            project_slug: "3jn".into(),
            project_name: "3 Juan".into(),
            slug: slug.into(),
            name: slug.to_uppercase(),
            r#type: type_.into(),
            translate_mode: "all".into(),
            checking_level: "3".into(),
            version: "1".into(),
            pub_date: String::new(),
            mime_type: String::new(),
            url: String::new(),
            modified_at: 0,
            downloaded: false,
        }
    }

    #[test]
    fn converts_legacy_source_with_tw_cat() {
        let src = r#"{"chapters":[{"number":"1","title":"Capítulo 1","ref":"","frames":[
            {"id":"01-01","format":"usx","text":"<verse number=\"1\" style=\"v\" />El anciano<verse number=\"2\" style=\"v\" />Amado"},
            {"id":"01-05","format":"usx","text":"<verse number=\"5\" style=\"v\" />Fiel"}]}],"date_modified":"20170801"}"#;
        let cat = r#"{"chapters":[{"id":"01","frames":[{"id":"01","items":[{"id":"elder"},{"id":"love"}]}]}]}"#;
        let rc = book_container(&row("ulb", "book"), src, Some(cat)).unwrap();
        assert_eq!(rc.slug(), "es-419_3jn_ulb");
        assert_eq!(rc.chapters(), vec!["01"]);
        assert_eq!(rc.chunks("01"), vec!["01", "05"]);
        assert_eq!(rc.chapter_title("01"), "Capítulo 1");
        assert_eq!(rc.book_title(), "3 Juan");
        assert_eq!(rc.source_chunk("01", "01").unwrap().verses.len(), 2);
        assert_eq!(rc.words_for_chunk("01", "01"), vec!["elder", "love"]);
        assert_eq!(rc.chunk_markers()["01"], vec![1, 5]);
        assert_eq!(rc.package.modified_at, 20170801);
    }

    #[test]
    fn converts_notes_questions_words() {
        let notes = r#"[{"id":"01-01","tn":[{"ref":"El anciano","text":"Se refiere a Juan."},{"ref":"Gayo","text":"Un creyente."}]},{"id":"01-05","tn":[]}]"#;
        let rc = notes_container(&row("tn", "help"), notes).unwrap();
        let e = rc.help_entries("01", "01");
        assert_eq!(e.len(), 2);
        assert_eq!(e[1].title, "Gayo");
        assert!(rc.help_entries("01", "05").is_empty());

        let q = r#"[{"id":"01","cq":[{"q":"¿Quién escribe?","a":"El anciano.","ref":["01-01"]},{"q":"¿A quién?","a":"A Gayo.","ref":["01-01","01-05"]}]}]"#;
        let rc = questions_container(&row("tq", "help"), q).unwrap();
        assert_eq!(rc.help_entries("01", "01").len(), 2);
        assert_eq!(rc.help_entries("01", "05")[0].title, "¿A quién?");

        let w = r#"[{"id":"love","term":"amor","def":"Querer el bien.","def_title":"Definición","aliases":["amar"],"cf":["god"],"ex":[{"ref":"01-01","text":"a quien amo"}],"sub":""},{"id":"god","term":"Dios","def":"El creador."}]"#;
        let rc = words_container(&row("tw", "dict"), w).unwrap();
        assert_eq!(rc.slug(), "es-419_bible_tw");
        assert_eq!(rc.word_slugs(), vec!["god", "love"]);
        let love = rc.word("love").unwrap();
        assert_eq!(love.title, "amor");
        assert!(love.body.contains("## Definición"));
        assert_eq!(love.see_also, vec!["god"]);
        assert_eq!(love.aliases, vec!["amar"]);
        assert_eq!(rc.search_words("amar")[0].slug, "love");
    }

    #[test]
    fn rejects_empty_or_invalid_json() {
        assert!(book_container(&row("ulb", "book"), r#"{"chapters":[]}"#, None).is_err());
        assert!(notes_container(&row("tn", "help"), "no json").is_err());
        assert!(words_container(&row("tw", "dict"), "[]").is_err());
    }
}
