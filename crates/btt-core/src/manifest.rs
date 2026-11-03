//! `manifest.json` de una traducción objetivo (target translation).
//!
//! Compatible con `package_version` 8 que generan BTT Writer 1.6 (ts-desktop)
//! y 2.0 (ts-android). Los campos desconocidos se conservan en `extra`
//! para no perder información al reescribir el archivo.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const PACKAGE_VERSION: u32 = 8;
pub const GENERATOR_NAME: &str = "btt-writer-rs";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Generator {
    pub name: String,
    /// Puede venir como número en manifests antiguos (`"build": 19`).
    #[serde(deserialize_with = "string_or_number")]
    pub build: String,
}

/// Acepta cadena o número y devuelve siempre cadena.
pub fn string_or_number<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<String, D::Error> {
    let v = Value::deserialize(d)?;
    Ok(match v {
        Value::String(s) => s,
        Value::Number(n) => n.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    })
}

/// Acepta cadena o número y devuelve u64 (0 si no es numérico).
pub fn number_or_string_u64<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<u64, D::Error> {
    let v = Value::deserialize(d)?;
    Ok(match v {
        Value::Number(n) => n.as_u64().unwrap_or(0),
        Value::String(s) => s.trim().parse().unwrap_or(0),
        _ => 0,
    })
}

/// Nombre estándar de un recurso por su id.
pub fn resource_name(id: &str) -> &'static str {
    match id {
        "reg" => "Regular",
        "ulb" => "Unlocked Literal Bible",
        "udb" => "Unlocked Dynamic Bible",
        "obs" => "Open Bible Stories",
        "tn" => "translationNotes",
        "tq" => "translationQuestions",
        "tw" => "translationWords",
        _ => "",
    }
}

/// Nombre estándar de un tipo de proyecto por su id.
pub fn type_name(id: &str) -> &'static str {
    match id {
        "text" => "Text",
        "tn" => "Notes",
        "tq" => "Questions",
        "tw" => "Words",
        _ => "",
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TargetLanguage {
    pub id: String,
    pub name: String,
    #[serde(default = "ltr")]
    pub direction: String,
    #[serde(default)]
    pub anglicized_name: String,
    #[serde(default)]
    pub region: String,
    #[serde(default)]
    pub is_gateway_language: bool,
}

fn ltr() -> String {
    "ltr".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct IdName {
    pub id: String,
    #[serde(default)]
    pub name: String,
}

impl IdName {
    pub fn new(id: &str, name: &str) -> Self {
        IdName { id: id.into(), name: name.into() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SourceTranslation {
    pub language_id: String,
    pub resource_id: String,
    #[serde(default)]
    pub checking_level: String,
    #[serde(default, deserialize_with = "number_or_string_u64")]
    pub date_modified: u64,
    #[serde(default)]
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ParentDraft {
    #[serde(default)]
    pub resource_id: String,
    #[serde(default)]
    pub checking_entity: String,
    #[serde(default)]
    pub checking_level: String,
    #[serde(default)]
    pub comments: String,
    #[serde(default)]
    pub contributors: String,
    #[serde(default)]
    pub publish_date: u64,
    #[serde(default)]
    pub source_text: String,
    #[serde(default)]
    pub source_text_version: String,
    #[serde(default)]
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    pub package_version: u32,
    #[serde(default = "usfm")]
    pub format: String,
    pub generator: Generator,
    pub target_language: TargetLanguage,
    pub project: IdName,
    #[serde(rename = "type")]
    pub type_: IdName,
    pub resource: IdName,
    #[serde(default)]
    pub source_translations: Vec<SourceTranslation>,
    #[serde(default)]
    pub translators: Vec<String>,
    #[serde(default)]
    pub finished_chunks: Vec<String>,
    #[serde(default)]
    pub parent_draft: ParentDraft,
    /// Campos desconocidos que se conservan tal cual.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

fn usfm() -> String {
    "usfm".into()
}

impl Manifest {
    pub fn new(
        target_language: TargetLanguage,
        project: IdName,
        type_: IdName,
        resource: IdName,
        build: &str,
    ) -> Self {
        Manifest {
            package_version: PACKAGE_VERSION,
            format: if type_.id == "text" || type_.id == "obs" {
                "usfm".into()
            } else {
                "markdown".into()
            },
            generator: Generator { name: GENERATOR_NAME.into(), build: build.into() },
            target_language,
            project,
            type_,
            resource,
            source_translations: vec![],
            translators: vec![],
            finished_chunks: vec![],
            parent_draft: ParentDraft::default(),
            extra: BTreeMap::new(),
        }
    }

    /// Identificador único: `{lang}_{project}_{type}_{resource}` (p. ej. `cbt_3jn_text_reg`).
    /// Para tipos distintos de `text` el recurso se omite (`es_gen_tn`).
    pub fn unique_id(&self) -> String {
        unique_id(&self.target_language.id, &self.project.id, &self.type_.id, &self.resource.id)
    }

    pub fn is_chunk_finished(&self, chapter: &str, chunk: &str) -> bool {
        let key = format!("{chapter}-{chunk}");
        self.finished_chunks.iter().any(|c| c == &key)
    }

    pub fn set_chunk_finished(&mut self, chapter: &str, chunk: &str, finished: bool) {
        let key = format!("{chapter}-{chunk}");
        self.finished_chunks.retain(|c| c != &key);
        if finished {
            self.finished_chunks.push(key);
            self.finished_chunks.sort();
        }
    }

    pub fn add_translator(&mut self, name: &str) {
        let name = name.trim();
        if !name.is_empty() && !self.translators.iter().any(|t| t == name) {
            self.translators.push(name.to_string());
        }
    }

    pub fn add_source_translation(&mut self, st: SourceTranslation) {
        self.source_translations
            .retain(|s| !(s.language_id == st.language_id && s.resource_id == st.resource_id));
        self.source_translations.push(st);
    }

    pub fn remove_source_translation(&mut self, language_id: &str, resource_id: &str) {
        self.source_translations
            .retain(|s| !(s.language_id == language_id && s.resource_id == resource_id));
    }

    pub fn parse(json: &str) -> crate::Result<Self> {
        let mut m: Manifest = serde_json::from_str(json)
            .map_err(|e| crate::Error::InvalidManifest(e.to_string()))?;
        if m.target_language.id.is_empty() || m.project.id.is_empty() {
            return Err(crate::Error::InvalidManifest(
                "faltan target_language.id o project.id".into(),
            ));
        }
        m.migrate()?;
        Ok(m)
    }

    /// Lleva un manifest de versiones 5–7 al formato 8 (rellena nombres,
    /// normaliza y ordena). Versiones anteriores a 5 no se aceptan.
    pub fn migrate(&mut self) -> crate::Result<bool> {
        if self.package_version < 5 {
            return Err(crate::Error::InvalidManifest(format!(
                "package_version {} es demasiado antiguo; abre el proyecto en BTT Writer 1.6 para actualizarlo",
                self.package_version
            )));
        }
        let mut changed = false;
        if self.type_.id.is_empty() {
            self.type_.id = "text".into();
            changed = true;
        }
        if self.type_.name.is_empty() {
            self.type_.name = type_name(&self.type_.id).to_string();
            changed = true;
        }
        if self.type_.id == "text" && self.resource.id.is_empty() {
            self.resource.id = "reg".into();
            changed = true;
        }
        if self.resource.name.is_empty() && !self.resource.id.is_empty() {
            self.resource.name = resource_name(&self.resource.id).to_string();
            changed = true;
        }
        if self.format.is_empty() || self.format == "usx" || self.format == "default" {
            self.format = if self.type_.id == "text" && self.resource.id != "obs" { "usfm".into() } else { "markdown".into() };
            changed = true;
        }
        if self.target_language.direction.is_empty() {
            self.target_language.direction = "ltr".into();
            changed = true;
        }
        // Duplicados en source_translations (visto en archivos reales).
        let mut seen = std::collections::BTreeSet::new();
        let before = self.source_translations.len();
        self.source_translations.retain(|s| seen.insert((s.language_id.clone(), s.resource_id.clone())));
        changed |= before != self.source_translations.len();
        let mut fc = self.finished_chunks.clone();
        fc.sort();
        fc.dedup();
        if fc != self.finished_chunks {
            self.finished_chunks = fc;
            changed = true;
        }
        if self.package_version != PACKAGE_VERSION {
            self.package_version = PACKAGE_VERSION;
            changed = true;
        }
        Ok(changed)
    }

    /// Fusiona otro manifest (p. ej. el remoto): unión de traductores,
    /// chunks terminados y fuentes. Los demás campos se conservan de `self`.
    pub fn merge_from(&mut self, other: &Manifest) {
        for t in &other.translators {
            self.add_translator(t);
        }
        for c in &other.finished_chunks {
            if !self.finished_chunks.contains(c) {
                self.finished_chunks.push(c.clone());
            }
        }
        self.finished_chunks.sort();
        for s in &other.source_translations {
            if !self.source_translations.iter().any(|x| x.language_id == s.language_id && x.resource_id == s.resource_id) {
                self.source_translations.push(s.clone());
            }
        }
        if self.parent_draft == ParentDraft::default() && other.parent_draft != ParentDraft::default() {
            self.parent_draft = other.parent_draft.clone();
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("manifest serializable")
    }
}

pub fn unique_id(lang: &str, project: &str, type_: &str, resource: &str) -> String {
    if type_ == "text" {
        format!("{lang}_{project}_{type_}_{resource}")
    } else {
        format!("{lang}_{project}_{type_}")
    }
}

/// Descompone un id único (`cbt_3jn_text_reg` o `es_gen_tn`) en sus partes.
/// Devuelve (lang, project, type, resource). El idioma puede contener guiones
/// (`es-419`) pero nunca guiones bajos.
pub fn parse_unique_id(id: &str) -> Option<(String, String, String, String)> {
    let parts: Vec<&str> = id.split('_').collect();
    match parts.len() {
        4 => Some((parts[0].into(), parts[1].into(), parts[2].into(), parts[3].into())),
        3 => Some((parts[0].into(), parts[1].into(), parts[2].into(), parts[2].into())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL: &str = r#"{
  "package_version": 8,
  "format": "usfm",
  "generator": { "name": "ts-android", "build": "56" },
  "target_language": { "id": "cbt", "name": "Chayahuita", "direction": "ltr", "anglicized_name": "Chayahuita", "region": "Americas", "is_gateway_language": false },
  "project": { "id": "3jn", "name": "" },
  "type": { "id": "text", "name": "Text" },
  "resource": { "id": "reg", "name": "Regular" },
  "source_translations": [],
  "translators": [ "Joseeliezerr123" ],
  "finished_chunks": [],
  "parent_draft": { "resource_id": "", "checking_entity": "", "checking_level": "", "comments": "", "contributors": "", "publish_date": 0, "source_text": "", "source_text_version": "", "version": "" },
  "campo_desconocido": { "a": 1 }
}"#;

    #[test]
    fn parses_real_manifest_and_round_trips() {
        let m = Manifest::parse(REAL).unwrap();
        assert_eq!(m.unique_id(), "cbt_3jn_text_reg");
        assert_eq!(m.translators, vec!["Joseeliezerr123"]);
        assert!(m.extra.contains_key("campo_desconocido"));
        let again = Manifest::parse(&m.to_json()).unwrap();
        assert_eq!(m, again);
    }

    #[test]
    fn finished_chunks_toggle() {
        let mut m = Manifest::parse(REAL).unwrap();
        m.set_chunk_finished("01", "05", true);
        m.set_chunk_finished("01", "01", true);
        assert_eq!(m.finished_chunks, vec!["01-01", "01-05"]);
        assert!(m.is_chunk_finished("01", "05"));
        m.set_chunk_finished("01", "05", false);
        assert!(!m.is_chunk_finished("01", "05"));
        assert_eq!(m.finished_chunks, vec!["01-01"]);
    }

    #[test]
    fn unique_id_for_helps_omits_resource() {
        assert_eq!(unique_id("es", "gen", "tn", "tn"), "es_gen_tn");
        assert_eq!(unique_id("es", "gen", "text", "ulb"), "es_gen_text_ulb");
        assert_eq!(
            parse_unique_id("es-419_gen_text_ulb"),
            Some(("es-419".into(), "gen".into(), "text".into(), "ulb".into()))
        );
        assert_eq!(parse_unique_id("es_gen_tn").unwrap().3, "tn");
        assert_eq!(parse_unique_id("bad"), None);
    }

    #[test]
    fn rejects_incomplete_manifest() {
        assert!(Manifest::parse(r#"{"package_version":8,"generator":{"name":"","build":""},"target_language":{"id":"","name":""},"project":{"id":"gen"},"type":{"id":"text"},"resource":{"id":"reg"}}"#).is_err());
    }

    #[test]
    fn translators_dedupe() {
        let mut m = Manifest::parse(REAL).unwrap();
        m.add_translator(" Joseeliezerr123 ");
        m.add_translator("Ana");
        m.add_translator("");
        assert_eq!(m.translators, vec!["Joseeliezerr123", "Ana"]);
    }
}
