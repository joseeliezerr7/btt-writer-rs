//! Progreso del método MAST (los 8 pasos de Wycliffe Associates) por capítulo.
//!
//! Pasos: Consume, Verbalize, Chunk, Blind draft, Self-edit, Peer-edit, Key word
//! check, Verse-by-verse check, y al final la colocación de marcas de versículo
//! (como en V-MAST). Cada paso se da por hecho cuando quien traduce lo confirma;
//! los tres pasos con revisor necesitan además la confirmación del revisor.
//!
//! Vive en `mast.json` dentro del proyecto: viaja por git y, si dos copias lo
//! cambian a la vez, se unen (`merge_json`) sin dejar marcadores de conflicto.

use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

pub const MAST_FILE: &str = "mast.json";

/// Los pasos en orden. Los ids son los que usa la interfaz y el archivo.
pub const STEPS: [&str; 9] = ["consume", "verbalize", "chunk", "blind_draft", "self_edit", "peer_edit", "key_words", "verse_check", "markers"];

/// Pasos que hace otra persona junto con quien traduce.
pub fn needs_checker(step: &str) -> bool {
    matches!(step, "peer_edit" | "key_words" | "verse_check")
}

pub fn is_step(step: &str) -> bool {
    STEPS.contains(&step)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Confirmation {
    pub by: String,
    pub at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct StepState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translator: Option<Confirmation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checker: Option<Confirmation>,
}

impl StepState {
    /// `passed`, `waiting` (falta una de las dos confirmaciones) o `not_started`.
    pub fn status(&self, step: &str) -> &'static str {
        let t = self.translator.is_some();
        let c = self.checker.is_some();
        if t && (c || !needs_checker(step)) {
            "passed"
        } else if t || c {
            "waiting"
        } else {
            "not_started"
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Note {
    pub by: String,
    pub at: u64,
    #[serde(default)]
    pub step: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ChapterProgress {
    #[serde(default)]
    pub steps: BTreeMap<String, StepState>,
    /// Revisor asignado al capítulo (nombre), si se indicó.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub checker: String,
    /// Notas por chunk (`05` → notas), de los pasos con revisor.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub notes: BTreeMap<String, Vec<Note>>,
}

impl ChapterProgress {
    /// Primer paso que no está `passed` (o el último si todo está hecho).
    pub fn current_step(&self) -> &'static str {
        STEPS
            .iter()
            .copied()
            .find(|s| self.steps.get(*s).map(|st| st.status(s) != "passed").unwrap_or(true))
            .unwrap_or("markers")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct MastFile {
    #[serde(default)]
    pub chapters: BTreeMap<String, ChapterProgress>,
}

fn parse(text: &str) -> Option<MastFile> {
    serde_json::from_str(text).ok()
}

pub fn load(root: &Path) -> MastFile {
    std::fs::read_to_string(root.join(MAST_FILE)).ok().and_then(|s| parse(&s)).unwrap_or_default()
}

fn save(root: &Path, f: &MastFile) -> Result<()> {
    let mut json = serde_json::to_string_pretty(f)?;
    json.push('\n');
    crate::project::atomic_write(&root.join(MAST_FILE), &json)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Translator,
    Checker,
}

/// Confirma (`on`) o retira la confirmación de un paso para un rol.
pub fn confirm(root: &Path, chapter: &str, step: &str, role: Role, user: &str, on: bool, now: u64) -> Result<MastFile> {
    let _guard = crate::project::lock_project(root);
    let mut f = load(root);
    let ch = f.chapters.entry(chapter.to_string()).or_default();
    let st = ch.steps.entry(step.to_string()).or_default();
    let slot = match role {
        Role::Translator => &mut st.translator,
        Role::Checker => &mut st.checker,
    };
    *slot = if on { Some(Confirmation { by: user.to_string(), at: now }) } else { None };
    if role == Role::Checker && on && ch.checker.is_empty() {
        ch.checker = user.to_string();
    }
    if st.translator.is_none() && st.checker.is_none() {
        ch.steps.remove(step);
    }
    save(root, &f)?;
    Ok(f)
}

pub fn set_checker(root: &Path, chapter: &str, checker: &str) -> Result<MastFile> {
    let _guard = crate::project::lock_project(root);
    let mut f = load(root);
    f.chapters.entry(chapter.to_string()).or_default().checker = checker.trim().to_string();
    save(root, &f)?;
    Ok(f)
}

pub fn add_note(root: &Path, chapter: &str, chunk: &str, step: &str, user: &str, text: &str, now: u64) -> Result<MastFile> {
    let _guard = crate::project::lock_project(root);
    let mut f = load(root);
    let text = text.trim();
    if !text.is_empty() {
        f.chapters
            .entry(chapter.to_string())
            .or_default()
            .notes
            .entry(chunk.to_string())
            .or_default()
            .push(Note { by: user.to_string(), at: now, step: step.to_string(), text: text.to_string() });
        save(root, &f)?;
    }
    Ok(f)
}

/// Quita una nota propia (misma persona y misma fecha).
pub fn remove_note(root: &Path, chapter: &str, chunk: &str, user: &str, at: u64) -> Result<MastFile> {
    let _guard = crate::project::lock_project(root);
    let mut f = load(root);
    if let Some(ch) = f.chapters.get_mut(chapter) {
        if let Some(list) = ch.notes.get_mut(chunk) {
            list.retain(|n| !(n.by == user && n.at == at));
            if list.is_empty() {
                ch.notes.remove(chunk);
            }
        }
    }
    save(root, &f)?;
    Ok(f)
}

fn later(a: &Option<Confirmation>, b: &Option<Confirmation>) -> Option<Confirmation> {
    match (a, b) {
        (Some(x), Some(y)) => Some(if y.at > x.at { y.clone() } else { x.clone() }),
        (Some(x), None) => Some(x.clone()),
        (None, Some(y)) => Some(y.clone()),
        (None, None) => None,
    }
}

/// Unión de dos versiones: una confirmación hecha en cualquiera de las dos vale,
/// y las notas se juntan sin repetir.
pub fn merge(a: &MastFile, b: &MastFile) -> MastFile {
    let mut out = a.clone();
    for (chapter, bc) in &b.chapters {
        let oc = out.chapters.entry(chapter.clone()).or_default();
        if oc.checker.is_empty() {
            oc.checker = bc.checker.clone();
        }
        for (step, bs) in &bc.steps {
            let os = oc.steps.entry(step.clone()).or_default();
            os.translator = later(&os.translator, &bs.translator);
            os.checker = later(&os.checker, &bs.checker);
        }
        for (chunk, notes) in &bc.notes {
            let list = oc.notes.entry(chunk.clone()).or_default();
            for n in notes {
                if !list.contains(n) {
                    list.push(n.clone());
                }
            }
            list.sort_by_key(|n| n.at);
        }
    }
    out
}

/// Fusión de los dos textos de `mast.json` en un conflicto de git.
pub fn merge_json(ours: &str, theirs: &str) -> String {
    let a = parse(ours).unwrap_or_default();
    let b = parse(theirs).unwrap_or_default();
    let mut json = serde_json::to_string_pretty(&merge(&a, &b)).unwrap_or_else(|_| "{}".into());
    json.push('\n');
    json
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_pasos_con_revisor_esperan_las_dos_confirmaciones() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let f = confirm(root, "01", "consume", Role::Translator, "ana", true, 10).unwrap();
        let ch = &f.chapters["01"];
        assert_eq!(ch.steps["consume"].status("consume"), "passed");
        assert_eq!(ch.current_step(), "verbalize");

        let f = confirm(root, "01", "peer_edit", Role::Translator, "ana", true, 20).unwrap();
        assert_eq!(f.chapters["01"].steps["peer_edit"].status("peer_edit"), "waiting");
        let f = confirm(root, "01", "peer_edit", Role::Checker, "beto", true, 30).unwrap();
        let ch = &f.chapters["01"];
        assert_eq!(ch.steps["peer_edit"].status("peer_edit"), "passed");
        assert_eq!(ch.checker, "beto", "el primer revisor que confirma queda asignado");

        // Retirar la única confirmación borra el paso del archivo.
        let f = confirm(root, "01", "consume", Role::Translator, "ana", false, 40).unwrap();
        assert!(!f.chapters["01"].steps.contains_key("consume"));
        assert_eq!(f.chapters["01"].current_step(), "consume");
    }

    #[test]
    fn las_notas_se_agregan_y_se_quitan() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        add_note(root, "01", "05", "peer_edit", "beto", "  falta 'en verdad'  ", 5).unwrap();
        let f = add_note(root, "01", "05", "peer_edit", "beto", "", 6).unwrap();
        assert_eq!(f.chapters["01"].notes["05"].len(), 1, "una nota vacía no se guarda");
        assert_eq!(f.chapters["01"].notes["05"][0].text, "falta 'en verdad'");
        let f = remove_note(root, "01", "05", "beto", 5).unwrap();
        assert!(f.chapters["01"].notes.is_empty());
    }

    #[test]
    fn dos_copias_se_unen_sin_perder_confirmaciones_ni_notas() {
        let mut a = MastFile::default();
        let mut b = MastFile::default();
        let ca = a.chapters.entry("01".into()).or_default();
        ca.steps.entry("peer_edit".into()).or_default().translator = Some(Confirmation { by: "ana".into(), at: 1 });
        ca.notes.entry("05".into()).or_default().push(Note { by: "ana".into(), at: 1, step: "peer_edit".into(), text: "x".into() });
        let cb = b.chapters.entry("01".into()).or_default();
        cb.checker = "beto".into();
        cb.steps.entry("peer_edit".into()).or_default().checker = Some(Confirmation { by: "beto".into(), at: 2 });
        cb.notes.entry("05".into()).or_default().push(Note { by: "beto".into(), at: 2, step: "peer_edit".into(), text: "y".into() });
        cb.notes.entry("05".into()).or_default().push(Note { by: "ana".into(), at: 1, step: "peer_edit".into(), text: "x".into() });
        let m = merge(&a, &b);
        let ch = &m.chapters["01"];
        assert_eq!(ch.steps["peer_edit"].status("peer_edit"), "passed");
        assert_eq!(ch.checker, "beto");
        assert_eq!(ch.notes["05"].iter().map(|n| n.text.as_str()).collect::<Vec<_>>(), vec!["x", "y"]);
        let json = merge_json(&serde_json::to_string(&a).unwrap(), "no es json");
        assert!(json.contains("\"ana\""));
    }
}
