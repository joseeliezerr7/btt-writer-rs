# BTT Writer (Rust + Tauri)

Reescritura de **BTT Writer** (Bible Translation Tools, Wycliffe Associates) con el núcleo en Rust y la interfaz en Tauri 2 + React. Toma como referencia funcional **BTT-Writer 2.0** (Kotlin Multiplatform, agosto 2026) y mantiene compatibilidad total de datos con 1.6 y 2.0:

- Mismos proyectos en disco (`manifest.json` v8, `{cap}/{chunk}.txt`, `.git/`).
- Mismo `index.sqlite` y mismos contenedores de recursos `.tsrc` (tar.bz2).
- Mismo formato de respaldo `.tstudio` (lee v1, v2 y v3; escribe v3).
- Misma exportación USFM (`\s5` entre chunks) e importación USFM.
- Mismo servidor WACS/DCS (API Gitea) para subir, buscar y clonar proyectos.

Plataformas: Windows, macOS, Linux y, con el mismo código, Android e iOS (Tauri 2 móvil).

## Estructura

```
crates/btt-core/        núcleo (sin UI): manifest, proyectos, USFM/USX, contenedores,
                        biblioteca SQLite, catálogo, git, conflictos, .tstudio,
                        validación, WACS, respaldos, impresión, ajustes
apps/desktop/           app Tauri 2 (src-tauri = comandos Rust, src = React + TS)
apps/desktop/src-tauri/resources/   index.sqlite y containers.zip empaquetados
docs/inventario-btt-writer-2.0.md   inventario funcional de la versión original
scripts/cargo.cmd       envoltorio de cargo con el entorno de Visual Studio (Windows)
```

## Requisitos (Windows)

- Rust 1.80+ (`rustup`), Visual Studio Build Tools 2022 (C++ y Windows SDK).
- Node 20+ y npm.
- Para Android: Android Studio (SDK + NDK) y `rustup target add aarch64-linux-android`.

## Comandos

```bash
# pruebas del núcleo (55 pruebas; las de red se ejecutan con --ignored)
.\scripts\cargo.cmd test -p btt-core

# lint
.\scripts\cargo.cmd clippy -p btt-core --all-targets

# app de escritorio en desarrollo (compila Rust + Vite)
cd apps/desktop && npm install && npm run tauri dev

# instaladores (MSI/NSIS en Windows, DMG en macOS, DEB/AppImage en Linux)
cd apps/desktop && npm run tauri build
# en Windows, con el entorno de Visual Studio cargado:
.\scripts\tauri.cmd build     # deja los instaladores en %USERPROFILE%\.cargo-target\btt-writer-rs\release\bundle\{msi,nsis}

# macOS/Linux (no se pueden generar desde Windows): el flujo .github/workflows/release.yml
# compila DMG universal, MSI/NSIS y DEB/RPM/AppImage en GitHub Actions al publicar una etiqueta:
#   git tag v3.0.1 && git push origin main --tags   (o a mano desde la pestaña Actions)

# Publicar una versión nueva (las apps instaladas se actualizan solas):
#   1. sube "version" en apps/desktop/src-tauri/tauri.conf.json, apps/desktop/package.json y Cargo.toml
#   2. git commit -am "v3.0.2" && git tag v3.0.2 && git push origin main --tags
#   GitHub Actions publica el Release con los instaladores y latest.json firmado con la clave del
#   actualizador (secreto TAURI_SIGNING_PRIVATE_KEY; la clave local vive en %USERPROFILE%\.tauri\btt-writer.key).

# Android (una vez: npm run tauri android init)
cd apps/desktop && npm run tauri android dev
```

En Windows, si `cargo` falla con `LNK1104: no se puede abrir msvcrt.lib`, ejecuta los comandos a través de `.\scripts\cargo.cmd` o desde un "Developer Command Prompt", que cargan el entorno del compilador.

## Datos

Los datos del usuario viven en `~/BTT-Writer-3` (configurable con `BTT_WRITER_HOME`):

```
translations/   proyectos        backups/   respaldos automáticos .tstudio
database/       index.sqlite     resource_containers/   fuentes .tsrc
exports/        USFM, HTML       settings.json  profile.json
```

La primera vez se despliegan el `index.sqlite` y los contenedores empaquetados en `containers.zip` (inglés: ULB/UDB/tN de todos los libros, tA y tW; español latinoamericano `es-419`: ULB, tN, tQ de todos los libros y tW), así que en español no hace falta descargar nada para empezar. En cada arranque se copian los que falten, de modo que una versión nueva añade contenedores también a instalaciones existentes. Desde **Inicio → Más opciones → Migrar de la app anterior** se importan los proyectos de `~/BTT-Writer-2.0` o `~/BTT-Writer` sin tocar la carpeta original.

## Funciones

- Cuenta sin conexión o con WACS (token por dispositivo), aviso de privacidad y términos.
- Nuevo proyecto: idioma destino (búsqueda, idiomas temporales), libro por categoría, tipo (Regular; con modo Gateway Language: ULB, UDB, Notas, Preguntas, Palabras; OBS).
- Traducción en tres modos: lectura (capítulo completo), chunk (fuente ↔ borrador con autoguardado) y revisión (marcar terminado con lista de verificación y validación de versos, notas al pie, historial/deshacer vía git, búsqueda, recursos tN/tW/tQ con índice de palabras, borrador a ciegas).
- Hasta 3 fuentes abiertas; descarga de fuentes y ayudas desde el catálogo; importación manual de `.tsrc` e `index.sqlite`.
- Conflictos de git resueltos en la propia tarjeta del chunk (incluye anidados).
- Importar: desde WACS (clonar), `.tstudio`, USFM, respaldos. Exportar: `.tstudio`, USFM, impresión/PDF (HTML con portada, licencia, índice y notas), subida a WACS con pull/push y rama de respaldo.
- Publicar: validación de todo el proyecto, colaboradores, subida.
- Respaldos automáticos cada N minutos (solo si hubo cambios).
- Ajustes: tema claro/oscuro, idioma de la interfaz (es/en), fuentes y tamaños, servidor (WACS/WACS DEV/DCS), intervalo de respaldo.

## Limitaciones conocidas

- El CDN de fuentes (`cdn.bibletranslationtools.org`) está detrás de una protección anti-bot de Cloudflare que rechaza identificaciones con URL (como `curl`). La app se identifica como `BTT-Writer/3.0` y descarga correctamente (prueba en vivo: `.\scripts\cargo.cmd test -p btt-core --test live_download -- --ignored`). Si alguna vez rechaza la descarga, la app lo indica y permite importar `.tsrc` o `index.sqlite` manualmente.
- El envío de comentarios guarda un archivo en `logs/` (el webhook del helpdesk original requiere un token privado).
- La subida a WACS usa HTTPS con token (no claves SSH).
- En Android `git2` se compila sin transporte HTTPS (compilar OpenSSL desde Windows falla): el historial git local funciona, pero subir/clonar desde WACS en el móvil queda pendiente (se resuelve compilando el APK desde Linux/macOS o con OpenSSL precompilado para NDK).
- Compilar Android en Windows: `.\scripts\android.cmd build --debug --target aarch64 --apk` (carga el entorno de Visual Studio, el SDK/NDK y el JDK de Android Studio).
