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

fn is_false(b: &bool) -> bool {
    !*b
}

/// Una confirmación, o su retirada (`undone`): se guarda igual, con fecha, para
/// que al fusionar dos copias gane siempre lo más reciente.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Confirmation {
    pub by: String,
    pub at: u64,
    #[serde(default, skip_serializing_if = "is_false")]
    pub undone: bool,
}

fn active(c: &Option<Confirmation>) -> bool {
    c.as_ref().map(|c| !c.undone).unwrap_or(false)
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
        let t = active(&self.translator);
        let c = active(&self.checker);
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
    /// Borrada por su autor: se conserva como lápida para que no vuelva al fusionar.
    #[serde(default, skip_serializing_if = "is_false")]
    pub deleted: bool,
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

/// Una revisión que espera al revisor asignado.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PendingReview {
    pub chapter: String,
    pub step: String,
    /// Quien terminó su parte y espera.
    pub translator: String,
    pub since: u64,
}

fn same_user(a: &str, b: &str) -> bool {
    !a.is_empty() && a.eq_ignore_ascii_case(b)
}

impl MastFile {
    /// Pasos con revisor en los que `user` es el revisor del capítulo, quien traduce
    /// ya confirmó y la revisión sigue pendiente. En orden de capítulo.
    pub fn pending_for(&self, user: &str) -> Vec<PendingReview> {
        let mut out = vec![];
        for (chapter, ch) in &self.chapters {
            if !same_user(&ch.checker, user) {
                continue;
            }
            for step in STEPS.iter().filter(|s| needs_checker(s)) {
                if let Some(st) = ch.steps.get(*step) {
                    if active(&st.translator) && !active(&st.checker) {
                        let t = st.translator.as_ref().unwrap();
                        out.push(PendingReview { chapter: chapter.clone(), step: step.to_string(), translator: t.by.clone(), since: t.at });
                    }
                }
            }
        }
        out
    }
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
    // Nunca se retrocede en el tiempo: una confirmación recibida del equipo con
    // fecha posterior (reloj adelantado) no se pisa con una más antigua.
    let at = slot.as_ref().map(|c| c.at + 1).unwrap_or(0).max(now);
    // El revisor del capítulo solo se asigna a propósito (`set_checker`): quien confirma
    // la revisión sin estar asignado (trabajo sin equipo) no queda atado al papel de revisor.
    *slot = Some(Confirmation { by: user.to_string(), at, undone: !on });
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
        let list = f.chapters.entry(chapter.to_string()).or_default().notes.entry(chunk.to_string()).or_default();
        // (autor, fecha) identifica la nota: dos seguidas en el mismo segundo no comparten clave.
        let at = list.iter().filter(|n| n.by == user).map(|n| n.at + 1).max().unwrap_or(0).max(now);
        list.push(Note { by: user.to_string(), at, step: step.to_string(), text: text.to_string(), deleted: false });
        save(root, &f)?;
    }
    Ok(f)
}

/// Borra una nota propia (misma persona y misma fecha). Queda como lápida, sin
/// texto, para que la fusión con otra copia no la resucite.
pub fn remove_note(root: &Path, chapter: &str, chunk: &str, user: &str, at: u64) -> Result<MastFile> {
    let _guard = crate::project::lock_project(root);
    let mut f = load(root);
    if let Some(list) = f.chapters.get_mut(chapter).and_then(|ch| ch.notes.get_mut(chunk)) {
        if let Some(n) = list.iter_mut().find(|n| n.by == user && n.at == at && !n.deleted) {
            n.deleted = true;
            n.text.clear();
        }
    }
    save(root, &f)?;
    Ok(f)
}

/// Notas visibles (sin las borradas) de un chunk.
pub fn visible_notes(list: &[Note]) -> Vec<&Note> {
    list.iter().filter(|n| !n.deleted).collect()
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
                match list.iter_mut().find(|m| m.by == n.by && m.at == n.at) {
                    // La misma nota en las dos copias: si alguna la borró, borrada queda.
                    Some(m) => {
                        if n.deleted && !m.deleted {
                            *m = n.clone();
                        }
                    }
                    None => list.push(n.clone()),
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
        assert_eq!(ch.steps["peer_edit"].checker.as_ref().unwrap().by, "beto");
        assert!(ch.checker.is_empty(), "confirmar como revisor no asigna el papel de revisor");

        // Deshacer deja una lápida con fecha: el paso vuelve a estar pendiente.
        let f = confirm(root, "01", "consume", Role::Translator, "ana", false, 40).unwrap();
        let c = f.chapters["01"].steps["consume"].translator.clone().unwrap();
        assert!(c.undone && c.at == 40);
        assert_eq!(f.chapters["01"].steps["consume"].status("consume"), "not_started");
        assert_eq!(f.chapters["01"].current_step(), "consume");
        // Y esa retirada gana al fusionar con una copia que aún tenía la confirmación vieja.
        let mut old = MastFile::default();
        old.chapters.entry("01".into()).or_default().steps.entry("consume".into()).or_default().translator = Some(Confirmation { by: "ana".into(), at: 10, undone: false });
        assert_eq!(merge(&old, &f).chapters["01"].steps["consume"].status("consume"), "not_started");
        assert_eq!(merge(&f, &old).chapters["01"].steps["consume"].status("consume"), "not_started");
        // Confirmar de nuevo con un reloj atrasado no retrocede la fecha.
        let f = confirm(root, "01", "consume", Role::Translator, "ana", true, 5).unwrap();
        assert!(f.chapters["01"].steps["consume"].translator.as_ref().unwrap().at > 40);
    }

    #[test]
    fn el_revisor_asignado_ve_sus_revisiones_pendientes() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        set_checker(root, "01", "Juan_H").unwrap();
        confirm(root, "01", "peer_edit", Role::Translator, "ana", true, 10).unwrap();
        let f = confirm(root, "01", "key_words", Role::Translator, "ana", true, 11).unwrap();
        // Sin distinguir mayúsculas (los nombres de WACS llegan como los escribió cada uno).
        let p = f.pending_for("juan_h");
        assert_eq!(p.iter().map(|p| p.step.as_str()).collect::<Vec<_>>(), vec!["peer_edit", "key_words"]);
        assert_eq!(p[0].translator, "ana");
        assert!(f.pending_for("ana").is_empty());
        // Al revisar, desaparece de la lista.
        let f = confirm(root, "01", "peer_edit", Role::Checker, "Juan_H", true, 20).unwrap();
        assert_eq!(f.pending_for("Juan_H").len(), 1);
    }

    #[test]
    fn las_notas_se_agregan_y_se_quitan() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        add_note(root, "01", "05", "peer_edit", "beto", "  falta 'en verdad'  ", 5).unwrap();
        let f = add_note(root, "01", "05", "peer_edit", "beto", "", 6).unwrap();
        assert_eq!(f.chapters["01"].notes["05"].len(), 1, "una nota vacía no se guarda");
        assert_eq!(f.chapters["01"].notes["05"][0].text, "falta 'en verdad'");
        // Dos notas en el mismo segundo no comparten clave.
        let f = add_note(root, "01", "05", "peer_edit", "beto", "otra", 5).unwrap();
        assert_eq!(f.chapters["01"].notes["05"][1].at, 6);
        let f = remove_note(root, "01", "05", "beto", 5).unwrap();
        let notes = &f.chapters["01"].notes["05"];
        assert_eq!(visible_notes(notes).iter().map(|n| n.text.as_str()).collect::<Vec<_>>(), vec!["otra"]);
        assert!(notes[0].deleted && notes[0].text.is_empty(), "queda la lápida sin texto");
        // La copia que todavía tenía la nota no la resucita al fusionar.
        let mut old = MastFile::default();
        old.chapters.entry("01".into()).or_default().notes.entry("05".into()).or_default().push(Note { by: "beto".into(), at: 5, step: "peer_edit".into(), text: "falta 'en verdad'".into(), deleted: false });
        assert_eq!(visible_notes(&merge(&old, &f).chapters["01"].notes["05"]).len(), 1);
    }

    #[test]
    fn dos_copias_se_unen_sin_perder_confirmaciones_ni_notas() {
        let mut a = MastFile::default();
        let mut b = MastFile::default();
        let ca = a.chapters.entry("01".into()).or_default();
        ca.steps.entry("peer_edit".into()).or_default().translator = Some(Confirmation { by: "ana".into(), at: 1, undone: false });
        ca.notes.entry("05".into()).or_default().push(Note { by: "ana".into(), at: 1, step: "peer_edit".into(), text: "x".into(), deleted: false });
        let cb = b.chapters.entry("01".into()).or_default();
        cb.checker = "beto".into();
        cb.steps.entry("peer_edit".into()).or_default().checker = Some(Confirmation { by: "beto".into(), at: 2, undone: false });
        cb.notes.entry("05".into()).or_default().push(Note { by: "beto".into(), at: 2, step: "peer_edit".into(), text: "y".into(), deleted: false });
        cb.notes.entry("05".into()).or_default().push(Note { by: "ana".into(), at: 1, step: "peer_edit".into(), text: "x".into(), deleted: false });
        let m = merge(&a, &b);
        let ch = &m.chapters["01"];
        assert_eq!(ch.steps["peer_edit"].status("peer_edit"), "passed");
        assert_eq!(ch.checker, "beto");
        assert_eq!(ch.notes["05"].iter().map(|n| n.text.as_str()).collect::<Vec<_>>(), vec!["x", "y"]);
        let json = merge_json(&serde_json::to_string(&a).unwrap(), "no es json");
        assert!(json.contains("\"ana\""));
    }
}
