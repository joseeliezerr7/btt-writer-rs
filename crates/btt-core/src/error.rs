//! Tipo de error unificado del núcleo.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("E/S: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON inválido: {0}")]
    Json(#[from] serde_json::Error),
    #[error("YAML inválido: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("Base de datos: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Archivo zip: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("Git: {0}")]
    Git(#[from] git2::Error),
    #[error("Red: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Proyecto no encontrado: {0}")]
    ProjectNotFound(String),
    #[error("El proyecto ya existe: {0}")]
    ProjectExists(String),
    #[error("Manifest inválido: {0}")]
    InvalidManifest(String),
    #[error("USFM inválido: {0}")]
    InvalidUsfm(String),
    #[error("Contenedor de recursos inválido: {0}")]
    InvalidContainer(String),
    #[error("Archivo .tstudio inválido: {0}")]
    InvalidTstudio(String),
    #[error("Ruta no permitida (zip slip): {0}")]
    UnsafePath(String),
    #[error("Error del servidor ({status}): {body}")]
    Server { status: u16, body: String },
    #[error("Autenticación requerida")]
    Unauthorized,
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<String> for Error {
    fn from(s: String) -> Self {
        Error::Other(s)
    }
}
impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Error::Other(s.to_string())
    }
}
