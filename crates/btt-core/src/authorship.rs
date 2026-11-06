//! Autoría por versículo, deducida del historial git del chunk.
//!
//! No se guarda nada aparte: git ya registra quién confirmó cada versión del
//! archivo. Recorriendo las versiones de la más antigua a la más nueva, cada
//! versículo cuyo texto cambió se atribuye al autor de esa versión. Así se sabe
//! quién escribió `\v 1-3` y quién `\v 4-6` aunque estén en el mismo chunk, y
//! funciona también con proyectos antiguos, sin cambiar el formato de tS/WACS.

use crate::gitrepo;
use crate::usfm;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerseAuthor {
    /// Número tal como aparece en el texto (`1`, `2-3`, `4a`).
    pub verse: String,
    pub author: String,
    /// Fecha del commit; 0 si el texto aún no está confirmado (cambios de ahora mismo).
    pub time: i64,
}

fn norm(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn verses(text: &str) -> Vec<(String, String)> {
    usfm::verses_of_chunk(text).into_iter().map(|v| (v.number, norm(&v.text))).collect()
}

/// Quién escribió por última vez cada versículo del chunk `file` (`01/05.txt`).
/// `current` es el texto actual del archivo: si difiere de la última versión
/// confirmada, lo distinto se atribuye a `me` con fecha 0.
pub fn verse_authors(root: &Path, file: &str, current: &str, me: &str) -> Vec<VerseAuthor> {
    let mut by_verse: BTreeMap<String, (String, i64)> = BTreeMap::new();
    let mut prev: BTreeMap<String, String> = BTreeMap::new();
    let mut apply = |text: &str, author: &str, time: i64| {
        if crate::conflicts::has_conflicts(text) {
            return;
        }
        let now: BTreeMap<String, String> = verses(text).into_iter().collect();
        for (n, t) in &now {
            if prev.get(n) != Some(t) {
                by_verse.insert(n.clone(), (author.to_string(), time));
            }
        }
        prev = now;
    };
    let mut history = gitrepo::file_history(root, file).unwrap_or_default();
    history.reverse();
    for c in history {
        if let Ok(text) = gitrepo::file_at(root, &c.id, file) {
            apply(&text, &c.author, c.time);
        }
    }
    apply(current, me, 0);
    // En el orden del texto actual.
    verses(current)
        .into_iter()
        .filter_map(|(n, _)| by_verse.get(&n).map(|(a, t)| VerseAuthor { verse: n, author: a.clone(), time: *t }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gitrepo::{commit_all, Author, AUTOSAVE_MESSAGE};

    fn write(root: &Path, text: &str, who: &str) {
        std::fs::create_dir_all(root.join("01")).unwrap();
        std::fs::write(root.join("01/01.txt"), text).unwrap();
        commit_all(root, &Author::new(who, &format!("{who}@x")), AUTOSAVE_MESSAGE).unwrap();
        // Los commits del mismo segundo se ordenan igual por fecha: basta con que existan.
    }

    #[test]
    fn cada_versiculo_se_atribuye_a_quien_lo_cambio_por_ultima_vez() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write(root, "\\v 1 uno \\v 2 dos \\v 3 tres", "ana");
        std::thread::sleep(std::time::Duration::from_millis(1100));
        write(root, "\\v 1 uno \\v 2 dos \\v 3 tres \\v 4 cuatro \\v 5 cinco", "beto");
        std::thread::sleep(std::time::Duration::from_millis(1100));
        write(root, "\\v 1 uno \\v 2 DOS \\v 3 tres \\v 4 cuatro \\v 5 cinco", "ana");
        let current = std::fs::read_to_string(root.join("01/01.txt")).unwrap();
        let got: Vec<(String, String)> = verse_authors(root, "01/01.txt", &current, "yo").into_iter().map(|v| (v.verse, v.author)).collect();
        let s = |a: &str, b: &str| (a.to_string(), b.to_string());
        assert_eq!(got, vec![s("1", "ana"), s("2", "ana"), s("3", "ana"), s("4", "beto"), s("5", "beto")]);

        // Lo escrito ahora mismo (sin confirmar) es de quien tiene el proyecto abierto.
        let pending = "\\v 1 uno \\v 2 DOS \\v 3 tres \\v 4 CUATRO \\v 5 cinco";
        let got = verse_authors(root, "01/01.txt", pending, "yo");
        assert_eq!(got[3].author, "yo");
        assert_eq!(got[3].time, 0);
        assert_eq!(got[4].author, "beto");
    }
}
