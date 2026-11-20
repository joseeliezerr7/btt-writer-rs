// Los 8 pasos MAST en pantalla: barra de pasos con su estado, panel con la guía
// y las confirmaciones de cada paso, lista de chunks y notas de revisión.
// El progreso viene de `mast.json` (ver btt_core::mast) y lo maneja Translate.
import { useEffect, useState, type ReactNode } from "react";
import { Check, ChevronLeft, ChevronRight, Clock, MessageSquare, Trash2, Users } from "lucide-react";
import { MAST_STEPS, mastNeedsChecker, type ChunkInfo, type MastConfirmation, type MastFile, type MastNote, type MastStep, type MastStepState } from "../api";
import { useT } from "../i18n";
import { Button } from "./ui";
import { ago } from "./collab";

export type StepStatus = "passed" | "waiting" | "not_started";

export function stepState(file: MastFile | null, chapter: string, step: MastStep): MastStepState {
  return file?.chapters[chapter]?.steps[step] ?? {};
}

/** `passed` con las confirmaciones necesarias, `waiting` si falta una de las dos, `not_started` si ninguna. */
/** Confirmación vigente (no retirada). */
export const confirmedBy = (c: MastConfirmation | undefined) => (c && !c.undone ? c : undefined);

export function stepStatus(st: MastStepState, step: MastStep): StepStatus {
  const t = !!confirmedBy(st.translator);
  const c = !!confirmedBy(st.checker);
  if (t && (c || !mastNeedsChecker(step))) return "passed";
  return t || c ? "waiting" : "not_started";
}

/** Primer paso sin terminar del capítulo (o el último si todo está hecho). */
export function currentStep(file: MastFile | null, chapter: string): MastStep {
  return MAST_STEPS.find((s) => stepStatus(stepState(file, chapter, s), s) !== "passed") ?? "markers";
}

export function MastStepBar({ file, chapter, selected, onSelect }: { file: MastFile | null; chapter: string; selected: MastStep; onSelect: (s: MastStep) => void }) {
  const { t } = useT();
  const cur = currentStep(file, chapter);
  return (
    <div className="mast-steps" role="tablist">
      {MAST_STEPS.map((s, i) => {
        const status = stepStatus(stepState(file, chapter, s), s);
        const label = status === "passed" ? t("mast.status.passed") : status === "waiting" ? t("mast.status.waiting") : s === cur ? t("mast.status.in_progress") : t("mast.status.not_started");
        return (
          <button key={s} type="button" role="tab" aria-selected={s === selected} title={label} className={"mast-step " + status + (s === cur ? " current" : "") + (s === selected ? " selected" : "")} onClick={() => onSelect(s)}>
            <span className="n">{status === "passed" ? <Check size={11} /> : i + 1}</span>
            {t(`mast.step.${s}`)}
          </button>
        );
      })}
    </div>
  );
}

export function MastStepPanel(props: {
  step: MastStep;
  state: MastStepState;
  checker: string;
  onChecker: (name: string) => void;
  onConfirm: (role: "translator" | "checker", on: boolean) => void;
  onPrev?: () => void;
  onNext?: () => void;
  team: boolean;
  busy: boolean;
  children?: ReactNode;
}) {
  const { t } = useT();
  const { step, state } = props;
  const [checker, setChecker] = useState(props.checker);
  useEffect(() => setChecker(props.checker), [props.checker]);
  const time = t(`mast.time.${step}`);
  const asksChecker = step === "verbalize" || mastNeedsChecker(step);
  const confirmed = (c: { by: string; at: number } | undefined, role: "translator" | "checker", label: string) =>
    c ? (
      <span className="row" style={{ gap: ".4rem" }}>
        <span className="badge ok"><Check size={11} /> {label}</span>
        <span className="small muted">{t("mast.confirmed_by", { user: c.by, ago: ago(c.at, t) })}</span>
        <Button size="sm" variant="ghost" disabled={props.busy} onClick={() => props.onConfirm(role, false)}>{t("mast.undo")}</Button>
      </span>
    ) : (
      <Button size="sm" variant={role === "translator" ? "primary" : "ghost"} busy={props.busy} onClick={() => props.onConfirm(role, true)}><Check size={14} /> {label}</Button>
    );
  return (
    <div className="mast-panel">
      <h3>
        <span className="badge">{MAST_STEPS.indexOf(step) + 1}</span> {t(`mast.step.${step}`)}
        {time && <span className="small muted row" style={{ gap: ".25rem", fontWeight: 400 }}><Clock size={13} /> {time}</span>}
      </h3>
      <p className="guide">{t(`mast.guide.${step}`)}</p>
      {step === "verse_check" && <p className="guide"><strong>{t("mast.questions")}</strong></p>}
      {step === "key_words" && <p className="guide small muted">{t("mast.keyword_hint")}</p>}
      {step === "blind_draft" && <p className="guide small muted">{t("mast.blind_writing")}</p>}
      {props.children}
      {asksChecker && (
        <label className="row" style={{ gap: ".5rem", flexWrap: "wrap" }}>
          <span className="small muted row" style={{ gap: ".3rem" }}><Users size={13} /> {t("mast.checker")}</span>
          <input className="input" style={{ maxWidth: 260 }} value={checker} placeholder={t("mast.checker_hint")} onChange={(e) => setChecker(e.target.value)} onBlur={() => { if (checker.trim() !== props.checker) props.onChecker(checker); }} />
        </label>
      )}
      <div className="mast-confirm">
        {confirmed(confirmedBy(state.translator), "translator", t("mast.confirm_translator"))}
        {mastNeedsChecker(step) && confirmed(confirmedBy(state.checker), "checker", t("mast.confirm_checker"))}
        {props.team && <span className="small muted">{t("mast.team_hint")}</span>}
        <span className="grow" />
        {props.onPrev && <Button size="sm" variant="ghost" onClick={props.onPrev}><ChevronLeft size={14} /> {t("mast.prev")}</Button>}
        {props.onNext && <Button size="sm" variant="ghost" onClick={props.onNext}>{t("mast.next")} <ChevronRight size={14} /></Button>}
      </div>
    </div>
  );
}

/** Lista de los chunks del capítulo con sus versículos (el paso 3, que BTT Writer ya trae hecho). */
export function MastChunkList({ chunks }: { chunks: ChunkInfo[] }) {
  const { t } = useT();
  const verses = chunks.map((c) => c.id).filter((id) => /^\d+$/.test(id)).map(Number).sort((a, b) => a - b);
  return (
    <div>
      <p className="guide small">{t("mast.chunk_list", { n: verses.length })}</p>
      <ol className="mast-chunks small">
        {verses.map((from, i) => {
          const to = i + 1 < verses.length ? verses[i + 1] - 1 : null;
          const label = to === null ? t("mast.chunk_from", { n: i + 1, from }) : to === from ? t("mast.chunk_one", { n: i + 1, from }) : t("mast.chunk_range", { n: i + 1, from, to });
          return <li key={from}>{label}</li>;
        })}
      </ol>
    </div>
  );
}

/** Notas de revisión de un chunk (pasos con revisor): se leen, se añaden y cada quien borra las suyas. */
export function MastNotes({ notes, me, onAdd, onRemove }: { notes: MastNote[]; me: string; onAdd: (text: string) => void; onRemove: (at: number) => void }) {
  const { t } = useT();
  const [text, setText] = useState("");
  const submit = () => {
    const v = text.trim();
    if (!v) return;
    onAdd(v);
    setText("");
  };
  return (
    <div className="mast-notes">
      <span className="small muted row" style={{ gap: ".3rem" }}><MessageSquare size={13} /> {t("mast.notes")}</span>
      {notes.filter((n) => !n.deleted).map((n) => (
        <div key={n.by + n.at} className="mast-note small">
          <span className="who">{n.by}</span>
          <span className="grow">{n.text}</span>
          <span className="muted" title={new Date(n.at * 1000).toLocaleString()}>{ago(n.at, t)}</span>
          {n.by === me && <button type="button" className="btn ghost sm" title={t("common.delete")} onClick={() => onRemove(n.at)}><Trash2 size={13} /></button>}
        </div>
      ))}
      <div className="row" style={{ gap: ".4rem" }}>
        <input className="input grow" value={text} placeholder={t("mast.note_placeholder")} onChange={(e) => setText(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") submit(); }} />
        <Button size="sm" disabled={!text.trim()} onClick={submit}>{t("mast.save_note")}</Button>
      </div>
    </div>
  );
}
