// Pruebas de los segmentos y del movimiento de marcadores por posición (editor con fichas).
import assert from "node:assert/strict";
import { findVerse, moveVerseTo, removeVerse, segment } from "../src/components/verse-markers.ts";
import { authorGroups } from "../src/components/authors.ts";
import { applySuggestion, suggestWords, tokens, wordAtCaret } from "../src/components/predict.ts";

assert.deepEqual(segment("\\v 1 Hola \\v 2 mundo"), [
  { kind: "marker", n: "1", raw: "\\v 1 " },
  { kind: "text", raw: "Hola " },
  { kind: "marker", n: "2", raw: "\\v 2 " },
  { kind: "text", raw: "mundo" },
]);
// `\v 10` no es `\v 1`; un sufijo de letra forma parte de la etiqueta.
assert.deepEqual(segment("\\v 10 x \\v 2a y").map((s) => (s.kind === "marker" ? s.n : s.raw)), ["10", "x ", "2a", "y"]);
// En el editor solo se vuelven ficha los marcadores cerrados con espacio: `\v 1` a medio escribir sigue siendo texto.
assert.deepEqual(segment("\\v 13 Hola \\v 1", true).map((s) => (s.kind === "marker" ? s.n : s.raw)), ["13", "Hola \\v 1"]);
assert.deepEqual(segment("\\v 13 Hola \\v 14 ", true).map((s) => (s.kind === "marker" ? s.n : s.raw)), ["13", "Hola ", "14"]);
assert.deepEqual(findVerse("a \\v 2 b", "2"), { start: 2, end: 7 });
assert.equal(findVerse("a \\v 2 b", "3"), null);

assert.equal(removeVerse("a \\v 2 b", "2"), "a b");
assert.equal(removeVerse("\\v 1 \\v 2 x", "1"), "\\v 2 x");
assert.equal(removeVerse("x \\v 2", "2"), "x ");
assert.equal(removeVerse("\\v 1 x", "9"), "\\v 1 x");

// Mover hacia delante: la posición se refiere al texto antes de quitar el marcador.
assert.deepEqual(moveVerseTo("\\v 1 Hola mundo \\v 2 fin", "1", 10), { text: "Hola \\v 1 mundo \\v 2 fin", caret: 10 });
// Mover hacia atrás.
assert.deepEqual(moveVerseTo("Hola mundo \\v 2 fin", "2", 0), { text: "\\v 2 Hola mundo fin", caret: 5 });
// Soltar en mitad de una palabra lo acerca al borde más próximo.
assert.equal(moveVerseTo("Hola mundo \\v 2 fin", "2", 6).text, "Hola \\v 2 mundo fin");
// Un marcador que no existe simplemente se inserta.
assert.equal(moveVerseTo("Hola", "3", 4).text, "Hola \\v 3 ");

console.log("editor.test.ts ok");

// Agrupación de la autoría por versículo en tramos.

assert.deepEqual(
  authorGroups([
    { verse: "1", author: "ana", time: 10 }, { verse: "2", author: "ana", time: 20 }, { verse: "3", author: "ana", time: 5 },
    { verse: "4-5", author: "beto", time: 7 }, { verse: "6", author: "ana", time: 0 },
  ]),
  [{ author: "ana", label: "1-3", time: 20 }, { author: "beto", label: "4-5", time: 7 }, { author: "ana", label: "6", time: 0 }],
);
console.log("authors ok");

// Predicción de palabras: la palabra bajo el cursor, la anterior y cómo completarla.

assert.deepEqual(wordAtCaret("\\v 1 Dijo el ho", 15), { start: 13, prefix: "ho", prev: "el" });
// Tras un punto, la palabra anterior ya no cuenta.
assert.deepEqual(wordAtCaret("Hola. Ho", 8), { start: 6, prefix: "Ho", prev: null });
// Un marcador entre medias se salta.
assert.deepEqual(wordAtCaret("Dijo \\v 2 ho", 12), { start: 10, prefix: "ho", prev: "Dijo" });
// En medio de una palabra, con pocas letras o dentro de un marcador no hay sugerencia.
assert.equal(wordAtCaret("Hola", 2), null);
assert.equal(wordAtCaret("h", 1), null);
assert.equal(wordAtCaret("\\v", 2), null);
assert.deepEqual(applySuggestion("Dijo el ho", 8, 10, "hombre"), { text: "Dijo el hombre ", caret: 15 });
assert.deepEqual(applySuggestion("ho mundo", 0, 2, "hola"), { text: "hola mundo", caret: 5 });
assert.deepEqual(tokens("\\v 1 Ho'ñiꞌ dios, ¿ko-ta? \\f + \\ft nota\\f* \\v 2 Dios 3."), ["Ho'ñiꞌ", "dios", "ko-ta", "Dios"]);
const corpus = ["\\v 1 Hola mundo. Hola tierra.", "\\v 2 Hombre bueno, el hombre malo. Honra a tu padre."];
assert.deepEqual(suggestWords(corpus, null, "Ho").map((s) => s.word), ["Hola", "Hombre"]);
assert.equal(suggestWords(corpus, "el", "ho")[0].word, "hombre");
assert.deepEqual(suggestWords(corpus, null, "hola"), []);
console.log("predict ok");
