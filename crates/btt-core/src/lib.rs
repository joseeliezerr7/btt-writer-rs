//! # btt-core
//!
//! Núcleo de BTT Writer reescrito en Rust. No depende de ninguna interfaz
//! gráfica: la app Tauri (escritorio y móvil) lo envuelve con comandos.
//!
//! Módulos:
//! * [`manifest`]   — `manifest.json` de una traducción objetivo.
//! * [`project`]    — proyectos en disco (capítulos y chunks).
//! * [`usfm`]       — análisis/generación USFM, división en chunks.
//! * [`usx`]        — análisis de fragmentos USX de los textos fuente.
//! * [`rc`]         — contenedores de recursos `.tsrc` (fuentes, tN, tQ, tW).
//! * [`library`]    — `index.sqlite`: idiomas, libros, recursos, catálogo.
//! * [`sources`]    — descarga del catálogo y conversión a contenedores.
//! * [`gitrepo`]    — historial automático y sincronización git.
//! * [`conflicts`]  — marcadores de conflicto de git.
//! * [`tstudio`]    — respaldo/importación `.tstudio`.
//! * [`validation`] — validación de chunks y formato de ayudas.
//! * [`wacs`]       — API de WACS/DCS (Gitea).
//! * [`collab`]     — trabajo en equipo sobre un repositorio compartido.
//! * [`backup`]     — respaldos automáticos.
//! * [`print`]      — HTML imprimible para PDF.
//! * [`appdata`]    — rutas, ajustes, perfil y migración.

pub mod appdata;
pub mod authorship;
pub mod backup;
pub mod collab;
pub mod conflicts;
pub mod error;
pub mod gitrepo;
pub mod hold;
pub mod predict;
pub mod library;
pub mod manifest;
pub mod print;
pub mod project;
pub mod rc;
pub mod sources;
pub mod tstudio;
pub mod usfm;
pub mod usx;
pub mod validation;
pub mod wacs;

pub use error::{Error, Result};

/// Versión del núcleo (coincide con la del paquete).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Orden bíblico de los libros (slugs), para ordenar listas de proyectos.
pub const BIBLE_ORDER: [&str; 66] = [
    "gen", "exo", "lev", "num", "deu", "jos", "jdg", "rut", "1sa", "2sa", "1ki", "2ki", "1ch", "2ch", "ezr", "neh",
    "est", "job", "psa", "pro", "ecc", "sng", "isa", "jer", "lam", "ezk", "dan", "hos", "jol", "amo", "oba", "jon",
    "mic", "nam", "hab", "zep", "hag", "zec", "mal", "mat", "mrk", "luk", "jhn", "act", "rom", "1co", "2co", "gal",
    "eph", "php", "col", "1th", "2th", "1ti", "2ti", "tit", "phm", "heb", "jas", "1pe", "2pe", "1jn", "2jn", "3jn",
    "jud", "rev",
];

/// Posición de un libro en el orden bíblico (los no bíblicos van al final).
pub fn bible_sort_index(slug: &str) -> usize {
    BIBLE_ORDER.iter().position(|s| *s == slug).unwrap_or(usize::MAX)
}
