// Predicción de palabras en el editor: qué se está tecleando, cómo completarlo
// y (para la versión web de prueba) un índice igual al de btt-core.

/** Letras, diacríticos y los apóstrofos/guiones que muchas lenguas usan dentro de la palabra. */
const WORD_CHAR = /[\p{L}\p{M}'’ʼꞌʔ-]/u;
const EDGE = /^['’ʼ-]+|['’ʼ-]+$/g;
export const MIN_PREFIX = 2;
export const MIN_COUNT = 2;

export interface WordAtCaret {
  /** Dónde empieza la palabra a medio escribir (el cursor es su final). */
  start: number;
  prefix: string;
  /** Palabra anterior en la misma frase, o null. */
  prev: string | null;
}

/** Palabra a medio escribir justo antes del cursor y la que la precede. */
export function wordAtCaret(text: string, caret: number): WordAtCaret | null {
  // Solo al final de la palabra: en medio de una no se sugiere.
  if (caret < text.length && WORD_CHAR.test(text[caret])) return null;
  let start = caret;
  while (start > 0 && WORD_CHAR.test(text[start - 1])) start--;
  // `\v 3` o `\f` escritos a mano no son palabras.
  if (start > 0 && text[start - 1] === "\\") return null;
  const prefix = text.slice(start, caret);
  if ([...prefix].length < MIN_PREFIX) return null;
  const before = text.slice(0, start).replace(/\\[a-z0-9]+\*?\s*/g, " ");
  const m = /([\p{L}\p{M}'’ʼꞌʔ-]+)([^\p{L}\p{M}]*)$/u.exec(before);
  const prev = m && !/[.!?;:]/.test(m[2]) ? m[1].replace(EDGE, "") : null;
  return { start, prefix, prev: prev || null };
}

/** Sustituye lo tecleado (`start`..`end`) por `word` y deja el cursor tras el espacio siguiente. */
export function applySuggestion(text: string, start: number, end: number, word: string): { text: string; caret: number } {
  const after = text.slice(end);
  const space = after.startsWith(" ") ? "" : " ";
  return { text: text.slice(0, start) + word + space + after, caret: start + word.length + 1 };
}

/** Palabras de un chunk sin marcadores USFM, notas ni números (como `predict::tokens`). */
export function tokens(text: string): string[] {
  const plain = text.replace(/\\fe\s[\s\S]*?\\fe\*|\\f\s[\s\S]*?\\f\*|\\x\s[\s\S]*?\\x\*/g, " ").replace(/\\\+?[a-z0-9]+\*?\s?/g, " ");
  return (plain.match(/[\p{L}\p{M}'’ʼꞌʔ-]+/gu) ?? []).map((w) => w.replace(EDGE, "")).filter((w) => /\p{L}/u.test(w));
}

/** Sugerencias a partir de unos textos (la versión web de prueba; la app usa btt-core). */
export function suggestWords(texts: string[], prev: string | null, prefix: string, limit = 5): { word: string; count: number }[] {
  prefix = prefix.replace(EDGE, "");
  if ([...prefix].length < MIN_PREFIX) return [];
  const words = new Map<string, { count: number; forms: Map<string, number> }>();
  const next = new Map<string, Map<string, number>>();
  for (const t of texts) {
    let p: string | null = null;
    for (const tok of tokens(t)) {
      const k = tok.toLowerCase();
      const w = words.get(k) ?? { count: 0, forms: new Map() };
      w.count++;
      w.forms.set(tok, (w.forms.get(tok) ?? 0) + 1);
      words.set(k, w);
      if (p) {
        const n = next.get(p) ?? new Map<string, number>();
        n.set(k, (n.get(k) ?? 0) + 1);
        next.set(p, n);
      }
      p = k;
    }
  }
  const key = prefix.toLowerCase();
  const follow = prev ? next.get(prev.toLowerCase()) : undefined;
  const upper = /^\p{Lu}/u.test(prefix);
  return [...words.entries()]
    .filter(([k, w]) => w.count >= MIN_COUNT && k.startsWith(key) && k !== key)
    .map(([k, w]) => ({ k, w, score: w.count + 20 * (follow?.get(k) ?? 0) }))
    .sort((a, b) => b.score - a.score || (a.k < b.k ? -1 : 1))
    .slice(0, limit)
    .map(({ w }) => {
      // La forma más frecuente; en empate, la que coincide en mayúscula con lo tecleado.
      const fits = (f: string) => (/^\p{Lu}/u.test(f) === upper ? 0 : 1);
      const form = [...w.forms.entries()].sort((a, b) => b[1] - a[1] || fits(a[0]) - fits(b[0]) || (a[0] < b[0] ? -1 : 1))[0][0];
      return { word: upper ? form.charAt(0).toUpperCase() + form.slice(1) : form, count: w.count };
    });
}
