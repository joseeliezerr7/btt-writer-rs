//! Biblioteca local: `index.sqlite` (catálogo de idiomas, libros, recursos y
//! marcadores de chunk) más la carpeta de contenedores `.tsrc`.
//!
//! El esquema es el mismo que usan BTT Writer 1.6 y 2.0, por lo que el
//! `index.sqlite` que ya tenga el usuario funciona sin conversión.

use crate::rc::{container_slug, ResourceContainer};
use crate::{Error, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const MIN_CHECKING_LEVEL: i64 = 3;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TargetLanguageRow {
    pub slug: String,
    pub name: String,
    pub anglicized_name: String,
    pub direction: String,
    pub region: String,
    pub is_gateway_language: bool,
    /// `true` si proviene de `temp_target_language`.
    pub temporary: bool,
}

impl From<TargetLanguageRow> for crate::manifest::TargetLanguage {
    fn from(r: TargetLanguageRow) -> Self {
        crate::manifest::TargetLanguage {
            id: r.slug,
            name: r.name,
            direction: r.direction,
            anglicized_name: r.anglicized_name,
            region: r.region,
            is_gateway_language: r.is_gateway_language,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceLanguageRow {
    pub slug: String,
    pub name: String,
    pub direction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CategoryRow {
    pub slug: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectRow {
    pub slug: String,
    pub name: String,
    pub desc: String,
    pub sort: i64,
    pub chunks_url: String,
    pub source_language: String,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceRow {
    pub language_slug: String,
    pub language_name: String,
    pub language_direction: String,
    pub project_slug: String,
    pub project_name: String,
    pub slug: String,
    pub name: String,
    /// `book`, `help`, `dict`, `man`
    pub r#type: String,
    pub translate_mode: String,
    pub checking_level: String,
    pub version: String,
    pub pub_date: String,
    pub mime_type: String,
    pub url: String,
    pub modified_at: i64,
    /// Contenedor presente en disco.
    pub downloaded: bool,
}

impl ResourceRow {
    pub fn container_slug(&self) -> String {
        container_slug(&self.language_slug, &self.project_slug, &self.slug)
    }
}

/// Catálogo local (`index.sqlite`) + carpeta de contenedores.
pub struct Library {
    conn: Connection,
    pub containers_dir: PathBuf,
}

impl Library {
    pub fn open(index_path: &Path, containers_dir: &Path) -> Result<Library> {
        std::fs::create_dir_all(containers_dir)?;
        let conn = Connection::open(index_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        Ok(Library { conn, containers_dir: containers_dir.to_path_buf() })
    }

    pub fn open_in_memory(containers_dir: &Path) -> Result<Library> {
        std::fs::create_dir_all(containers_dir)?;
        let conn = Connection::open_in_memory()?;
        let lib = Library { conn, containers_dir: containers_dir.to_path_buf() };
        lib.ensure_schema()?;
        Ok(lib)
    }

    /// Crea las tablas que falten (para índices vacíos o antiguos).
    pub fn ensure_schema(&self) -> Result<()> {
        self.conn.execute_batch(SCHEMA)?;
        Ok(())
    }

    // ---------------------------------------------------------------- idiomas

    /// Idiomas destino (incluye temporales), filtrados por `query`
    /// (slug, nombre, nombre anglicanizado). `limit` 0 = todos.
    pub fn target_languages(&self, query: &str, limit: usize) -> Result<Vec<TargetLanguageRow>> {
        let q = format!("%{}%", query.trim().to_lowercase());
        let starts = format!("{}%", query.trim().to_lowercase());
        let mut stmt = self.conn.prepare(
            "SELECT * FROM (
             SELECT slug, name, anglicized_name, direction, region, is_gateway_language, 0 AS temporary
               FROM target_language
              WHERE lower(slug) LIKE ?1 OR lower(name) LIKE ?1 OR lower(anglicized_name) LIKE ?1
             UNION ALL
             SELECT slug, name, anglicized_name, direction, region, is_gateway_language, 1
               FROM temp_target_language
              WHERE approved_target_language_slug IS NULL
                AND (lower(slug) LIKE ?1 OR lower(name) LIKE ?1 OR lower(anglicized_name) LIKE ?1)
             )
             ORDER BY (lower(slug) LIKE ?2) DESC, (lower(name) LIKE ?2) DESC, slug",
        )?;
        let rows = stmt.query_map(params![q, starts], |r| {
            Ok(TargetLanguageRow {
                slug: r.get(0)?,
                name: r.get(1)?,
                anglicized_name: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                direction: r.get(3)?,
                region: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
                is_gateway_language: r.get::<_, i64>(5)? != 0,
                temporary: r.get::<_, i64>(6)? != 0,
            })
        })?;
        let mut out = vec![];
        for r in rows {
            out.push(r?);
            if limit > 0 && out.len() >= limit {
                break;
            }
        }
        Ok(out)
    }

    pub fn target_language(&self, slug: &str) -> Result<Option<TargetLanguageRow>> {
        Ok(self.target_languages(slug, 0)?.into_iter().find(|l| l.slug == slug))
    }

    pub fn source_languages(&self) -> Result<Vec<SourceLanguageRow>> {
        let mut stmt =
            self.conn.prepare("SELECT slug, name, direction FROM source_language ORDER BY slug")?;
        let rows = stmt.query_map([], |r| {
            Ok(SourceLanguageRow { slug: r.get(0)?, name: r.get(1)?, direction: r.get(2)? })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    // ---------------------------------------------------------------- proyectos

    /// Categorías de primer nivel (`bible-ot`, `bible-nt`, `other`...) con el
    /// nombre en `lang_slug` (fallback inglés).
    pub fn categories(&self, lang_slug: &str) -> Result<Vec<CategoryRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.slug,
                    COALESCE((SELECT cn.name FROM category_name cn JOIN source_language sl ON sl.id = cn.source_language_id
                              WHERE cn.category_id = c.id AND sl.slug = ?1),
                             (SELECT cn.name FROM category_name cn JOIN source_language sl ON sl.id = cn.source_language_id
                              WHERE cn.category_id = c.id AND sl.slug = 'en'),
                             c.slug)
               FROM category c WHERE c.parent_id = 0 ORDER BY c.id",
        )?;
        let rows =
            stmt.query_map(params![lang_slug], |r| Ok(CategoryRow { slug: r.get(0)?, name: r.get(1)? }))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Libros/proyectos disponibles, con nombre en el idioma fuente indicado
    /// (fallback `en`). Un proyecto aparece una sola vez.
    pub fn projects(&self, lang_slug: &str) -> Result<Vec<ProjectRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT p.slug,
                    COALESCE((SELECT p2.name FROM project p2 JOIN source_language sl2 ON sl2.id = p2.source_language_id
                              WHERE p2.slug = p.slug AND sl2.slug = ?1),
                             (SELECT p2.name FROM project p2 JOIN source_language sl2 ON sl2.id = p2.source_language_id
                              WHERE p2.slug = p.slug AND sl2.slug = 'en'), p.name),
                    COALESCE(p.desc, ''), p.sort, COALESCE(p.chunks_url, ''),
                    (SELECT slug FROM source_language WHERE id = p.source_language_id),
                    COALESCE((SELECT slug FROM category WHERE id = p.category_id), '')
               FROM project p
              WHERE p.id IN (SELECT MIN(id) FROM project GROUP BY slug)
              ORDER BY p.sort, p.slug",
        )?;
        let rows = stmt.query_map(params![lang_slug], |r| {
            Ok(ProjectRow {
                slug: r.get(0)?,
                name: r.get(1)?,
                desc: r.get(2)?,
                sort: r.get(3)?,
                chunks_url: r.get(4)?,
                source_language: r.get(5)?,
                category: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn project(&self, slug: &str, lang_slug: &str) -> Result<Option<ProjectRow>> {
        Ok(self.projects(lang_slug)?.into_iter().find(|p| p.slug == slug))
    }

    /// Marcadores de chunk (verso inicial por capítulo) de la versificación.
    pub fn chunk_markers(&self, project_slug: &str) -> Result<BTreeMap<String, Vec<u32>>> {
        let mut stmt = self.conn.prepare(
            "SELECT chapter, verse FROM chunk_marker WHERE project_slug = ?1 ORDER BY CAST(chapter AS INTEGER), CAST(verse AS INTEGER)",
        )?;
        let mut out: BTreeMap<String, Vec<u32>> = BTreeMap::new();
        let rows = stmt.query_map(params![project_slug], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        for r in rows {
            let (ch, v) = r?;
            let n = crate::usfm::first_number(&v);
            out.entry(format!("{:0>2}", ch)).or_default().push(n);
        }
        Ok(out)
    }

    pub fn set_chunk_markers(&self, project_slug: &str, markers: &BTreeMap<String, Vec<u32>>) -> Result<()> {
        let vid = self.versification_id()?;
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM chunk_marker WHERE project_slug = ?1 AND versification_id = ?2",
            params![project_slug, vid],
        )?;
        for (ch, verses) in markers {
            for v in verses {
                tx.execute(
                    "INSERT OR IGNORE INTO chunk_marker (chapter, verse, project_slug, versification_id) VALUES (?1, ?2, ?3, ?4)",
                    params![ch, format!("{v:02}"), project_slug, vid],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    fn versification_id(&self) -> Result<i64> {
        if let Some(id) = self
            .conn
            .query_row("SELECT id FROM versification WHERE slug = 'en-US'", [], |r| r.get(0))
            .optional()?
        {
            return Ok(id);
        }
        self.conn.execute("INSERT INTO versification (slug) VALUES ('en-US')", [])?;
        Ok(self.conn.last_insert_rowid())
    }

    // ---------------------------------------------------------------- recursos

    /// Recursos (fuentes) de un proyecto. `project_slug` vacío = todos.
    pub fn resources(&self, project_slug: &str) -> Result<Vec<ResourceRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT sl.slug, sl.name, sl.direction, p.slug, p.name, r.slug, r.name, r.type,
                    r.translate_mode, CAST(r.checking_level AS TEXT), CAST(r.version AS TEXT), CAST(COALESCE(r.pub_date, '') AS TEXT),
                    COALESCE(f.mime_type, ''), COALESCE(f.url, ''), COALESCE(f.modified_at, 0)
               FROM resource r
               JOIN project p ON p.id = r.project_id
               JOIN source_language sl ON sl.id = p.source_language_id
               LEFT JOIN resource_format f ON f.resource_id = r.id
              WHERE (?1 = '' OR p.slug = ?1)
              ORDER BY sl.slug, p.sort, r.slug",
        )?;
        let rows = stmt.query_map(params![project_slug], |r| {
            Ok(ResourceRow {
                language_slug: r.get(0)?,
                language_name: r.get(1)?,
                language_direction: r.get(2)?,
                project_slug: r.get(3)?,
                project_name: r.get(4)?,
                slug: r.get(5)?,
                name: r.get(6)?,
                r#type: r.get(7)?,
                translate_mode: r.get(8)?,
                checking_level: r.get(9)?,
                version: r.get(10)?,
                pub_date: r.get(11)?,
                mime_type: r.get(12)?,
                url: r.get(13)?,
                modified_at: r.get(14)?,
                downloaded: false,
            })
        })?;
        let mut out = vec![];
        for r in rows {
            let mut r = r?;
            r.downloaded = self.container_path(&r.container_slug()).is_file();
            out.push(r);
        }
        Ok(out)
    }

    /// Textos fuente (`book`) utilizables para traducir un proyecto:
    /// checking_level ≥ 3. Con `gl_mode` también se incluyen los `gl`.
    pub fn source_texts(&self, project_slug: &str, gl_mode: bool) -> Result<Vec<ResourceRow>> {
        Ok(self
            .resources(project_slug)?
            .into_iter()
            .filter(|r| r.r#type == "book")
            .filter(|r| r.checking_level.parse::<i64>().unwrap_or(0) >= MIN_CHECKING_LEVEL)
            .filter(|r| gl_mode || r.translate_mode != "gl" || r.downloaded)
            .collect())
    }

    pub fn resource(&self, lang: &str, project: &str, resource: &str) -> Result<Option<ResourceRow>> {
        Ok(self
            .resources(project)?
            .into_iter()
            .find(|r| r.language_slug == lang && r.slug == resource))
    }

    // ---------------------------------------------------------------- contenedores

    pub fn container_path(&self, slug: &str) -> PathBuf {
        self.containers_dir.join(format!("{slug}.tsrc"))
    }

    /// Abre un contenedor descargado.
    pub fn open_container(&self, lang: &str, project: &str, resource: &str) -> Result<ResourceContainer> {
        let slug = container_slug(lang, project, resource);
        let path = self.container_path(&slug);
        if !path.is_file() {
            return Err(Error::InvalidContainer(format!("{slug} no está descargado")));
        }
        ResourceContainer::open(&path)
    }

    pub fn has_container(&self, lang: &str, project: &str, resource: &str) -> bool {
        self.container_path(&container_slug(lang, project, resource)).is_file()
    }

    /// Guarda un contenedor y registra el recurso en el índice si no existía
    /// (contenedores importados manualmente).
    pub fn import_container(&self, rc: &ResourceContainer) -> Result<PathBuf> {
        let path = self.container_path(&rc.slug());
        rc.save(&path)?;
        self.register_container(rc)?;
        Ok(path)
    }

    pub fn delete_container(&self, lang: &str, project: &str, resource: &str) -> Result<()> {
        let path = self.container_path(&container_slug(lang, project, resource));
        if path.is_file() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    /// Lista de contenedores presentes en disco (slugs).
    pub fn downloaded_containers(&self) -> Result<Vec<String>> {
        let mut out = vec![];
        for e in std::fs::read_dir(&self.containers_dir)? {
            let e = e?;
            let name = e.file_name().to_string_lossy().to_string();
            if let Some(s) = name.strip_suffix(".tsrc") {
                out.push(s.to_string());
            }
        }
        out.sort();
        Ok(out)
    }

    /// Inserta en el índice los datos de un contenedor (idioma, proyecto, recurso).
    fn register_container(&self, rc: &ResourceContainer) -> Result<()> {
        let p = &rc.package;
        let lang_id = self.upsert_source_language(&p.language.slug, &p.language.name, &p.language.direction)?;
        let cat_id = self.upsert_category(&p.project.category_slug)?;
        let proj_id = self.upsert_project(
            lang_id,
            &p.project.slug,
            &p.project.name,
            &p.project.desc,
            p.project.sort,
            &p.project.chunks_url,
            cat_id,
        )?;
        let mime = format!("application/tsrc+{}", p.resource.r#type);
        self.upsert_resource(
            proj_id,
            &p.resource.slug,
            &p.resource.name,
            &p.resource.r#type,
            &p.resource.status.translate_mode,
            &p.resource.status.checking_level,
            &p.resource.status.version,
            &p.resource.status.pub_date,
            &mime,
            "",
            p.modified_at,
            true,
        )?;
        Ok(())
    }

    // ---------------------------------------------------------------- upserts

    fn upsert_source_language(&self, slug: &str, name: &str, direction: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO source_language (slug, name, direction) VALUES (?1, ?2, ?3)
             ON CONFLICT(slug) DO UPDATE SET name = excluded.name, direction = excluded.direction",
            params![slug, name, if direction.is_empty() { "ltr" } else { direction }],
        )?;
        Ok(self.conn.query_row("SELECT id FROM source_language WHERE slug = ?1", params![slug], |r| r.get(0))?)
    }

    fn upsert_category(&self, slug: &str) -> Result<i64> {
        if slug.is_empty() {
            return Ok(0);
        }
        self.conn.execute(
            "INSERT OR IGNORE INTO category (slug, parent_id) VALUES (?1, 0)",
            params![slug],
        )?;
        Ok(self.conn.query_row(
            "SELECT id FROM category WHERE slug = ?1 AND parent_id = 0",
            params![slug],
            |r| r.get(0),
        )?)
    }

    #[allow(clippy::too_many_arguments)]
    fn upsert_project(
        &self,
        lang_id: i64,
        slug: &str,
        name: &str,
        desc: &str,
        sort: i64,
        chunks_url: &str,
        category_id: i64,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO project (slug, name, desc, sort, chunks_url, source_language_id, category_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(source_language_id, slug) DO UPDATE SET
                name = CASE WHEN excluded.name = '' THEN project.name ELSE excluded.name END,
                desc = excluded.desc, sort = excluded.sort,
                chunks_url = CASE WHEN excluded.chunks_url = '' THEN project.chunks_url ELSE excluded.chunks_url END,
                category_id = CASE WHEN excluded.category_id = 0 THEN project.category_id ELSE excluded.category_id END",
            params![slug, name, desc, sort, chunks_url, lang_id, category_id],
        )?;
        Ok(self.conn.query_row(
            "SELECT id FROM project WHERE source_language_id = ?1 AND slug = ?2",
            params![lang_id, slug],
            |r| r.get(0),
        )?)
    }

    #[allow(clippy::too_many_arguments)]
    fn upsert_resource(
        &self,
        project_id: i64,
        slug: &str,
        name: &str,
        type_: &str,
        translate_mode: &str,
        checking_level: &str,
        version: &str,
        pub_date: &str,
        mime: &str,
        url: &str,
        modified_at: i64,
        imported: bool,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO resource (slug, name, type, translate_mode, checking_level, comments, pub_date, license, version, project_id)
             VALUES (?1, ?2, ?3, ?4, ?5, '', ?6, '', ?7, ?8)
             ON CONFLICT(project_id, slug) DO UPDATE SET
                name = excluded.name, type = excluded.type, translate_mode = excluded.translate_mode,
                checking_level = excluded.checking_level, pub_date = excluded.pub_date, version = excluded.version",
            params![
                slug,
                name,
                type_,
                if translate_mode.is_empty() { "all" } else { translate_mode },
                checking_level,
                pub_date,
                version,
                project_id
            ],
        )?;
        let rid: i64 = self.conn.query_row(
            "SELECT id FROM resource WHERE project_id = ?1 AND slug = ?2",
            params![project_id, slug],
            |r| r.get(0),
        )?;
        self.conn.execute(
            "INSERT INTO resource_format (package_version, mime_type, modified_at, imported, url, resource_id)
             VALUES ('0.1', ?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(mime_type, resource_id) DO UPDATE SET
                modified_at = excluded.modified_at,
                imported = MAX(resource_format.imported, excluded.imported),
                url = CASE WHEN excluded.url = '' THEN resource_format.url ELSE excluded.url END",
            params![mime, modified_at, imported as i64, url, rid],
        )?;
        Ok(rid)
    }

    // ---------------------------------------------------------------- catálogo tS v2

    /// Actualiza el índice con el catálogo tS v2 ya descargado:
    /// * `catalog`: `catalog.json` (lista de proyectos con `lang_catalog`).
    /// * `fetch`: función que descarga una URL y devuelve el JSON como texto.
    ///
    /// Devuelve los slugs de recursos nuevos o actualizados (`lang_proj_res`).
    pub fn update_sources_from_catalog<F>(&self, catalog_json: &str, mut fetch: F) -> Result<Vec<String>>
    where
        F: FnMut(&str) -> Result<String>,
    {
        let catalog: Vec<CatalogProject> = serde_json::from_str(catalog_json)?;
        let mut changed = vec![];
        for cp in catalog {
            let langs_json = match fetch(&cp.lang_catalog) {
                Ok(j) => j,
                Err(e) => {
                    log::warn!("catálogo de idiomas {}: {e}", cp.lang_catalog);
                    continue;
                }
            };
            let langs: Vec<CatalogLanguageEntry> = match serde_json::from_str(&langs_json) {
                Ok(l) => l,
                Err(e) => {
                    log::warn!("JSON de idiomas inválido {}: {e}", cp.lang_catalog);
                    continue;
                }
            };
            let category = cp.meta.first().cloned().unwrap_or_default();
            for le in langs {
                let res_json = match fetch(&le.res_catalog) {
                    Ok(j) => j,
                    Err(e) => {
                        log::warn!("catálogo de recursos {}: {e}", le.res_catalog);
                        continue;
                    }
                };
                let resources: Vec<CatalogResource> = match serde_json::from_str(&res_json) {
                    Ok(r) => r,
                    Err(_) => continue,
                };
                let lang_id =
                    self.upsert_source_language(&le.language.slug, &le.language.name, &le.language.direction)?;
                let cat_id = self.upsert_category(&category)?;
                let sort = cp.sort.as_i64();
                let proj_id = self.upsert_project(
                    lang_id,
                    &cp.slug,
                    &le.project.name,
                    &le.project.desc,
                    sort,
                    resources.first().map(|r| r.chunks.clone()).unwrap_or_default().as_str(),
                    cat_id,
                )?;
                for r in resources {
                    let modified = r.date_modified.as_i64();
                    let translate_mode = "all";
                    // Texto bíblico
                    let before = self.resource_modified(proj_id, &r.slug)?;
                    self.upsert_resource(
                        proj_id,
                        &r.slug,
                        &r.name,
                        "book",
                        translate_mode,
                        &r.status.checking_level,
                        &r.status.version,
                        &r.status.publish_date,
                        "application/tsrc+book",
                        &r.source,
                        modified,
                        false,
                    )?;
                    if before != Some(modified) {
                        changed.push(container_slug(&le.language.slug, &cp.slug, &r.slug));
                    }
                    // Ayudas asociadas (tN, tQ) y diccionario (tW) del mismo idioma.
                    if !r.notes.is_empty() {
                        self.upsert_resource(proj_id, "tn", "translationNotes", "help", "gl", &r.status.checking_level, &r.status.version, &r.status.publish_date, "application/tsrc+help", &r.notes, modified, false)?;
                    }
                    if !r.checking_questions.is_empty() {
                        self.upsert_resource(proj_id, "tq", "translationQuestions", "help", "gl", &r.status.checking_level, &r.status.version, &r.status.publish_date, "application/tsrc+help", &r.checking_questions, modified, false)?;
                    }
                    if !r.terms.is_empty() {
                        let bible_id = self.upsert_project(lang_id, "bible", "translationWords", "", 0, "", 0)?;
                        let rid = self.upsert_resource(bible_id, "tw", "translationWords", "dict", "gl", &r.status.checking_level, &r.status.version, &r.status.publish_date, "application/tsrc+dict", &r.terms, modified, false)?;
                        let _ = rid;
                    }
                    if !r.tw_cat.is_empty() {
                        let rid: i64 = self.conn.query_row(
                            "SELECT id FROM resource WHERE project_id = ?1 AND slug = ?2",
                            params![proj_id, r.slug],
                            |row| row.get(0),
                        )?;
                        self.conn.execute(
                            "INSERT INTO legacy_resource_info (translation_words_assignments_url, resource_id) VALUES (?1, ?2)
                             ON CONFLICT(resource_id) DO UPDATE SET translation_words_assignments_url = excluded.translation_words_assignments_url",
                            params![r.tw_cat, rid],
                        )?;
                    }
                }
            }
        }
        Ok(changed)
    }

    fn resource_modified(&self, project_id: i64, slug: &str) -> Result<Option<i64>> {
        Ok(self
            .conn
            .query_row(
                "SELECT f.modified_at FROM resource r JOIN resource_format f ON f.resource_id = r.id
                  WHERE r.project_id = ?1 AND r.slug = ?2 LIMIT 1",
                params![project_id, slug],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// URL de `tw_cat.json` (asignación de palabras por chunk) de un texto fuente.
    pub fn tw_assignments_url(&self, lang: &str, project: &str, resource: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT l.translation_words_assignments_url FROM legacy_resource_info l
                   JOIN resource r ON r.id = l.resource_id
                   JOIN project p ON p.id = r.project_id
                   JOIN source_language sl ON sl.id = p.source_language_id
                  WHERE sl.slug = ?1 AND p.slug = ?2 AND r.slug = ?3",
                params![lang, project, resource],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Actualiza idiomas destino desde `langnames.json`. Devuelve cuántos se insertaron/actualizaron.
    pub fn update_target_languages(&self, langnames_json: &str) -> Result<usize> {
        let langs: Vec<LangName> = serde_json::from_str(langnames_json)?;
        let tx = self.conn.unchecked_transaction()?;
        let mut n = 0;
        for l in langs {
            if l.lc.is_empty() {
                continue;
            }
            n += tx.execute(
                "INSERT INTO target_language (slug, name, anglicized_name, direction, region, is_gateway_language)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(slug) DO UPDATE SET name = excluded.name, anglicized_name = excluded.anglicized_name,
                    direction = excluded.direction, region = excluded.region, is_gateway_language = excluded.is_gateway_language",
                params![l.lc, l.ln, l.ang, if l.ld.is_empty() { "ltr".to_string() } else { l.ld }, l.lr, l.gw as i64],
            )?;
        }
        tx.commit()?;
        Ok(n)
    }

    /// Actualiza idiomas temporales (`templanguages`) y las aprobaciones.
    pub fn update_temp_languages(&self, temp_json: &str, approved_json: Option<&str>) -> Result<usize> {
        let langs: Vec<LangName> = serde_json::from_str(temp_json)?;
        let tx = self.conn.unchecked_transaction()?;
        let mut n = 0;
        for l in langs {
            if l.lc.is_empty() {
                continue;
            }
            n += tx.execute(
                "INSERT INTO temp_target_language (slug, name, anglicized_name, direction, region, is_gateway_language)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(slug) DO UPDATE SET name = excluded.name, anglicized_name = excluded.anglicized_name,
                    direction = excluded.direction, region = excluded.region",
                params![l.lc, l.ln, l.ang, if l.ld.is_empty() { "ltr".to_string() } else { l.ld }, l.lr, l.gw as i64],
            )?;
        }
        if let Some(a) = approved_json {
            let approved: Vec<BTreeMap<String, String>> = serde_json::from_str(a)?;
            for m in approved {
                for (temp, real) in m {
                    tx.execute(
                        "UPDATE temp_target_language SET approved_target_language_slug = ?1 WHERE slug = ?2",
                        params![real, temp],
                    )?;
                }
            }
        }
        tx.commit()?;
        Ok(n)
    }

    /// Si un idioma temporal fue aprobado, devuelve el slug definitivo.
    pub fn approved_slug(&self, temp_slug: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT approved_target_language_slug FROM temp_target_language WHERE slug = ?1",
                params![temp_slug],
                |r| r.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten())
    }

    pub fn catalog_url(&self, slug: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT url FROM catalog WHERE slug = ?1", params![slug], |r| r.get(0))
            .optional()?)
    }

    /// Nombre de un libro en el idioma dado (fallback inglés / slug en mayúsculas).
    pub fn project_name(&self, slug: &str, lang: &str) -> String {
        self.project(slug, lang)
            .ok()
            .flatten()
            .map(|p| p.name)
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| slug.to_uppercase())
    }
}

// ---------------------------------------------------------------------- JSON del catálogo tS v2

#[derive(Debug, Deserialize)]
struct CatalogProject {
    slug: String,
    lang_catalog: String,
    #[serde(default)]
    meta: Vec<String>,
    #[serde(default)]
    sort: IntOrString,
}

#[derive(Debug, Deserialize)]
struct CatalogLanguageEntry {
    language: CatalogLanguage,
    #[serde(default)]
    project: CatalogProjectInfo,
    res_catalog: String,
}

#[derive(Debug, Deserialize)]
struct CatalogLanguage {
    slug: String,
    name: String,
    #[serde(default)]
    direction: String,
}

#[derive(Debug, Deserialize, Default)]
struct CatalogProjectInfo {
    #[serde(default)]
    name: String,
    #[serde(default)]
    desc: String,
}

#[derive(Debug, Deserialize)]
struct CatalogResource {
    slug: String,
    name: String,
    #[serde(default)]
    date_modified: IntOrString,
    #[serde(default)]
    status: CatalogStatus,
    #[serde(default)]
    source: String,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    checking_questions: String,
    #[serde(default)]
    terms: String,
    #[serde(default)]
    tw_cat: String,
    #[serde(default)]
    chunks: String,
}

#[derive(Debug, Deserialize, Default)]
struct CatalogStatus {
    #[serde(default)]
    checking_level: String,
    #[serde(default)]
    publish_date: String,
    #[serde(default)]
    version: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(untagged)]
pub enum IntOrString {
    Int(i64),
    Str(String),
    #[default]
    None,
}

impl IntOrString {
    pub fn as_i64(&self) -> i64 {
        match self {
            IntOrString::Int(i) => *i,
            IntOrString::Str(s) => s.trim().parse().unwrap_or(0),
            IntOrString::None => 0,
        }
    }
}

#[derive(Debug, Deserialize)]
struct LangName {
    #[serde(default)]
    lc: String,
    #[serde(default)]
    ln: String,
    #[serde(default)]
    ang: String,
    #[serde(default)]
    ld: String,
    #[serde(default)]
    lr: String,
    #[serde(default)]
    gw: bool,
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS source_language (id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT, slug TEXT NOT NULL, name TEXT NOT NULL, direction TEXT NOT NULL DEFAULT ltr, UNIQUE (slug));
CREATE TABLE IF NOT EXISTS category (id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT, slug TEXT NOT NULL, parent_id INTEGER NOT NULL DEFAULT 0, UNIQUE(slug, parent_id));
CREATE TABLE IF NOT EXISTS category_name (id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT, source_language_id INTEGER NOT NULL, category_id INTEGER NOT NULL, name TEXT NOT NULL, UNIQUE(source_language_id, category_id));
CREATE TABLE IF NOT EXISTS project (id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT, slug TEXT NOT NULL, name TEXT NOT NULL, desc TEXT NULL DEFAULT NULL, icon TEXT NULL DEFAULT NULL, sort INTEGER NOT NULL DEFAULT 0, chunks_url TEXT NULL DEFAULT NULL, source_language_id INTEGER NOT NULL, category_id INTEGER NOT NULL DEFAULT 0, UNIQUE (source_language_id, slug));
CREATE TABLE IF NOT EXISTS resource (id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT, slug TEXT NOT NULL, name TEXT NOT NULL, type TEXT NOT NULL, translate_mode TEXT NOT NULL, checking_level TEXT NOT NULL, comments TEXT NULL DEFAULT NULL, pub_date DATE NULL DEFAULT NULL, license TEXT NULL DEFAULT NULL, version TEXT NOT NULL, project_id INTEGER NOT NULL, UNIQUE (project_id, slug));
CREATE TABLE IF NOT EXISTS legacy_resource_info (id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT, translation_words_assignments_url TEXT NULL DEFAULT NULL, resource_id INTEGER NOT NULL, UNIQUE (resource_id));
CREATE TABLE IF NOT EXISTS resource_format (id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT, package_version TEXT NOT NULL, mime_type TEXT NOT NULL, modified_at INTEGER NOT NULL DEFAULT 0, imported INTEGER NOT NULL DEFAULT 0, url TEXT NOT NULL, resource_id INTEGER NOT NULL, UNIQUE (mime_type, resource_id));
CREATE TABLE IF NOT EXISTS target_language (id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT, slug TEXT NOT NULL, name TEXT NOT NULL, anglicized_name TEXT NULL DEFAULT NULL, direction TEXT NOT NULL DEFAULT ltr, region TEXT NOT NULL DEFAULT '', is_gateway_language INTEGER NOT NULL DEFAULT 0, UNIQUE(slug));
CREATE TABLE IF NOT EXISTS temp_target_language (id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT, slug TEXT NOT NULL, name TEXT NOT NULL, anglicized_name TEXT NULL DEFAULT NULL, direction TEXT NOT NULL DEFAULT ltr, region TEXT NOT NULL DEFAULT '', is_gateway_language INTEGER NOT NULL DEFAULT 0, approved_target_language_slug TEXT NULL DEFAULT NULL, UNIQUE(slug));
CREATE TABLE IF NOT EXISTS versification (id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT, slug TEXT NOT NULL, UNIQUE(slug));
CREATE TABLE IF NOT EXISTS versification_name (id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT, source_language_id INTEGER NOT NULL, versification_id INTEGER NOT NULL, name TEXT NOT NULL, UNIQUE(source_language_id, versification_id));
CREATE TABLE IF NOT EXISTS chunk_marker (id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT, chapter TEXT NOT NULL, verse TEXT NOT NULL, project_slug TEXT NOT NULL, versification_id INTEGER NOT NULL, UNIQUE (versification_id, project_slug, chapter, verse));
CREATE TABLE IF NOT EXISTS catalog (id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT, slug TEXT NOT NULL, url TEXT NOT NULL, modified_at INTEGER NOT NULL, UNIQUE(slug));
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_index() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/index.sqlite")
    }

    fn open_fixture() -> (tempfile::TempDir, Library) {
        let tmp = tempfile::tempdir().unwrap();
        let idx = tmp.path().join("index.sqlite");
        std::fs::copy(fixture_index(), &idx).unwrap();
        let lib = Library::open(&idx, &tmp.path().join("rc")).unwrap();
        (tmp, lib)
    }

    #[test]
    fn reads_languages_projects_resources_from_real_index() {
        let (_tmp, lib) = open_fixture();
        let langs = lib.target_languages("espa", 10).unwrap();
        assert!(langs.iter().any(|l| l.slug == "es"), "{langs:?}");
        let es = lib.target_language("es-419").unwrap().unwrap();
        assert_eq!(es.direction, "ltr");
        assert!(lib.target_languages("", 5).unwrap().len() == 5);

        let projects = lib.projects("en").unwrap();
        assert!(projects.iter().any(|p| p.slug == "gen" && p.name == "Genesis"));
        assert!(projects.iter().any(|p| p.slug == "3jn"));
        let projects_es = lib.projects("es-419").unwrap();
        let gen = projects_es.iter().find(|p| p.slug == "gen").unwrap();
        assert!(gen.name.starts_with("G"), "{}", gen.name);

        let cats = lib.categories("en").unwrap();
        assert!(cats.iter().any(|c| c.slug == "bible-nt"));

        let res = lib.resources("3jn").unwrap();
        assert!(res.iter().any(|r| r.language_slug == "en" && r.slug == "ulb" && r.r#type == "book"));
        let sources = lib.source_texts("3jn", false).unwrap();
        assert!(sources.iter().all(|r| r.r#type == "book"));
        assert!(!sources.is_empty());

        let markers = lib.chunk_markers("3jn").unwrap();
        assert_eq!(markers["01"], vec![1, 5, 9, 11, 13]);
        assert!(lib.source_languages().unwrap().iter().any(|l| l.slug == "en"));
    }

    #[test]
    fn container_roundtrip_registers_resource() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::open_in_memory(&tmp.path().join("rc")).unwrap();
        let mut f = BTreeMap::new();
        f.insert("package.json".into(), br#"{"content_mime_type":"text/usx","modified_at":5,"language":{"slug":"xx","name":"Xx","direction":"rtl"},"project":{"slug":"jud","name":"Judas","sort":65,"category_slug":"bible-nt"},"resource":{"slug":"ulb","name":"ULB","type":"book","status":{"translate_mode":"all","checking_level":"3","version":"1"}}}"#.to_vec());
        f.insert("content/01/01.usx".into(), b"<verse number=\"1\" style=\"v\" />a".to_vec());
        let rc = ResourceContainer::from_files(f).unwrap();
        lib.import_container(&rc).unwrap();
        assert!(lib.has_container("xx", "jud", "ulb"));
        let r = lib.resource("xx", "jud", "ulb").unwrap().unwrap();
        assert!(r.downloaded);
        assert_eq!(r.language_direction, "rtl");
        assert_eq!(lib.downloaded_containers().unwrap(), vec!["xx_jud_ulb"]);
        let opened = lib.open_container("xx", "jud", "ulb").unwrap();
        assert_eq!(opened.chapters(), vec!["01"]);
        lib.delete_container("xx", "jud", "ulb").unwrap();
        assert!(!lib.has_container("xx", "jud", "ulb"));
        assert!(lib.open_container("xx", "jud", "ulb").is_err());
    }

    #[test]
    fn catalog_update_inserts_resources() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::open_in_memory(&tmp.path().join("rc")).unwrap();
        let fx = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/catalog");
        let read = |n: &str| std::fs::read_to_string(fx.join(n)).unwrap().replace("{datetime}", "20240101").replace("\"{date}\"", "\"2024-01-01\"");
        let catalog = read("catalog.json").replace("{server}", "srv:/");
        let changed = lib
            .update_sources_from_catalog(&catalog, |url| {
                let name = url.trim_start_matches("srv://").to_string() + ".json";
                if !fx.join(&name).is_file() {
                    return Err(Error::Other(format!("sin fixture {url}")));
                }
                Ok(read(&name).replace("{server}", "srv:/"))
            })
            .unwrap();
        assert!(changed.contains(&"es-419_mat_ulb".to_string()), "{changed:?}");
        let res = lib.resources("mat").unwrap();
        assert!(res.iter().any(|r| r.language_slug == "es-419" && r.slug == "udb" && r.url.ends_with("source.json")));
        assert!(res.iter().any(|r| r.slug == "tn" && r.r#type == "help"));
        let tw = lib.resources("bible").unwrap();
        assert!(tw.iter().any(|r| r.slug == "tw" && r.r#type == "dict"));
        assert_eq!(lib.tw_assignments_url("es-419", "mat", "ulb").unwrap().unwrap(), "srv://mat/es/tw_cat.json");
        // Segunda pasada sin cambios no reporta novedades.
        let again = lib.update_sources_from_catalog(&catalog, |url| {
            let name = url.trim_start_matches("srv://").to_string() + ".json";
            Ok(read(&name).replace("{server}", "srv:/"))
        }).unwrap();
        assert!(again.is_empty(), "{again:?}");
    }

    #[test]
    fn langnames_and_temp_languages() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::open_in_memory(&tmp.path().join("rc")).unwrap();
        let fx = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/catalog");
        let n = lib.update_target_languages(&std::fs::read_to_string(fx.join("langnames.json")).unwrap()).unwrap();
        assert!(n > 0);
        assert!(lib.target_language("aa").unwrap().is_some());
        let temp = std::fs::read_to_string(fx.join("temp_langs.json")).unwrap();
        let approved = std::fs::read_to_string(fx.join("approved_temp_langs.json")).unwrap();
        lib.update_temp_languages(&temp, Some(&approved)).unwrap();
        let temps = lib.target_languages("qaa", 0).unwrap();
        assert!(temps.iter().all(|t| t.temporary));
        let approved_list: Vec<BTreeMap<String, String>> = serde_json::from_str(&approved).unwrap();
        if let Some((t, real)) = approved_list.first().and_then(|m| m.iter().next()) {
            assert_eq!(lib.approved_slug(t).unwrap().as_deref(), Some(real.as_str()));
        }
    }

    #[test]
    fn chunk_markers_can_be_set() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::open_in_memory(&tmp.path().join("rc")).unwrap();
        let mut m = BTreeMap::new();
        m.insert("01".to_string(), vec![1, 4, 9]);
        m.insert("02".to_string(), vec![1]);
        lib.set_chunk_markers("jud", &m).unwrap();
        assert_eq!(lib.chunk_markers("jud").unwrap(), m);
        assert!(lib.chunk_markers("nope").unwrap().is_empty());
    }
}
