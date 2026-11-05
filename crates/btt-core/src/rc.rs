//! Contenedores de recursos (`*.tsrc`): archivos `tar.bz2` con
//! `package.json`, `LICENSE.md` y `content/`.
//!
//! * Libros (`type = book`, `text/usx`): `content/{cap}/{chunk}.usx`,
//!   `content/{cap}/title.usx`, `content/front/title.usx`, `content/toc.yml`
//!   y `content/config.yml` (palabras tW asociadas a cada chunk).
//! * Ayudas (`type = help`, `text/markdown`): tN y tQ en `content/{cap}/{chunk}.md`.
//! * Diccionario (`type = dict`): tW en `content/{palabra}/01.md` + `config.yml`.
//!
//! El contenedor se carga completo en memoria (los más grandes rondan pocos MB).

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

/// Acepta `null`, números o cadenas (los package.json reales traen `null`).
fn lenient<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<String, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(match v {
        None | Some(serde_json::Value::Null) => String::new(),
        Some(serde_json::Value::String(s)) => s,
        Some(serde_json::Value::Number(n)) => n.to_string(),
        Some(other) => other.to_string(),
    })
}

fn lenient_i64<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<i64, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(match v {
        Some(serde_json::Value::Number(n)) => n.as_i64().unwrap_or(0),
        Some(serde_json::Value::String(s)) => s.trim().parse().unwrap_or(0),
        _ => 0,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RcLanguage {
    pub slug: String,
    #[serde(default, deserialize_with = "lenient")]
    pub name: String,
    #[serde(default = "ltr", deserialize_with = "lenient")]
    pub direction: String,
}
fn ltr() -> String {
    "ltr".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RcProject {
    pub slug: String,
    #[serde(default, deserialize_with = "lenient")]
    pub name: String,
    #[serde(default, deserialize_with = "lenient")]
    pub desc: String,
    #[serde(default, deserialize_with = "lenient")]
    pub icon: String,
    #[serde(default, deserialize_with = "lenient_i64")]
    pub sort: i64,
    #[serde(default, deserialize_with = "lenient")]
    pub chunks_url: String,
    #[serde(default, deserialize_with = "lenient")]
    pub category_slug: String,
    #[serde(default)]
    pub categories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RcStatus {
    #[serde(default, deserialize_with = "lenient")]
    pub translate_mode: String,
    #[serde(default, deserialize_with = "lenient")]
    pub checking_level: String,
    #[serde(default, deserialize_with = "lenient")]
    pub comments: String,
    #[serde(default, deserialize_with = "lenient")]
    pub pub_date: String,
    #[serde(default, deserialize_with = "lenient")]
    pub license: String,
    #[serde(default, deserialize_with = "lenient")]
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RcResource {
    pub slug: String,
    #[serde(default, deserialize_with = "lenient")]
    pub name: String,
    /// `book`, `help`, `dict`, `man`
    #[serde(default, deserialize_with = "lenient")]
    pub r#type: String,
    #[serde(default)]
    pub translation_words_assignments_url: Option<String>,
    #[serde(default)]
    pub status: RcStatus,
    #[serde(default)]
    pub imported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RcPackage {
    #[serde(default, deserialize_with = "lenient")]
    pub package_version: String,
    #[serde(default, deserialize_with = "lenient_i64")]
    pub modified_at: i64,
    #[serde(default, deserialize_with = "lenient")]
    pub content_mime_type: String,
    pub language: RcLanguage,
    pub project: RcProject,
    pub resource: RcResource,
    #[serde(default)]
    pub chunk_status: Vec<String>,
}

impl RcPackage {
    /// `{lang}_{project}_{resource}` — nombre de archivo del contenedor.
    pub fn slug(&self) -> String {
        container_slug(&self.language.slug, &self.project.slug, &self.resource.slug)
    }
}

pub fn container_slug(lang: &str, project: &str, resource: &str) -> String {
    format!("{lang}_{project}_{resource}")
}

/// Una nota de traducción (tN) o pregunta (tQ): título + cuerpo en Markdown.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HelpEntry {
    pub title: String,
    pub body: String,
}

/// Artículo de translationWords.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct WordArticle {
    pub slug: String,
    pub title: String,
    /// Cuerpo Markdown (definición, ejemplos, "ver también").
    pub body: String,
    pub aliases: Vec<String>,
    pub see_also: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ResourceContainer {
    pub package: RcPackage,
    files: BTreeMap<String, Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TocChapter {
    pub chapter: String,
    #[serde(default)]
    pub chunks: Vec<String>,
}

impl ResourceContainer {
    /// Abre un `.tsrc` (tar.bz2). Rechaza rutas peligrosas dentro del tar.
    pub fn open(path: &Path) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        Self::from_reader(file)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        Self::from_reader(std::io::Cursor::new(bytes))
    }

    pub fn from_reader<R: Read>(reader: R) -> Result<Self> {
        let decoder = bzip2::read::MultiBzDecoder::new(reader);
        let mut archive = tar::Archive::new(decoder);
        let mut files = BTreeMap::new();
        for entry in archive.entries()? {
            let mut entry = entry?;
            if !entry.header().entry_type().is_file() {
                continue;
            }
            let raw = entry.path()?.to_string_lossy().replace('\\', "/");
            let clean = normalize_path(&raw)?;
            // El tamaño de la cabecera tar no es de fiar: un valor enorme abortaría la app al reservar.
            let mut data = Vec::with_capacity(entry.size().min(8 * 1024 * 1024) as usize);
            entry.read_to_end(&mut data)?;
            files.insert(clean, data);
        }
        Self::from_files(files)
    }

    /// Construye un contenedor a partir de archivos ya en memoria
    /// (rutas relativas: `package.json`, `content/01/01.usx`...).
    pub fn from_files(files: BTreeMap<String, Vec<u8>>) -> Result<Self> {
        let pkg = files
            .get("package.json")
            .ok_or_else(|| Error::InvalidContainer("falta package.json".into()))?;
        let package: RcPackage = serde_json::from_slice(pkg)
            .map_err(|e| Error::InvalidContainer(format!("package.json: {e}")))?;
        if package.language.slug.is_empty() || package.project.slug.is_empty() {
            return Err(Error::InvalidContainer("package.json incompleto".into()));
        }
        Ok(ResourceContainer { package, files })
    }

    /// Guarda el contenedor como `.tsrc` (tar.bz2).
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("tsrc.tmp");
        {
            let file = std::fs::File::create(&tmp)?;
            let enc = bzip2::write::BzEncoder::new(file, bzip2::Compression::default());
            let mut builder = tar::Builder::new(enc);
            for (name, data) in &self.files {
                let mut header = tar::Header::new_gnu();
                header.set_size(data.len() as u64);
                header.set_mode(0o644);
                header.set_mtime(self.package.modified_at.max(0) as u64);
                header.set_cksum();
                builder.append_data(&mut header, name, data.as_slice())?;
            }
            let enc = builder.into_inner()?;
            enc.finish()?;
        }
        std::fs::rename(&tmp, path).or_else(|_| {
            std::fs::copy(&tmp, path).map(|_| ()).and_then(|_| std::fs::remove_file(&tmp))
        })?;
        Ok(())
    }

    pub fn slug(&self) -> String {
        self.package.slug()
    }

    pub fn file(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path).map(|v| v.as_slice())
    }

    pub fn file_str(&self, path: &str) -> Option<String> {
        self.file(path).map(|b| String::from_utf8_lossy(b).into_owned())
    }

    pub fn file_names(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(|s| s.as_str())
    }

    fn content_ext(&self) -> &'static str {
        if self.package.content_mime_type.contains("usx") {
            "usx"
        } else if self.package.content_mime_type.contains("usfm") {
            "usfm"
        } else {
            "md"
        }
    }

    /// Tabla de contenidos (`content/toc.yml`); si falta, se deriva de los archivos.
    pub fn toc(&self) -> Vec<TocChapter> {
        if let Some(y) = self.file_str("content/toc.yml") {
            if let Ok(toc) = serde_yaml::from_str::<Vec<TocChapter>>(&y) {
                if !toc.is_empty() {
                    return toc;
                }
            }
        }
        let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for name in self.files.keys() {
            let Some(rest) = name.strip_prefix("content/") else { continue };
            let mut parts = rest.split('/');
            let (Some(ch), Some(file), None) = (parts.next(), parts.next(), parts.next()) else {
                continue;
            };
            let Some(stem) = file.rsplit_once('.').map(|(s, _)| s) else { continue };
            map.entry(ch.to_string()).or_default().push(stem.to_string());
        }
        let mut chapters: Vec<TocChapter> = map
            .into_iter()
            .map(|(chapter, mut chunks)| {
                chunks.sort_by(|a, b| crate::project::chunk_cmp(a, b));
                TocChapter { chapter, chunks }
            })
            .collect();
        chapters.sort_by(|a, b| crate::project::natural_cmp(&a.chapter, &b.chapter));
        chapters
    }

    /// Capítulos (sin `front`).
    pub fn chapters(&self) -> Vec<String> {
        self.toc().into_iter().map(|c| c.chapter).filter(|c| c != "front").collect()
    }

    /// Chunks de un capítulo, sin `title`/`reference`.
    pub fn chunks(&self, chapter: &str) -> Vec<String> {
        self.toc()
            .into_iter()
            .find(|c| c.chapter == chapter)
            .map(|c| {
                c.chunks.into_iter().filter(|k| k != "title" && k != "reference").collect()
            })
            .unwrap_or_default()
    }

    /// Marcadores de chunk (verso inicial) por capítulo, derivados del contenido.
    pub fn chunk_markers(&self) -> BTreeMap<String, Vec<u32>> {
        let mut out = BTreeMap::new();
        for ch in self.chapters() {
            let v: Vec<u32> = self
                .chunks(&ch)
                .iter()
                .map(|c| crate::usfm::first_number(c))
                .filter(|&n| n > 0)
                .collect();
            if !v.is_empty() {
                out.insert(ch, v);
            }
        }
        out
    }

    /// Texto crudo de un chunk (`usx` o `md`).
    pub fn read_chunk(&self, chapter: &str, chunk: &str) -> Option<String> {
        let ext = self.content_ext();
        self.file_str(&format!("content/{chapter}/{chunk}.{ext}"))
            .or_else(|| self.file_str(&format!("content/{chapter}/{chunk}.usfm")))
            .or_else(|| self.file_str(&format!("content/{chapter}/{chunk}.md")))
    }

    pub fn book_title(&self) -> String {
        self.read_chunk("front", "title")
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| self.package.project.name.clone())
    }

    pub fn chapter_title(&self, chapter: &str) -> String {
        self.read_chunk(chapter, "title").map(|t| t.trim().to_string()).unwrap_or_default()
    }

    /// Chunk fuente analizado (solo contenedores `book` USX).
    pub fn source_chunk(&self, chapter: &str, chunk: &str) -> Option<crate::usx::SourceChunk> {
        self.read_chunk(chapter, chunk).map(|t| crate::usx::parse_chunk(&t))
    }

    /// Notas/preguntas de un chunk (contenedores `help`).
    pub fn help_entries(&self, chapter: &str, chunk: &str) -> Vec<HelpEntry> {
        self.read_chunk(chapter, chunk).map(|md| parse_help_markdown(&md)).unwrap_or_default()
    }

    /// Palabras tW asignadas a un chunk (desde `content/config.yml` de un libro).
    /// Devuelve slugs (`obey`, `god`...).
    pub fn words_for_chunk(&self, chapter: &str, chunk: &str) -> Vec<String> {
        let Some(y) = self.file_str("content/config.yml") else { return vec![] };
        let Ok(v) = serde_yaml::from_str::<serde_yaml::Value>(&y) else { return vec![] };
        let words = v
            .get("content")
            .and_then(|c| c.get(chapter))
            .and_then(|c| c.get(chunk))
            .and_then(|c| c.get("words"))
            .and_then(|w| w.as_sequence());
        let mut out: Vec<String> = words
            .map(|seq| {
                seq.iter()
                    .filter_map(|w| w.as_str())
                    .map(|w| w.rsplit('/').next().unwrap_or(w).to_string())
                    .collect()
            })
            .unwrap_or_default();
        out.sort();
        out.dedup();
        out
    }

    /// Lista de palabras de un diccionario tW.
    pub fn word_slugs(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .files
            .keys()
            .filter_map(|k| k.strip_prefix("content/"))
            .filter(|k| k.ends_with("/01.md"))
            .map(|k| k.trim_end_matches("/01.md").to_string())
            .filter(|k| !k.contains('/'))
            .collect();
        out.sort();
        out
    }

    /// Artículo tW completo.
    pub fn word(&self, slug: &str) -> Option<WordArticle> {
        let md = self.file_str(&format!("content/{slug}/01.md"))?;
        let (title, body) = split_title(&md);
        let mut art = WordArticle { slug: slug.to_string(), title, body, ..Default::default() };
        if let Some(y) = self.file_str("content/config.yml") {
            if let Ok(v) = serde_yaml::from_str::<serde_yaml::Value>(&y) {
                if let Some(entry) = v.get(slug) {
                    art.see_also = yaml_str_list(entry.get("see_also"));
                    art.aliases = yaml_str_list(entry.get("aliases"));
                }
            }
        }
        Ok::<_, ()>(art).ok()
    }

    /// Título y cuerpo de un artículo tW sin leer `config.yml` (rápido para listados largos).
    pub fn word_text(&self, slug: &str) -> Option<(String, String)> {
        self.file_str(&format!("content/{slug}/01.md")).map(|md| split_title(&md))
    }

    /// Palabras tW de todos los chunks de un libro, con una sola lectura de `content/config.yml`.
    /// Clave: (capítulo, chunk) tal como aparecen en el contenedor (`"01"`, `"05"`).
    pub fn words_by_chunk(&self) -> BTreeMap<(String, String), Vec<String>> {
        let mut out = BTreeMap::new();
        let Some(y) = self.file_str("content/config.yml") else { return out };
        let Ok(v) = serde_yaml::from_str::<serde_yaml::Value>(&y) else { return out };
        let Some(content) = v.get("content").and_then(|c| c.as_mapping()) else { return out };
        for (ch, chunks) in content {
            let (Some(ch), Some(chunks)) = (yaml_key(ch), chunks.as_mapping()) else { continue };
            for (c, entry) in chunks {
                let Some(c) = yaml_key(c) else { continue };
                let mut words: Vec<String> = entry
                    .get("words")
                    .and_then(|w| w.as_sequence())
                    .map(|seq| seq.iter().filter_map(|w| w.as_str()).map(|w| w.rsplit('/').next().unwrap_or(w).to_string()).collect())
                    .unwrap_or_default();
                words.sort();
                words.dedup();
                if !words.is_empty() {
                    out.insert((ch.clone(), c), words);
                }
            }
        }
        out
    }

    /// Busca palabras por título o alias (insensible a mayúsculas).
    pub fn search_words(&self, query: &str) -> Vec<WordArticle> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return vec![];
        }
        self.word_slugs()
            .iter()
            .filter_map(|s| self.word(s))
            .filter(|w| {
                w.title.to_lowercase().contains(&q)
                    || w.slug.contains(&q)
                    || w.aliases.iter().any(|a| a.to_lowercase().contains(&q))
            })
            .collect()
    }
}

/// Clave YAML como texto; los números (`01` leído como 1) se devuelven con dos dígitos.
fn yaml_key(k: &serde_yaml::Value) -> Option<String> {
    match k {
        serde_yaml::Value::String(s) => Some(s.clone()),
        serde_yaml::Value::Number(n) => n.as_u64().map(|n| format!("{n:02}")),
        _ => None,
    }
}

fn yaml_str_list(v: Option<&serde_yaml::Value>) -> Vec<String> {
    v.and_then(|v| v.as_sequence())
        .map(|s| s.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

/// Separa el primer encabezado Markdown del resto.
pub fn split_title(md: &str) -> (String, String) {
    let md = md.replace("\r\n", "\n");
    let mut lines = md.lines();
    let mut title = String::new();
    let mut rest = vec![];
    for l in lines.by_ref() {
        let t = l.trim();
        if t.is_empty() {
            continue;
        }
        if let Some(h) = t.strip_prefix('#') {
            title = h.trim_start_matches('#').trim().to_string();
        } else {
            rest.push(l);
        }
        break;
    }
    rest.extend(lines);
    (title, rest.join("\n").trim().to_string())
}

/// Convierte el Markdown de tN/tQ (`#título\n\ncuerpo` repetido) en entradas.
pub fn parse_help_markdown(md: &str) -> Vec<HelpEntry> {
    let md = md.replace("\r\n", "\n");
    let mut out: Vec<HelpEntry> = vec![];
    let mut current: Option<HelpEntry> = None;
    for line in md.lines() {
        let t = line.trim_end();
        if let Some(h) = t.trim_start().strip_prefix('#') {
            if let Some(e) = current.take() {
                out.push(e);
            }
            current = Some(HelpEntry {
                title: h.trim_start_matches('#').trim().to_string(),
                body: String::new(),
            });
        } else if let Some(e) = current.as_mut() {
            if !(e.body.is_empty() && t.trim().is_empty()) {
                e.body.push_str(t);
                e.body.push('\n');
            }
        } else if !t.trim().is_empty() {
            current = Some(HelpEntry { title: String::new(), body: format!("{t}\n") });
        }
    }
    if let Some(e) = current.take() {
        out.push(e);
    }
    for e in &mut out {
        e.body = e.body.trim().to_string();
    }
    out
}

/// Normaliza una ruta de archivo dentro de un contenedor y rechaza `..`/absolutas.
pub fn normalize_path(raw: &str) -> Result<String> {
    let mut parts = vec![];
    for p in raw.split('/') {
        match p {
            "" | "." => continue,
            ".." => return Err(Error::UnsafePath(raw.to_string())),
            _ if p.contains(':') => return Err(Error::UnsafePath(raw.to_string())),
            _ => parts.push(p),
        }
    }
    if parts.is_empty() {
        return Err(Error::UnsafePath(raw.to_string()));
    }
    Ok(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book_files() -> BTreeMap<String, Vec<u8>> {
        let mut f = BTreeMap::new();
        f.insert("package.json".into(), br#"{"package_version":"0.1","modified_at":20231121,"content_mime_type":"text/usx","language":{"slug":"am","name":"Amharic","direction":"ltr"},"project":{"slug":"1jn","name":"1 John","sort":63,"chunks_url":"","category_slug":"bible-nt","categories":["bible-nt"]},"resource":{"slug":"ulb","name":"ULB","type":"book","status":{"translate_mode":"all","checking_level":"3","version":"7.2"}},"chunk_status":[]}"#.to_vec());
        f.insert("content/front/title.usx".into(), b"1 John".to_vec());
        f.insert("content/01/title.usx".into(), b"Chapter 1".to_vec());
        f.insert("content/01/01.usx".into(), b"<verse number=\"1\" style=\"v\" />Uno<verse number=\"2\" style=\"v\" />Dos".to_vec());
        f.insert("content/01/03.usx".into(), b"<verse number=\"3\" style=\"v\" />Tres".to_vec());
        f.insert("content/02/01.usx".into(), b"<verse number=\"1\" style=\"v\" />Cap2".to_vec());
        f.insert("content/toc.yml".into(), b"-\n  chapter: front\n  chunks:\n    - title\n-\n  chapter: '01'\n  chunks:\n    - title\n    - '01'\n    - '03'\n-\n  chapter: '02'\n  chunks:\n    - '01'\n".to_vec());
        f.insert("content/config.yml".into(), b"content:\n  '01':\n    '01':\n      words:\n        - //bible/tw/obey\n        - //bible/tw/god\n        - //bible/tw/obey\n".to_vec());
        f
    }

    #[test]
    fn book_container_navigation() {
        let rc = ResourceContainer::from_files(book_files()).unwrap();
        assert_eq!(rc.slug(), "am_1jn_ulb");
        assert_eq!(rc.chapters(), vec!["01", "02"]);
        assert_eq!(rc.chunks("01"), vec!["01", "03"]);
        assert_eq!(rc.book_title(), "1 John");
        assert_eq!(rc.chapter_title("01"), "Chapter 1");
        let sc = rc.source_chunk("01", "01").unwrap();
        assert_eq!(sc.verses.len(), 2);
        assert_eq!(rc.words_for_chunk("01", "01"), vec!["god", "obey"]);
        assert!(rc.words_for_chunk("02", "01").is_empty());
        let markers = rc.chunk_markers();
        assert_eq!(markers["01"], vec![1, 3]);
    }

    #[test]
    fn save_and_reopen_tar_bz2() {
        let rc = ResourceContainer::from_files(book_files()).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("am_1jn_ulb.tsrc");
        rc.save(&path).unwrap();
        let again = ResourceContainer::open(&path).unwrap();
        assert_eq!(again.chapters(), vec!["01", "02"]);
        assert_eq!(again.read_chunk("02", "01").unwrap(), "<verse number=\"1\" style=\"v\" />Cap2");
    }

    #[test]
    fn toc_fallback_without_yaml() {
        let mut f = book_files();
        f.remove("content/toc.yml");
        let rc = ResourceContainer::from_files(f).unwrap();
        assert_eq!(rc.chapters(), vec!["01", "02"]);
        assert_eq!(rc.chunks("01"), vec!["01", "03"]);
    }

    #[test]
    fn help_markdown_parsing() {
        let md = "#Título uno\n\nCuerpo uno\nlínea 2\n\n#Título dos\n\nCuerpo dos\n";
        let e = parse_help_markdown(md);
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].title, "Título uno");
        assert_eq!(e[0].body, "Cuerpo uno\nlínea 2");
        assert_eq!(e[1].body, "Cuerpo dos");
        assert_eq!(parse_help_markdown("solo texto")[0].body, "solo texto");
        assert!(parse_help_markdown("").is_empty());
    }

    #[test]
    fn dictionary_words() {
        let mut f = BTreeMap::new();
        f.insert("package.json".into(), br#"{"content_mime_type":"text/markdown","language":{"slug":"en","name":"English"},"project":{"slug":"bible","name":"Bible"},"resource":{"slug":"tw","name":"translationWords","type":"dict"}}"#.to_vec());
        f.insert("content/god/01.md".into(), b"# God\n\n## Definition\n\nThe creator.\n".to_vec());
        f.insert("content/obey/01.md".into(), b"# obey, obedient\n\nTo do what is commanded.".to_vec());
        f.insert("content/config.yml".into(), b"god:\n  see_also:\n    - holy\n    - spirit\n  aliases:\n    - deity\n".to_vec());
        let rc = ResourceContainer::from_files(f).unwrap();
        assert_eq!(rc.word_slugs(), vec!["god", "obey"]);
        let g = rc.word("god").unwrap();
        assert_eq!(g.title, "God");
        assert!(g.body.starts_with("## Definition"));
        assert_eq!(g.see_also, vec!["holy", "spirit"]);
        assert_eq!(rc.search_words("DEITY").len(), 1);
        assert_eq!(rc.search_words("obed")[0].slug, "obey");
        assert!(rc.word("nope").is_none());
    }

    #[test]
    fn rejects_zip_slip_paths() {
        assert!(normalize_path("../etc/passwd").is_err());
        assert!(normalize_path("C:/x").is_err());
        assert_eq!(normalize_path("./content//01/01.usx").unwrap(), "content/01/01.usx");
    }

    #[test]
    fn rejects_container_without_package() {
        let mut f = BTreeMap::new();
        f.insert("content/01/01.usx".into(), b"x".to_vec());
        assert!(matches!(ResourceContainer::from_files(f), Err(Error::InvalidContainer(_))));
    }
}
