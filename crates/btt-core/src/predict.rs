//! Predicción de palabras mientras se traduce.
//!
//! Las lenguas minoritarias no tienen diccionario en ningún teclado: el único
//! posible es el que forma la propia traducción. Este índice se construye con el
//! texto ya escrito en los proyectos de un idioma (cuántas veces aparece cada
//! palabra y cuál suele seguir a cuál) y sugiere, para lo que se está tecleando,
//! las palabras del idioma que empiezan igual. Todo local, sin modelos ni red.
//!
//! Solo se sugieren palabras vistas al menos [`MIN_COUNT`] veces, para no
//! propagar errores de tipeo.

use crate::project::Project;
use crate::usfm;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Veces que debe aparecer una palabra para sugerirse.
pub const MIN_COUNT: u32 = 2;
/// Letras tecleadas antes de empezar a sugerir.
pub const MIN_PREFIX: usize = 2;

#[derive(Debug, Clone, Default)]
struct Word {
    count: u32,
    /// Formas con su mayúscula/minúscula original, para mostrar la más habitual.
    forms: HashMap<String, u32>,
}

/// Índice de un idioma: palabras y qué palabra sigue a cuál.
#[derive(Debug, Clone, Default)]
pub struct Index {
    words: HashMap<String, Word>,
    next: HashMap<String, HashMap<String, u32>>,
    /// Palabras (con repetición) con las que se construyó.
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Suggestion {
    pub word: String,
    pub count: u32,
}

fn is_mark(c: char) -> bool {
    matches!(c, '\u{0300}'..='\u{036F}' | '\u{1AB0}'..='\u{1AFF}' | '\u{1DC0}'..='\u{1DFF}' | '\u{20D0}'..='\u{20FF}' | '\u{FE20}'..='\u{FE2F}')
}

/// Letras, diacríticos combinados y los apóstrofos/guiones que muchas lenguas
/// usan dentro de la palabra (saltillo, oclusiva glotal...).
pub fn is_word_char(c: char) -> bool {
    c.is_alphabetic() || is_mark(c) || matches!(c, '\'' | '’' | 'ʼ' | 'ꞌ' | 'ʔ' | '-')
}

fn is_edge(c: char) -> bool {
    matches!(c, '\'' | '’' | 'ʼ' | '-')
}

/// Palabras del texto de un chunk, sin marcadores USFM, notas ni números.
pub fn tokens(text: &str) -> Vec<String> {
    let plain = usfm::to_plain_text(text);
    let mut out = vec![];
    let mut cur = String::new();
    let flush = |cur: &mut String, out: &mut Vec<String>| {
        let w = cur.trim_matches(is_edge);
        if w.chars().any(char::is_alphabetic) {
            out.push(w.to_string());
        }
        cur.clear();
    };
    for c in plain.chars() {
        if is_word_char(c) {
            cur.push(c);
        } else if !cur.is_empty() {
            flush(&mut cur, &mut out);
        }
    }
    if !cur.is_empty() {
        flush(&mut cur, &mut out);
    }
    out
}

impl Index {
    pub fn new() -> Index {
        Index::default()
    }

    /// Añade el texto de un chunk.
    pub fn add_text(&mut self, text: &str) {
        let toks = tokens(text);
        let mut prev: Option<String> = None;
        for t in toks {
            let key = t.to_lowercase();
            let w = self.words.entry(key.clone()).or_default();
            w.count += 1;
            *w.forms.entry(t).or_default() += 1;
            if let Some(p) = prev {
                *self.next.entry(p).or_default().entry(key.clone()).or_default() += 1;
            }
            prev = Some(key);
            self.total += 1;
        }
    }

    /// Índice con todos los chunks de texto del proyecto (no las notas ni preguntas).
    pub fn add_project(&mut self, p: &Project) {
        if p.manifest.type_.id != "text" {
            return;
        }
        for c in p.all_chunks().unwrap_or_default() {
            if let Ok(t) = p.read_chunk(&c.chapter, &c.chunk) {
                self.add_text(&t);
            }
        }
    }

    /// Índice de un idioma: lo escrito en todos los proyectos locales con ese idioma meta.
    pub fn for_language(translations_dir: &std::path::Path, language_id: &str) -> Index {
        let mut ix = Index::new();
        for p in Project::list(translations_dir).unwrap_or_default() {
            if p.manifest.target_language.id == language_id {
                ix.add_project(&p);
            }
        }
        ix
    }

    pub fn is_empty(&self) -> bool {
        self.total == 0
    }

    /// Palabras que empiezan por `prefix`, las más probables primero. `prev` (la
    /// palabra anterior) favorece las que suelen seguirla.
    pub fn suggest(&self, prev: Option<&str>, prefix: &str, limit: usize) -> Vec<Suggestion> {
        let prefix = prefix.trim_matches(is_edge);
        if prefix.chars().count() < MIN_PREFIX {
            return vec![];
        }
        let key = prefix.to_lowercase();
        let follow = prev.map(|p| p.to_lowercase()).and_then(|p| self.next.get(&p));
        let mut c: Vec<(u64, &String, &Word)> = self
            .words
            .iter()
            .filter(|(k, w)| w.count >= MIN_COUNT && k.starts_with(&key) && **k != key)
            .map(|(k, w)| {
                let bigram = follow.and_then(|f| f.get(k)).copied().unwrap_or(0) as u64;
                (w.count as u64 + bigram * 20, k, w)
            })
            .collect();
        c.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
        let upper = prefix.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
        c.into_iter()
            .take(limit)
            .map(|(_, _, w)| {
                // La forma más frecuente; en empate, la que coincide en mayúscula con lo tecleado.
                let fits = |f: &str| f.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) == upper;
                let form = w.forms.iter().max_by_key(|(f, n)| (**n, fits(f), std::cmp::Reverse((*f).clone()))).map(|(f, _)| f.clone()).unwrap_or_default();
                let word = if upper { capitalize(&form) } else { form };
                Suggestion { word, count: w.count }
            })
            .collect()
    }
}

fn capitalize(s: &str) -> String {
    let mut cs = s.chars();
    match cs.next() {
        Some(c) => c.to_uppercase().collect::<String>() + cs.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn las_palabras_salen_sin_marcadores_ni_numeros() {
        let t = tokens("\\v 1 Ho'ñiꞌ dios, ¿ko-ta? \\f + \\ft nota\\f* \\v 2 Dios 3 ho'ñiꞌ.");
        assert_eq!(t, vec!["Ho'ñiꞌ", "dios", "ko-ta", "Dios", "ho'ñiꞌ"]);
    }

    #[test]
    fn sugiere_por_prefijo_y_prefiere_lo_que_sigue_a_la_anterior() {
        let mut ix = Index::new();
        ix.add_text("\\v 1 Hola mundo. Hola tierra. Hombre bueno, hombre malo. \\v 2 Dijo el hombre: hola.");
        ix.add_text("\\v 3 Honra a tu padre.");
        // "Honra" solo aparece una vez: no se sugiere (podría ser un error de tipeo).
        let s = ix.suggest(None, "Ho", 5);
        // Empatadas a 3: en orden alfabético.
        assert_eq!(s.iter().map(|s| s.word.as_str()).collect::<Vec<_>>(), vec!["Hola", "Hombre"]);
        assert_eq!(s[0].count, 3);
        // Tras "el" lo habitual es "hombre"; la forma se muestra como se tecleó (minúscula).
        let s = ix.suggest(Some("el"), "ho", 5);
        assert_eq!(s[0].word, "hombre");
        // Lo ya escrito completo no se repite como sugerencia.
        assert!(ix.suggest(None, "hola", 5).is_empty());
        // Hace falta un mínimo de letras.
        assert!(ix.suggest(None, "h", 5).is_empty());
    }

    #[test]
    fn la_forma_mostrada_es_la_mas_frecuente_pero_respeta_la_mayuscula_tecleada() {
        let mut ix = Index::new();
        ix.add_text("dios dios Dios");
        assert_eq!(ix.suggest(None, "di", 1)[0].word, "dios");
        assert_eq!(ix.suggest(None, "Di", 1)[0].word, "Dios");
        // En empate gana la forma que coincide con la mayúscula tecleada.
        ix.add_text("Hombre y hombre");
        assert_eq!(ix.suggest(None, "ho", 1)[0].word, "hombre");
        assert_eq!(ix.suggest(None, "Ho", 1)[0].word, "Hombre");
    }
}
