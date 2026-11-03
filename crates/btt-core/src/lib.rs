//! # btt-core
//!
//! Núcleo de BTT Writer reescrito en Rust. No depende de ninguna interfaz
//! gráfica: la app Tauri (escritorio y móvil) lo envuelve con comandos.
//!
//! Módulos:
//! * [`manifest`]  — `manifest.json` de una traducción objetivo.
//! * [`project`]   — proyectos en disco (capítulos y chunks).
//! * [`usfm`]      — análisis/generación USFM, división en chunks.
//! * [`usx`]       — análisis de fragmentos USX de los textos fuente.
//! * [`rc`]        — contenedores de recursos `.tsrc` (fuentes, tN, tQ, tW).
//! * [`library`]   — `index.sqlite`: idiomas, libros, recursos, catálogo.
//! * [`gitrepo`]   — historial automático y sincronización git.
//! * [`conflicts`] — marcadores de conflicto de git.
//! * [`tstudio`]   — respaldo/importación `.tstudio`.

pub mod conflicts;
pub mod error;
pub mod gitrepo;
pub mod library;
pub mod manifest;
pub mod project;
pub mod rc;
pub mod tstudio;
pub mod usfm;
pub mod usx;

pub use error::{Error, Result};

/// Versión del núcleo (coincide con la del paquete).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
