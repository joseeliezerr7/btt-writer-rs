// Pruebas de los segmentos y del movimiento de marcadores por posición (editor con fichas).
import assert from "node:assert/strict";
import { findVerse, moveVerseTo, removeVerse, segment } from "../src/components/verse-markers.ts";
import { authorGroups } from "../src/components/authors.ts";

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
