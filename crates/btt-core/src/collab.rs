//! Trabajo en equipo sobre un repositorio compartido de WACS.
//!
//! Varias personas traducen el mismo libro desde dispositivos distintos. Cada
//! una tiene su copia local completa (git) y la app sincroniza sola: guarda,
//! baja lo del servidor, lo fusiona y sube. Los chunks son archivos separados,
//! así que trabajar en versículos distintos nunca choca. El chunk que se está
//! editando se retiene (`hold`) y solo se comparte al cerrarlo con "Listo"; si
//! otra persona lo cambió mientras tanto, queda un único conflicto que se
//! resuelve en la app.
//!
//! `collab.json` (dentro del proyecto, viaja por git) guarda las *reservas*:
//! quién está traduciendo cada chunk. Son avisos, no candados: caducan solas y
//! cualquiera puede entrar de todos modos tras un aviso.

use crate::gitrepo::{self, Author, Credentials, MergeStatus, PushStatus};
use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

pub const COLLAB_FILE: &str = "collab.json";
/// Una reserva dura esto desde la última sincronización de quien la tiene.
pub const CLAIM_TTL_SECS: u64 = 15 * 60;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Claim {
    pub user: String,
    pub since: u64,
    pub until: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CollabFile {
    pub version: u32,
    /// `capítulo/chunk` → reserva.
    #[serde(default)]
    pub claims: BTreeMap<String, Claim>,
}

impl Default for CollabFile {
    fn default() -> Self {
        CollabFile { version: 1, claims: BTreeMap::new() }
    }
}

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn load(root: &Path) -> CollabFile {
    std::fs::read_to_string(root.join(COLLAB_FILE)).ok().and_then(|s| parse(&s)).unwrap_or_default()
}

fn parse(s: &str) -> Option<CollabFile> {
    // Un archivo con marcadores de conflicto (no debería: se fusiona aparte) se descarta.
    if crate::conflicts::has_conflicts(s) {
        return None;
    }
    serde_json::from_str(s).ok()
}

pub fn save(root: &Path, f: &CollabFile) -> Result<()> {
    let mut json = serde_json::to_string_pretty(f)?;
    json.push('\n');
    crate::project::atomic_write(&root.join(COLLAB_FILE), &json)
}

/// Unión de dos versiones: para una misma reserva gana la que caduca más tarde
/// (la que se renovó más recientemente); en empate, la nuestra.
pub fn merge(ours: &CollabFile, theirs: &CollabFile) -> CollabFile {
    let mut out = ours.clone();
    for (k, c) in &theirs.claims {
        match out.claims.get(k) {
            Some(mine) if mine.until >= c.until => {}
            _ => {
                out.claims.insert(k.clone(), c.clone());
            }
        }
    }
    out
}

/// Fusión de los dos textos de `collab.json` en un conflicto de git.
pub fn merge_json(ours: &str, theirs: &str) -> String {
    let a = parse(ours).unwrap_or_default();
    let b = parse(theirs).unwrap_or_default();
    let mut json = serde_json::to_string_pretty(&merge(&a, &b)).unwrap_or_else(|_| "{}".into());
    json.push('\n');
    json
}

fn prune(f: &mut CollabFile, now: u64) {
    f.claims.retain(|_, c| c.until > now);
}

/// Reserva (o renueva) `key` para `user`. Devuelve `false` si otra persona la tiene vigente.
pub fn claim(root: &Path, key: &str, user: &str, now: u64) -> Result<bool> {
    let mut f = load(root);
    prune(&mut f, now);
    if let Some(c) = f.claims.get(key) {
        if c.user != user {
            return Ok(false);
        }
    }
    let since = f.claims.get(key).map(|c| c.since).unwrap_or(now);
    f.claims.insert(key.into(), Claim { user: user.into(), since, until: now + CLAIM_TTL_SECS });
    save(root, &f)?;
    Ok(true)
}

/// Quita la reserva de `key` si es de `user`.
pub fn release(root: &Path, key: &str, user: &str, now: u64) -> Result<()> {
    let mut f = load(root);
    prune(&mut f, now);
    if f.claims.get(key).map(|c| c.user == user).unwrap_or(false) {
        f.claims.remove(key);
    }
    save(root, &f)
}

/// Quita todas las reservas de `user` (al salir del proyecto o desactivar el equipo).
pub fn release_all(root: &Path, user: &str, now: u64) -> Result<()> {
    let mut f = load(root);
    prune(&mut f, now);
    f.claims.retain(|_, c| c.user != user);
    save(root, &f)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaimView {
    pub key: String,
    pub user: String,
    pub since: u64,
    pub until: u64,
}

/// Reservas vigentes (incluidas las propias).
pub fn active(root: &Path, now: u64) -> Vec<ClaimView> {
    load(root)
        .claims
        .into_iter()
        .filter(|(_, c)| c.until > now)
        .map(|(key, c)| ClaimView { key, user: c.user, since: c.since, until: c.until })
        .collect()
}

#[derive(Debug, Clone, Default)]
pub struct SyncOptions {
    /// Chunk (`capítulo/chunk`) que esta persona está editando: se renueva su reserva.
    pub editing: Option<String>,
    pub user: String,
    /// Subir lo local tras fusionar (si no hay conflictos).
    pub push: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SyncReport {
    pub merge: Option<MergeStatus>,
    pub push: Option<PushStatus>,
    /// Chunks (`capítulo/chunk.txt`) que cambiaron al bajar lo del servidor.
    pub changed: Vec<String>,
    /// Chunks con marcadores de conflicto pendientes de resolver.
    pub conflicts: Vec<String>,
    pub claims: Vec<ClaimView>,
    /// No se pudo contactar con el servidor; se reintenta en la próxima pasada.
    pub offline: bool,
    pub error: Option<String>,
    pub at: u64,
}

fn is_chunk(path: &str) -> bool {
    path.ends_with(".txt")
}

/// Una pasada completa: guarda, baja y fusiona, y sube si procede. Nunca
/// descarta trabajo local: `pull` hace commit y rama de respaldo antes de fusionar.
pub fn sync(root: &Path, url: &str, creds: Option<&Credentials>, author: &Author, opts: &SyncOptions) -> SyncReport {
    let now = now();
    let mut report = SyncReport { at: now, ..Default::default() };
    let held = opts.editing.as_deref().map(crate::hold::chunk_path);
    if let Some(k) = &opts.editing {
        if let Err(e) = claim(root, k, &opts.user, now) {
            report.error = Some(e.to_string());
        }
        // El chunk en edición se retiene: no se sube hasta cerrarlo con "Listo".
        if let Err(e) = crate::hold::start(root, held.as_deref().unwrap_or_default(), now) {
            report.error = Some(e.to_string());
        }
    }
    // Los chunks que se cerraron se comparan con lo que llegó del servidor mientras
    // estaban retenidos: si ambos cambiaron queda un único conflicto para resolver.
    report.changed = crate::hold::close_all(root, held.as_deref());
    // Primero el commit de lo propio: así `changed` solo recoge lo que vino del servidor.
    if let Err(e) = gitrepo::commit_all(root, author, gitrepo::AUTOSAVE_MESSAGE) {
        report.error = Some(e.to_string());
        report.claims = active(root, now);
        return report;
    }
    let before = gitrepo::head_hash(root);
    match gitrepo::pull(root, url, creds, author) {
        Ok(m) => report.merge = Some(m),
        Err(e) => {
            if gitrepo::is_network_error(&e) {
                report.offline = true;
            } else {
                report.error = Some(e.to_string());
            }
            report.claims = active(root, now);
            return report;
        }
    }
    let after = gitrepo::head_hash(root);
    report.changed.extend(gitrepo::changed_files(root, before.as_deref(), after.as_deref()).unwrap_or_default().into_iter().filter(|p| is_chunk(p)));
    report.conflicts = gitrepo::conflicted_files(root).unwrap_or_default();

    let can_push = opts.push
        && report.conflicts.is_empty()
        && !matches!(report.merge, Some(MergeStatus::AuthFailure) | Some(MergeStatus::NoRemoteRepo));
    if let (true, Some(c)) = (can_push, creds) {
        // Si alguien subió entre nuestro pull y nuestro push, se vuelve a fusionar (pocas veces).
        for _ in 0..3 {
            match gitrepo::push(root, url, c) {
                Ok(PushStatus::RejectedNonFastForward) => {
                    let b2 = gitrepo::head_hash(root);
                    match gitrepo::pull(root, url, creds, author) {
                        Ok(m) => {
                            let a2 = gitrepo::head_hash(root);
                            let more = gitrepo::changed_files(root, b2.as_deref(), a2.as_deref()).unwrap_or_default();
                            report.changed.extend(more.into_iter().filter(|p| is_chunk(p)));
                            report.conflicts = gitrepo::conflicted_files(root).unwrap_or_default();
                            report.merge = Some(m);
                            if !report.conflicts.is_empty() {
                                break;
                            }
                        }
                        Err(e) => {
                            report.error = Some(e.to_string());
                            break;
                        }
                    }
                }
                Ok(p) => {
                    report.push = Some(p);
                    break;
                }
                Err(e) => {
                    if gitrepo::is_network_error(&e) {
                        report.offline = true;
                    } else {
                        report.error = Some(e.to_string());
                    }
                    break;
                }
            }
        }
    }
    // El chunk en edición nunca se recarga en pantalla: lo que llegó del servidor
    // espera en HEAD hasta que se cierre.
    if let Some(h) = &held {
        report.changed.retain(|p| p != h);
    }
    report.changed.sort();
    report.changed.dedup();
    report.claims = active(root, now);
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{IdName, Manifest, TargetLanguage};
    use crate::project::Project;

    fn author(name: &str) -> Author {
        Author::new(name, &format!("{name}@x.org"))
    }

    fn manifest() -> Manifest {
        Manifest::new(
            TargetLanguage { id: "cbt".into(), name: "Chayahuita".into(), ..Default::default() },
            IdName::new("3jn", "3 John"),
            IdName::new("text", "Text"),
            IdName::new("reg", "Regular"),
            "1",
        )
    }

    /// Un "servidor" (repo bare) y dos personas con su copia local del mismo proyecto.
    fn team(tmp: &Path) -> (String, Project, Project) {
        let server = tmp.join("server.git");
        git2::Repository::init_bare(&server).unwrap();
        let url = server.to_string_lossy().replace('\\', "/");
        let a = Project::create(&tmp.join("ana"), manifest()).unwrap();
        let b = Project::create(&tmp.join("beto"), manifest()).unwrap();
        (url, a, b)
    }

    fn creds() -> Credentials {
        Credentials { username: "x".into(), token: "y".into() }
    }

    fn opts(user: &str, editing: Option<&str>) -> SyncOptions {
        SyncOptions { editing: editing.map(String::from), user: user.into(), push: true }
    }

    #[test]
    fn dos_personas_en_chunks_distintos_se_fusionan_sin_conflicto() {
        let tmp = tempfile::tempdir().unwrap();
        let (url, a, b) = team(tmp.path());
        a.write_chunk("01", "01", "\\v 1 Ana traduce").unwrap();
        let r = sync(&a.root, &url, Some(&creds()), &author("ana"), &opts("ana", Some("01/01")));
        assert!(r.error.is_none() && !r.offline, "{r:?}");
        assert_eq!(r.push, Some(PushStatus::Ok));
        // Ana cierra el chunk ("Listo"): ahí se comparte.
        sync(&a.root, &url, Some(&creds()), &author("ana"), &opts("ana", None));

        b.write_chunk("01", "02", "\\v 4 Beto traduce").unwrap();
        let r = sync(&b.root, &url, Some(&creds()), &author("beto"), &opts("beto", Some("01/02")));
        assert!(r.error.is_none(), "{r:?}");
        assert!(r.conflicts.is_empty());
        assert_eq!(r.changed, vec!["01/01.txt"]);
        assert_eq!(b.read_chunk("01", "01").unwrap(), "\\v 1 Ana traduce");
        // Beto ve la reserva de Ana y la suya.
        let users: Vec<_> = r.claims.iter().map(|c| (c.key.as_str(), c.user.as_str())).collect();
        assert_eq!(users, vec![("01/01", "ana"), ("01/02", "beto")]);
        sync(&b.root, &url, Some(&creds()), &author("beto"), &opts("beto", None));

        let r = sync(&a.root, &url, Some(&creds()), &author("ana"), &opts("ana", None));
        assert_eq!(r.changed, vec!["01/02.txt"]);
        assert_eq!(a.read_chunk("01", "02").unwrap(), "\\v 4 Beto traduce");
        assert!(r.conflicts.is_empty());
    }

    #[test]
    fn el_mismo_chunk_deja_conflicto_y_no_se_sube_hasta_resolverlo() {
        let tmp = tempfile::tempdir().unwrap();
        let (url, a, b) = team(tmp.path());
        a.write_chunk("01", "01", "\\v 1 base").unwrap();
        sync(&a.root, &url, Some(&creds()), &author("ana"), &opts("ana", None));
        sync(&b.root, &url, Some(&creds()), &author("beto"), &opts("beto", None));
        assert_eq!(b.read_chunk("01", "01").unwrap(), "\\v 1 base");

        a.write_chunk("01", "01", "\\v 1 versión de Ana").unwrap();
        let r = sync(&a.root, &url, Some(&creds()), &author("ana"), &opts("ana", None));
        assert_eq!(r.push, Some(PushStatus::Ok));
        b.write_chunk("01", "01", "\\v 1 versión de Beto").unwrap();
        let r = sync(&b.root, &url, Some(&creds()), &author("beto"), &opts("beto", None));
        assert_eq!(r.conflicts, vec!["01/01.txt"]);
        assert!(r.push.is_none(), "con conflictos no se sube: {r:?}");
        let text = b.read_chunk("01", "01").unwrap();
        let options = crate::conflicts::parse(&text);
        assert!(options.iter().any(|o| o.contains("Ana")) && options.iter().any(|o| o.contains("Beto")));

        // Beto resuelve (se queda con la suya) y la siguiente pasada sí sube.
        b.write_chunk("01", "01", "\\v 1 versión de Beto").unwrap();
        let r = sync(&b.root, &url, Some(&creds()), &author("beto"), &opts("beto", None));
        assert!(r.conflicts.is_empty());
        assert_eq!(r.push, Some(PushStatus::Ok));
        let r = sync(&a.root, &url, Some(&creds()), &author("ana"), &opts("ana", None));
        assert_eq!(a.read_chunk("01", "01").unwrap(), "\\v 1 versión de Beto");
        assert!(r.conflicts.is_empty());
    }

    #[test]
    fn el_chunk_en_edicion_no_se_comparte_hasta_cerrarlo() {
        let tmp = tempfile::tempdir().unwrap();
        let (url, a, b) = team(tmp.path());
        a.write_chunk("01", "01", "\\v 1 base").unwrap();
        sync(&a.root, &url, Some(&creds()), &author("ana"), &opts("ana", None));
        sync(&b.root, &url, Some(&creds()), &author("beto"), &opts("beto", None));

        // Ana edita: varias pasadas con el texto a medias no suben nada de ese chunk.
        a.write_chunk("01", "01", "\\v 1 Ana escri").unwrap();
        let r = sync(&a.root, &url, Some(&creds()), &author("ana"), &opts("ana", Some("01/01")));
        assert!(r.error.is_none(), "{r:?}");
        a.write_chunk("01", "01", "\\v 1 Ana escribiendo").unwrap();
        sync(&a.root, &url, Some(&creds()), &author("ana"), &opts("ana", Some("01/01")));
        let r = sync(&b.root, &url, Some(&creds()), &author("beto"), &opts("beto", None));
        assert!(r.changed.is_empty(), "{r:?}");
        assert_eq!(b.read_chunk("01", "01").unwrap(), "\\v 1 base");
        assert_eq!(r.claims.iter().map(|c| c.user.as_str()).collect::<Vec<_>>(), vec!["ana"]);

        // Al cerrar ("Listo") se sube, y Beto lo recibe.
        let r = sync(&a.root, &url, Some(&creds()), &author("ana"), &opts("ana", None));
        assert_eq!(r.push, Some(PushStatus::Ok));
        assert!(r.conflicts.is_empty());
        let r = sync(&b.root, &url, Some(&creds()), &author("beto"), &opts("beto", None));
        assert_eq!(r.changed, vec!["01/01.txt"]);
        assert_eq!(b.read_chunk("01", "01").unwrap(), "\\v 1 Ana escribiendo");
    }

    #[test]
    fn si_otro_cambio_el_chunk_mientras_se_editaba_hay_un_solo_conflicto_al_cerrar() {
        let tmp = tempfile::tempdir().unwrap();
        let (url, a, b) = team(tmp.path());
        a.write_chunk("01", "01", "\\v 1 base").unwrap();
        sync(&a.root, &url, Some(&creds()), &author("ana"), &opts("ana", None));
        sync(&b.root, &url, Some(&creds()), &author("beto"), &opts("beto", None));

        // Ana abre el chunk; Beto lo cambia y sube sin hacer caso a la reserva.
        sync(&a.root, &url, Some(&creds()), &author("ana"), &opts("ana", Some("01/01")));
        a.write_chunk("01", "01", "\\v 1 versión de Ana").unwrap();
        b.write_chunk("01", "01", "\\v 1 versión de Beto").unwrap();
        let r = sync(&b.root, &url, Some(&creds()), &author("beto"), &opts("beto", None));
        assert_eq!(r.push, Some(PushStatus::Ok));

        // Mientras Ana sigue escribiendo no se le interrumpe: ni recarga ni conflicto.
        for _ in 0..2 {
            let r = sync(&a.root, &url, Some(&creds()), &author("ana"), &opts("ana", Some("01/01")));
            assert!(r.changed.is_empty() && r.conflicts.is_empty(), "{r:?}");
            assert_eq!(a.read_chunk("01", "01").unwrap(), "\\v 1 versión de Ana");
        }

        // Al cerrar aparece el conflicto una sola vez, con las dos versiones, y no se sube.
        let r = sync(&a.root, &url, Some(&creds()), &author("ana"), &opts("ana", None));
        assert_eq!(r.conflicts, vec!["01/01.txt"]);
        assert_eq!(r.changed, vec!["01/01.txt"]);
        assert!(r.push.is_none(), "{r:?}");
        let text = a.read_chunk("01", "01").unwrap();
        let options: Vec<String> = crate::conflicts::parse(&text).iter().map(|o| o.trim().to_string()).collect();
        assert_eq!(options, vec!["\\v 1 versión de Ana", "\\v 1 versión de Beto"]);
        let r = sync(&a.root, &url, Some(&creds()), &author("ana"), &opts("ana", None));
        assert!(r.changed.is_empty(), "el conflicto pendiente no se repite: {r:?}");

        // Ana resuelve y se sube.
        a.write_chunk("01", "01", "\\v 1 versión de Ana").unwrap();
        let r = sync(&a.root, &url, Some(&creds()), &author("ana"), &opts("ana", None));
        assert!(r.conflicts.is_empty());
        assert_eq!(r.push, Some(PushStatus::Ok));
        sync(&b.root, &url, Some(&creds()), &author("beto"), &opts("beto", None));
        assert_eq!(b.read_chunk("01", "01").unwrap(), "\\v 1 versión de Ana");
    }

    #[test]
    fn cerrar_sin_haber_escrito_adopta_lo_del_servidor() {
        let tmp = tempfile::tempdir().unwrap();
        let (url, a, b) = team(tmp.path());
        a.write_chunk("01", "01", "\\v 1 base").unwrap();
        sync(&a.root, &url, Some(&creds()), &author("ana"), &opts("ana", None));
        sync(&b.root, &url, Some(&creds()), &author("beto"), &opts("beto", None));

        sync(&a.root, &url, Some(&creds()), &author("ana"), &opts("ana", Some("01/01")));
        b.write_chunk("01", "01", "\\v 1 versión de Beto").unwrap();
        sync(&b.root, &url, Some(&creds()), &author("beto"), &opts("beto", None));
        sync(&a.root, &url, Some(&creds()), &author("ana"), &opts("ana", Some("01/01")));
        assert_eq!(a.read_chunk("01", "01").unwrap(), "\\v 1 base");

        let r = sync(&a.root, &url, Some(&creds()), &author("ana"), &opts("ana", None));
        assert!(r.conflicts.is_empty(), "{r:?}");
        assert_eq!(r.changed, vec!["01/01.txt"]);
        assert_eq!(a.read_chunk("01", "01").unwrap(), "\\v 1 versión de Beto");
    }

    #[test]
    fn las_reservas_se_fusionan_y_caducan() {
        let tmp = tempfile::tempdir().unwrap();
        let (url, a, b) = team(tmp.path());
        // Ambos reservan sin haber visto al otro: collab.json entra en conflicto y se une.
        a.write_chunk("01", "01", "a").unwrap();
        sync(&a.root, &url, Some(&creds()), &author("ana"), &opts("ana", Some("01/01")));
        b.write_chunk("01", "02", "b").unwrap();
        // Beto reserva antes de bajar nada (su collab.json nace sin la de Ana).
        assert!(claim(&b.root, "01/02", "beto", now()).unwrap());
        let r = sync(&b.root, &url, Some(&creds()), &author("beto"), &opts("beto", Some("01/02")));
        assert!(r.conflicts.is_empty(), "collab.json nunca queda con marcadores: {r:?}");
        assert_eq!(r.claims.len(), 2);
        // Reservar un chunk que tiene otro: se rechaza.
        assert!(!claim(&b.root, "01/01", "beto", now()).unwrap());
        // Una reserva vieja desaparece sola.
        let mut f = load(&b.root);
        f.claims.get_mut("01/01").unwrap().until = 10;
        save(&b.root, &f).unwrap();
        assert_eq!(active(&b.root, now()).len(), 1);
        assert!(claim(&b.root, "01/01", "beto", now()).unwrap());
        release(&b.root, "01/01", "beto", now()).unwrap();
        assert!(active(&b.root, now()).iter().all(|c| c.key != "01/01"));
    }

    #[test]
    fn merge_json_une_y_gana_la_mas_reciente() {
        let a = r#"{"version":1,"claims":{"01/01":{"user":"ana","since":1,"until":100}}}"#;
        let b = r#"{"version":1,"claims":{"01/01":{"user":"beto","since":2,"until":200},"01/02":{"user":"beto","since":2,"until":50}}}"#;
        let m: CollabFile = serde_json::from_str(&merge_json(a, b)).unwrap();
        assert_eq!(m.claims["01/01"].user, "beto");
        assert_eq!(m.claims["01/02"].user, "beto");
        let m2: CollabFile = serde_json::from_str(&merge_json("basura", b)).unwrap();
        assert_eq!(m2.claims.len(), 2);
    }

    #[test]
    fn sin_servidor_se_informa_como_sin_conexion() {
        let tmp = tempfile::tempdir().unwrap();
        let a = Project::create(&tmp.path().join("ana"), manifest()).unwrap();
        a.write_chunk("01", "01", "a").unwrap();
        let r = sync(&a.root, "https://127.0.0.1:9/no/existe.git", Some(&creds()), &author("ana"), &opts("ana", None));
        assert!(r.offline, "{r:?}");
        // Lo local sigue guardado y con commit.
        assert!(gitrepo::head_hash(&a.root).is_some());
    }
}
