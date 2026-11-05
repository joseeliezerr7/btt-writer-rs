// Modo "Mover versículos" del editor: fichas arrastrables entre las palabras,
// con toque como alternativa. Las funciones puras viven en verse-markers.ts.
import { useEffect, useRef, useState, type DragEvent, type ReactNode } from "react";
import { useT } from "../i18n";
import { insertMarker, MARKER_MIME, moveMarker, removeMarker, tokenize } from "./verse-markers";

export { MARKER_MIME, normalizeVerseMarkers, removeMarker } from "./verse-markers";

/**
 * Vista del texto con los versículos como fichas que se pueden arrastrar entre las
 * palabras (o tocar y luego tocar el hueco). `pickedNew` es un versículo de la barra
 * que se quiere colocar con un toque.
 */
export function VerseArranger({ text, rtl, onChange, pickedNew, onPlacedNew }: { text: string; rtl: boolean; onChange: (t: string) => void; pickedNew: number | null; onPlacedNew: () => void }) {
  const { t } = useT();
  const [picked, setPicked] = useState<number | null>(null);
  const [dragging, setDragging] = useState(false);
  const [over, setOver] = useState<number | null>(null);
  const tokens = tokenize(text);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    setPicked(null);
  }, [text]);

  const active = dragging || picked !== null || pickedNew !== null;

  const place = (gap: number) => {
    if (picked !== null) {
      onChange(moveMarker(text, picked, gap));
      setPicked(null);
    } else if (pickedNew !== null) {
      onChange(insertMarker(text, pickedNew, gap));
      onPlacedNew();
    }
  };

  const onDrop = (gap: number) => (e: DragEvent) => {
    e.preventDefault();
    setOver(null);
    setDragging(false);
    const idx = e.dataTransfer.getData(MARKER_MIME);
    if (idx !== "") {
      onChange(moveMarker(text, Number(idx), gap));
      return;
    }
    const plain = e.dataTransfer.getData("text/plain");
    const m = /\\v\s+(\d+)/.exec(plain);
    if (m) onChange(insertMarker(text, Number(m[1]), gap));
  };
  const onDragOver = (gap: number) => (e: DragEvent) => {
    if (e.dataTransfer.types.includes(MARKER_MIME) || e.dataTransfer.types.includes("text/plain")) {
      e.preventDefault();
      e.dataTransfer.dropEffect = "move";
      if (over !== gap) setOver(gap);
    }
  };

  const gap = (i: number) => (
    <span
      key={`g${i}`}
      className={"va-gap" + (over === i ? " over" : "")}
      onDragOver={onDragOver(i)}
      onDragLeave={() => over === i && setOver(null)}
      onDrop={onDrop(i)}
      onClick={() => active && place(i)}
      role={active ? "button" : undefined}
    />
  );

  const out: ReactNode[] = [];
  let lastWasGap = false;
  tokens.forEach((tok, i) => {
    if (tok.kind === "space") {
      if (tok.raw.includes("\n")) {
        if (!lastWasGap) { out.push(gap(i)); lastWasGap = true; }
        out.push(<br key={`br${i}`} />);
      }
      return;
    }
    if (!lastWasGap) out.push(gap(i));
    lastWasGap = false;
    if (tok.kind === "marker") {
      out.push(
        <span
          key={`m${i}`}
          className={"va-marker" + (picked === i ? " picked" : "")}
          draggable
          title={picked === i ? t("translate.remove_marker", { n: tok.n }) : t("translate.arrange_hint")}
          onDragStart={(e) => { e.dataTransfer.setData(MARKER_MIME, String(i)); e.dataTransfer.setData("text/plain", `\\v ${tok.n} `); e.dataTransfer.effectAllowed = "move"; setDragging(true); setPicked(null); }}
          onDragEnd={() => { setDragging(false); setOver(null); }}
          onClick={(e) => {
            e.stopPropagation();
            if (picked === i) onChange(removeMarker(text, i));
            else if (picked !== null) onChange(moveMarker(text, picked, i));
            else setPicked(i);
          }}
        >
          {tok.n}
        </span>,
      );
    } else {
      out.push(<span key={`w${i}`} className="va-word" onClick={() => active && place(i)}>{tok.raw}</span>);
    }
  });
  if (!lastWasGap) out.push(gap(tokens.length));

  const pickedN = picked !== null ? (tokens[picked] as { n: string }).n : pickedNew;
  return (
    <div className="col" style={{ gap: ".4rem", flex: 1 }}>
      <div ref={rootRef} className={"verse-arranger target-text" + (rtl ? " rtl" : "") + (active ? " active" : "")}>
        {tokens.length === 0 ? <span className="muted">{t("translate.arrange_empty")}</span> : out}
      </div>
      <div className="small muted">{pickedN !== null && pickedN !== undefined ? t("translate.picked", { n: pickedN }) : t("translate.arrange_hint")}</div>
    </div>
  );
}
