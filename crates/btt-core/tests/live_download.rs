//! Prueba en vivo contra el servidor de contenidos (requiere red).
//! Se ejecuta a propósito: `cargo test -p btt-core --test live_download -- --ignored`.
use std::path::Path;

use btt_core::library::Library;
use btt_core::sources::{download_resource, http_client};

#[test]
#[ignore]
fn descarga_fuente_real_desde_cdn() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/index.sqlite");
    let tmp = tempfile::tempdir().unwrap();
    let index = tmp.path().join("index.sqlite");
    std::fs::copy(&fixture, &index).unwrap();
    let containers = tmp.path().join("resource_containers");
    std::fs::create_dir_all(&containers).unwrap();

    let lib = Library::open(&index, &containers).unwrap();
    let row = lib.resource("es-419", "jon", "ulb").unwrap().expect("es-419 jon ulb en el índice");
    assert!(row.url.contains("cdn.bibletranslationtools.org"), "url: {}", row.url);
    assert!(!row.downloaded);

    let client = http_client().unwrap();
    let slugs = download_resource(&lib, &row, &client, false).expect("descarga desde el CDN");
    assert!(slugs.iter().any(|s| s == "es-419_jon_ulb"), "slugs: {slugs:?}");

    let rc = lib.open_container("es-419", "jon", "ulb").unwrap();
    let chapters = rc.chapters();
    assert!(chapters.iter().any(|c| c == "01"), "capítulos: {chapters:?}");
    let chunk = rc.read_chunk("01", "01").expect("chunk 01/01");
    assert!(chunk.contains("Jonás") || chunk.contains("Jon"), "chunk: {chunk}");
}
