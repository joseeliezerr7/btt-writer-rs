// Funciones puras sobre marcadores de versículo `\v N` (sin React): normalización
// de lo pegado al marcador y mover/insertar/quitar marcadores entre palabras.

/**
 * Separa lo que quede pegado a un marcador de versículo mientras se escribe:
 * `hola\v 2` → `hola \v 2`, `\v2` → `\v 2`, `\v 2texto` → `\v 2 texto`.
 * Un sufijo de una letra seguido de espacio (`\v 2a `) se respeta.
 * Devuelve el texto y la posición del cursor ajustada a los espacios añadidos.
 */
export function normalizeVerseMarkers(text: string, caret: number): { text: string; caret: number } {
  let out = "";
  let c = caret;
  let i = 0;
  const re = /\\v(\s*)(\d+(?:-\d+)?)([^\s\\]*)/g;
  for (const m of text.matchAll(re)) {
    const at = m.index ?? 0;
    const [whole, ws, num, tail] = m;
    out += text.slice(i, at);
    // Espacio antes del marcador si va pegado a una palabra.
    if (out.length && !/\s$/.test(out)) {
      out += " ";
      if (at < caret) c++;
    }
    out += "\\v ";
    if (!ws) {
      // `\v2` → `\v 2`: el espacio se inserta justo después de `\v`.
      if (at + 2 < caret) c++;
    }
    out += num;
    // Después del número: una sola letra seguida de espacio/fin es un sufijo (2a);
    // cualquier otra cosa pegada se separa con un espacio.
    const afterTail = text.slice(at + whole.length, at + whole.length + 1);
    const isSuffix = /^[a-z]$/i.test(tail) && (afterTail === "" || /\s/.test(afterTail));
    if (tail && !isSuffix) {
      out += " ";
      const numEnd = at + whole.length - tail.length;
      if (numEnd < caret) c++;
    }
    out += tail;
    i = at + whole.length;
  }
  out += text.slice(i);
  return { text: out, caret: c };
}

type Token = { kind: "marker"; n: string; raw: string } | { kind: "word"; raw: string } | { kind: "space"; raw: string };

/** Divide el texto en marcadores, palabras (las notas al pie `\f …\f*` son una sola palabra) y espacios. */
export function tokenize(text: string): Token[] {
  const out: Token[] = [];
  const re = /\\f\s[\s\S]*?\\f\*|\\v\s+(\d+(?:-\d+)?[a-z]?)|(\s+)|(\S+)/g;
  for (const m of text.matchAll(re)) {
    if (m[1] !== undefined) out.push({ kind: "marker", n: m[1], raw: m[0] });
    else if (m[2] !== undefined) out.push({ kind: "space", raw: m[0] });
    else out.push({ kind: "word", raw: m[0] });
  }
  return out;
}

function join(tokens: Token[]): string {
  let s = "";
  for (const t of tokens) {
    if (t.kind === "marker") {
      if (s.length && !/\s$/.test(s)) s += " ";
      s += `\\v ${t.n} `;
    } else if (t.kind === "space") {
      if (!s.length) continue;
      if (!/\s$/.test(s)) s += t.raw;
      else if (t.raw.includes("\n") && !s.endsWith("\n")) s = s.replace(/[ \t]+$/, "") + t.raw;
    } else {
      if (s.length && !/\s$/.test(s)) s += " ";
      s += t.raw;
    }
  }
  return s.replace(/[ \t]+$/, "");
}

/** Mueve el marcador que está en `from` (índice de token) para que quede antes del token `gap`. */
export function moveMarker(text: string, from: number, gap: number): string {
  const tokens = tokenize(text);
  const tok = tokens[from];
  if (!tok || tok.kind !== "marker") return text;
  const rest = tokens.filter((_, i) => i !== from);
  let at = gap > from ? gap - 1 : gap;
  at = Math.max(0, Math.min(at, rest.length));
  rest.splice(at, 0, tok);
  return join(rest);
}

/** Inserta un marcador nuevo `\v n` antes del token `gap`. */
export function insertMarker(text: string, n: number, gap: number): string {
  const tokens = tokenize(text);
  const at = Math.max(0, Math.min(gap, tokens.length));
  tokens.splice(at, 0, { kind: "marker", n: String(n), raw: `\\v ${n} ` });
  return join(tokens);
}

/** Quita el marcador que está en `from`. */
export function removeMarker(text: string, from: number): string {
  const tokens = tokenize(text);
  if (tokens[from]?.kind !== "marker") return text;
  return join(tokens.filter((_, i) => i !== from));
}

export const MARKER_MIME = "application/x-btt-verse-marker";

/**
 * Acerca una posición del texto al borde de palabra más próximo, para que un
 * marcador soltado en mitad de una palabra no la parta.
 */
export function snapToWordBoundary(text: string, pos: number): number {
  pos = Math.max(0, Math.min(pos, text.length));
  if (pos === 0 || pos === text.length || /\s/.test(text[pos - 1]) || /\s/.test(text[pos])) return pos;
  let start = pos;
  while (start > 0 && !/\s/.test(text[start - 1])) start--;
  let end = pos;
  while (end < text.length && !/\s/.test(text[end])) end++;
  return pos - start <= end - pos ? start : end;
}

/**
 * Inserta `\\v N` en la posición dada (ajustada al borde de palabra) con los
 * espacios necesarios. Devuelve el texto y la posición del cursor tras el marcador.
 */
export function insertMarkerAt(text: string, n: number | string, pos: number): { text: string; caret: number } {
  const at = snapToWordBoundary(text, pos);
  const before = text.slice(0, at);
  const after = text.slice(at).replace(/^[ \t]+/, "");
  const lead = before.length && !/\s$/.test(before) ? " " : "";
  const snippet = `${lead}\\v ${n} `;
  return { text: before + snippet + after, caret: before.length + snippet.length };
}

/** Segmentos del editor: texto plano y marcadores `\v N` (cada marcador se lleva el espacio que le sigue). */
export type Segment = { kind: "text"; raw: string } | { kind: "marker"; n: string; raw: string };

const MARKER_RE = /\\v\s+(\d+(?:-\d+)?[a-z]?)(?![\w-]) ?/g;
// Solo marcadores "cerrados" (número seguido de espacio): mientras se escribe
// `\v 13`, la ficha no se forma al teclear el `1`.
const COMPLETE_MARKER_RE = /\\v\s+(\d+(?:-\d+)?[a-z]?)(?=\s) ?/g;

export function segment(text: string, completeOnly = false): Segment[] {
  const out: Segment[] = [];
  let i = 0;
  for (const m of text.matchAll(completeOnly ? COMPLETE_MARKER_RE : MARKER_RE)) {
    const at = m.index ?? 0;
    if (at > i) out.push({ kind: "text", raw: text.slice(i, at) });
    out.push({ kind: "marker", n: m[1], raw: m[0] });
    i = at + m[0].length;
  }
  if (i < text.length) out.push({ kind: "text", raw: text.slice(i) });
  return out;
}

/** Posición (inicio y fin) de la primera aparición del marcador `\v n` en el texto, o null. */
export function findVerse(text: string, n: string): { start: number; end: number } | null {
  for (const m of text.matchAll(MARKER_RE)) {
    if (m[1] === n) return { start: m.index ?? 0, end: (m.index ?? 0) + m[0].length };
  }
  return null;
}

/** Quita la primera aparición de `\v n`, dejando un solo espacio entre lo que había a los lados. */
export function removeVerse(text: string, n: string): string {
  const at = findVerse(text, n);
  if (!at) return text;
  const before = text.slice(0, at.start);
  const after = text.slice(at.end);
  if (/\s$/.test(before) && /^[ \t]/.test(after)) return before + after.replace(/^[ \t]+/, "");
  return before + after;
}

/** Mueve el marcador `\v n` a la posición `pos` del texto (ajustada al borde de palabra). */
export function moveVerseTo(text: string, n: string, pos: number): { text: string; caret: number } {
  const at = findVerse(text, n);
  if (!at) return insertMarkerAt(text, n, pos);
  const without = removeVerse(text, n);
  let p = pos;
  if (pos >= at.end) p = pos - (text.length - without.length);
  else if (pos > at.start) p = at.start;
  return insertMarkerAt(without, n, Math.max(0, Math.min(p, without.length)));
}
