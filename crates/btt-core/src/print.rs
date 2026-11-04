//! Generación de HTML imprimible (la app lo convierte a PDF con el motor web).
//!
//! El formato predeterminado reproduce el PDF clásico de los equipos de
//! traducción: Arial 12, márgenes de 2,5 cm, título de capítulo en azul a la
//! izquierda, versículos en superíndice, notas de traducción a dos columnas
//! después de cada capítulo, glosario de palabras al final y número de página
//! en el pie; la portada y la licencia son las del BTT Writer original.
//! Todo es configurable con [`PrintOptions`].

use crate::project::Project;
use crate::rc::ResourceContainer;
use crate::usfm;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintOptions {
    pub include_incomplete: bool,
    pub include_images: bool,
    pub font_family: String,
    pub font_size_pt: u32,
    /// Ruta/URL base de las imágenes OBS (`{base}/obs-en-01-01.jpg`).
    pub images_base: String,
    /// Columnas de texto por página (1 o 2).
    #[serde(default = "one")]
    pub columns: u8,
    /// Texto justificado.
    #[serde(default = "yes")]
    pub justify: bool,
    /// Portada con título, idioma y traductores.
    #[serde(default)]
    pub cover: bool,
    /// Página con la licencia.
    #[serde(default)]
    pub license: bool,
    /// Índice de capítulos.
    #[serde(default)]
    pub toc: bool,
    /// Notas de traducción (tN) después de cada capítulo.
    #[serde(default = "yes")]
    pub notes: bool,
    /// Glosario de palabras de traducción (tW) al final.
    #[serde(default = "yes")]
    pub words: bool,
    /// "Página N de M" en el pie.
    #[serde(default = "yes")]
    pub page_numbers: bool,
    /// Números de versículo en azul (si no, del color del texto).
    #[serde(default)]
    pub verse_color: bool,
    /// Separación de párrafos: `spaced` (línea en blanco) o `indent` (sangría).
    #[serde(default = "spaced")]
    pub paragraphs: String,
    /// Título de capítulo: `left` (azul, a la izquierda) o `centered`.
    #[serde(default = "left")]
    pub heading: String,
    /// Márgenes de página en centímetros.
    #[serde(default = "margin_default")]
    pub margin_cm: f32,
    /// Interlineado (1.0–2.0).
    #[serde(default = "line_height_default")]
    pub line_height: f32,
    /// Idioma de los rótulos fijos (`es` o `en`).
    #[serde(default)]
    pub lang: String,
}

fn one() -> u8 {
    1
}
fn yes() -> bool {
    true
}
fn spaced() -> String {
    "spaced".into()
}
fn left() -> String {
    "left".into()
}
fn margin_default() -> f32 {
    2.5
}
fn line_height_default() -> f32 {
    1.5
}

impl Default for PrintOptions {
    fn default() -> Self {
        PrintOptions {
            include_incomplete: true,
            include_images: false,
            font_family: "Arial, Helvetica, sans-serif".into(),
            font_size_pt: 12,
            images_base: String::new(),
            columns: 1,
            justify: true,
            cover: true,
            license: true,
            toc: false,
            notes: true,
            words: true,
            page_numbers: true,
            verse_color: false,
            paragraphs: spaced(),
            heading: left(),
            margin_cm: margin_default(),
            line_height: line_height_default(),
            lang: String::new(),
        }
    }
}

/// Contenedores de recursos que enriquecen el documento (todos opcionales).
#[derive(Default, Clone, Copy)]
pub struct PrintSources<'a> {
    /// Texto fuente (define la división en chunks y las palabras tW de cada chunk).
    pub source: Option<&'a ResourceContainer>,
    /// Notas de traducción (tN).
    pub notes: Option<&'a ResourceContainer>,
    /// Diccionario de palabras de traducción (tW).
    pub words: Option<&'a ResourceContainer>,
}

struct Labels {
    chapter: &'static str,
    page: &'static str,
    of: &'static str,
    contents: &'static str,
    words: &'static str,
}

fn labels(lang: &str) -> Labels {
    if lang.starts_with("en") {
        Labels { chapter: "Chapter", page: "Page", of: "of", contents: "Contents", words: "Translation Words" }
    } else {
        Labels { chapter: "Capítulo", page: "Página", of: "de", contents: "Contenido", words: "Palabras de traducción" }
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Convierte el USFM de un chunk a HTML. Devuelve (html, notas).
fn chunk_html(text: &str, notes: &mut Vec<String>) -> String {
    let mut html = String::new();
    let text = text.replace("\r\n", "\n");
    // Extraer notas al pie.
    let mut body = String::new();
    let mut rest = text.as_str();
    while let Some(start) = rest.find("\\f ") {
        body.push_str(&rest[..start]);
        let after = &rest[start + 3..];
        match after.find("\\f*") {
            Some(end) => {
                let inner = &after[..end];
                let note_text = usfm::to_plain_text(&inner.replacen('+', "", 1)).replace('\n', " ");
                notes.push(note_text.trim().to_string());
                // Marcador temporal: se convierte en <sup> tras escapar el HTML.
                body.push_str(&format!("\u{E000}{}\u{E001}", notes.len()));
                rest = &after[end + 3..];
            }
            None => {
                body.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    body.push_str(rest);

    let mut open_para = false;
    let open = |html: &mut String, open_para: &mut bool, class: &str| {
        if *open_para {
            html.push_str("</p>");
        }
        html.push_str(&format!("<p class=\"{class}\">"));
        *open_para = true;
    };
    for line in body.lines() {
        let line = line.trim_end();
        if line.trim().is_empty() {
            continue;
        }
        // `\c N` puede ir solo o delante del primer verso de la línea: se quita solo el marcador.
        let line = match line.strip_prefix("\\c ") {
            Some(r) => r.trim_start().trim_start_matches(|c: char| c.is_ascii_digit()).trim(),
            None => line,
        };
        if line.is_empty() {
            continue;
        }
        // Marcadores de libro que no llevan texto imprimible.
        if ["\\id", "\\h", "\\toc1", "\\toc2", "\\toc3", "\\mt", "\\mt1", "\\mt2", "\\ide", "\\cl", "\\cp"].iter().any(|m| line == *m || line.starts_with(&format!("{m} "))) && !line.contains("\\v ") {
            continue;
        }
        let mut handled = false;
        for (marker, class) in [("\\q1", "q1"), ("\\q2", "q2"), ("\\q", "q1"), ("\\qr", "qr"), ("\\qc", "qc"), ("\\m", "m"), ("\\pi", "pi"), ("\\p", "p"), ("\\b", "b"), ("\\li", "li"), ("\\nb", "p")] {
            if line == marker || line.starts_with(&format!("{marker} ")) {
                open(&mut html, &mut open_para, class);
                let rest = line[marker.len()..].trim();
                if !rest.is_empty() {
                    html.push_str(&inline_html(rest));
                }
                handled = true;
                break;
            }
        }
        if handled {
            continue;
        }
        if let Some(rest) = line.strip_prefix("\\s5") {
            if !rest.trim().is_empty() {
                html.push_str(&format!("<h4 class=\"s\">{}</h4>", esc(rest.trim())));
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("\\s ").or_else(|| line.strip_prefix("\\s1 ")) {
            if open_para {
                html.push_str("</p>");
                open_para = false;
            }
            html.push_str(&format!("<h4 class=\"s\">{}</h4>", esc(rest.trim())));
            continue;
        }
        if !open_para {
            open(&mut html, &mut open_para, "p");
        } else if !html.ends_with('>') {
            html.push(' ');
        }
        html.push_str(&inline_html(line));
    }
    if open_para {
        html.push_str("</p>");
    }
    html.replace('\u{E000}', "<sup class=\"note\">").replace('\u{E001}', "</sup>")
}

/// Versos y marcadores de carácter dentro de una línea.
fn inline_html(line: &str) -> String {
    let mut out = String::new();
    let verses = usfm::verses_of_chunk(line);
    if verses.is_empty() {
        return esc(&usfm::to_plain_text(line));
    }
    // Texto antes del primer \v
    if let Some(i) = line.find("\\v ") {
        let pre = usfm::to_plain_text(&line[..i]);
        if !pre.trim().is_empty() {
            out.push_str(&esc(&pre));
            out.push(' ');
        }
    }
    for v in verses {
        out.push_str(&format!("<sup class=\"v\">{}</sup>{} ", esc(&v.number), esc(&usfm::to_plain_text(&v.text))));
    }
    out.trim_end().to_string()
}

/// Decodifica las entidades HTML habituales (`&amp;quot;` doblemente codificada incluida).
fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}

/// Convierte el HTML que traen algunos artículos (`<p>`, `<ul><li>`, `<strong>`) a Markdown sencillo.
fn html_to_md(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let Some(j) = after.find('>') else {
            out.push('<');
            rest = after;
            continue;
        };
        let tag = after[..j].trim().trim_end_matches('/').trim();
        let closing = tag.starts_with('/');
        let name = tag.trim_start_matches('/').split_whitespace().next().unwrap_or("").to_ascii_lowercase();
        match (name.as_str(), closing) {
            ("br", _) => out.push('\n'),
            ("p" | "div", false) => out.push_str("\n\n"),
            ("p" | "div", true) => out.push_str("\n\n"),
            ("ul" | "ol", false) => out.push('\n'),
            ("ul" | "ol", true) => out.push_str("\n\n"),
            ("li", false) => out.push_str("\n* "),
            ("li", true) => {}
            ("h1" | "h2" | "h3" | "h4" | "h5" | "h6", false) => {
                out.push('\n');
                out.push_str(&"#".repeat(name[1..].parse::<usize>().unwrap_or(1)));
                out.push(' ');
            }
            ("h1" | "h2" | "h3" | "h4" | "h5" | "h6", true) => out.push('\n'),
            ("strong" | "b", _) => out.push_str("**"),
            ("em" | "i", _) => out.push('*'),
            _ => {}
        }
        rest = &after[j + 1..];
    }
    out.push_str(rest);
    out
}

/// Normaliza el cuerpo de una nota o artículo: entidades, HTML incrustado y enlaces antiguos.
fn normalize_help(s: &str) -> String {
    let decoded = decode_entities(s);
    let looks_html = ["<p", "<li", "<ul", "<ol", "<br", "<div", "<h1", "<h2", "<h3", "<strong", "<em"].iter().any(|t| decoded.contains(t));
    if looks_html { html_to_md(&decoded) } else { decoded }
}

/// Sustituye los enlaces `[[rc://…/figs-metaphor]]`, `[texto](url)` y los antiguos
/// `::ta:vol2:translate:figs-metaphor` por su último segmento.
fn strip_links(s: &str) -> String {
    let s = strip_old_ta_links(s);
    let s = s.as_str();
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find("[[") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 2..];
        match after.find("]]") {
            Some(j) => {
                let inner = &after[..j];
                out.push_str(inner.trim_end_matches('/').rsplit('/').next().unwrap_or(inner));
                rest = &after[j + 2..];
            }
            None => {
                out.push_str("[[");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    // [texto](url) → texto
    let mut res = String::new();
    let mut rest = out.as_str();
    while let Some(i) = rest.find('[') {
        let after = &rest[i + 1..];
        if let Some(j) = after.find("](") {
            if let Some(k) = after[j + 2..].find(')') {
                res.push_str(&rest[..i]);
                res.push_str(&after[..j]);
                rest = &after[j + 2 + k + 1..];
                continue;
            }
        }
        res.push_str(&rest[..=i]);
        rest = after;
    }
    res.push_str(rest);
    res
}

/// `::ta:vol2:translate:figs-metaphor` o `:ta:...` → `figs-metaphor` (conservando la puntuación que siga).
fn strip_old_ta_links(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(':') {
        let prev_ok = i == 0 || rest[..i].ends_with(|c: char| c.is_whitespace() || c == '(' || c == '[');
        let token_end = rest[i..].find(|c: char| c.is_whitespace() || c == ')' || c == ']' || c == ',' || c == ';').map(|k| i + k).unwrap_or(rest.len());
        let token = &rest[i..token_end];
        if prev_ok && token.matches(':').count() >= 3 && token.chars().all(|c| c.is_alphanumeric() || c == ':' || c == '-' || c == '_') {
            out.push_str(&rest[..i]);
            out.push_str(token.rsplit(':').find(|p| !p.is_empty()).unwrap_or(""));
            rest = &rest[token_end..];
        } else {
            out.push_str(&rest[..=i]);
            rest = &rest[i + 1..];
        }
    }
    out.push_str(rest);
    out
}

/// Negritas, cursivas y código de una línea Markdown (ya sin enlaces).
fn md_inline(s: &str) -> String {
    let mut out = esc(&strip_links(s));
    for (mark, tag) in [("**", "strong"), ("*", "em")] {
        let parts: Vec<&str> = out.split(mark).collect();
        if parts.len() < 3 {
            continue;
        }
        let open_tag = format!("<{tag}>");
        let close_tag = format!("</{tag}>");
        let mut res = parts[0].to_string();
        let mut open = false;
        let paired = parts.len() - 1 - ((parts.len() - 1) % 2); // marcas que forman pareja
        for (i, p) in parts.iter().enumerate().skip(1) {
            if i <= paired {
                res.push_str(if open { &close_tag } else { &open_tag });
                open = !open;
            } else {
                res.push_str(mark); // marca sin pareja: se deja tal cual
            }
            res.push_str(p);
        }
        out = res;
    }
    out
}

/// Markdown sencillo (encabezados, párrafos, viñetas) a HTML, para notas y palabras.
pub fn md_html(md: &str) -> String {
    let md = normalize_help(md);
    let mut html = String::new();
    let mut para: Vec<String> = vec![];
    let mut in_list = false;
    let flush_para = |html: &mut String, para: &mut Vec<String>| {
        if !para.is_empty() {
            html.push_str(&format!("<p>{}</p>", para.join(" ")));
            para.clear();
        }
    };
    for raw in md.replace("\r\n", "\n").lines() {
        let line = raw.trim();
        if line.is_empty() {
            flush_para(&mut html, &mut para);
            if in_list {
                html.push_str("</ul>");
                in_list = false;
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix('#') {
            flush_para(&mut html, &mut para);
            if in_list {
                html.push_str("</ul>");
                in_list = false;
            }
            let level = 1 + rest.chars().take_while(|c| *c == '#').count();
            let text = rest.trim_start_matches('#').trim();
            let level = level.clamp(1, 4) + 2; // h3..h6 para no competir con los capítulos
            html.push_str(&format!("<h{level}>{}</h{level}>", md_inline(text)));
            continue;
        }
        if let Some(item) = line.strip_prefix("* ").or_else(|| line.strip_prefix("- ")).or_else(|| line.strip_prefix("+ ")) {
            flush_para(&mut html, &mut para);
            if !in_list {
                html.push_str("<ul>");
                in_list = true;
            }
            html.push_str(&format!("<li>{}</li>", md_inline(item)));
            continue;
        }
        if in_list {
            html.push_str("</ul>");
            in_list = false;
        }
        para.push(md_inline(line));
    }
    flush_para(&mut html, &mut para);
    if in_list {
        html.push_str("</ul>");
    }
    html
}

/// Genera el documento HTML completo de un proyecto (solo con el texto fuente).
pub fn render_html(project: &Project, source: Option<&ResourceContainer>, opts: &PrintOptions) -> crate::Result<String> {
    render_document(project, PrintSources { source, ..Default::default() }, opts)
}

/// Genera el documento HTML completo de un proyecto con notas y palabras si se proporcionan.
pub fn render_document(project: &Project, sources: PrintSources<'_>, opts: &PrintOptions) -> crate::Result<String> {
    let m = &project.manifest;
    let lb = labels(if opts.lang.is_empty() { "es" } else { &opts.lang });
    let title = {
        let t = project.read_book_title()?.trim().to_string();
        if t.is_empty() { if m.project.name.is_empty() { m.project.id.to_uppercase() } else { m.project.name.clone() } } else { t }
    };
    let dir = if m.target_language.direction == "rtl" { "rtl" } else { "ltr" };
    let is_obs = m.resource.id == "obs";
    let mut chapters_html = String::new();
    let mut toc = String::new();
    let mut word_slugs: BTreeSet<String> = BTreeSet::new();
    let words_index = match (opts.words, sources.source, sources.words) {
        (true, Some(book), Some(_)) => Some(book.words_by_chunk()),
        _ => None,
    };
    let chapters = project.chapters()?;
    for ch in &chapters {
        let chunks: Vec<String> = match sources.source {
            Some(rc) if !rc.chunks(ch).is_empty() => rc.chunks(ch),
            _ => project.chunks(ch)?.into_iter().filter(|c| c != "title" && c != "reference").collect(),
        };
        let number = ch.trim_start_matches('0');
        let custom = project.read_chapter_title(ch)?.trim().to_string();
        let ch_title = if is_obs {
            if custom.is_empty() { number.to_string() } else { format!("{number}. {custom}") }
        } else if custom.is_empty() {
            format!("{title} {} {number}", lb.chapter)
        } else if custom == number || custom.to_lowercase().starts_with("cap") || custom.to_lowercase().starts_with("chap") {
            // El título personalizado ya es "Capítulo N": no se repite el número.
            format!("{title} {custom}")
        } else {
            format!("{title} {number} · {custom}")
        };
        let mut notes = vec![];
        let mut body = String::new();
        let mut helps = String::new();
        let mut any = false;
        for c in &chunks {
            let text = project.read_chunk(ch, c)?;
            if text.trim().is_empty() {
                continue;
            }
            if !opts.include_incomplete && !m.is_chunk_finished(ch, c) {
                continue;
            }
            any = true;
            if is_obs && opts.include_images && !opts.images_base.is_empty() {
                body.push_str(&format!("<img class=\"obs\" src=\"{}/obs-en-{}-{}.jpg\" alt=\"\">", esc(&opts.images_base), ch, c));
            }
            if is_obs {
                body.push_str(&format!("<p class=\"p\">{}</p>", esc(&text).replace('\n', "<br>")));
            } else {
                body.push_str(&chunk_html(&text, &mut notes));
            }
            if opts.notes {
                if let Some(tn) = sources.notes {
                    let entries = tn.help_entries(ch, c);
                    if !entries.is_empty() {
                        let reference = if is_obs { format!("{number}:{}", c.trim_start_matches('0')) } else { format!("{title} {number}:{}", c.trim_start_matches('0')) };
                        helps.push_str(&format!("<div class=\"ref\">{}</div>", esc(&reference)));
                        for e in entries {
                            helps.push_str("<div class=\"note\">");
                            if !e.title.trim().is_empty() {
                                helps.push_str(&format!("<div class=\"note-title\">{}</div>", md_inline(&decode_entities(e.title.trim()))));
                            }
                            helps.push_str(&md_html(&e.body));
                            helps.push_str("</div>");
                        }
                    }
                }
            }
            if let Some(index) = words_index.as_ref() {
                if let Some(slugs) = index.get(&(ch.clone(), c.clone())) {
                    word_slugs.extend(slugs.iter().cloned());
                }
            }
        }
        if !any {
            continue;
        }
        let anchor = format!("ch-{ch}");
        toc.push_str(&format!("<li><a href=\"#{anchor}\">{}</a></li>", esc(&ch_title)));
        chapters_html.push_str(&format!("<section class=\"chapter\" id=\"{anchor}\"><h2>{}</h2>", esc(&ch_title)));
        let reference = project.read_chapter_reference(ch)?;
        if !reference.trim().is_empty() {
            chapters_html.push_str(&format!("<p class=\"reference\">{}</p>", esc(reference.trim())));
        }
        chapters_html.push_str("<div class=\"text\">");
        chapters_html.push_str(&body);
        chapters_html.push_str("</div>");
        if !notes.is_empty() {
            chapters_html.push_str("<ol class=\"notes\">");
            for n in notes {
                chapters_html.push_str(&format!("<li>{}</li>", esc(&n)));
            }
            chapters_html.push_str("</ol>");
        }
        chapters_html.push_str("</section>");
        if !helps.is_empty() {
            chapters_html.push_str(&format!("<section class=\"helps\"><div class=\"helps-body\">{helps}</div></section>"));
        }
    }
    let mut words_html = String::new();
    if let Some(dict) = sources.words {
        // Ordenadas por título (sin comillas ni mayúsculas), como un glosario.
        let mut articles: Vec<(String, String, String)> = word_slugs
            .iter()
            .filter_map(|slug| dict.word_text(slug).map(|(t, b)| (if t.trim().is_empty() { slug.clone() } else { t.trim().to_string() }, b, slug.clone())))
            .collect();
        articles.sort_by_cached_key(|(t, _, slug)| (t.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase(), slug.clone()));
        for (title, body, _) in articles {
            words_html.push_str(&format!("<article class=\"word\"><h3>{}</h3>{}</article>", esc(&decode_entities(&title)), md_html(&body)));
        }
    }
    let columns_css = if opts.columns >= 2 { "columns: 2; column-gap: 2em; column-rule: 1px solid #ddd;" } else { "" };
    let align = if opts.justify { "justify" } else { "start" };
    let margin = opts.margin_cm.clamp(1.0, 4.0);
    let line_height = opts.line_height.clamp(1.0, 2.2);
    let page_footer = if opts.page_numbers {
        format!("@bottom-center {{ content: \"{} \" counter(page) \" {} \" counter(pages); font-family: Cambria, Georgia, serif; font-size: 10pt; color: #333; }}", lb.page, lb.of)
    } else {
        String::new()
    };
    let para_css = if opts.paragraphs == "indent" {
        "p { margin: 0; } p.p { text-indent: 1.4em; } p.p + p.p { margin-top: 0; }"
    } else {
        "p { margin: 0 0 1em; } p.p { text-indent: 0; }"
    };
    let heading_css = if opts.heading == "centered" {
        ".chapter h2 { font-size: 1.7em; text-align: center; margin: 0 0 .8em; padding-bottom: .3em; border-bottom: 1px solid #cfd6e0; color: #0b4f70; }"
    } else {
        ".chapter h2 { font-size: 1em; text-align: start; margin: 0 0 1.2em; color: #2f6fb5; }"
    };
    let verse_css = if opts.verse_color {
        "sup.v { color: #0e6f9a; font-weight: 700; }"
    } else {
        "sup.v { color: inherit; font-weight: 400; }"
    };
    // Portada como la de BTT Writer: solo el título del libro (y la descripción si existe),
    // centrado, en negrita, a 2,5× el tamaño del texto y un poco por encima de la mitad de la página.
    let cover = if opts.cover {
        let description = project.read_chunk("front", "description").unwrap_or_default();
        let description = description.trim();
        let desc_html = if description.is_empty() { String::new() } else { format!("<p class=\"description\">{}</p>", esc(description)) };
        format!("<section class=\"cover\"><h1>{}</h1>{desc_html}</section>", esc(&title))
    } else {
        String::new()
    };
    // Licencia: el mismo texto y estructura que la página de licencia del BTT Writer original.
    let license_html = if opts.license {
        let artwork = if is_obs { ORIGINAL_ARTWORK_ATTRIBUTION } else { "" };
        format!("<section class=\"license\">{ORIGINAL_LICENSE_HTML}{artwork}</section>")
    } else {
        String::new()
    };
    let toc_html = if opts.toc { format!("<section class=\"toc\"><h3>{}</h3><ol>{toc}</ol></section>", lb.contents) } else { String::new() };
    let words_section = if words_html.is_empty() { String::new() } else { format!("<section class=\"words\"><h2>{}</h2>{words_html}</section>", lb.words) };
    Ok(format!(
        r#"<!doctype html><html lang="{lang}" dir="{dir}"><head><meta charset="utf-8"><title>{title}</title>
<style>
@page {{ size: Letter; margin: {margin}cm; {page_footer} }}
* {{ box-sizing: border-box; }}
html {{ background: #e9ecf1; }}
body {{ font-family: {font}; font-size: {size}pt; line-height: {line_height}; color: #111; margin: 0; background: #fff; -webkit-print-color-adjust: exact; print-color-adjust: exact; }}
@media screen {{
  body {{ max-width: 8.5in; margin: 1.2rem auto; padding: {margin}cm; box-shadow: 0 2px 20px rgba(0,0,0,.18); min-height: 11in; }}
  /* Vista previa paginada (la construye el script al final del documento). */
  html.paged body {{ max-width: none; margin: 0; padding: 0; box-shadow: none; min-height: 0; background: transparent; }}
  html.paged #doc {{ display: none; }}
  .pages {{ padding: 14px 0 20px; }}
  .sheet {{ width: 8.5in; height: 11in; padding: {margin}cm; background: #fff; box-shadow: 0 2px 16px rgba(0,0,0,.2); margin: 0 auto 16px; position: relative; overflow: hidden; }}
  .sheet-body {{ height: 100%; overflow: hidden; }}
  .sheet-footer {{ position: absolute; left: 0; right: 0; bottom: calc({margin}cm * .4); text-align: center; font-family: Cambria, Georgia, serif; font-size: 10pt; color: #333; }}
}}
@media print {{ .pages {{ display: none !important; }} #doc {{ display: block !important; }} }}
p.cont {{ text-indent: 0 !important; }}
h1, h2, h3, h4 {{ font-weight: 700; line-height: 1.2; }}
a {{ color: inherit; text-decoration: none; }}
.cover {{ text-align: center; page-break-after: always; padding-top: calc(5.5in - 2in - {margin}cm); }}
.cover h1 {{ font-size: 2.5em; font-weight: 700; margin: 0; line-height: 1.25; }}
.cover .description {{ font-weight: 700; margin: .8em 0 0; }}
.license {{ page-break-after: always; }}
.license h2 {{ font-size: 1.4em; margin: 0 0 .6em; }}
.license h3 {{ font-size: 1.15em; margin: 1em 0 .3em; }}
.license h4 {{ font-size: 1em; font-weight: 700; margin: .6em 0; }}
.license p {{ margin: 0 0 .6em; }}
.license .bullet {{ margin: .3em 0; }}
.toc h3 {{ font-size: 1.3em; border-bottom: 2px solid #2f6fb5; padding-bottom: .3em; margin-bottom: .8em; }}
.toc {{ page-break-after: always; }}
.toc ol {{ columns: 2; column-gap: 2em; padding-inline-start: 1.5em; }}
.toc li {{ margin: .15em 0; break-inside: avoid; }}
.chapter {{ page-break-before: always; }}
.chapter:first-of-type {{ page-break-before: auto; }}
{heading_css}
.chapter .text {{ {columns} text-align: {align}; hyphens: auto; -webkit-hyphens: auto; orphans: 2; widows: 2; }}
.reference {{ font-style: italic; color: #555; margin: -.6em 0 1em; }}
sup.v {{ font-size: .62em; line-height: 0; vertical-align: super; margin-inline-end: .05em; }}
{verse_css}
sup.note {{ color: #b7791f; font-size: .62em; line-height: 0; }}
{para_css}
p.q1 {{ margin-inline-start: 2em; text-indent: -1em; }} p.q2 {{ margin-inline-start: 4em; text-indent: -1em; }} p.qr {{ text-align: end; }} p.qc {{ text-align: center; }}
p.m {{ text-indent: 0; }} p.pi {{ margin-inline-start: 2em; }} p.b {{ height: .8em; margin: 0; }} p.li {{ margin-inline-start: 1.5em; }}
h4.s {{ font-size: 1.05em; text-align: center; margin: 1em 0 .4em; break-after: avoid; }}
ol.notes {{ border-top: 1px solid #999; margin-top: 1.5em; padding-top: .5em; padding-inline-start: 1.4em; font-size: .85em; color: #444; column-span: all; }}
ol.notes li {{ margin: .15em 0; }}
img.obs {{ display: block; max-width: 100%; margin: .5em auto; break-inside: avoid; }}
.helps {{ page-break-before: always; font-size: .92em; }}
.helps-body {{ columns: 2; column-gap: 2.2em; text-align: start; hyphens: auto; }}
.helps .ref {{ font-style: italic; color: #2f6fb5; margin: 0 0 .6em 2em; break-after: avoid; }}
.helps .note {{ break-inside: avoid; margin: 0 0 .9em; }}
.helps .note-title {{ color: #2f6fb5; break-after: avoid; }}
.helps .note p {{ margin: 0 0 .3em; }}
.helps .note h3, .helps .note h4, .helps .note h5, .helps .note h6 {{ font-size: 1em; font-weight: 400; color: #2f6fb5; margin: .2em 0; }}
.helps ul {{ margin: .2em 0 .4em; padding-inline-start: 1.4em; }}
.words {{ page-break-before: always; }}
.words h2 {{ font-size: 1.15em; color: #2f6fb5; margin: 0 0 1em; }}
.word {{ border-bottom: 1px solid #999; padding-bottom: .6em; margin-bottom: 1em; }}
.word h3 {{ font-size: 1em; color: #2f6fb5; margin: 0; }}
.word h4, .word h5, .word h6 {{ font-size: 1em; font-weight: 400; font-style: italic; color: #2f6fb5; margin: .2em 0 0 1.5em; }}
.word p {{ margin: 0 0 .4em; }}
.word ul {{ margin: .3em 0 .5em; padding-inline-start: 2.2em; }}
.word li {{ margin: 0 0 .25em; }}
</style></head><body data-page-numbers="{page_numbers}" data-page-label="{page_label}">
<div id="doc">{cover}{license_html}{toc_html}{chapters}{words_section}</div>
<script>{preview_script}</script>
</body></html>"#,
        page_numbers = if opts.page_numbers { "1" } else { "0" },
        page_label = format!("{} {{n}} {} {{m}}", lb.page, lb.of),
        preview_script = PREVIEW_SCRIPT,
        lang = esc(&m.target_language.id),
        dir = dir,
        title = esc(&title),
        font = esc(&opts.font_family),
        size = opts.font_size_pt,
        margin = margin,
        line_height = line_height,
        page_footer = page_footer,
        heading_css = heading_css,
        columns = columns_css,
        align = align,
        verse_css = verse_css,
        para_css = para_css,
        cover = cover,
        license_html = license_html,
        toc_html = toc_html,
        chapters = chapters_html,
        words_section = words_section,
    ))
}

/// Página de licencia del BTT Writer original (texto en inglés, tal como lo imprime la app).
const ORIGINAL_LICENSE_HTML: &str = r#"<h2>Creative Commons Attribution-ShareAlike 4.0 International</h2>
<h3>Human-Readable Summary</h3>
<p>The Creative Commons Attribution-Share Alike 4.0 International License is available at <a href="http://creativecommons.org/licenses/by-sa/4.0/">http://creativecommons.org/licenses/by-sa/4.0/</a></p>
<h4>This is a human-readable summary of (and not a substitute for) the License</h4>
<h3>You are free to:</h3>
<p class="bullet"><b>&#8226; Share</b> - copy and redistribute the material in any medium or format</p>
<p class="bullet"><b>&#8226; Adapt</b> - remix, transform, and build upon the material for any purpose, even commercially.</p>
<p>The licensor cannot revoke these freedoms as long as you follow the license terms.</p>
<h3>Under the following terms:</h3>
<p class="bullet"><b>&#8226; Attribution</b> - You must give appropriate credit, provide a link to the license, and indicate if changes were made. You may do so in any reasonable manner, but not in any way that suggests the licensor endorses you or your use.</p>
<p class="bullet"><b>&#8226; ShareAlike</b> - If you remix, transform, or build upon the material, you must distribute your contributions under the same license as the original.</p>
<p class="bullet"><b>&#8226; No additional restrictions</b> - You may not apply legal terms or technological measures that legally restrict others from doing anything the license permits.</p>
<h3>Notices:</h3>
<p class="bullet">&#8226; You do not have to comply with the license for elements of the material in the public domain or where your use is permitted by an applicable exception or limitation.</p>
<p class="bullet">&#8226; No warranties are given. The license may not give you all of the permissions necessary for your intended use. For example, other rights such as publicity, privacy, or moral rights may limit how you use the material.</p>
"#;

/// Vista previa paginada en pantalla: reparte el documento (`#doc`) en hojas carta con los
/// márgenes elegidos, respetando los saltos de sección, partiendo los párrafos largos y
/// numerando las hojas. Al imprimir se usa el documento original y el motor pagina solo.
const PREVIEW_SCRIPT: &str = r#"(function () {
  if (window.matchMedia && window.matchMedia('print').matches) return;
  var doc = document.getElementById('doc');
  if (!doc) return;
  var pageNumbers = document.body.getAttribute('data-page-numbers') === '1';
  var label = document.body.getAttribute('data-page-label') || '{n} / {m}';
  var pagesEl = document.createElement('div');
  pagesEl.className = 'pages';
  var sheets = [], body = null, stack = [];
  function isMulticol(el) { var c = getComputedStyle(el).columnCount; return c && c !== 'auto' && parseInt(c, 10) > 1; }
  function sizeMulticol(el) {
    el.style.columnFill = 'auto'; el.style.overflow = 'hidden';
    var r = el.getBoundingClientRect(), b = body.getBoundingClientRect();
    el.style.height = Math.max(40, b.bottom - r.top) + 'px';
  }
  function newPage() {
    // Contenedores que quedaron vacíos al final de la hoja anterior (solo su borde o relleno).
    for (var j = stack.length - 1; j >= 0; j--) { var d = stack[j].dest; if (d && !d.childNodes.length) d.remove(); else break; }
    var s = document.createElement('div'); s.className = 'sheet';
    body = document.createElement('div'); body.className = 'sheet-body';
    s.appendChild(body); pagesEl.appendChild(s); sheets.push(s);
    var parent = body;
    for (var i = 0; i < stack.length; i++) {
      var shell = stack[i].src.cloneNode(false); shell.removeAttribute('id');
      parent.appendChild(shell); stack[i].dest = shell; parent = shell;
      if (isMulticol(shell)) sizeMulticol(shell);
    }
  }
  function dest() { return stack.length ? stack[stack.length - 1].dest : body; }
  function fits() {
    if (body.scrollHeight > body.clientHeight + 0.5) return false;
    for (var i = 0; i < stack.length; i++) { var d = stack[i].dest; if (isMulticol(d) && d.scrollWidth > d.clientWidth + 0.5) return false; }
    return true;
  }
  function pageEmpty() { return body.textContent.trim() === '' && !body.querySelector('img'); }
  var LEAF = /^(P|H1|H2|H3|H4|H5|H6|LI|IMG|HR)$/;
  function tokens(p) {
    var out = [], kids = p.childNodes;
    for (var i = 0; i < kids.length; i++) {
      var k = kids[i];
      if (k.nodeType === 3) { var m = k.textContent.match(/\S+\s*|\s+/g) || []; for (var j = 0; j < m.length; j++) out.push(m[j]); }
      else out.push(k);
    }
    return out;
  }
  function render(p, toks, from, to) {
    p.innerHTML = '';
    for (var i = from; i < to; i++) { var t = toks[i]; p.appendChild(typeof t === 'string' ? document.createTextNode(t) : t.cloneNode(true)); }
  }
  function fill(p, toks, from) {
    var lo = from, hi = toks.length;
    while (lo < hi) { var mid = (lo + hi + 1) >> 1; render(p, toks, from, mid); if (fits()) lo = mid; else hi = mid - 1; }
    render(p, toks, from, lo);
    return lo;
  }
  function splitParagraph(node, force) {
    var toks = tokens(node);
    if (toks.length < 2) return false;
    var p = node.cloneNode(false);
    dest().appendChild(p);
    var k = fill(p, toks, 0);
    if (k < 4 && !force) { p.remove(); return false; }
    if (k === 0) p.remove();
    var rest = k;
    while (rest < toks.length) {
      newPage();
      var q = node.cloneNode(false); q.classList.add('cont');
      dest().appendChild(q);
      var next = fill(q, toks, rest);
      if (next === rest) { next = rest + 1; render(q, toks, rest, next); }
      rest = next;
    }
    return true;
  }
  function placeLeaf(node) {
    var el = node.cloneNode(true);
    dest().appendChild(el);
    if (fits()) return;
    el.remove();
    if (el.tagName === 'P' && el.textContent.length > 60 && !pageEmpty() && splitParagraph(node, false)) return;
    if (pageEmpty()) { dest().appendChild(el); if (!fits() && el.tagName === 'P') { el.remove(); splitParagraph(node, true); } return; }
    newPage();
    dest().appendChild(el);
    if (!fits()) { el.remove(); if (el.tagName === 'P') splitParagraph(node, true); else dest().appendChild(el); }
  }
  function flow(node) {
    if (node.nodeType === 3) {
      if (!node.textContent.trim()) return;
      var t = document.createTextNode(node.textContent);
      dest().appendChild(t);
      if (!fits() && !pageEmpty()) { t.remove(); newPage(); dest().appendChild(t); }
      return;
    }
    if (node.nodeType !== 1) return;
    if (LEAF.test(node.tagName) || node.classList.contains('note-title') || node.classList.contains('ref')) { placeLeaf(node); return; }
    var shell = node.cloneNode(false); shell.removeAttribute('id');
    dest().appendChild(shell);
    stack.push({ src: node, dest: shell });
    if (isMulticol(shell)) sizeMulticol(shell);
    var kids = Array.prototype.slice.call(node.childNodes);
    for (var i = 0; i < kids.length; i++) flow(kids[i]);
    stack.pop();
  }
  document.documentElement.classList.add('paged');
  document.body.appendChild(pagesEl);
  var sections = Array.prototype.slice.call(doc.children);
  for (var i = 0; i < sections.length; i++) { newPage(); flow(sections[i]); }
  if (sheets.length > 1 && pageEmpty()) sheets.pop().remove();
  if (pageNumbers) {
    for (var n = 0; n < sheets.length; n++) {
      var f = document.createElement('div'); f.className = 'sheet-footer';
      f.textContent = label.replace('{n}', n + 1).replace('{m}', sheets.length);
      sheets[n].appendChild(f);
    }
  }
  function fit() {
    var first = pagesEl.firstChild; if (!first) return;
    var w = document.documentElement.clientWidth - 24;
    pagesEl.style.zoom = Math.min(1, w / first.offsetWidth);
  }
  fit();
  window.addEventListener('resize', fit);
})();"#;

/// Atribución de las imágenes de OBS, como en el original.
const ORIGINAL_ARTWORK_ATTRIBUTION: &str = r#"<p><b>Attribution of artwork:</b> All images used in these stories are © Sweet Publishing (<a href="http://www.sweetpublishing.com">www.sweetpublishing.com</a>) and are made available under a Creative Commons Attribution-Share Alike License (<a href="http://creativecommons.org/licenses/by-sa/3.0">http://creativecommons.org/licenses/by-sa/3.0</a>).</p>
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{IdName, Manifest, TargetLanguage};

    fn sample_project(tmp: &std::path::Path) -> Project {
        let m = Manifest::new(
            TargetLanguage { id: "arb".into(), name: "Arabic".into(), direction: "rtl".into(), ..Default::default() },
            IdName::new("3jn", "3 John"),
            IdName::new("text", "Text"),
            IdName::new("reg", "Regular"),
            "1",
        );
        let mut p = Project::create(tmp, m).unwrap();
        p.write_book_title("Tercera de Juan").unwrap();
        p.write_chunk("01", "01", "\\p\n\\c 1 \\v 1 El anciano \\f + \\ft nota uno\\f* a Gayo.\n\\q1 \\v 2 Amado & fiel").unwrap();
        p.write_chunk("01", "05", "\\v 5 Sin terminar").unwrap();
        p.set_chunk_finished("01", "01", true).unwrap();
        p.manifest.add_translator("Ana");
        p
    }

    #[test]
    fn renders_chapters_verses_and_footnotes() {
        let tmp = tempfile::tempdir().unwrap();
        let p = sample_project(tmp.path());

        let html = render_html(&p, None, &PrintOptions::default()).unwrap();
        assert!(html.contains("dir=\"rtl\""));
        // Formato predeterminado: portada y licencia como el original, título "Libro Capítulo N" y pie de página.
        assert!(html.contains("<section class=\"cover\"><h1>Tercera de Juan</h1></section>"));
        assert!(html.contains("<h2>Creative Commons Attribution-ShareAlike 4.0 International</h2>"));
        assert!(!html.contains("Attribution of artwork"));
        assert!(html.contains("<h2>Tercera de Juan Capítulo 1</h2>"));
        assert!(html.contains("@bottom-center { content: \"Página \" counter(page) \" de \" counter(pages)"));
        assert!(html.contains("font-size: 12pt"));
        assert!(html.contains("<p class=\"p\"><sup class=\"v\">1</sup>El anciano <sup class=\"note\">1</sup> a Gayo."), "el \\c no debe imprimirse: {html}");
        assert!(html.contains("<p class=\"q1\"><sup class=\"v\">2</sup>Amado &amp; fiel"));
        assert!(html.contains("<ol class=\"notes\"><li>nota uno</li></ol>"));
        assert!(html.contains("Sin terminar"));

        let only_done = render_html(&p, None, &PrintOptions { include_incomplete: false, ..Default::default() }).unwrap();
        assert!(!only_done.contains("Sin terminar"));
        assert!(only_done.contains("El anciano"));
    }

    #[test]
    fn optional_sections_follow_options() {
        let tmp = tempfile::tempdir().unwrap();
        let p = sample_project(tmp.path());
        let opts = PrintOptions { cover: false, license: false, toc: true, page_numbers: false, heading: "centered".into(), paragraphs: "indent".into(), verse_color: true, lang: "en".into(), ..Default::default() };
        let html = render_html(&p, None, &opts).unwrap();
        assert!(!html.contains("class=\"cover\""));
        assert!(!html.contains("Creative Commons"));
        assert!(html.contains("<h3>Contents</h3>"));
        assert!(html.contains("<h2>Tercera de Juan Chapter 1</h2>"));
        assert!(!html.contains("@bottom-center"));
        assert!(html.contains("text-align: center"));
        assert!(html.contains("p.p { text-indent: 1.4em; }"));
        assert!(html.contains("sup.v { color: #0e6f9a"));
    }

    /// La CSP de la app solo permite scripts propios: el script de la vista previa va
    /// autorizado por su hash en `tauri.conf.json`, que debe actualizarse si cambia.
    #[test]
    fn preview_script_hash_is_in_tauri_csp() {
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(PREVIEW_SCRIPT.as_bytes());
        let hash = base64(&digest);
        let conf = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/desktop/src-tauri/tauri.conf.json")).unwrap();
        assert!(conf.contains(&format!("'sha256-{hash}'")), "pon 'sha256-{hash}' en script-src de la CSP (apps/desktop/src-tauri/tauri.conf.json)");
    }

    fn base64(bytes: &[u8]) -> String {
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let n = chunk.iter().enumerate().fold(0u32, |acc, (i, b)| acc | (*b as u32) << (16 - 8 * i));
            for i in 0..4 {
                if i <= chunk.len() {
                    out.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char);
                } else {
                    out.push('=');
                }
            }
        }
        out
    }

    #[test]
    fn embedded_html_and_old_links_are_normalized() {
        let html = md_html("&lt;p&gt;El t&eacute;rmino &amp;quot;casa&amp;quot; es &lt;strong&gt;clave&lt;/strong&gt;.&lt;/p&gt; &lt;ul&gt; &lt;li&gt;Uno&lt;/li&gt; &lt;li&gt;Dos&lt;/li&gt; &lt;/ul&gt;");
        assert_eq!(html, "<p>El t&amp;eacute;rmino &quot;casa&quot; es <strong>clave</strong>.</p><ul><li>Uno</li><li>Dos</li></ul>");
        assert_eq!(strip_old_ta_links("(Véase: ::ta:vol2:translate:translate-names) y hora 10:30"), "(Véase: translate-names) y hora 10:30");
    }

    #[test]
    fn markdown_helper_handles_links_lists_and_emphasis() {
        let html = md_html("# Definición:\n\nEl **pan** es *comida*. (Ver: [[rc://es/ta/man/translate/figs-metaphor]] y [Juan](rc://es/ulb/book/jhn/01/01))\n\n* uno\n* dos\n");
        assert_eq!(
            html,
            "<h3>Definición:</h3><p>El <strong>pan</strong> es <em>comida</em>. (Ver: figs-metaphor y Juan)</p><ul><li>uno</li><li>dos</li></ul>"
        );
    }
}
