//! Marcadores de conflicto de git (`<<<<<<< HEAD` / `=======` / `>>>>>>>`).
//!
//! `parse` divide un texto con conflictos (incluso anidados o de varias vías)
//! en la lista de versiones completas entre las que el usuario puede elegir,
//! igual que hacía BTT Writer 2.0 (`ParseMergeConflicts`).

pub const START: &str = "<<<<<<< ";
pub const SEP: &str = "=======";
pub const END: &str = ">>>>>>> ";

/// Marca "sin salto de línea aquí" al separar un marcador que estaba en medio de
/// una línea (p. ej. `\c 23 <<<<<<< HEAD`, como deja git en el primer chunk de un
/// capítulo).
const JOIN: char = '\u{1}';

pub fn has_conflicts(text: &str) -> bool {
    text.lines().any(|l| l.starts_with(START) || l.contains(" <<<<<<< "))
}

/// Pone cada marcador en su propia línea; lo que lo precedía queda en una línea
/// terminada en `JOIN` para volver a unirlo después sin salto.
fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for (i, line) in text.replace("\r\n", "\n").split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let mid = line.find(" <<<<<<< ").or_else(|| line.find(" >>>>>>> ")).map(|p| p + 1);
        match mid {
            Some(p) if p > 0 => {
                out.push_str(&line[..p]);
                out.push(JOIN);
                out.push('\n');
                out.push_str(&line[p..]);
            }
            _ => out.push_str(line),
        }
    }
    out
}

/// Devuelve las versiones alternativas del texto. Sin conflictos → `[text]`.
pub fn parse(text: &str) -> Vec<String> {
    let text = normalize(text);
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out: Vec<String> = expand(&lines).into_iter().map(rejoin).collect();
    dedup(&mut out);
    out
}

/// Deshace los cortes de `normalize`. Si el texto previo al marcador se repite al
/// inicio de la versión (`\c 23 ` + `\c 23 \v 1…`), se deja una sola vez.
fn rejoin(s: String) -> String {
    if !s.contains(JOIN) {
        return s;
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s.as_str();
    while let Some(p) = rest.find(JOIN) {
        let before = &rest[..p];
        let after = rest[p + JOIN.len_utf8()..].strip_prefix('\n').unwrap_or(&rest[p + JOIN.len_utf8()..]);
        // `before` es lo que había antes del marcador en esa línea (desde su inicio).
        let line_start = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let prefix = &before[line_start..];
        let dup = !prefix.trim().is_empty() && after.trim_start().starts_with(prefix.trim_end());
        if dup {
            out.push_str(&before[..line_start]);
        } else {
            out.push_str(before);
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

/// Expande recursivamente. Trabaja con líneas para no romper marcadores.
fn expand(lines: &[&str]) -> Vec<String> {
    let Some(start) = lines.iter().position(|l| l.starts_with(START)) else {
        return vec![lines.join("\n")];
    };
    // Buscar el fin emparejado y los separadores de nivel 0.
    let mut depth = 0usize;
    let mut seps = vec![];
    let mut end = None;
    for (i, l) in lines.iter().enumerate().skip(start) {
        if l.starts_with(START) {
            depth += 1;
        } else if l.starts_with(END) {
            depth -= 1;
            if depth == 0 {
                end = Some(i);
                break;
            }
        } else if *l == SEP && depth == 1 {
            seps.push(i);
        }
    }
    let Some(end) = end else {
        // Conflicto sin cerrar: se devuelve tal cual.
        return vec![lines.join("\n")];
    };
    let prefix = &lines[..start];
    let suffix = &lines[end + 1..];

    // Secciones del conflicto actual (puede haber varias `=======`).
    let mut bounds = vec![start + 1];
    for s in &seps {
        bounds.push(*s);
    }
    bounds.push(end);
    let mut variants: Vec<String> = vec![];
    for w in bounds.windows(2) {
        let (a, b) = (w[0], w[1]);
        let a = if lines.get(a) == Some(&SEP) { a + 1 } else { a };
        let section = &lines[a..b];
        for v in expand(section) {
            variants.push(v);
        }
    }
    dedup(&mut variants);

    let tails = expand(suffix);
    let n = variants.len().max(tails.len());
    let mut out = vec![];
    for k in 0..n {
        let v = &variants[k.min(variants.len() - 1)];
        let t = &tails[k.min(tails.len() - 1)];
        let mut s = String::new();
        if !prefix.is_empty() {
            s.push_str(&prefix.join("\n"));
            s.push('\n');
        }
        s.push_str(v);
        if !suffix.is_empty() {
            s.push('\n');
            s.push_str(t);
        }
        out.push(s);
    }
    out
}

fn dedup(v: &mut Vec<String>) {
    let mut seen = std::collections::HashSet::new();
    v.retain(|s| seen.insert(s.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn marker_after_chapter_marker_on_same_line() {
        // Forma real dejada por git en el primer chunk de un capítulo (Hechos 23, pech).
        let raw = "\\c 23 <<<<<<< HEAD\n\\c 23 \\v 1 Pablo local \\v 2 b\n=======\n\\c 23 \\v 1 Pablo servidor \\v 2 b\n>>>>>>> 316c2de191a84615800cf7a11d416841e49c6643\n";
        assert!(has_conflicts(raw));
        let got = parse(raw);
        assert_eq!(got, vec!["\\c 23 \\v 1 Pablo local \\v 2 b\n", "\\c 23 \\v 1 Pablo servidor \\v 2 b\n"]);
        for v in &got {
            assert!(!has_conflicts(v));
        }
        // Prefijo que no se repite: se une sin salto de línea.
        let raw2 = "intro <<<<<<< HEAD\nuno\n=======\ndos\n>>>>>>> x";
        assert_eq!(parse(raw2), vec!["intro uno", "intro dos"]);
    }

    #[test]
    fn simple_two_way() {
        let raw = "a\n<<<<<<< HEAD\nlocal\n=======\nremote\n>>>>>>> refs/heads/new\nz";
        assert!(has_conflicts(raw));
        assert_eq!(parse(raw), vec!["a\nlocal\nz", "a\nremote\nz"]);
        assert_eq!(parse("sin conflicto"), vec!["sin conflicto"]);
        assert!(!has_conflicts("sin conflicto"));
    }

    #[test]
    fn matches_all_original_fixtures() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/merge");
        let mut checked = 0;
        for entry in std::fs::read_dir(&dir).unwrap() {
            let p = entry.unwrap().path();
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            let Some(base) = name.strip_suffix("_raw.data") else { continue };
            let raw = std::fs::read_to_string(&p).unwrap();
            let mut expected = vec![];
            for i in 1..=9 {
                let part = dir.join(format!("{base}_part{i}.data"));
                if part.is_file() {
                    expected.push(std::fs::read_to_string(part).unwrap().replace("\r\n", "\n").trim_end().to_string());
                }
            }
            let mut got: Vec<String> = parse(&raw).into_iter().map(|s| s.trim_end().to_string()).collect();
            let mut exp = expected.clone();
            got.sort();
            exp.sort();
            assert_eq!(got, exp, "fixture {base}");
            checked += 1;
        }
        assert!(checked >= 8, "se revisaron {checked} fixtures");
    }
}
