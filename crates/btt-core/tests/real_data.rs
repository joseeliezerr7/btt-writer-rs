//! Pruebas contra datos reales de BTT Writer 1.6/2.0 presentes en este equipo.
//! Se omiten (pasan) si las carpetas no existen, para que la suite funcione en CI.

use btt_core::gitrepo::Author;
use btt_core::library::Library;
use btt_core::project::Project;
use btt_core::rc::ResourceContainer;
use btt_core::tstudio::{self, ImportMode, ImportStatus};
use std::path::{Path, PathBuf};

fn home() -> PathBuf {
    PathBuf::from(std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).unwrap_or_default())
}

fn author() -> Author {
    Author::new("Prueba", "prueba@example.com")
}

#[test]
fn imports_real_tstudio_from_btt_writer_2() {
    // El export de 2.0 trae el manifest raíz con ruta absoluta y barras invertidas.
    let file = home().join("BTT-Writer-2.0/backups/cbt_3jn_text_reg.tstudio");
    if !file.is_file() {
        eprintln!("omitida: no existe {}", file.display());
        return;
    }
    let info = tstudio::inspect(&file).unwrap();
    assert_eq!(info.package_version, 3);
    assert_eq!(info.target_translations[0].id, "cbt_3jn_text_reg");
    assert!(!info.target_translations[0].commit_hash.is_empty());

    let tmp = tempfile::tempdir().unwrap();
    let out = tstudio::import(&file, tmp.path(), ImportMode::Skip, &author()).unwrap();
    assert_eq!(out[0].status, ImportStatus::Imported, "{:?}", out[0]);
    let p = Project::open_by_id(tmp.path(), "cbt_3jn_text_reg").unwrap();
    assert_eq!(p.manifest.target_language.name, "Chayahuita");
    assert_eq!(p.manifest.translators, vec!["Joseeliezerr123"]);
    assert_eq!(p.chunks("01").unwrap(), vec!["title", "01", "05", "09", "11", "13"]);
    assert!(p.read_chunk("01", "01").unwrap().starts_with("\\v 1 El anciano"));
    // El historial git original viaja en el archivo.
    assert!(btt_core::gitrepo::file_history(&p.root, "01/01.txt").unwrap().len() >= 1);
    // Export USFM del proyecto real.
    let usfm = btt_core::usfm::export_book(&p).unwrap();
    assert!(usfm.starts_with("\\id 3JN"));
    assert!(usfm.contains("\\c 1\n\\s5\n\\p\n\\v 1 El anciano"));
}

#[test]
fn imports_all_automatic_backups_from_btt_writer_1_6() {
    let dir = home().join("BTT-Writer/automatic_backups");
    if !dir.is_dir() {
        eprintln!("omitida: no existe {}", dir.display());
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let mut ok = 0;
    for entry in std::fs::read_dir(&dir).unwrap().flatten() {
        let path = entry.path();
        if path.extension().map(|e| e == "tstudio").unwrap_or(false) {
            let out = tstudio::import(&path, tmp.path(), ImportMode::Skip, &author())
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            for o in out {
                assert!(matches!(o.status, ImportStatus::Imported | ImportStatus::Exists), "{}: {:?}", path.display(), o);
                let p = Project::open_by_id(tmp.path(), &o.id).unwrap();
                assert_eq!(p.manifest.package_version, 8);
                ok += 1;
            }
        }
    }
    eprintln!("{ok} proyectos reales importados");
    assert!(ok > 0);
}

#[test]
fn opens_real_index_and_containers_from_btt_writer_2() {
    let root = home().join("BTT-Writer-2.0");
    if !root.join("database/index.sqlite").is_file() {
        eprintln!("omitida: no existe {}", root.display());
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let idx = tmp.path().join("index.sqlite");
    std::fs::copy(root.join("database/index.sqlite"), &idx).unwrap();
    let lib = Library::open(&idx, &root.join("resource_containers")).unwrap();
    let langs = lib.target_languages("", 0).unwrap();
    assert!(langs.len() > 9000, "{}", langs.len());
    let sources = lib.source_texts("1jn", false).unwrap();
    assert!(sources.iter().any(|s| s.language_slug == "am" && s.slug == "ulb" && s.downloaded));
    let rc = lib.open_container("am", "1jn", "ulb").unwrap();
    assert_eq!(rc.chapters().len(), 5);
    let chunk = rc.source_chunk("01", "01").unwrap();
    assert_eq!(chunk.verses[0].number, "1");
    assert!(!chunk.verses[0].text.is_empty());
    assert_eq!(rc.words_for_chunk("01", "01").first().map(String::as_str), Some("declare"));
    let tn = lib.open_container("am", "1jn", "tn").unwrap();
    assert!(!tn.help_entries("01", "01").is_empty());
    let tw = lib.open_container("am", "bible", "tw").unwrap();
    assert!(tw.word_slugs().len() > 100);
    assert!(tw.word("god").is_some());
}

#[test]
fn all_real_containers_open() {
    let dir = home().join("BTT-Writer-2.0/resource_containers");
    if !dir.is_dir() {
        return;
    }
    let mut n = 0;
    for entry in std::fs::read_dir(&dir).unwrap().flatten() {
        let p: PathBuf = entry.path();
        if p.extension().map(|e| e == "tsrc").unwrap_or(false) {
            let rc = ResourceContainer::open(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            assert!(!rc.slug().is_empty());
            n += 1;
        }
    }
    assert!(n > 0, "sin contenedores en {}", dir.display());
    let _ = Path::new("");
}
