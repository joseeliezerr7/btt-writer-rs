//! Validación de chunks antes de marcarlos como terminados y formato de los
//! chunks de ayudas (tN/tQ/tW).

use crate::project::Project;
use crate::rc::ResourceContainer;
use crate::usfm;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Issue {
    /// El chunk está vacío.
    Empty,
    /// Dos marcadores `\v` seguidos sin texto entre ellos.
    ConsecutiveMarkers { verse: u32 },
    /// Verso que no existe en la fuente.
    OutOfRange { verse: u32 },
    /// Verso repetido.
    Duplicate { verse: u32 },
    /// Versos en desorden.
    OutOfOrder { verse: u32 },
    /// Verso de la fuente que falta en la traducción.
    Missing { verse: u32 },
    /// Marcadores de conflicto de git sin resolver.
    Conflict,
}

/// Valida el texto de un chunk USFM contra los versos esperados de la fuente.
/// Si `expected` está vacío solo se comprueban vacío, duplicados y orden.
pub fn validate_chunk(text: &str, expected: &[u32]) -> Vec<Issue> {
    let mut issues = vec![];
    if text.trim().is_empty() {
        issues.push(Issue::Empty);
        return issues;
    }
    if crate::conflicts::has_conflicts(text) {
        issues.push(Issue::Conflict);
        return issues;
    }
    let verses = usfm::verses_of_chunk(text);
    let mut seen: Vec<u32> = vec![];
    let mut last = 0u32;
    for v in &verses {
        if v.text.trim().is_empty() {
            issues.push(Issue::ConsecutiveMarkers { verse: v.start() });
        }
        for n in v.start()..=v.end().max(v.start()) {
            if seen.contains(&n) {
                issues.push(Issue::Duplicate { verse: n });
            } else {
                seen.push(n);
            }
            if !expected.is_empty() && !expected.contains(&n) {
                issues.push(Issue::OutOfRange { verse: n });
            }
            if n < last {
                issues.push(Issue::OutOfOrder { verse: n });
            }
            last = last.max(n);
        }
    }
    for e in expected {
        if !seen.contains(e) {
            issues.push(Issue::Missing { verse: *e });
        }
    }
    issues
}

/// Añade al inicio los marcadores `\v N ` de los versos esperados que faltan,
/// como hace BTT Writer al abrir un chunk.
pub fn add_missing_verses(text: &str, expected: &[u32]) -> String {
    if expected.is_empty() || crate::conflicts::has_conflicts(text) {
        return text.to_string();
    }
    let present = usfm::verse_numbers(text);
    let mut missing: Vec<u32> = expected.iter().copied().filter(|v| !present.contains(v)).collect();
    if missing.is_empty() {
        return text.to_string();
    }
    missing.sort_unstable();
    if text.trim().is_empty() {
        return missing.iter().map(|m| format!("\\v {m}")).collect::<Vec<_>>().join(" ");
    }
    // Cada marcador que falta se inserta en su posición: antes del primer verso
    // presente mayor que él (así `\v 2 b` con [1,2,3] da `\v 1 \v 2 b \v 3`).
    let re = regex::Regex::new(r"\\v\s+(\d+)").expect("regex de versos");
    let mut out = String::new();
    let mut cursor = 0;
    let mut missing = missing.into_iter().peekable();
    for m in re.captures_iter(text) {
        let n: u32 = m[1].parse().unwrap_or(0);
        let at = m.get(0).map(|g| g.start()).unwrap_or(0);
        while matches!(missing.peek(), Some(&mm) if mm < n) {
            out.push_str(&text[cursor..at]);
            cursor = at;
            out.push_str(&format!("\\v {} ", missing.next().unwrap_or(0)));
        }
    }
    out.push_str(&text[cursor..]);
    for mm in missing {
        if !out.ends_with(' ') && !out.ends_with('\n') {
            out.push(' ');
        }
        out.push_str(&format!("\\v {mm} "));
    }
    out.trim_end().to_string()
}

/// Entrada de ayuda (nota, pregunta o palabra) tal como se guarda en los
/// chunks de proyectos tN/tQ/tW: JSON `[{"title":"…","body":"…"}]`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct HelpItem {
    pub title: String,
    pub body: String,
}

pub fn parse_helps(text: &str) -> Vec<HelpItem> {
    if text.trim().is_empty() {
        return vec![];
    }
    serde_json::from_str(text).unwrap_or_else(|_| {
        // Texto plano antiguo: una sola entrada sin título.
        vec![HelpItem { title: String::new(), body: text.trim().to_string() }]
    })
}

pub fn serialize_helps(items: &[HelpItem]) -> String {
    let items: Vec<&HelpItem> = items.iter().filter(|i| !(i.title.trim().is_empty() && i.body.trim().is_empty())).collect();
    if items.is_empty() {
        return String::new();
    }
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(b"\t");
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    serde::Serialize::serialize(&items, &mut ser).expect("helps serializables");
    String::from_utf8(buf).unwrap()
}

/// Un chunk de ayudas está completo si tiene entradas y ninguna está a medias.
pub fn helps_complete(items: &[HelpItem]) -> bool {
    !items.is_empty() && items.iter().all(|i| !i.title.trim().is_empty() && !i.body.trim().is_empty())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChunkValidation {
    pub chapter: String,
    pub chunk: String,
    pub finished: bool,
    pub issues: Vec<Issue>,
}

/// Valida todo un proyecto de texto contra su fuente (para "Publicar").
pub fn validate_project(project: &Project, source: Option<&ResourceContainer>) -> crate::Result<Vec<ChunkValidation>> {
    let mut out = vec![];
    let is_text = project.manifest.type_.id == "text";
    let chapters: Vec<String> = match source {
        Some(rc) => rc.chapters(),
        None => project.chapters()?,
    };
    for ch in chapters {
        let chunks: Vec<String> = match source {
            Some(rc) => rc.chunks(&ch),
            None => project.chunks(&ch)?.into_iter().filter(|c| c != "title" && c != "reference").collect(),
        };
        for c in chunks {
            let text = project.read_chunk(&ch, &c)?;
            let issues = if is_text && project.manifest.resource.id != "obs" {
                let expected: Vec<u32> = source
                    .and_then(|rc| rc.source_chunk(&ch, &c))
                    .map(|sc| {
                        sc.verses
                            .iter()
                            .flat_map(|v| {
                                let a = usfm::first_number(&v.number);
                                let b = usfm::last_number(&v.number).max(a);
                                a..=b
                            })
                            .filter(|&n| n > 0)
                            .collect()
                    })
                    .unwrap_or_default();
                validate_chunk(&text, &expected)
            } else if is_text {
                if text.trim().is_empty() { vec![Issue::Empty] } else { vec![] }
            } else if helps_complete(&parse_helps(&text)) {
                vec![]
            } else {
                vec![Issue::Empty]
            };
            out.push(ChunkValidation {
                chapter: ch.clone(),
                chunk: c.clone(),
                finished: project.manifest.is_chunk_finished(&ch, &c),
                issues,
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_all_issue_kinds() {
        assert_eq!(validate_chunk("   ", &[1]), vec![Issue::Empty]);
        let issues = validate_chunk("\\v 1 \\v 2 hola \\v 2 otra \\v 9 fuera \\v 4 tarde", &[1, 2, 3, 4]);
        assert!(issues.contains(&Issue::ConsecutiveMarkers { verse: 1 }));
        assert!(issues.contains(&Issue::Duplicate { verse: 2 }));
        assert!(issues.contains(&Issue::OutOfRange { verse: 9 }));
        assert!(issues.contains(&Issue::OutOfOrder { verse: 4 }));
        assert!(issues.contains(&Issue::Missing { verse: 3 }));
        assert!(validate_chunk("\\v 1 a\n\\v 2-3 b\n\\v 4 c", &[1, 2, 3, 4]).is_empty());
        assert_eq!(validate_chunk("<<<<<<< HEAD\na\n=======\nb\n>>>>>>> x", &[1]), vec![Issue::Conflict]);
    }

    #[test]
    fn adds_missing_verse_markers() {
        assert_eq!(add_missing_verses("", &[1, 2]), "\\v 1 \\v 2");
        assert_eq!(add_missing_verses("\\v 2 b", &[1, 2, 3]), "\\v 1 \\v 2 b \\v 3");
        assert_eq!(add_missing_verses("\\c 1 \\v 2 b", &[1, 2]), "\\c 1 \\v 1 \\v 2 b");
        assert_eq!(add_missing_verses("", &[1, 2]), "\\v 1 \\v 2");
        // Lo que la app añade no debe salir luego como "versos en desorden".
        let issues = validate_chunk(&add_missing_verses("\\v 2 b", &[1, 2, 3]), &[1, 2, 3]);
        assert!(!issues.iter().any(|i| matches!(i, Issue::OutOfOrder { .. })), "{issues:?}");
        assert_eq!(add_missing_verses("\\v 1 a \\v 2 b", &[1, 2]), "\\v 1 a \\v 2 b");
        assert_eq!(add_missing_verses("texto", &[]), "texto");
    }

    #[test]
    fn helps_json_round_trip() {
        let items = vec![
            HelpItem { title: "Ref".into(), body: "Nota".into() },
            HelpItem { title: "".into(), body: "".into() },
        ];
        let s = serialize_helps(&items);
        assert!(s.starts_with("[\n\t{"));
        let back = parse_helps(&s);
        assert_eq!(back.len(), 1);
        assert!(helps_complete(&back));
        assert!(!helps_complete(&[HelpItem { title: "x".into(), body: "".into() }]));
        assert_eq!(serialize_helps(&[]), "");
        assert_eq!(parse_helps("texto plano")[0].body, "texto plano");
        assert!(parse_helps("").is_empty());
    }
}
