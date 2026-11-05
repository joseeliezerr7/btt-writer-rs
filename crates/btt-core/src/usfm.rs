//! Análisis y generación de USFM.
//!
//! * `parse_book` convierte un archivo USFM completo en capítulos/versos.
//! * `split_into_chunks` agrupa versos según marcadores de chunk
//!   (`\s5` del propio archivo, o la versificación de la biblioteca).
//! * `export_book` genera el USFM de un proyecto completo.
//! * `verses_of_chunk` extrae los versos de un chunk (texto de un `NN.txt`).

use crate::project::{natural_cmp, Project};
use crate::{Error, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Verse {
    /// Número tal como aparece (`1`, `2-3`, `4a`).
    pub number: String,
    /// Texto USFM crudo del verso (puede incluir `\p`, `\q`, notas...).
    pub text: String,
}

impl Verse {
    /// Primer número entero del verso (`2-3` → 2).
    pub fn start(&self) -> u32 {
        first_number(&self.number)
    }
    pub fn end(&self) -> u32 {
        last_number(&self.number)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct Chapter {
    pub number: String,
    pub title: String,
    /// Texto previo al primer verso (p. ej. `\s` o `\p` iniciales).
    pub preamble: String,
    pub verses: Vec<Verse>,
    /// Posiciones (índices en `verses`) donde el archivo tenía `\s5`.
    pub s5_breaks: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct Book {
    pub id: String,
    pub header_title: String,
    pub chapters: Vec<Chapter>,
}

fn re(s: &str, cell: &'static OnceLock<Regex>) -> &'static Regex {
    cell.get_or_init(|| Regex::new(s).unwrap())
}

static RE_MARKER: OnceLock<Regex> = OnceLock::new();
static RE_VERSE_SPLIT: OnceLock<Regex> = OnceLock::new();
static RE_INLINE: OnceLock<Regex> = OnceLock::new();
static RE_NOTE: OnceLock<Regex> = OnceLock::new();
static RE_WS: OnceLock<Regex> = OnceLock::new();

/// Analiza un libro USFM completo.
pub fn parse_book(usfm: &str) -> Result<Book> {
    let text = usfm.replace("\r\n", "\n").replace('\u{feff}', "");
    let mut book = Book::default();
    let mut current: Option<Chapter> = None;
    let mut buf = String::new(); // texto acumulado del verso/preámbulo actual
    let mut in_verse = false;

    // Tokeniza por marcadores de párrafo (al inicio de línea o tras espacio).
    let marker = re(r"(?m)\\(id|ide|h|toc[123]|mt[1-3]?|c|cl|cp|v|s5|s[1-4]?|p|m|q[1-4]?|b|li[1-4]?|nb|pi[1-3]?|mi|d|sp|ms[1-3]?|mr|r|rem|usfm|sts|is[1-3]?|ip|iot|io[1-3]?|imt[1-3]?|ie|f|x|fe|ft|fq|fqa|fk|fr|fv|nd|w|qt|bk|add|sls|tl|wj|em|bd|it|bdit|no|sc|pn|k|ord|qs|qac|va|vp)\*?(\s|$)", &RE_MARKER);

    // Recorremos línea a línea; cada línea puede contener varios marcadores.
    let flush_verse = |buf: &mut String, current: &mut Option<Chapter>, in_verse: bool| {
        if let Some(ch) = current.as_mut() {
            let t = buf.trim_end().to_string();
            if in_verse {
                if let Some(v) = ch.verses.last_mut() {
                    v.text = t;
                }
            } else {
                ch.preamble = t;
            }
        }
        buf.clear();
    };

    for line in text.lines() {
        let line = line.trim_end();
        if line.trim().is_empty() {
            if !buf.is_empty() {
                buf.push('\n');
            }
            continue;
        }
        let mut pos = 0;
        let mut handled_any = false;
        for m in marker.find_iter(line) {
            if m.start() < pos {
                continue;
            }
            // Texto antes del marcador.
            let before = &line[pos..m.start()];
            if !before.trim().is_empty() || !buf.is_empty() {
                buf.push_str(before);
            }
            let tag_full = m.as_str().trim();
            let had_space = m.as_str().ends_with(char::is_whitespace);
            let tag = tag_full.trim_start_matches('\\');
            // Resto de la línea tras el marcador hasta el siguiente marcador.
            let rest_start = m.end();
            let next = marker
                .find_at(line, rest_start)
                .map(|n| n.start())
                .unwrap_or(line.len());
            let rest = &line[rest_start..next];
            pos = next;
            handled_any = true;

            match tag {
                "id" => {
                    let mut parts = rest.trim().splitn(2, ' ');
                    book.id = parts.next().unwrap_or("").to_uppercase();
                    if let Some(t) = parts.next() {
                        if book.header_title.is_empty() {
                            book.header_title = t.trim().to_string();
                        }
                    }
                }
                "h" | "toc2" | "mt" | "mt1" => {
                    let t = rest.trim();
                    if !t.is_empty() && (tag == "h" || book.header_title.is_empty()) {
                        book.header_title = t.to_string();
                    }
                }
                "ide" | "toc1" | "toc3" | "mt2" | "mt3" | "rem" | "usfm" | "sts" | "is" | "is1"
                | "is2" | "is3" | "ip" | "iot" | "io" | "io1" | "io2" | "io3" | "imt" | "imt1"
                | "imt2" | "imt3" | "ie" => {}
                "c" => {
                    flush_verse(&mut buf, &mut current, in_verse);
                    if let Some(ch) = current.take() {
                        book.chapters.push(ch);
                    }
                    let num = rest.split_whitespace().next().unwrap_or("").to_string();
                    if num.is_empty() {
                        return Err(Error::InvalidUsfm("\\c sin número".into()));
                    }
                    current = Some(Chapter { number: num, ..Default::default() });
                    in_verse = false;
                }
                "cl" | "cp" => {
                    if let Some(ch) = current.as_mut() {
                        if tag == "cl" {
                            ch.title = rest.trim().to_string();
                        }
                    }
                }
                "s5" => {
                    if let Some(ch) = current.as_mut() {
                        let idx = ch.verses.len();
                        if !ch.s5_breaks.contains(&idx) {
                            ch.s5_breaks.push(idx);
                        }
                    }
                }
                "v" => {
                    flush_verse(&mut buf, &mut current, in_verse);
                    let rest_trim = rest.trim_start();
                    let (num, after) = match rest_trim.find(char::is_whitespace) {
                        Some(i) => (&rest_trim[..i], &rest_trim[i..]),
                        None => (rest_trim, ""),
                    };
                    let ch = current.get_or_insert_with(|| Chapter {
                        number: "1".into(),
                        ..Default::default()
                    });
                    ch.verses.push(Verse { number: num.to_string(), text: String::new() });
                    in_verse = true;
                    buf.push_str(after.trim_start());
                }
                // Marcadores de párrafo/poesía: se conservan dentro del texto.
                "p" | "m" | "q" | "q1" | "q2" | "q3" | "q4" | "b" | "li" | "li1" | "li2" | "li3"
                | "li4" | "nb" | "pi" | "pi1" | "pi2" | "pi3" | "mi" | "d" | "sp" | "s" | "s1"
                | "s2" | "s3" | "s4" | "ms" | "ms1" | "ms2" | "ms3" | "mr" | "r" => {
                    if current.is_some() {
                        if !buf.is_empty() && !buf.ends_with('\n') {
                            buf.push('\n');
                        }
                        buf.push_str(tag_full);
                        let r = rest.trim_end();
                        if !r.trim().is_empty() {
                            buf.push(' ');
                            buf.push_str(r.trim_start());
                        }
                        if next >= line.len() {
                            buf.push('\n');
                        }
                    }
                }
                // Marcadores de carácter / notas: forman parte del texto del verso.
                _ => {
                    buf.push_str(tag_full);
                    if had_space {
                        buf.push(' ');
                    }
                    buf.push_str(rest);
                }
            }
        }
        if !handled_any {
            if !buf.is_empty() && !buf.ends_with('\n') && !buf.ends_with(' ') {
                buf.push(' ');
            }
            buf.push_str(line.trim());
        } else if pos < line.len() {
            buf.push_str(&line[pos..]);
        }
    }
    flush_verse(&mut buf, &mut current, in_verse);
    if let Some(ch) = current.take() {
        book.chapters.push(ch);
    }
    if book.chapters.is_empty() {
        return Err(Error::InvalidUsfm("el archivo no contiene capítulos (\\c)".into()));
    }
    Ok(book)
}

/// Un chunk calculado: su id (verso inicial con dos dígitos) y su texto USFM.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChunkText {
    pub chunk: String,
    pub text: String,
}

/// Divide los versos de un capítulo en chunks.
///
/// `markers`: versos iniciales de cada chunk (p. ej. `[1, 5, 9]`). Si está
/// vacío se usan los `\s5` del archivo; si tampoco hay, cada verso es un chunk.
pub fn split_into_chunks(chapter: &Chapter, markers: &[u32]) -> Vec<ChunkText> {
    let mut out: Vec<ChunkText> = vec![];
    if chapter.verses.is_empty() {
        return out;
    }
    let boundaries: Vec<usize> = if !markers.is_empty() {
        let mut b = vec![0usize];
        for (i, v) in chapter.verses.iter().enumerate() {
            if i > 0 && markers.contains(&v.start()) && b.last() != Some(&i) {
                b.push(i);
            }
        }
        b
    } else if !chapter.s5_breaks.is_empty() {
        let mut b: Vec<usize> =
            chapter.s5_breaks.iter().copied().filter(|&i| i < chapter.verses.len()).collect();
        if b.first() != Some(&0) {
            b.insert(0, 0);
        }
        b.dedup();
        b
    } else {
        (0..chapter.verses.len()).collect()
    };

    for (bi, &start) in boundaries.iter().enumerate() {
        let end = boundaries.get(bi + 1).copied().unwrap_or(chapter.verses.len());
        let verses = &chapter.verses[start..end];
        let id = format!("{:02}", verses[0].start());
        let mut text = String::new();
        if bi == 0 && !chapter.preamble.trim().is_empty() {
            text.push_str(chapter.preamble.trim_end());
            text.push('\n');
        }
        for v in verses {
            text.push_str(&format!("\\v {} {}", v.number, v.text.trim_end()));
            text.push('\n');
        }
        let text = text.trim_end().to_string();
        if let Some(prev) = out.iter_mut().find(|c| c.chunk == id) {
            prev.text.push('\n');
            prev.text.push_str(&text);
        } else {
            out.push(ChunkText { chunk: id, text });
        }
    }
    out
}

/// Marcadores de chunk de un capítulo cuando no hay versificación: se derivan
/// de la posición de los `\s5`. Devuelve los versos iniciales.
pub fn markers_from_s5(chapter: &Chapter) -> Vec<u32> {
    split_into_chunks(chapter, &[]).iter().map(|c| first_number(&c.chunk)).collect()
}

/// Extrae versos del texto de un chunk (`\v 1 ... \v 2 ...`).
pub fn verses_of_chunk(text: &str) -> Vec<Verse> {
    let split = verse_marker_re();
    let mut out = vec![];
    let mut last: Option<(String, usize)> = None;
    for m in split.captures_iter(text) {
        let whole = m.get(0).unwrap();
        if let Some((num, start)) = last.take() {
            out.push(Verse { number: num, text: text[start..whole.start()].trim().to_string() });
        }
        let num = m.get(1).or_else(|| m.get(2)).map(|g| g.as_str()).unwrap_or("0");
        last = Some((num.to_string(), whole.end()));
    }
    if let Some((num, start)) = last {
        out.push(Verse { number: num, text: text[start..].trim().to_string() });
    }
    out
}

/// Marcador de versículo. Un sufijo de letra (`\v 1a`) solo cuenta como parte
/// del número si va seguido de espacio o fin de texto; si no, es texto pegado
/// (`\v 1texto` → versículo 1 y texto "texto").
fn verse_marker_re() -> &'static Regex {
    re(r"\\v\s+(?:(\d+[a-z]?(?:-\d+[a-z]?)?)(?:\s+|$)|(\d+(?:-\d+)?))", &RE_VERSE_SPLIT)
}

/// Separa lo que quede pegado a un marcador de versículo: `hola\v 2` →
/// `hola \v 2` y `\v 2texto` → `\v 2 texto`. El texto correcto no cambia.
pub fn tidy_verse_markers(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    let mut cursor = 0;
    for m in verse_marker_re().captures_iter(text) {
        let whole = m.get(0).unwrap();
        out.push_str(&text[cursor..whole.start()]);
        if !out.is_empty() && !out.ends_with(char::is_whitespace) {
            out.push(' ');
        }
        out.push_str(whole.as_str());
        // Alternativa 2: número seguido de algo que no es espacio.
        if m.get(2).is_some() && text[whole.end()..].chars().next().is_some_and(|c| !c.is_whitespace()) {
            out.push(' ');
        }
        cursor = whole.end();
    }
    out.push_str(&text[cursor..]);
    out
}

/// Números de verso (enteros) presentes en un chunk.
pub fn verse_numbers(text: &str) -> Vec<u32> {
    let mut v = vec![];
    for verse in verses_of_chunk(text) {
        for n in verse.start()..=verse.end().max(verse.start()) {
            v.push(n);
        }
    }
    v
}

/// Quita marcadores USFM y notas, dejando texto plano legible.
pub fn to_plain_text(usfm: &str) -> String {
    let note = re(r"\\fe\s.*?\\fe\*|\\f\s.*?\\f\*|\\x\s.*?\\x\*", &RE_NOTE);
    let inline = re(r"\\\+?[a-z0-9]+\*?\s?", &RE_INLINE);
    let ws = re(r"[ \t]+", &RE_WS);
    let no_notes = note.replace_all(usfm, "");
    // Quitar atributos de \w word|strong="H1234"\w*
    let mut s = String::with_capacity(no_notes.len());
    for part in no_notes.split("\\w ") {
        if s.is_empty() && !no_notes.starts_with("\\w ") {
            s.push_str(part);
            continue;
        }
        match part.find("\\w*") {
            Some(end) => {
                let word = &part[..end];
                let word = word.split('|').next().unwrap_or(word);
                s.push_str(word);
                s.push_str(&part[end + 3..]);
            }
            None => s.push_str(part),
        }
    }
    let s = inline.replace_all(&s, " ");
    let s = ws.replace_all(&s, " ");
    s.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")
}

/// Genera el USFM completo de un proyecto de texto bíblico.
///
/// Formato compatible con BTT Writer (marcas `\s5` entre chunks).
pub fn export_book(project: &Project) -> Result<String> {
    let book_id = project.manifest.project.id.to_uppercase();
    let title = {
        let t = project.read_book_title()?.trim().to_string();
        if t.is_empty() {
            project.manifest.project.name.clone()
        } else {
            t
        }
    };
    let mut out = String::new();
    out.push_str(&format!("\\id {book_id} {title}\n"));
    out.push_str("\\ide UTF-8\n");
    out.push_str(&format!("\\h {title}\n"));
    out.push_str(&format!("\\toc1 {title}\n"));
    out.push_str(&format!("\\toc2 {title}\n"));
    out.push_str(&format!("\\mt {title}\n\n"));

    let mut chapters = project.chapters()?;
    chapters.sort_by(|a, b| natural_cmp(a, b));
    /// El primer chunk de cada capítulo suele guardarse como `\c 1 \v 1 …`; el `\c`
    /// ya lo escribe la exportación, así que se quita para no duplicarlo.
    fn strip_chapter_marker(text: &str) -> &str {
        let Some(rest) = text.strip_prefix("\\c") else { return text };
        let rest = rest.trim_start();
        let digits = rest.trim_start_matches(|c: char| c.is_ascii_digit());
        if digits.len() == rest.len() {
            return text;
        }
        digits.trim_start()
    }
    for ch in chapters {
        let num = ch.trim_start_matches('0');
        let num = if num.is_empty() { "0" } else { num };
        out.push_str(&format!("\\c {num}\n"));
        let ch_title = project.read_chapter_title(&ch)?;
        if !ch_title.trim().is_empty() {
            out.push_str(&format!("\\cl {}\n", ch_title.trim()));
        }
        for chunk in project.chunks(&ch)? {
            if chunk == "title" || chunk == "reference" {
                continue;
            }
            let text = project.read_chunk(&ch, &chunk)?;
            let text = tidy_verse_markers(&text.replace("\r\n", "\n"));
            let text = strip_chapter_marker(text.trim());
            if text.is_empty() {
                continue;
            }
            out.push_str("\\s5\n");
            // Un chunk que empieza directamente con \v recibe un \p para que
            // los renderizadores lo muestren como párrafo.
            if text.starts_with("\\v") {
                out.push_str("\\p\n");
            }
            out.push_str(text);
            out.push_str("\n\n");
        }
    }
    Ok(out.trim_end().to_string() + "\n")
}

/// Importa un libro USFM dentro de un proyecto existente (sobrescribe chunks).
/// `markers_by_chapter`: versificación (versos iniciales por capítulo), puede estar vacía.
pub fn import_book(
    project: &Project,
    book: &Book,
    markers_by_chapter: &BTreeMap<String, Vec<u32>>,
) -> Result<usize> {
    let mut written = 0;
    if !book.header_title.is_empty() && project.read_book_title()?.trim().is_empty() {
        project.write_book_title(&book.header_title)?;
    }
    for ch in &book.chapters {
        let ch_id = format!("{:0>2}", ch.number);
        let markers = markers_by_chapter.get(&ch_id).cloned().unwrap_or_default();
        if !ch.title.is_empty() {
            project.write_chunk(&ch_id, "title", &ch.title)?;
        }
        for c in split_into_chunks(ch, &markers) {
            project.write_chunk(&ch_id, &c.chunk, &c.text)?;
            written += 1;
        }
    }
    Ok(written)
}

pub fn first_number(s: &str) -> u32 {
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().unwrap_or(0)
}

pub fn last_number(s: &str) -> u32 {
    match s.rsplit_once('-') {
        Some((_, b)) => first_number(b).max(first_number(s)),
        None => first_number(s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\\id 3JN Tercera de Juan\n\\ide UTF-8\n\\h 3 Juan\n\\toc1 3 Juan\n\\mt 3 Juan\n\n\\c 1\n\\s5\n\\p\n\\v 1 El anciano a Gayo el amado, a quien amo en verdad.\n\\p\n\\v 2 Amado, oro para que seas prosperado.\n\\v 3 Pues mucho me regocijé.\n\\v 4 No tengo yo mayor gozo.\n\\s5\n\\p\n\\v 5 Amado, fielmente te conduces \\f + \\ft nota al pie\\f* cuando prestas servicio.\n\\v 6-7 Ellos han dado testimonio.\n\\c 2\n\\q1 Verso poético\n\\v 1 Hola \\w mundo|strong=\"G2889\"\\w* fin.\n";

    #[test]
    fn parses_book_structure() {
        let b = parse_book(SAMPLE).unwrap();
        assert_eq!(b.id, "3JN");
        assert_eq!(b.header_title, "3 Juan");
        assert_eq!(b.chapters.len(), 2);
        let c1 = &b.chapters[0];
        assert_eq!(c1.number, "1");
        assert_eq!(c1.verses.len(), 6);
        assert_eq!(c1.verses[0].number, "1");
        assert_eq!(c1.verses[5].number, "6-7");
        assert_eq!(c1.s5_breaks, vec![0, 4]);
        assert!(c1.verses[1].text.starts_with("Amado"));
        assert!(c1.verses[4].text.contains("\\f + \\ft nota al pie\\f*"));
        // Marcador \p entre versos queda dentro del texto del verso 1.
        assert!(c1.verses[0].text.contains("\\p"));
        let c2 = &b.chapters[1];
        assert_eq!(c2.preamble.trim(), "\\q1 Verso poético");
    }

    #[test]
    fn splits_by_s5_and_by_markers() {
        let b = parse_book(SAMPLE).unwrap();
        let by_s5 = split_into_chunks(&b.chapters[0], &[]);
        assert_eq!(by_s5.iter().map(|c| c.chunk.as_str()).collect::<Vec<_>>(), vec!["01", "05"]);
        assert!(by_s5[0].text.contains("\\v 1 "));
        assert!(by_s5[1].text.contains("\\v 6-7"));

        let by_markers = split_into_chunks(&b.chapters[0], &[1, 3, 6]);
        assert_eq!(
            by_markers.iter().map(|c| c.chunk.as_str()).collect::<Vec<_>>(),
            vec!["01", "03", "06"]
        );
        let each = split_into_chunks(&b.chapters[1], &[]);
        assert_eq!(each.len(), 1);
        assert!(each[0].text.starts_with("\\q1 Verso poético\n\\v 1"));
    }

    #[test]
    fn pegado_al_marcador_no_forma_parte_del_numero() {
        let v = verses_of_chunk("\\v 1texto uno \\v 2a dos \\v 3-4b tres");
        assert_eq!(v.iter().map(|x| x.number.as_str()).collect::<Vec<_>>(), vec!["1", "2a", "3-4b"]);
        assert_eq!(v[0].text, "texto uno");
        assert_eq!(verse_numbers("\\v 12abc \\v 13."), vec![12, 13]);
        assert_eq!(tidy_verse_markers("hola\\v 2texto \\v 3 bien \\v 4a ok"), "hola \\v 2 texto \\v 3 bien \\v 4a ok");
        assert_eq!(tidy_verse_markers("\\v 1 a\n\\v 2 b"), "\\v 1 a\n\\v 2 b");
    }

    #[test]
    fn verses_of_chunk_handles_ranges_and_notes() {
        let v = verses_of_chunk("\\v 5 Amado \\f + \\ft n\\f* x\n\\v 6-7 Ellos.");
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].number, "5");
        assert_eq!(v[1].number, "6-7");
        assert_eq!(v[1].start(), 6);
        assert_eq!(v[1].end(), 7);
        assert_eq!(verse_numbers("\\v 5 a\n\\v 6-7 b"), vec![5, 6, 7]);
        assert!(verses_of_chunk("sin versos").is_empty());
    }

    #[test]
    fn plain_text_strips_markers() {
        let p = to_plain_text("\\p\n\\v 1 Hola \\w mundo|strong=\"G2889\"\\w* fin \\f + \\ft nota\\f* ya.\n\\q1 Poesía \\nd Señor\\nd*.");
        assert_eq!(p, "1 Hola mundo fin ya.\nPoesía Señor .");
    }

    #[test]
    fn export_import_round_trip() {
        use crate::manifest::{IdName, Manifest, TargetLanguage};
        let tmp = tempfile::tempdir().unwrap();
        let m = Manifest::new(
            TargetLanguage { id: "es".into(), name: "Español".into(), ..Default::default() },
            IdName::new("3jn", "3 John"),
            IdName::new("text", "Text"),
            IdName::new("reg", "Regular"),
            "1",
        );
        let p = Project::create(tmp.path(), m).unwrap();
        let book = parse_book(SAMPLE).unwrap();
        let n = import_book(&p, &book, &BTreeMap::new()).unwrap();
        assert_eq!(n, 3);
        assert_eq!(p.read_book_title().unwrap(), "3 Juan");
        assert_eq!(p.chunks("01").unwrap(), vec!["01", "05"]);
        let usfm = export_book(&p).unwrap();
        assert!(usfm.starts_with("\\id 3JN 3 Juan\n\\ide UTF-8\n\\h 3 Juan\n"));
        assert!(usfm.contains("\\c 1\n\\s5\n\\p\n\\v 1 El anciano"));
        assert!(usfm.contains("\\c 2\n\\s5\n\\q1 Verso poético\n\\v 1 Hola"));
        // Reimportar el export produce los mismos chunks.
        let book2 = parse_book(&usfm).unwrap();
        let c = split_into_chunks(&book2.chapters[0], &[]);
        assert_eq!(c.len(), 2);
        assert_eq!(verse_numbers(&c[1].text), vec![5, 6, 7]);

        // Los proyectos reales guardan el primer chunk como `\c 1 \v 1 …`: el
        // marcador de capítulo no debe salir dos veces en el USFM exportado.
        p.write_chunk("01", "01", "\\c 1 \\v 1 El anciano \\v 2 Amado").unwrap();
        let usfm = export_book(&p).unwrap();
        assert_eq!(usfm.matches("\\c 1").count(), 1, "{usfm}");
        assert!(usfm.contains("\\c 1\n\\s5\n\\p\n\\v 1 El anciano"), "{usfm}");
        assert_eq!(parse_book(&usfm).unwrap().chapters.len(), 2);
    }

    #[test]
    fn rejects_without_chapters() {
        assert!(parse_book("\\id GEN\n\\h Génesis").is_err());
    }

    #[test]
    fn parses_markers_glued_on_one_line() {
        let b = parse_book("\\id GEN\n\\c 1 \\p \\v 1 En el principio \\v 2 La tierra").unwrap();
        assert_eq!(b.chapters[0].verses.len(), 2);
        assert_eq!(b.chapters[0].verses[1].text, "La tierra");
    }
}
