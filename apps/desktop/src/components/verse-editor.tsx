// Editor de traducción con los marcadores de versículo como fichas dentro del
// texto (como en el BTT Writer original): se escribe alrededor de ellas y se
// arrastran a otra palabra, o a la barra de versículos para quitarlas. El DOM
// se construye a mano para no pelear con React mientras el usuario escribe.
import { forwardRef, useEffect, useImperativeHandle, useRef, useState, type ClipboardEvent, type DragEvent, type KeyboardEvent, type MouseEvent } from "react";
import { useT } from "../i18n";
import { insertMarkerAt, moveVerseTo, normalizeVerseMarkers, segment } from "./verse-markers";

/** Etiqueta del versículo que se arrastra desde dentro del editor. */
export const VERSE_MIME = "application/x-btt-verse";

export interface VerseEditorHandle {
  focus(): void;
  /** Posición del cursor en el texto serializado (o el final si no hay selección). */
  getCaret(): number;
  setCaret(pos: number): void;
}

interface Props {
  text: string;
  rtl: boolean;
  placeholder: string;
  onChange: (text: string) => void;
  onFocus?: () => void;
}

const MARKER_CLASS = "ve-marker";

function isMarker(node: Node | null): node is HTMLElement {
  return !!node && node.nodeType === Node.ELEMENT_NODE && (node as HTMLElement).classList.contains(MARKER_CLASS);
}

/** Construye el contenido del editor a partir del texto. */
function build(root: HTMLElement, text: string, picked: string | null) {
  root.textContent = "";
  for (const g of segment(text)) {
    if (g.kind === "text") {
      root.appendChild(document.createTextNode(g.raw));
    } else {
      const el = document.createElement("span");
      el.className = MARKER_CLASS + (picked === g.n ? " picked" : "");
      el.contentEditable = "false";
      el.draggable = true;
      el.dataset.n = g.n;
      el.textContent = g.n;
      root.appendChild(el);
    }
  }
}

/**
 * Recorre el editor y devuelve el texto serializado; si se indica un punto del
 * DOM (contenedor + desplazamiento), también la posición que le corresponde en el texto.
 */
function walk(root: HTMLElement, at?: { node: Node; offset: number }): { text: string; caret: number | null } {
  let s = "";
  let caret: number | null = null;
  const visit = (node: Node, index: number, parent: Node) => {
    if (at && caret === null && at.node === parent && at.offset === index) caret = s.length;
    if (node.nodeType === Node.TEXT_NODE) {
      const v = node.nodeValue ?? "";
      if (at && caret === null && at.node === node) caret = s.length + Math.min(at.offset, v.length);
      s += v;
      return;
    }
    if (isMarker(node)) {
      if (s.length && !/\s$/.test(s)) s += " ";
      s += `\\v ${node.dataset.n} `;
      return;
    }
    const el = node as HTMLElement;
    if (el.tagName === "BR") {
      // Chromium deja un <br> final cuando el texto acaba en salto de línea: no cuenta.
      if (!(parent === root && index === root.childNodes.length - 1)) s += "\n";
      return;
    }
    if (el.tagName === "DIV" && s.length && !s.endsWith("\n")) s += "\n";
    el.childNodes.forEach((c, i) => visit(c, i, el));
    if (at && caret === null && at.node === el && at.offset >= el.childNodes.length) caret = s.length;
  };
  root.childNodes.forEach((c, i) => visit(c, i, root));
  if (at && caret === null && at.node === root) caret = s.length;
  return { text: s, caret };
}

/** Coloca el cursor en la posición `pos` del texto serializado. */
function placeCaret(root: HTMLElement, pos: number) {
  const sel = window.getSelection();
  if (!sel) return;
  const range = document.createRange();
  let s = 0;
  let last = " "; // último carácter serializado (decide si un marcador lleva espacio delante)
  let done = false;
  const visit = (node: Node) => {
    if (done) return;
    if (node.nodeType === Node.TEXT_NODE) {
      const v = node.nodeValue ?? "";
      if (pos <= s + v.length) {
        range.setStart(node, Math.max(0, pos - s));
        done = true;
      } else {
        s += v.length;
        if (v.length) last = v[v.length - 1];
      }
      return;
    }
    if (isMarker(node)) {
      const len = (s > 0 && !/\s/.test(last) ? 1 : 0) + `\\v ${node.dataset.n} `.length;
      if (pos <= s + len) {
        const parent = node.parentNode!;
        range.setStart(parent, Array.prototype.indexOf.call(parent.childNodes, node) + (pos <= s ? 0 : 1));
        done = true;
      } else {
        s += len;
        last = " ";
      }
      return;
    }
    const el = node as HTMLElement;
    if (el.tagName === "BR") { s += 1; last = "\n"; return; }
    el.childNodes.forEach(visit);
  };
  root.childNodes.forEach(visit);
  if (!done) range.setStart(root, root.childNodes.length);
  range.collapse(true);
  sel.removeAllRanges();
  sel.addRange(range);
}

/** Posición del texto bajo un punto de la pantalla (para soltar fichas). */
function caretFromPoint(root: HTMLElement, x: number, y: number): number {
  const doc = document as Document & { caretPositionFromPoint?: (x: number, y: number) => { offsetNode: Node; offset: number } | null };
  const p = doc.caretPositionFromPoint?.(x, y);
  if (p && root.contains(p.offsetNode)) return walk(root, { node: p.offsetNode, offset: p.offset }).caret ?? walk(root).text.length;
  const r = doc.caretRangeFromPoint?.(x, y);
  if (r && root.contains(r.startContainer)) return walk(root, { node: r.startContainer, offset: r.startOffset }).caret ?? walk(root).text.length;
  return walk(root).text.length;
}

export const VerseEditor = forwardRef<VerseEditorHandle, Props>(function VerseEditor({ text, rtl, placeholder, onChange, onFocus }, ref) {
  const { t } = useT();
  const root = useRef<HTMLDivElement>(null);
  const composing = useRef(false);
  const [picked, setPicked] = useState<string | null>(null);
  const [over, setOver] = useState(false);

  const currentCaret = (): number | null => {
    const el = root.current;
    const sel = window.getSelection();
    if (!el || !sel || !sel.rangeCount) return null;
    const r = sel.getRangeAt(0);
    if (!el.contains(r.startContainer)) return null;
    return walk(el, { node: r.startContainer, offset: r.startOffset }).caret;
  };

  /** Reconstruye el DOM con `value` y deja el cursor en `caret` (si el editor tiene el foco). */
  const rebuild = (value: string, caret: number | null) => {
    const el = root.current;
    if (!el) return;
    const focused = document.activeElement === el;
    build(el, value, picked);
    if (focused && caret !== null) placeCaret(el, caret);
  };

  // Cuando el texto cambia desde fuera (recarga, historial, ficha de la barra) se redibuja.
  useEffect(() => {
    const el = root.current;
    if (!el) return;
    if (walk(el).text !== text) rebuild(text, currentCaret());
  }, [text]);

  useEffect(() => {
    const el = root.current;
    if (!el) return;
    el.querySelectorAll("." + MARKER_CLASS).forEach((m) => m.classList.toggle("picked", (m as HTMLElement).dataset.n === picked));
  }, [picked]);

  useImperativeHandle(ref, () => ({
    focus: () => root.current?.focus(),
    getCaret: () => currentCaret() ?? (root.current ? walk(root.current).text.length : 0),
    setCaret: (pos) => { if (root.current) placeCaret(root.current, pos); },
  }));

  const onInput = () => {
    const el = root.current;
    if (!el) return;
    const caret = currentCaret();
    const raw = walk(el).text;
    if (composing.current) { onChange(raw); return; }
    const fixed = normalizeVerseMarkers(raw, caret ?? raw.length);
    // Un `\v 3` escrito a mano pasa a ser ficha; los espacios añadidos se reflejan en el DOM.
    const typedMarker = Array.from(el.childNodes).some((n) => n.nodeType === Node.TEXT_NODE && /\\v\s*\d/.test(n.nodeValue ?? ""));
    if (typedMarker || fixed.text !== raw) rebuild(fixed.text, fixed.caret);
    onChange(fixed.text);
  };

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Escape" && picked) setPicked(null);
  };

  const onPaste = (e: ClipboardEvent) => {
    e.preventDefault();
    document.execCommand("insertText", false, e.clipboardData.getData("text/plain"));
  };

  /** Aplica un texto nuevo producido por mover o insertar una ficha. */
  const apply = (r: { text: string; caret: number }) => {
    const el = root.current;
    if (!el) return;
    el.focus();
    build(el, r.text, null);
    placeCaret(el, r.caret);
    setPicked(null);
    onChange(r.text);
  };

  const onDragStart = (e: DragEvent) => {
    const target = e.target as HTMLElement;
    if (!isMarker(target)) return;
    const n = target.dataset.n ?? "";
    e.dataTransfer.setData(VERSE_MIME, n);
    e.dataTransfer.setData("text/plain", `\\v ${n} `);
    e.dataTransfer.effectAllowed = "move";
    setPicked(null);
  };

  const onDragOver = (e: DragEvent) => {
    const types = e.dataTransfer.types;
    if (types.includes(VERSE_MIME) || types.includes("text/plain")) {
      e.preventDefault();
      e.dataTransfer.dropEffect = types.includes(VERSE_MIME) ? "move" : "copy";
      if (!over) setOver(true);
    }
  };

  const onDrop = (e: DragEvent) => {
    const el = root.current;
    setOver(false);
    if (!el) return;
    const current = walk(el).text;
    const pos = caretFromPoint(el, e.clientX, e.clientY);
    const n = e.dataTransfer.getData(VERSE_MIME);
    if (n) {
      e.preventDefault();
      apply(moveVerseTo(current, n, pos));
      return;
    }
    const m = /^\\v\s+(\d+)\s*$/.exec(e.dataTransfer.getData("text/plain"));
    if (m) {
      e.preventDefault();
      apply(insertMarkerAt(current, m[1], pos));
    }
    // Otro texto arrastrado lo coloca el propio navegador.
  };

  const onClick = (e: MouseEvent) => {
    const el = root.current;
    if (!el) return;
    const target = e.target as HTMLElement;
    if (isMarker(target)) {
      e.preventDefault();
      const n = target.dataset.n ?? "";
      if (picked === n) setPicked(null);
      else if (picked) {
        // Una ficha elegida y un toque sobre otra: va justo antes de esa otra.
        const pos = walk(el, { node: el, offset: Array.prototype.indexOf.call(el.childNodes, target) }).caret ?? 0;
        apply(moveVerseTo(walk(el).text, picked, pos));
      } else setPicked(n);
      return;
    }
    if (picked) {
      // El toque ya colocó el cursor: la ficha elegida va ahí.
      const pos = currentCaret();
      if (pos !== null) apply(moveVerseTo(walk(el).text, picked, pos));
      else setPicked(null);
    }
  };

  return (
    <div className="col" style={{ flex: 1, gap: ".3rem" }}>
      <div
        ref={root}
        className={"editor verse-editor target-text" + (rtl ? " rtl" : "") + (over ? " drop-over" : "")}
        // "plaintext-only": Enter inserta un salto de línea como texto (sin <div>) y lo pegado llega sin formato.
        contentEditable="plaintext-only"
        suppressContentEditableWarning
        spellCheck={false}
        data-placeholder={placeholder}
        onInput={onInput}
        onKeyDown={onKeyDown}
        onPaste={onPaste}
        onFocus={onFocus}
        onCompositionStart={() => { composing.current = true; }}
        onCompositionEnd={() => { composing.current = false; onInput(); }}
        onDragStart={onDragStart}
        onDragOver={onDragOver}
        onDragLeave={() => setOver(false)}
        onDrop={onDrop}
        onClick={onClick}
      />
      {picked && <div className="small muted">{t("translate.picked", { n: picked })}</div>}
    </div>
  );
});
