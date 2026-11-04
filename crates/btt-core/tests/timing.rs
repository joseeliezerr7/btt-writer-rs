//! Medición de tiempos de la carga de la lista con los datos reales de ~/BTT-Writer-3.
//! `cargo test -p btt-core --test timing -- --ignored --nocapture`
use std::path::PathBuf;
use std::time::Instant;

use btt_core::gitrepo;
use btt_core::library::Library;
use btt_core::project::Project;

fn home() -> PathBuf {
    PathBuf::from(std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).unwrap_or_default())
}

#[test]
#[ignore]
fn tiempos_de_lista_de_proyectos() {
    let root = home().join("BTT-Writer-3");
    if !root.join("translations").is_dir() {
        return;
    }
    let t = Instant::now();
    let projects = Project::list(&root.join("translations")).unwrap();
    eprintln!("Project::list ({}): {:?}", projects.len(), t.elapsed());

    let t = Instant::now();
    for p in &projects {
        let _ = p.summary();
    }
    eprintln!("summary x{}: {:?}", projects.len(), t.elapsed());

    let t = Instant::now();
    for p in &projects {
        let _ = gitrepo::conflicted_files(&p.root);
    }
    eprintln!("conflicted_files x{}: {:?}", projects.len(), t.elapsed());

    let tmp = tempfile::tempdir().unwrap();
    std::fs::copy(root.join("database/index.sqlite"), tmp.path().join("index.sqlite")).unwrap();
    let lib = Library::open(&tmp.path().join("index.sqlite"), &root.join("resource_containers")).unwrap();
    let t = Instant::now();
    let mut opened = 0;
    let mut cache = std::collections::HashSet::new();
    for p in &projects {
        for s in &p.manifest.source_translations {
            let key = format!("{}_{}_{}", s.language_id, p.manifest.project.id, s.resource_id);
            if cache.insert(key) {
                if lib.open_container(&s.language_id, &p.manifest.project.id, &s.resource_id).is_ok() {
                    opened += 1;
                }
            }
        }
    }
    eprintln!("open_container x{opened}: {:?}", t.elapsed());

    let t = Instant::now();
    for p in &projects {
        let _ = lib.chunk_markers(&p.manifest.project.id);
    }
    eprintln!("chunk_markers x{}: {:?}", projects.len(), t.elapsed());

    let t = Instant::now();
    for p in &projects {
        let _ = gitrepo::head_hash(&p.root);
    }
    eprintln!("head_hash x{}: {:?}", projects.len(), t.elapsed());
}

#[test]
fn scan_cuenta_chunks_y_detecta_conflictos() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("aa_jud_text_reg");
    std::fs::create_dir_all(root.join("01")).unwrap();
    std::fs::create_dir_all(root.join("front")).unwrap();
    std::fs::write(
        root.join("manifest.json"),
        r#"{"package_version":8,"format":"usfm","generator":{"name":"ts-desktop","build":"1"},"target_language":{"id":"aa","name":"Afar","direction":"ltr"},"project":{"id":"jud","name":"Judas"},"type":{"id":"text","name":"Text"},"resource":{"id":"reg","name":"Regular"}}"#,
    )
    .unwrap();
    std::fs::write(root.join("front/title.txt"), "Judas").unwrap();
    std::fs::write(root.join("01/01.txt"), "texto").unwrap();
    std::fs::write(root.join("01/03.txt"), "   \n").unwrap();
    std::fs::write(root.join("01/05.txt"), "<<<<<<< HEAD\na\n=======\nb\n>>>>>>> refs/heads/master\n").unwrap();
    let p = Project::open(&root).unwrap();
    let scan = p.scan();
    // front/title.txt no cuenta (igual que `summary()`); 01/03 está vacío.
    assert_eq!(scan.translated_chunks, 2);
    assert!(scan.has_conflicts);
    assert!(scan.modified_at_ms > 0);
    let s = p.summary_from_scan(&scan);
    assert_eq!(s.translated_chunks, 2);
    assert_eq!(s.modified_at, scan.modified_at_ms / 1000);
    assert_eq!(p.summary().translated_chunks, 2);

    std::fs::remove_file(root.join("01/05.txt")).unwrap();
    let scan2 = p.scan();
    assert_eq!(scan2.translated_chunks, 1);
    assert!(!scan2.has_conflicts);
}

#[test]
#[ignore]
fn tiempos_de_scan() {
    let root = home().join("BTT-Writer-3");
    if !root.join("translations").is_dir() {
        return;
    }
    let projects = Project::list(&root.join("translations")).unwrap();
    let t = Instant::now();
    for p in &projects {
        let _ = p.modified_at_ms();
    }
    eprintln!("modified_at_ms x{}: {:?}", projects.len(), t.elapsed());
    let t = Instant::now();
    for p in &projects {
        let _ = p.scan();
    }
    eprintln!("scan x{}: {:?}", projects.len(), t.elapsed());
}
