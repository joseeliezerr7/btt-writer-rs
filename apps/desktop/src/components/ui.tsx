// Componentes de interfaz reutilizables.
import { createContext, useCallback, useContext, useEffect, useId, useMemo, useRef, useState, type ReactNode } from "react";
import { AlertCircle, CheckCircle2, ChevronRight, Info, X } from "lucide-react";

type Variant = "primary" | "danger" | "ghost" | "soft" | "default";

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
    <button type={props.type ?? "button"} className={cls} onClick={props.onClick} disabled={props.disabled || props.busy} title={props.title} aria-busy={props.busy || undefined}>
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

export function Tabs<T extends string>(props: { tabs: { id: T; label: ReactNode }[]; value: T; onChange: (t: T) => void; segmented?: boolean }) {
  return (
    <div className={"tabs" + (props.segmented ? " segmented" : "")} role="tablist">
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

// ---------------------------------------------------------------- marca y elementos visuales

/** Isotipo de la app: libro abierto sobre un degradado. */
export function Logo({ size = 32, className }: { size?: number; className?: string }) {
  const id = useId().replace(/:/g, "");
  return (
    <svg className={"logo " + (className ?? "")} width={size} height={size} viewBox="0 0 48 48" aria-hidden="true">
      <defs>
        <linearGradient id={`g${id}`} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#2b8fc2" />
          <stop offset="1" stopColor="#0b4668" />
        </linearGradient>
      </defs>
      {/* Mismo dibujo que el icono de la app (apps/desktop/icon-source.png). */}
      <rect width="48" height="48" rx="12" fill={`url(#g${id})`} />
      <path d="M23 15.5C18.8 13 14 12.8 9.5 14.6V34.4C14 32.6 18.8 32.8 23 35.3Z" fill="#fff" />
      <path d="M25 15.5C29.2 13 34 12.8 38.5 14.6V34.4C34 32.6 29.2 32.8 25 35.3Z" fill="#fff" opacity=".83" />
      <path d="M27.2 31.6L37.6 21.2" stroke="#f6c453" strokeWidth="3.4" strokeLinecap="round" />
    </svg>
  );
}

const hue = (s: string) => { let h = 0; for (const c of s) h = (h * 31 + c.charCodeAt(0)) % 360; return h; };

/** Avatar con iniciales y color estable derivado del nombre. */
export function Avatar({ name, size = 32 }: { name: string; size?: number }) {
  const initials = name.trim().split(/\s+/).slice(0, 2).map((w) => w[0]?.toUpperCase() ?? "").join("") || "?";
  return <span className="avatar" style={{ ["--h" as string]: hue(name), width: size, height: size, fontSize: size * 0.38 }}>{initials}</span>;
}

/** Monograma de libro (abreviatura) con color por tono. */
export function Monogram({ text, hueKey }: { text: string; hueKey: string }) {
  return <span className="monogram" style={{ ["--h" as string]: hue(hueKey) }}>{text.slice(0, 3)}</span>;
}

export function IconBadge({ children, tone, size }: { children: ReactNode; tone?: "accent" | "success" | "warning" | "danger" | "neutral"; size?: "lg" }) {
  return <span className={["icon-badge", tone ?? "", size ?? ""].join(" ")}>{children}</span>;
}

export function EmptyState(props: { icon: ReactNode; title: ReactNode; description?: ReactNode; children?: ReactNode; tone?: "accent" | "success" | "warning" | "danger" | "neutral" }) {
  return (
    <div className="empty">
      <IconBadge tone={props.tone}>{props.icon}</IconBadge>
      <h2>{props.title}</h2>
      {props.description ? <p>{props.description}</p> : null}
      {props.children ? <div className="row wrap" style={{ justifyContent: "center" }}>{props.children}</div> : null}
    </div>
  );
}

export function Stat({ label, value, icon, tone }: { label: ReactNode; value: ReactNode; icon: ReactNode; tone?: "accent" | "success" | "warning" | "danger" | "neutral" }) {
  return (
    <div className="card stat">
      <IconBadge tone={tone}>{icon}</IconBadge>
      <div>
        <div className="value">{value}</div>
        <div className="label">{label}</div>
      </div>
    </div>
  );
}

/** Fila de opción con icono, título y descripción (menús de acciones). */
export function OptionItem(props: { icon: ReactNode; title: ReactNode; description?: ReactNode; onClick?: () => void; right?: ReactNode; disabled?: boolean; busy?: boolean; tone?: "accent" | "success" | "warning" | "danger" | "neutral" }) {
  return (
    <button type="button" className="option-item" onClick={props.onClick} disabled={props.disabled || props.busy}>
      <IconBadge tone={props.tone}>{props.icon}</IconBadge>
      <span className="option-text">
        <strong>{props.title}</strong>
        {props.description ? <span>{props.description}</span> : null}
      </span>
      {props.busy ? <span className="spinner" style={{ width: 18, height: 18, borderWidth: 2 }} /> : props.right ?? <ChevronRight size={16} className="chev" />}
    </button>
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
            {t.kind === "error" ? <AlertCircle size={18} /> : t.kind === "success" ? <CheckCircle2 size={18} /> : <Info size={18} />}
            <span>{t.text}</span>
          </div>
        ))}
      </div>
    </FeedbackContext.Provider>
  );
}

/** Renderiza Markdown muy simple (encabezados, párrafos, listas, negrita, enlaces) de forma segura. */
const ENTITIES: Record<string, string> = { quot: '"', amp: "&", lt: "<", gt: ">", apos: "'", nbsp: " ", ldquo: "“", rdquo: "”", lsquo: "‘", rsquo: "’", hellip: "…", ndash: "–", mdash: "—" };

function decodeEntities(s: string): string {
  return s.replace(/&(#x[0-9a-f]+|#\d+|[a-z]+);/gi, (m, e: string) => {
    if (e[0] === "#") {
      const code = e[1].toLowerCase() === "x" ? parseInt(e.slice(2), 16) : parseInt(e.slice(1), 10);
      return Number.isFinite(code) ? String.fromCodePoint(code) : m;
    }
    return ENTITIES[e.toLowerCase()] ?? m;
  });
}

/** Las bibliotecas antiguas (tW/tN en español, etc.) guardan HTML en vez de Markdown.
 *  Convierte las etiquetas básicas (p, ul/ol/li, b/strong, i/em, br, h#, a) y las
 *  entidades a Markdown simple; cualquier otra etiqueta se elimina. */
export function htmlToMarkdown(s: string): string {
  if (!/<\/?(p|ul|ol|li|b|i|em|strong|br|a|h[1-6]|div|span)\b|&(quot|amp|lt|gt|apos|nbsp|#\d+|#x[0-9a-f]+);/i.test(s)) return s;
  let t = s.replace(/\r\n/g, "\n");
  t = t.replace(/<br\s*\/?>/gi, "\n");
  t = t.replace(/<h([1-6])[^>]*>/gi, (_, n: string) => "\n\n" + "#".repeat(Number(n)) + " ").replace(/<\/h[1-6]>/gi, "\n\n");
  t = t.replace(/<p[^>]*>/gi, "\n\n").replace(/<\/p>/gi, "\n\n");
  t = t.replace(/<\/?(ul|ol)[^>]*>/gi, "\n\n");
  t = t.replace(/<li[^>]*>\s*/gi, "\n* ").replace(/<\/li>/gi, "");
  t = t.replace(/<(b|strong)[^>]*>/gi, "**").replace(/<\/(b|strong)>/gi, "**");
  t = t.replace(/<\/?(i|em)[^>]*>/gi, "");
  t = t.replace(/<a\s+[^>]*href=["']([^"']*)["'][^>]*>([\s\S]*?)<\/a>/gi, "[$2]($1)");
  t = t.replace(/<[^>]+>/g, "");
  t = decodeEntities(t);
  // Una lista es un solo bloque: sus elementos van en líneas seguidas.
  t = t.replace(/\n{2,}(\* )/g, "\n\n$1").replace(/(\* [^\n]*)\n{2,}(?=\* )/g, "$1\n");
  return t.replace(/[ \t]+\n/g, "\n").replace(/\n{3,}/g, "\n\n").trim();
}

export function SimpleMarkdown({ text, onLink }: { text: string; onLink?: (href: string) => void }) {
  const blocks = htmlToMarkdown(text).replace(/\r\n/g, "\n").split(/\n{2,}/);
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
