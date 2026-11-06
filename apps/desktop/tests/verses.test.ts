// Pruebas rápidas de la normalización de marcadores (se ejecutan con `node tests/verses.test.ts`).
import assert from "node:assert/strict";
import { insertMarker, moveMarker, normalizeVerseMarkers, removeMarker, tokenize } from "../src/components/verse-markers.ts";

const n = (s: string, caret = s.length) => normalizeVerseMarkers(s, caret);

assert.deepEqual(n("hola\\v 2"), { text: "hola \\v 2", caret: 9 });
assert.deepEqual(n("\\v2"), { text: "\\v 2", caret: 4 });
assert.deepEqual(n("\\v 1texto"), { text: "\\v 1 texto", caret: 10 });
assert.deepEqual(n("\\v 1a algo"), { text: "\\v 1a algo", caret: 10 });
assert.deepEqual(n("\\v 1a"), { text: "\\v 1a", caret: 5 });
assert.deepEqual(n("\\v 1al"), { text: "\\v 1 al", caret: 7 });
assert.deepEqual(n("\\v 3-4 texto"), { text: "\\v 3-4 texto", caret: 12 });
assert.deepEqual(n("\\v 12."), { text: "\\v 12 .", caret: 7 });
// El cursor antes del espacio insertado no se mueve.
assert.deepEqual(n("holax\\v 2", 5), { text: "holax \\v 2", caret: 5 });
assert.deepEqual(n("\\v 1 a\n\\v 2 b"), { text: "\\v 1 a\n\\v 2 b", caret: 13 });

const txt = "\\v 1 En el principio creó Dios \\v 2 los cielos y la tierra.";
assert.equal(tokenize(txt).filter((t) => t.kind === "marker").length, 2);
// Mover el versículo 2 al final.
const toks = tokenize(txt);
const idx2 = toks.findIndex((t) => t.kind === "marker" && t.n === "2");
assert.equal(moveMarker(txt, idx2, toks.length), "\\v 1 En el principio creó Dios los cielos y la tierra. \\v 2");
// Mover el versículo 2 al inicio de "creó".
const idxCreo = toks.findIndex((t) => t.kind === "word" && t.raw === "creó");
assert.equal(moveMarker(txt, idx2, idxCreo), "\\v 1 En el principio \\v 2 creó Dios los cielos y la tierra.");
assert.equal(removeMarker(txt, idx2), "\\v 1 En el principio creó Dios los cielos y la tierra.");
assert.equal(insertMarker("En el principio", 1, 0), "\\v 1 En el principio");
assert.equal(insertMarker("En el principio", 1, tokenize("En el principio").length), "En el principio \\v 1");
// Las notas al pie se mueven enteras y los saltos de línea se conservan.
const fn = "\\v 1 a \\f + \\ft nota con espacios\\f* b\n\\v 2 c";
assert.equal(tokenize(fn).filter((t) => t.kind === "word").map((t) => t.raw).join("|"), "a|\\f + \\ft nota con espacios\\f*|b|c");
// Hueco 4 = antes de la nota (índices: 0 marcador, 1 espacio, 2 "a", 3 espacio, 4 nota).
assert.equal(moveMarker(fn, 0, 4), "a \\v 1 \\f + \\ft nota con espacios\\f* b\n\\v 2 c");
// Soltar en el hueco justo antes del propio marcador no cambia nada.
assert.equal(moveMarker(txt, idx2, idx2), txt);
console.log("verses.test.ts ok");

// Insertar en una posición del texto (arrastre sobre el editor)
{
  const { snapToWordBoundary, insertMarkerAt } = await import("../src/components/verse-markers.ts");
  assert.equal(snapToWordBoundary("hola mundo", 2), 0);
  assert.equal(snapToWordBoundary("hola mundo", 3), 4);
  assert.equal(snapToWordBoundary("hola mundo", 4), 4);
  assert.equal(snapToWordBoundary("hola mundo", 10), 10);
  assert.deepEqual(insertMarkerAt("hola mundo", 2, 5), { text: "hola \\v 2 mundo", caret: 10 });
  assert.deepEqual(insertMarkerAt("hola mundo", 2, 7), { text: "hola \\v 2 mundo", caret: 10 });
  assert.deepEqual(insertMarkerAt("hola", 1, 0), { text: "\\v 1 hola", caret: 5 });
  assert.deepEqual(insertMarkerAt("hola", 2, 4), { text: "hola \\v 2 ", caret: 10 });
  assert.deepEqual(insertMarkerAt("", 1, 0), { text: "\\v 1 ", caret: 5 });
}
console.log("insertMarkerAt ok");
