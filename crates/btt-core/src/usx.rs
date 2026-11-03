//! Análisis de fragmentos USX (formato de los textos fuente en los
//! contenedores `.tsrc`).
//!
//! Un chunk fuente luce así:
//! ```xml
//! <verse number="1" style="v" />Texto del verso <note caller="+" style="f"><char style="ft">nota</char></note>
//! <verse number="2" style="v" /><char style="w" strong="G2316">Dios</char> ...
//! ```
//! No se usa un analizador XML completo: los archivos son fragmentos sin
//! raíz y con entidades mínimas.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SourceVerse {
    pub number: String,
    /// Texto limpio (sin etiquetas), con notas extraídas.
    pub text: String,
    pub footnotes: Vec<String>,
    /// Marcadores de párrafo encontrados antes del verso (`p`, `q1`...).
    pub paragraphs_before: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SourceChunk {
    /// Texto previo al primer verso (títulos de sección, etc.).
    pub preamble: String,
    pub verses: Vec<SourceVerse>,
}

impl SourceChunk {
    pub fn plain_text(&self) -> String {
        let mut s = String::new();
        if !self.preamble.is_empty() {
            s.push_str(&self.preamble);
            s.push('\n');
        }
        for v in &self.verses {
            s.push_str(&format!("{} {}\n", v.number, v.text));
        }
        s.trim_end().to_string()
    }

    /// Convierte el chunk fuente a USFM (útil para "copiar fuente" y pruebas).
    pub fn to_usfm(&self) -> String {
        let mut s = String::new();
        if !self.preamble.is_empty() {
            s.push_str(&format!("\\s {}\n", self.preamble));
        }
        for v in &self.verses {
            for p in &v.paragraphs_before {
                s.push_str(&format!("\\{p}\n"));
            }
            s.push_str(&format!("\\v {} {}", v.number, v.text));
            for f in &v.footnotes {
                s.push_str(&format!(" \\f + \\ft {f}\\f*"));
            }
            s.push('\n');
        }
        s.trim_end().to_string()
    }
}

#[derive(Debug, PartialEq)]
enum Tok<'a> {
    Text(&'a str),
    Open { name: &'a str, attrs: &'a str, self_closing: bool },
    Close(&'a str),
}

fn tokenize(src: &str) -> Vec<Tok<'_>> {
    let mut out = vec![];
    let mut rest = src;
    while !rest.is_empty() {
        match rest.find('<') {
            None => {
                out.push(Tok::Text(rest));
                break;
            }
            Some(i) => {
                if i > 0 {
                    out.push(Tok::Text(&rest[..i]));
                }
                let after = &rest[i + 1..];
                let Some(end) = after.find('>') else {
                    out.push(Tok::Text(&rest[i..]));
                    break;
                };
                let inner = after[..end].trim();
                rest = &after[end + 1..];
                if inner.starts_with("!--") || inner.starts_with('?') {
                    continue;
                }
                if let Some(name) = inner.strip_prefix('/') {
                    out.push(Tok::Close(name.trim()));
                } else {
                    let self_closing = inner.ends_with('/');
                    let inner = inner.trim_end_matches('/').trim();
                    let (name, attrs) = match inner.find(char::is_whitespace) {
                        Some(j) => (&inner[..j], inner[j..].trim()),
                        None => (inner, ""),
                    };
                    out.push(Tok::Open { name, attrs, self_closing });
                }
            }
        }
    }
    out
}

fn attr<'a>(attrs: &'a str, key: &str) -> Option<&'a str> {
    let pat = format!("{key}=\"");
    let i = attrs.find(&pat)? + pat.len();
    let rest = &attrs[i..];
    let j = rest.find('"')?;
    Some(&rest[..j])
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&nbsp;", "\u{a0}")
        .replace("&amp;", "&")
}

fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(c);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

/// Analiza un fragmento USX.
pub fn parse_chunk(usx: &str) -> SourceChunk {
    let mut chunk = SourceChunk::default();
    let mut current: Option<SourceVerse> = None;
    let mut buf = String::new();
    let mut note_depth = 0usize;
    let mut note_buf = String::new();
    let mut pending_paragraphs: Vec<String> = vec![];

    let finish = |current: &mut Option<SourceVerse>, buf: &mut String, chunk: &mut SourceChunk| {
        let text = collapse_ws(&unescape(buf));
        match current.take() {
            Some(mut v) => {
                v.text = text;
                chunk.verses.push(v);
            }
            None => {
                if !text.is_empty() {
                    chunk.preamble = text;
                }
            }
        }
        buf.clear();
    };

    for tok in tokenize(usx) {
        match tok {
            Tok::Text(t) => {
                if note_depth > 0 {
                    note_buf.push_str(t);
                } else {
                    buf.push_str(t);
                }
            }
            Tok::Open { name, attrs, self_closing } => match name {
                "verse" => {
                    if attr(attrs, "eid").is_some() && attr(attrs, "number").is_none() {
                        continue; // marcador de fin USX 3
                    }
                    finish(&mut current, &mut buf, &mut chunk);
                    current = Some(SourceVerse {
                        number: attr(attrs, "number").unwrap_or("").to_string(),
                        paragraphs_before: std::mem::take(&mut pending_paragraphs),
                        ..Default::default()
                    });
                }
                "note" => {
                    note_depth += 1;
                    note_buf.clear();
                }
                "para" => {
                    let style = attr(attrs, "style").unwrap_or("p").to_string();
                    if current.is_some() && !self_closing {
                        // Párrafo que empieza dentro del verso actual.
                        buf.push('\n');
                    }
                    pending_paragraphs.push(style);
                }
                "char" if note_depth > 0 => {}
                "char" | "w" | "wg" | "wh" => {
                    // Se conserva solo el texto interno (strong, lemma... se descartan).
                }
                "optbreak" => buf.push(' '),
                _ => {}
            },
            Tok::Close(name) => match name {
                "note"
                    if note_depth > 0 => {
                        note_depth -= 1;
                        if note_depth == 0 {
                            let n = collapse_ws(&unescape(&note_buf));
                            if let Some(v) = current.as_mut() {
                                v.footnotes.push(n);
                            } else if !n.is_empty() {
                                // nota en el preámbulo
                                buf.push_str(&format!(" [{n}]"));
                            }
                        }
                    }
                "para"
                    if note_depth == 0 => {
                        buf.push('\n');
                    }
                _ => {}
            },
        }
    }
    finish(&mut current, &mut buf, &mut chunk);
    chunk
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_amharic_like_chunk() {
        let usx = "<verse number=\"1\" style=\"v\" />\nከመጀመሪያው ስለ ነበረው\n<verse number=\"2\" style=\"v\" />\nህይወትም ተገልጦ ነበር\n\n";
        let c = parse_chunk(usx);
        assert_eq!(c.verses.len(), 2);
        assert_eq!(c.verses[0].number, "1");
        assert_eq!(c.verses[0].text, "ከመጀመሪያው ስለ ነበረው");
        assert_eq!(c.verses[1].text, "ህይወትም ተገልጦ ነበር");
        assert_eq!(c.plain_text(), "1 ከመጀመሪያው ስለ ነበረው\n2 ህይወትም ተገልጦ ነበር");
    }

    #[test]
    fn strips_char_tags_and_extracts_notes() {
        let usx = r#"<para style="p" /><verse number="3" style="v" />En el <char style="w" strong="G746">principio</char> era <note caller="+" style="f"><char style="fr">1:3 </char><char style="ft">Algunos manuscritos dicen &quot;Verbo&quot;.</char></note>la Palabra &amp; luz."#;
        let c = parse_chunk(usx);
        assert_eq!(c.verses.len(), 1);
        let v = &c.verses[0];
        assert_eq!(v.text, "En el principio era la Palabra & luz.");
        assert_eq!(v.footnotes, vec!["1:3 Algunos manuscritos dicen \"Verbo\"."]);
        assert_eq!(v.paragraphs_before, vec!["p"]);
        assert!(c.to_usfm().starts_with("\\p\n\\v 3 En el principio"));
        assert!(c.to_usfm().contains("\\f + \\ft 1:3 Algunos"));
    }

    #[test]
    fn preamble_before_first_verse() {
        let c = parse_chunk("<para style=\"s\">Título</para><verse number=\"5\" style=\"v\" />Texto");
        assert_eq!(c.preamble, "Título");
        assert_eq!(c.verses[0].number, "5");
    }

    #[test]
    fn empty_and_garbage_input() {
        assert_eq!(parse_chunk("").verses.len(), 0);
        let c = parse_chunk("texto sin etiquetas <sin cerrar");
        assert!(c.verses.is_empty());
        assert!(c.preamble.starts_with("texto sin etiquetas"));
    }
}
