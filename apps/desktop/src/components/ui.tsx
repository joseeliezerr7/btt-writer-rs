// Componentes de interfaz reutilizables.
import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { X } from "lucide-react";

type Variant = "primary" | "danger" | "ghost" | "default";

export function Button(props: {
  children?: ReactNode;
  onClick?: () => void;
  variant?: Variant;
  disabled?: boolean;
  title?: string;
  className?: string;
  type?: "button" | "submit";
  size?: "sm";
  icon?: boolean;
  block?: boolean;
  busy?: boolean;
}) {
  const cls = ["btn", props.variant && props.variant !== "default" ? props.variant : "", props.size ?? "", props.icon ? "icon" : "", props.block ? "block" : "", props.className ?? ""].join(" ");
  return (
    <button type={props.type ?? "button"} className={cls} onClick={props.onClick} disabled={props.disabled || props.busy} title={props.title}>
      {props.busy ? <span className="spinner" style={{ width: 16, height: 16, borderWidth: 2 }} /> : null}
      {props.children}
    </button>
  );
}

export function Switch(props: { on: boolean; onChange: (v: boolean) => void; label?: ReactNode; disabled?: boolean }) {
  return (
    <span className={"switch" + (props.on ? " on" : "")} onClick={() => !props.disabled && props.onChange(!props.on)} role="switch" aria-checked={props.on}>
      <span className="track" />
      {props.label ? <span>{props.label}</span> : null}
    </span>
  );
}

export function Field(props: { label: string; children: ReactNode; hint?: string }) {
  return (
    <div className="field">
      <label>{props.label}</label>
      {props.children}
      {props.hint ? <span className="small muted">{props.hint}</span> : null}
    </div>
  );
}

export function Spinner({ label }: { label?: string }) {
  return (
    <div className="row" style={{ justifyContent: "center", padding: "1.5rem" }}>
      <span className="spinner" />
      {label ? <span className="muted">{label}</span> : null}
    </div>
  );
}

export function Dialog(props: {
  title: ReactNode;
  onClose?: () => void;
  children: ReactNode;
  footer?: ReactNode;
  wide?: boolean;
  narrow?: boolean;
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") props.onClose?.();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [props.onClose]);
  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && props.onClose?.()}>
      <div className={"dialog" + (props.wide ? " wide" : "") + (props.narrow ? " narrow" : "")} role="dialog" aria-modal="true">
        <div className="dialog-head">
          <h2>{props.title}</h2>
          {props.onClose ? (
            <button className="btn ghost icon" onClick={props.onClose} aria-label="close">
              <X size={18} />
            </button>
          ) : null}
        </div>
        <div className="dialog-body">{props.children}</div>
        {props.footer ? <div className="dialog-foot">{props.footer}</div> : null}
      </div>
    </div>
  );
}

export function Tabs<T extends string>(props: { tabs: { id: T; label: ReactNode }[]; value: T; onChange: (t: T) => void }) {
  return (
    <div className="tabs" role="tablist">
      {props.tabs.map((t) => (
        <button key={t.id} role="tab" className={"tab" + (t.id === props.value ? " active" : "")} onClick={() => props.onChange(t.id)}>
          {t.label}
        </button>
      ))}
    </div>
  );
}

export function ProgressRing({ value }: { value: number }) {
  const p = Math.round(Math.max(0, Math.min(1, value)) * 100);
  return (
    <div className="progress-ring" style={{ ["--p" as string]: p }}>
      <span>{p}%</span>
    </div>
  );
}

// ---------------------------------------------------------------- toasts y confirmaciones

interface Toast {
  id: number;
  text: string;
  kind: "info" | "error" | "success";
}

interface ConfirmReq {
  title: string;
  body: ReactNode;
  okLabel?: string;
  cancelLabel?: string;
  danger?: boolean;
  resolve: (v: boolean) => void;
}

interface FeedbackCtx {
  toast: (text: string, kind?: Toast["kind"]) => void;
  error: (e: unknown) => void;
  confirm: (title: string, body: ReactNode, opts?: { okLabel?: string; cancelLabel?: string; danger?: boolean }) => Promise<boolean>;
}

const FeedbackContext = createContext<FeedbackCtx>({ toast: () => {}, error: () => {}, confirm: async () => false });
export const useFeedback = () => useContext(FeedbackContext);

export function FeedbackProvider({ children, labels }: { children: ReactNode; labels: { ok: string; cancel: string } }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [confirmReq, setConfirmReq] = useState<ConfirmReq | null>(null);
  const seq = useRef(0);

  const toast = useCallback((text: string, kind: Toast["kind"] = "info") => {
    const id = ++seq.current;
    setToasts((t) => [...t, { id, text, kind }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), kind === "error" ? 7000 : 3500);
  }, []);
  const error = useCallback((e: unknown) => toast(typeof e === "string" ? e : (e as Error)?.message ?? String(e), "error"), [toast]);
  const confirm = useCallback(
    (title: string, body: ReactNode, opts?: { okLabel?: string; cancelLabel?: string; danger?: boolean }) =>
      new Promise<boolean>((resolve) => setConfirmReq({ title, body, resolve, ...opts })),
    [],
  );
  const value = useMemo(() => ({ toast, error, confirm }), [toast, error, confirm]);
  const close = (v: boolean) => {
    confirmReq?.resolve(v);
    setConfirmReq(null);
  };
  return (
    <FeedbackContext.Provider value={value}>
      {children}
      {confirmReq ? (
        <Dialog
          title={confirmReq.title}
          narrow
          onClose={() => close(false)}
          footer={
            <>
              <Button onClick={() => close(false)}>{confirmReq.cancelLabel ?? labels.cancel}</Button>
              <Button variant={confirmReq.danger ? "danger" : "primary"} onClick={() => close(true)}>
                {confirmReq.okLabel ?? labels.ok}
              </Button>
            </>
          }
        >
          <div>{confirmReq.body}</div>
        </Dialog>
      ) : null}
      <div className="toasts" aria-live="polite">
        {toasts.map((t) => (
          <div key={t.id} className={"toast " + (t.kind === "info" ? "" : t.kind)}>
            {t.text}
          </div>
        ))}
      </div>
    </FeedbackContext.Provider>
  );
}

/** Renderiza Markdown muy simple (encabezados, párrafos, listas, negrita, enlaces) de forma segura. */
export function SimpleMarkdown({ text, onLink }: { text: string; onLink?: (href: string) => void }) {
  const blocks = text.replace(/\r\n/g, "\n").split(/\n{2,}/);
  return (
    <div className="body">
      {blocks.map((b, i) => {
        const trimmed = b.trim();
        if (!trimmed) return null;
        if (/^#{1,6}\s/.test(trimmed)) return <h4 key={i}>{inline(trimmed.replace(/^#+\s*/, ""), onLink)}</h4>;
        if (/^(\*|-)\s/m.test(trimmed)) {
          return (
            <ul key={i} style={{ margin: "0 0 .4rem", paddingLeft: "1.2rem" }}>
              {trimmed.split("\n").map((l, j) => (
                <li key={j}>{inline(l.replace(/^(\*|-)\s*/, ""), onLink)}</li>
              ))}
            </ul>
          );
        }
        return <p key={i}>{inline(trimmed, onLink)}</p>;
      })}
    </div>
  );
}

function inline(s: string, onLink?: (href: string) => void): ReactNode[] {
  const out: ReactNode[] = [];
  const re = /(\*\*[^*]+\*\*|\[\[[^\]]+\]\]|\[[^\]]+\]\([^)]+\))/g;
  let last = 0;
  let m: RegExpExecArray | null;
  let k = 0;
  while ((m = re.exec(s))) {
    if (m.index > last) out.push(s.slice(last, m.index));
    const tok = m[0];
    if (tok.startsWith("**")) out.push(<strong key={k++}>{tok.slice(2, -2)}</strong>);
    else if (tok.startsWith("[[")) {
      const inner = tok.slice(2, -2);
      const [target, label] = inner.split("|");
      out.push(
        <a key={k++} href="#" onClick={(e) => { e.preventDefault(); onLink?.(target); }}>
          {label ?? target.split(":").pop()}
        </a>,
      );
    } else {
      const mm = /\[([^\]]+)\]\(([^)]+)\)/.exec(tok)!;
      out.push(
        <a key={k++} href="#" onClick={(e) => { e.preventDefault(); onLink?.(mm[2]); }}>
          {mm[1]}
        </a>,
      );
    }
    last = m.index + tok.length;
  }
  if (last < s.length) out.push(s.slice(last));
  return out;
}

export function useDebounced<T>(value: T, ms: number): T {
  const [v, setV] = useState(value);
  useEffect(() => {
    const t = setTimeout(() => setV(value), ms);
    return () => clearTimeout(t);
  }, [value, ms]);
  return v;
}
