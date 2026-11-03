//! Generación de HTML imprimible (la app lo convierte a PDF con el motor web).
//!
//! Reproduce el PDF de BTT Writer: portada, licencia, índice, capítulos con
//! versos en superíndice, notas al pie al final de cada capítulo y soporte RTL.

use crate::project::Project;
use crate::rc::ResourceContainer;
use crate::usfm;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintOptions {
    pub include_incomplete: bool,
    pub include_images: bool,
    pub font_family: String,
    pub font_size_pt: u32,
    /// Ruta/URL base de las imágenes OBS (`{base}/obs-en-01-01.jpg`).
    pub images_base: String,
}

impl Default for PrintOptions {
    fn default() -> Self {
        PrintOptions {
            include_incomplete: true,
            include_images: false,
            font_family: "Noto Sans, Noto Serif, serif".into(),
            font_size_pt: 11,
            images_base: String::new(),
        }
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
        } else {
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

/// Genera el documento HTML completo de un proyecto.
pub fn render_html(project: &Project, source: Option<&ResourceContainer>, opts: &PrintOptions) -> crate::Result<String> {
    let m = &project.manifest;
    let title = {
        let t = project.read_book_title()?.trim().to_string();
        if t.is_empty() { if m.project.name.is_empty() { m.project.id.to_uppercase() } else { m.project.name.clone() } } else { t }
    };
    let dir = if m.target_language.direction == "rtl" { "rtl" } else { "ltr" };
    let is_obs = m.resource.id == "obs";
    let mut chapters_html = String::new();
    let mut toc = String::new();
    let chapters = project.chapters()?;
    for ch in &chapters {
        let chunks: Vec<String> = match source {
            Some(rc) if !rc.chunks(ch).is_empty() => rc.chunks(ch),
            _ => project.chunks(ch)?.into_iter().filter(|c| c != "title" && c != "reference").collect(),
        };
        let ch_title = {
            let t = project.read_chapter_title(ch)?.trim().to_string();
            if t.is_empty() { format!("{} {}", if is_obs { "" } else { "Capítulo" }, ch.trim_start_matches('0')).trim().to_string() } else { t }
        };
        let mut notes = vec![];
        let mut body = String::new();
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
        chapters_html.push_str(&body);
        if !notes.is_empty() {
            chapters_html.push_str("<ol class=\"notes\">");
            for n in notes {
                chapters_html.push_str(&format!("<li>{}</li>", esc(&n)));
            }
            chapters_html.push_str("</ol>");
        }
        chapters_html.push_str("</section>");
    }
    let translators = if m.translators.is_empty() { String::new() } else { format!("<p class=\"translators\">{}</p>", esc(&m.translators.join(", "))) };
    let license = crate::project::LICENSE_MD.lines().filter(|l| !l.trim().is_empty()).map(|l| format!("<p>{}</p>", esc(l.trim_start_matches('#').trim()))).collect::<String>();
    Ok(format!(
        r#"<!doctype html><html lang="{lang}" dir="{dir}"><head><meta charset="utf-8"><title>{title}</title>
<style>
@page {{ size: Letter; margin: 1in; }}
body {{ font-family: {font}; font-size: {size}pt; line-height: 1.5; color: #111; }}
.cover {{ text-align: center; page-break-after: always; padding-top: 30%; }}
.cover h1 {{ font-size: 2.4em; margin: 0 0 .3em; }}
.license {{ page-break-after: always; font-size: .8em; color: #333; }}
.toc {{ page-break-after: always; }}
.toc ol {{ columns: 2; }}
.chapter {{ page-break-before: always; }}
.chapter h2 {{ font-size: 1.5em; text-align: center; }}
.reference {{ font-style: italic; text-align: center; color: #555; }}
sup.v {{ color: #777; font-size: .65em; margin-inline-end: .15em; }}
sup.note {{ color: #1a56db; font-size: .65em; }}
p.q1 {{ margin: 0 0 0 2em; }} p.q2 {{ margin: 0 0 0 4em; }} p.qr {{ text-align: end; }} p.qc {{ text-align: center; }}
p.m {{ text-indent: 0; }} p.p {{ text-indent: 1.5em; margin: .3em 0; }} p.pi {{ margin-left: 2em; }} p.b {{ height: 1em; }}
h4.s {{ font-size: 1.05em; text-align: center; margin: 1em 0 .3em; }}
ol.notes {{ border-top: 1px solid #999; margin-top: 2em; padding-top: .5em; font-size: .85em; color: #1a56db; }}
img.obs {{ display: block; max-width: 100%; margin: .5em auto; }}
</style></head><body>
<section class="cover"><h1>{title}</h1><p>{lang_name}</p>{translators}</section>
<section class="license"><h3>Licencia</h3>{license}</section>
<section class="toc"><h3>Contenido</h3><ol>{toc}</ol></section>
{chapters}
</body></html>"#,
        lang = esc(&m.target_language.id),
        dir = dir,
        title = esc(&title),
        font = esc(&opts.font_family),
        size = opts.font_size_pt,
        lang_name = esc(&m.target_language.name),
        translators = translators,
        license = license,
        toc = toc,
        chapters = chapters_html,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{IdName, Manifest, TargetLanguage};

    #[test]
    fn renders_chapters_verses_and_footnotes() {
        let tmp = tempfile::tempdir().unwrap();
        let m = Manifest::new(
            TargetLanguage { id: "arb".into(), name: "Arabic".into(), direction: "rtl".into(), ..Default::default() },
            IdName::new("3jn", "3 John"),
            IdName::new("text", "Text"),
            IdName::new("reg", "Regular"),
            "1",
        );
        let mut p = Project::create(tmp.path(), m).unwrap();
        p.write_book_title("Tercera de Juan").unwrap();
        p.write_chunk("01", "01", "\\p\n\\v 1 El anciano \\f + \\ft nota uno\\f* a Gayo.\n\\q1 \\v 2 Amado & fiel").unwrap();
        p.write_chunk("01", "05", "\\v 5 Sin terminar").unwrap();
        p.set_chunk_finished("01", "01", true).unwrap();
        p.manifest.add_translator("Ana");

        let html = render_html(&p, None, &PrintOptions::default()).unwrap();
        assert!(html.contains("dir=\"rtl\""));
        assert!(html.contains("<h1>Tercera de Juan</h1>"));
        assert!(html.contains("<sup class=\"v\">1</sup>El anciano <sup class=\"note\">1</sup> a Gayo."));
        assert!(html.contains("<p class=\"q1\"><sup class=\"v\">2</sup>Amado &amp; fiel"));
        assert!(html.contains("<ol class=\"notes\"><li>nota uno</li></ol>"));
        assert!(html.contains("Sin terminar"));
        assert!(html.contains("Ana"));

        let only_done = render_html(&p, None, &PrintOptions { include_incomplete: false, ..Default::default() }).unwrap();
        assert!(!only_done.contains("Sin terminar"));
        assert!(only_done.contains("El anciano"));
    }
}
