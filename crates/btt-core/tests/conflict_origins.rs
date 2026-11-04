//! Origen (autor y fecha) de cada versión de un conflicto según el historial git.
use btt_core::gitrepo::{self, Author};

#[test]
fn cada_version_se_atribuye_a_su_commit() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("aa_jud_text_reg");
    std::fs::create_dir_all(root.join("01")).unwrap();
    std::fs::write(root.join("manifest.json"), "{}").unwrap();

    let ana = Author::new("Ana", "ana@example.com");
    let beto = Author::new("Beto", "beto@example.com");

    std::fs::write(root.join("01/01.txt"), "\\v 1 version de Ana").unwrap();
    gitrepo::commit_all(&root, &ana, "autosave").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    std::fs::write(root.join("01/01.txt"), "\\v 1 version de Beto").unwrap();
    gitrepo::commit_all(&root, &beto, "autosave").unwrap();

    // Conflicto escrito por una fusión: HEAD (Beto) contra la de Ana.
    let conflict = "<<<<<<< HEAD\n\\v 1 version de Beto\n=======\n\\v 1 version de Ana\n>>>>>>> refs/heads/master\n";
    std::fs::write(root.join("01/01.txt"), conflict).unwrap();
    gitrepo::commit_all(&root, &beto, "autosave").unwrap();

    let versions = btt_core::conflicts::parse(conflict);
    assert_eq!(versions.len(), 2);
    let origins = gitrepo::version_origins(&root, "01/01.txt", &versions).unwrap();
    assert_eq!(origins.len(), 2);
    let b = origins[0].as_ref().expect("origen de la versión de Beto");
    let a = origins[1].as_ref().expect("origen de la versión de Ana");
    assert_eq!(b.author, "Beto");
    assert_eq!(a.author, "Ana");
    assert!(b.time > a.time, "Beto es más reciente");

    // Una versión que nunca se guardó no tiene origen.
    let origins = gitrepo::version_origins(&root, "01/01.txt", &["texto inventado".to_string()]).unwrap();
    assert!(origins[0].is_none());

    // El historial del chunk ya informa el autor.
    let history = gitrepo::file_history(&root, "01/01.txt").unwrap();
    assert_eq!(history[0].author, "Beto");
    assert_eq!(history.last().unwrap().author, "Ana");
}
