//! Chunks retenidos durante el trabajo en equipo.
//!
//! Mientras alguien edita un chunk, su archivo queda fuera de todos los commits
//! (y por tanto no se sube) hasta que lo cierra con "Listo". Así los demás no
//! reciben el texto a medio escribir ni se abre un conflicto en cada pasada de
//! sincronización. Si el servidor cambió ese chunk mientras estaba retenido, al
//! cerrarlo se deja un único conflicto para resolverlo en la app.
//!
//! El registro vive en `.git/btt-holds.json`: es de este dispositivo y no viaja.

use crate::gitrepo;
use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

const FILE: &str = "btt-holds.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Hold {
    /// Blob del archivo en HEAD cuando empezó la edición (`None` si no existía).
    pub base: Option<String>,
    pub since: u64,
    pub touched: u64,
}

fn file(root: &Path) -> std::path::PathBuf {
    root.join(".git").join(FILE)
}

/// Retenciones vigentes: ruta relativa del chunk (`01/05.txt`) → datos.
pub fn list(root: &Path) -> BTreeMap<String, Hold> {
    std::fs::read_to_string(file(root)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

/// Rutas retenidas (para excluirlas de los commits).
pub fn paths(root: &Path) -> Vec<String> {
    list(root).into_keys().collect()
}

fn save(root: &Path, holds: &BTreeMap<String, Hold>) -> Result<()> {
    let f = file(root);
    if holds.is_empty() {
        if f.is_file() {
            std::fs::remove_file(&f)?;
        }
        return Ok(());
    }
    if let Some(parent) = f.parent() {
        std::fs::create_dir_all(parent)?;
    }
    crate::project::atomic_write(&f, &serde_json::to_string_pretty(holds)?)
}

/// Ruta del archivo de un chunk `capítulo/chunk`.
pub fn chunk_path(key: &str) -> String {
    format!("{key}.txt")
}

/// Empieza (o renueva) la retención de `path`. La base es la versión en HEAD en ese
/// momento, que es la que vieron los demás.
pub fn start(root: &Path, path: &str, now: u64) -> Result<()> {
    let mut holds = list(root);
    match holds.get_mut(path) {
        Some(h) => h.touched = now,
        None => {
            let base = gitrepo::head_blob_id(root, path);
            holds.insert(path.to_string(), Hold { base, since: now, touched: now });
        }
    }
    save(root, &holds)
}

/// Resultado de cerrar una retención.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Closed {
    /// El archivo se queda como está (nadie más lo tocó, o no hubo cambios locales).
    Unchanged,
    /// No se editó localmente y el servidor sí lo cambió: se adoptó la versión del servidor.
    TookTheirs,
    /// Ambos lo cambiaron: el archivo quedó con las dos versiones para elegir.
    Conflict,
}

fn norm(s: &str) -> String {
    s.replace("\r\n", "\n").trim().to_string()
}

/// Cierra la retención de `path` y compara con lo que llegó del servidor mientras
/// tanto. Toma el candado del proyecto para no cruzarse con una escritura del editor.
pub fn close(root: &Path, path: &str) -> Result<Closed> {
    let _guard = crate::project::lock_project(root);
    let mut holds = list(root);
    let Some(hold) = holds.remove(path) else { return Ok(Closed::Unchanged) };
    save(root, &holds)?;
    let head_id = gitrepo::head_blob_id(root, path);
    if head_id == hold.base {
        // El servidor no lo tocó: se sube tal cual en el siguiente commit.
        return Ok(Closed::Unchanged);
    }
    let full = root.join(path);
    let ours = std::fs::read_to_string(&full).ok();
    let theirs = head_id.as_deref().and_then(|id| gitrepo::blob_by_id(root, id));
    let base = hold.base.as_deref().and_then(|id| gitrepo::blob_by_id(root, id));
    let local_changed = match (&ours, &base) {
        (None, _) => false,
        (Some(o), None) => !norm(o).is_empty(),
        (Some(o), Some(b)) => norm(o) != norm(b),
    };
    match (local_changed, theirs) {
        (false, Some(t)) => {
            if ours.as_deref().map(norm) != Some(norm(&t)) {
                crate::project::atomic_write(&full, &t)?;
                return Ok(Closed::TookTheirs);
            }
            Ok(Closed::Unchanged)
        }
        (false, None) => Ok(Closed::Unchanged),
        (true, Some(t)) => {
            let o = ours.unwrap_or_default();
            if norm(&o) == norm(&t) {
                return Ok(Closed::Unchanged);
            }
            let text = format!(
                "{}HEAD\n{}\n{}\n{}\n{}servidor\n",
                crate::conflicts::START,
                o.trim_end_matches(['\n', '\r']),
                crate::conflicts::SEP,
                t.trim_end_matches(['\n', '\r']),
                crate::conflicts::END
            );
            crate::project::atomic_write(&full, &text)?;
            Ok(Closed::Conflict)
        }
        // El servidor lo borró y aquí se escribió: se conserva lo local.
        (true, None) => Ok(Closed::Unchanged),
    }
}

/// Cierra todas las retenciones salvo `except` (el chunk que sigue en edición).
/// Devuelve los archivos que cambiaron en disco (versión del servidor o conflicto).
pub fn close_all(root: &Path, except: Option<&str>) -> Vec<String> {
    let mut changed = vec![];
    for path in paths(root) {
        if Some(path.as_str()) == except {
            continue;
        }
        match close(root, &path) {
            Ok(Closed::Unchanged) => {}
            Ok(_) => changed.push(path),
            Err(_) => {}
        }
    }
    changed
}

/// Olvida todas las retenciones sin comprobar nada (al desactivar el equipo tras cerrarlas).
pub fn clear(root: &Path) -> Result<()> {
    save(root, &BTreeMap::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gitrepo::{commit_all, Author, AUTOSAVE_MESSAGE};

    fn repo(tmp: &Path) -> std::path::PathBuf {
        let root = tmp.join("p");
        std::fs::create_dir_all(root.join("01")).unwrap();
        std::fs::write(root.join("01/01.txt"), "base").unwrap();
        commit_all(&root, &Author::new("a", "a@x"), AUTOSAVE_MESSAGE).unwrap();
        root
    }

    #[test]
    fn un_chunk_retenido_no_entra_en_los_commits() {
        let tmp = tempfile::tempdir().unwrap();
        let root = repo(tmp.path());
        start(&root, "01/01.txt", 10).unwrap();
        std::fs::write(root.join("01/01.txt"), "escribiendo").unwrap();
        std::fs::write(root.join("01/02.txt"), "otro").unwrap();
        let c = commit_all(&root, &Author::new("a", "a@x"), AUTOSAVE_MESSAGE).unwrap().unwrap();
        assert_eq!(gitrepo::file_at(&root, &c, "01/01.txt").unwrap(), "base");
        assert_eq!(gitrepo::file_at(&root, &c, "01/02.txt").unwrap(), "otro");
        // Al cerrar sin que el servidor lo tocara, el siguiente commit lo incluye.
        assert_eq!(close(&root, "01/01.txt").unwrap(), Closed::Unchanged);
        let c = commit_all(&root, &Author::new("a", "a@x"), AUTOSAVE_MESSAGE).unwrap().unwrap();
        assert_eq!(gitrepo::file_at(&root, &c, "01/01.txt").unwrap(), "escribiendo");
        assert!(list(&root).is_empty());
    }

    #[test]
    fn al_cerrar_se_compara_con_lo_que_cambio_en_head() {
        let tmp = tempfile::tempdir().unwrap();
        let root = repo(tmp.path());
        start(&root, "01/01.txt", 10).unwrap();
        // "Llega" otra versión a HEAD (como haría una fusión) mientras está retenido.
        let holds = list(&root);
        clear(&root).unwrap();
        std::fs::write(root.join("01/01.txt"), "del servidor").unwrap();
        commit_all(&root, &Author::new("b", "b@x"), AUTOSAVE_MESSAGE).unwrap();
        save(&root, &holds).unwrap();

        // Sin cambios locales: se adopta la del servidor.
        std::fs::write(root.join("01/01.txt"), "base").unwrap();
        let h = holds.clone();
        assert_eq!(close(&root, "01/01.txt").unwrap(), Closed::TookTheirs);
        assert_eq!(std::fs::read_to_string(root.join("01/01.txt")).unwrap(), "del servidor");

        // Con cambios locales: conflicto con las dos versiones.
        save(&root, &h).unwrap();
        std::fs::write(root.join("01/01.txt"), "lo mío").unwrap();
        assert_eq!(close(&root, "01/01.txt").unwrap(), Closed::Conflict);
        let text = std::fs::read_to_string(root.join("01/01.txt")).unwrap();
        let options: Vec<String> = crate::conflicts::parse(&text).iter().map(|o| o.trim().to_string()).collect();
        assert_eq!(options, vec!["lo mío", "del servidor"]);
        assert!(list(&root).is_empty());
    }
}
