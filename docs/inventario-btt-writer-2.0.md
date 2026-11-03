# Inventario funcional de BTT-Writer 2.0 (referencia para la reescritura en Rust)

Fuente analizada: `btt-writer-src/multi-default/BTT-Writer-default/` (Kotlin Multiplatform, v2.0.0 build 59, agosto 2026).
Stack original: Compose Multiplatform, Decompose, Koin, JGit + SSH Apache + BouncyCastle, Ktor, iText 5, DataStore.
Tres librerías externas sin código en el repo: `resource-container` (RC 0.1), `resource-catalog-client` (index.sqlite + catálogo tS v2), `gogs-client`.

## 1. Modelo de datos

### Entidades
- `TargetTranslation(path)`: id, idioma destino, proyecto, tipo (`text|tn|tq|tw`), recurso (solo text), formato, repo git, colaboradores, numTranslated, numFinished.
- `Chunk(chapterSlug, chunkSlug)`: `front/title`, `{cap}/title`, `{cap}/reference`, `{cap}/{verso}`. Config desde `source.config["content"][cap][chunk]` (`words: [//bible/tw/x]`).
- `TranslationHelp {title, body}`: contenido de chunks tN/tQ/tW como JSON `[{"title":"…","body":"…"}]` (indent tab).
- `Profile {fullName, gogsUser?, gogsToken?, termsOfUseLastAccepted}` (preferencia `profile`, JSON).
- `ResourceType`: TEXT("text","Text"), TRANSLATION_NOTE("tn","Notes"), TRANSLATION_QUESTION("tq","Questions"), TRANSLATION_WORD("tw","Words").
- `ProjectTypeClass`: STANDARD (text), HELPS (tn,tq), EXTANT (tw).
- `TranslationFormat`: usfm, usx, markdown, default, txt.
- `TranslationViewMode`: LOADING, READ, CHUNK, REVIEW.
- Orden bíblico: 39 AT (`gen…mal`) + 27 NT (`mat…rev`).

### manifest.json (package_version 8)
```json
{
  "package_version": 8,
  "format": "usfm",
  "generator": { "name": "ts-android", "build": "59" },
  "target_language": { "id": "aaa", "name": "Ghotuo", "direction": "ltr", "anglicized_name": "Ghotuo", "region": "Africa", "is_gateway_language": false },
  "project":  { "id": "mrk", "name": "" },
  "type":     { "id": "text", "name": "Text" },
  "resource": { "id": "reg", "name": "Regular" },
  "source_translations": [ { "language_id": "en", "resource_id": "ulb", "checking_level": "3", "date_modified": 20220324, "version": "21-05" } ],
  "translators": [ "Nombre" ],
  "finished_chunks": [ "01-01", "01-04", "01-title", "01-reference", "front-title" ],
  "parent_draft": { "resource_id": "", "checking_entity": "", "checking_level": "", "comments": "", "contributors": "", "publish_date": 0, "source_text": "", "source_text_version": "", "version": "" }
}
```
- Nombres de recurso: reg→Regular, ulb→Unlocked Literal Bible, udb→Unlocked Dynamic Bible, obs→Open Bible Stories. Para tn/tq/tw: `resource {"id":"","name":""}`.
- `finished_chunks` y `translators` ordenados. `mergeManifests` = unión de translators, finished_chunks, source_translations.
- Migraciones v2→v8 (`TargetTranslationMigrator`), slug antiguo `project-lang-res` → `lang_project_res`.

### Disco
```
~/BTT-Writer-2.0/translations/{id}/      id = {lang}_{project}_{type}[_{resource}]
  manifest.json, LICENSE.md (CC BY-SA 4.0), .git/
  front/title.txt, 01/title.txt, 01/reference.txt, 01/01.txt, 01/04.txt …
```
- Texto vacío → se borra el archivo. tW: capítulo fijo `01/` y chunk = slug de palabra (`01/aaron.txt`).
- Progreso = finished / total chunks del RC fuente (fallback en/ulb).

### Biblioteca local
- `~/BTT-Writer-2.0/database/index.sqlite` + `resource_containers/*.tsrc` (**tar.bz2**). Al arrancar por primera vez se despliegan `index.sqlite` (4.3 MB) y `containers.zip` (11 RC: en_mrk/en_gen ulb-udb-tn, en_bible_tw, ru_mrk ulb-tn-tq, ru_bible_tw).
- Tablas: source_language, project, category, category_name, resource (type book|help|dict|man, translate_mode all|gl), resource_format (mime `application/tsrc+book|help|dict|man`, url), legacy_resource_info, target_language, temp_target_language, versification, versification_name, chunk_marker, catalog (langnames, new-language-questions, temp-langnames, approved-temp-langnames), questionnaire, question, questionnaire_data_field.
- RC 0.1: `package.json`, `LICENSE.md`, `content/toc.yml`, `content/config.yml`, `content/front/title.usx`, `content/01/title.usx`, `content/01/01.usx`. tN/tQ: `content/01/01.md` (`#título\n\ncuerpo`). tW: `content/{palabra}/01.md` + config (`def_title/see_also/aliases`).
- Descarga de fuentes: `resource_format.url` (`https://cdn.bibletranslationtools.org//bible/{lang}/{res}/{book}/source.json`, `…/tn/{lang}/{book}/notes.json`, `…/tq/{lang}/{book}/questions.json`, `…/tw/{lang}/words.json`, `tw_cat.json`) → conversión a RC. Al bajar un libro también se bajan tW, tN y tQ del mismo idioma.

### Preferencias
`profile`, `last_translation`, `last_version_code`, `last_view_mode_{id}`, `last_focus_chapter_{id}`, `last_focus_frame_{id}`, `open_source_translations_{id}` (máx. 3, separadas por `|`), `selected_source_translation_{id}`, `last_backup_{id}`, `last_uploaded_{id}`, `translation_typeface`, `typeface_size`, `source_typeface`, `source_typeface_size`, `gogs_api`, `index_sqlite_url`, `lang_names_url`, `media_server`, `reader_server`, `create_account_url`, `color_theme`, `content_server`, `tm_url`, `check_hardware_requirements`, `enable_tm_links`, `gl_mode`, `backup_interval` (1/5/10/-1 min), `logging_level`, `sort_by_project`, `sort_by_book`, `search_source`.

## 2. Red
| Uso | WACS (defecto) | WACS DEV | DCS |
|---|---|---|---|
| API Gitea | `https://content.bibletranslationtools.org/api/v1` | `https://content.wacsdev.org/api/v1` | `https://git.door43.org/api/v1` |
| Media server (catálogo tS v2) | `https://api.bibletranslationtools.org` | `https://api-dev.bibletranslationtools.org` | `https://api.unfoldingword.org` |
| Reader | `https://read.bibleineverylanguage.org` | `https://read-dev.bibleineverylanguage.org` | `https://door43.org/u` |
| Crear cuenta | `{data}/user/sign_up` | | |
| langnames | `https://langnames.bibleineverylanguage.org/langnames.json` | | `https://td.unfoldingword.org/exports/langnames.json` |
| index.sqlite | `https://writer-resources.bibletranslationtools.org/index.sqlite` | | |

- Catálogo raíz: `{media}/ts/txt/2/catalog.json` → `lang_catalog` → `res_catalog` (`source`, `notes`, `checking_questions`, `terms`, `tw_cat`, `chunks`, `status.checking_level`). Solo se ofrecen fuentes con checking_level ≥ 3.
- Idiomas temporales: `https://td.unfoldingword.org/api/templanguages/` y `…/assignment/changed/`.
- Imágenes OBS: `https://cdn.unfoldingword.org/obs/jpg/obs-images-360px.zip`.
- Actualizaciones: `https://api.github.com/repos/Bible-Translation-Tools/BTT-Writer-Android/releases/latest`.
- Feedback/crash: `https://helpdesk.techadvancement.com/wp-json/fluent-support/v2/public/incoming_webhook/{HELPDESK_TOKEN}` (POST multipart `title`, `content`, `sender[email]`).
- Translation Manual: `https://read.bibleineverylanguage.org/WA-Catalog/en_tm?section=…#…`.

### API Gogs/Gitea
- Login: `GET /user` (Basic), `listTokens/deleteToken/createToken` (nombre `btt-writer-android__{fabricante}_{udid}__{device}`, scopes `write:repository`, `write:user`), `editUser`.
- SSH: `GET /users/{user}/keys`, `POST /user/keys`, `DELETE /user/keys/{id}` (Ed25519, título `btt-writer-android {udid}`).
- Repos: `POST /user/repos` (409 = ya existe), `GET /repos/search?q=&uid=&limit=` (`{"data":[…],"ok":true}`), `GET /repos/{owner}/{repo}`, `GET /users/search`. Nombre de repo = id del proyecto.

## 3. Git
- `git init` al abrir; commit "auto save" cada 2 min, al cerrar, antes de backup/export/push/pull/merge, al marcar chunk hecho.
- Push `refs/heads/master` sin force. Pull con rama `backup-master`; conflictos en manifest → THEIRS + mergeManifests; LICENSE.md → OURS; contenido → modo Review con filtro de conflictos; rechazo → "Upload failed because of changes on Server. Review conflicts?" (No → reset a backup-master).
- Marcadores `<<<<<<< HEAD` … `=======` … `>>>>>>>`; la UI muestra las opciones y se elige una.
- Historial por archivo (undo/redo con `git log -- path`).

## 4. USFM / USX
- Renderizado: `\c` consumido; `\p \m` párrafo; `\qN` poesía con sangría; `\qr` derecha; `\qs` Selah; `\s \sN` sección; `\ms` sección mayor; `\b` salto; `\cl` título; `\v N(-M)` verso; `\f + …\f*` nota (fr ft fk fq fqa fl fp fv fdc); otros marcadores se eliminan del texto mostrado.
- Modos de verso: PIN (arrastrable en Review), NUMBER, RAW (edición).
- Validación al marcar chunk hecho: no vacío, sin `\v` consecutivos, versos dentro del rango de la fuente, sin duplicados ni desorden; versos faltantes se añaden como `\v N ` al abrir el chunk.
- Export USFM: `\id MRK {título}, {libro}, {lang}, {idioma}` / `\ide usfm` / `\h` / `\toc1` / `\toc2` / `\toc3 MRK` / `\mt` / `\s5` / `\c 1` / `\cl` / `\cd` / `\p` / chunks separados por `\s5`.
- PDF: Letter, márgenes 1", portada, licencia, índice, notas al pie por capítulo, imágenes OBS, RTL; opciones "incluir imágenes" e "incluir chunks incompletos".
- Helps markdown: enlaces `[[:en:ta:…]]`→TM, `[[:en:bible:notes:gen:01:03]]`→pasaje, `[t](../kt/priest.md)`→tW, `rc://`.

## 5. UI (pantallas y diálogos)
- Splash (Slow Device, Migrate from Old App, UpdateApp) → Profile (Login online / Crear cuenta / Cuenta offline → Privacy Notice → Terms of Use con License Agreement, Translation Guidelines, Statement of Faith).
- Home: "Your Translation Projects", usuario actual, Log Out, FAB Add, tarjetas con tipo de recurso, idioma, pastel de progreso e Info; ordenación (proyecto/idioma/progreso; orden bíblico/alfabético); menú More Options: Update, Import, Feedback, Share, Log Out, Settings; reabre el último proyecto.
- Project details: proyecto, idioma (cambiar), tipo de recurso (cambiar ulb/udb/reg), progreso, colaboradores, último backup, última subida; Delete (backup zip previo), Print, Publish, Backup.
- New Translation: idioma (búsqueda) → categoría/libro → tipo (solo GL mode: ULB, UDB, Notes, Questions); sin GL crea `text/reg`; OBS → `text/obs`; translationWords → `tw`.
- Translate: barra con Read/Chunk/Review, aviso de conflictos, slider de capítulos; menú Home, borradores, Project Review, Upload/Export, Print, Feedback, Search, Mark All Chunks Done, Settings. Pestañas de fuentes (máx. 3) + Add Source. Sin fuente → "Choose source translation(s)".
  - Read: tarjeta por capítulo, fuente/destino que se voltea, "Begin translating".
  - Chunk: tarjeta por chunk, fuente ↔ destino editable (autoguardado 500 ms); chunk hecho pide confirmación para reabrir.
  - Review: fuente + destino con estados MARKER / EDIT (undo/redo, footnote en cursor, búsqueda) / COMPLETE; "Mark chunk as done" → checklist → validaciones; panel de recursos (Notes / Words con índice / Questions); búsqueda fuente/destino; Mark All Chunks Done; tarjeta de conflictos. tN/tQ: editor de entradas título/cuerpo + tarjeta con la traducción del libro; tW: editor palabra/definición.
- Draft: lista de borradores, vista por capítulo, Import Draft (reemplaza y fija parent_draft).
- Publish: pestañas Project (validación por chunk con botón Review), Contributors, Upload/Export.
- Settings: General (tema, GL mode, fuentes y tamaños 15/25/35/45, fuente personalizada .ttf/.otf, versión, actualizaciones), Server (WACS/WACS DEV/DCS y URLs), Legal, Advanced (migrar app antigua, hardware check, TM links, intervalo de backup, nivel de log, dev tools).
- Dev tools: versión/build/udid, regenerar claves SSH, leer log, simular crash, recursos del sistema, borrar biblioteca.
- Crash reporter y Feedback (email opcional + descripción, adjunta log).
- Diálogos: Import (desde servidor, archivo .tstudio/.zip, USFM, texto fuente RC, backups; "Project Already Exists" → Merge/Overwrite/Cancel), Import USFM (idioma → libro faltante → resumen → merge/overwrite → resultados), Export (Upload to Server, USFM, PDF, Project File, Send to another app; "Upload Complete … {reader}/{user}/{id}"), Source Selection (Selected / Available / Available Online con descarga y borrado, máx. 3), Update Library (lista de fuentes, index.sqlite por archivo o URL, descargar fuentes por idioma o libro, idiomas destino, actualización de app), Footnote, Print, Legal, Privacy, Login, Contributors, Confirm, Progress.
- Backups automáticos cada N minutos a `backups/{id}.tstudio` solo si cambió el commit; huérfanos a `backups/{id}.{fecha}.zip`.
- No existe blind draft ni soporte visual de `\w`/strong; la UI solo está en inglés.

## 6. Tipos de recurso
| Tipo | id | format | Creación | Modos | Export |
|---|---|---|---|---|---|
| Texto regular | text/reg | usfm | por defecto | Read/Chunk/Review | tstudio, USFM, PDF, upload |
| ULB/UDB | text/ulb, text/udb | usfm | GL mode | ídem | ídem |
| OBS | text/obs | markdown | proyecto obs | ídem | tstudio, PDF, upload |
| tN/tQ | tn, tq | markdown | GL mode, requiere texto del libro | solo Review | tstudio, upload |
| tW | tw (proyecto bible/bible-obs) | markdown | GL mode | solo Review | tstudio, upload |

## 7. Formato .tstudio
ZIP con `manifest.json` (archivo) + `{id}/` completo (manifest, LICENSE.md, `.git/**`, front, capítulos).
```json
{ "package_version": 3, "timestamp": 1724336290,
  "generator": { "name": "btt-writer-desktop", "build": "59" },
  "target_translations": [ { "id": "aaa_mrk_text_reg", "path": "aaa_mrk_text_reg", "commit_hash": "24c20db…", "direction": "ltr" } ] }
```
Se aceptan v1 (`projects`), v2 y v3. Al importar: descomprimir, migrar, merge git u overwrite, detectar conflictos.

## 8. Internacionalización
728 cadenas en inglés (`composeResources/values/strings.xml`), sin otros idiomas. 24 fuentes empaquetadas (Noto Sans por defecto, Abyssinica SIL, Andika, Charis, Doulos, Gentium Plus, Padauk, Scheherazade, Noto Nastaliq Urdu…). RTL por `direction`.

## 9. Plataformas
| | Desktop | Android |
|---|---|---|
| Datos internos | `%APPDATA%\BTT-Writer-2.0` / `~/Library/Application Support/BTT-Writer-2.0` / `~/.config/BTT-Writer-2.0` | `filesDir` |
| Datos de usuario | `~/BTT-Writer-2.0` | `getExternalFilesDir(null)` |
| Permisos | — | INTERNET, ACCESS_NETWORK_STATE, POST_NOTIFICATIONS |
| Backups | corrutina | Service + notificación |
| Paquete | DMG/MSI/DEB "BTT-Writer-2.0" | `org.bibletranslationtools.writer`, minSdk 26 |

## 10. Pruebas E2E del original (docs/E2E_TESTING_HANDOFF.md)
Flujos Maestro: arranque limpio; cuenta offline → Settings; nueva traducción (`aaa` → bible-nt → John → fuente English ULB); Review (editar `\v 4 … \v 5 …`, marcar hecho); menú de proyecto (Upload, Publish, Delete); GL mode (ULB/UDB/Notes/Questions, translationWords).
