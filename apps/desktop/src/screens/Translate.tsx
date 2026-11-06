// Pantalla de traducción: modos lectura / chunk / revisión, fuentes, recursos,
// búsqueda, historial, conflictos y edición de ayudas (tN/tQ/tW).
import { useCallback, useEffect, useRef, useState, type ChangeEvent, type ReactNode } from "react";
import { AlertTriangle, BookOpen, Check, CheckCheck, Download, Eye, EyeOff, History, Home, LayoutList, ListChecks, Move, Pencil, Plus, Printer, Search, Settings as SettingsIcon, Upload, Users, X } from "lucide-react";
import {
  api,
  type ChunkData,
  type ChunkInfo,
  type HelpItem,
  type HistoryEntry,
  type Helps,
  type ProjectDetail,
  type ResourceRow,
  type SourceChunkView,
  type WordArticle,
} from "../api";
import { useApp } from "../app-context";
import { formatDate, useT } from "../i18n";
import { Button, Dialog, IconBadge, Logo, SimpleMarkdown, Spinner, Switch, Tabs, useFeedback } from "../components/ui";
import { bookName, DownloadLanguageButton, ExportDialog, issueText, PrintDialog, SourceSelectionDialog } from "../components/dialogs";
import { ago, CollabDialog, CollabPill, useCollab, type Collab } from "../components/collab";
import { insertMarkerAt, MARKER_MIME, normalizeVerseMarkers, removeMarker, VerseArranger } from "../components/verses";
import { VerseEditor, VERSE_MIME, type VerseEditorHandle } from "../components/verse-editor";
import { removeVerse } from "../components/verse-markers";

/** Lista numerada de la verificación previa a marcar un chunk como terminado. */
function Checklist({ t }: { t: (k: string) => string }) {
  return (
    <>
      <p style={{ margin: "0 0 .5rem" }}>{t("translate.checklist_intro")}</p>
      <ol className="checklist">
        {t("translate.checklist_items").split("\n").map((item, i) => <li key={i}>{item}</li>)}
      </ol>
    </>
  );
}


type Mode = "read" | "chunk" | "review";

export default function TranslateScreen({ id, initialChapter, initialChunk, initialMode }: { id: string; initialChapter?: string; initialChunk?: string; initialMode?: Mode }) {
  const { t, lang } = useT();
  const { navigate, settings } = useApp();
  const fb = useFeedback();
  const [detail, setDetail] = useState<ProjectDetail | null>(null);
  const isHelps = detail ? detail.manifest.type.id !== "text" : false;
  const [mode, setMode] = useState<Mode>(initialMode ?? "read");
  const [chapter, setChapter] = useState<string>(initialChapter ?? "");
  const [sourceIdx, setSourceIdx] = useState(0);
  const [showSources, setShowSources] = useState(false);
  const [showExport, setShowExport] = useState(false);
  const [showPrint, setShowPrint] = useState(false);
  const [showSearch, setShowSearch] = useState(false);
  const [blind, setBlind] = useState(settings.blind_draft);
  const [focus, setFocus] = useState<string | null>(initialChunk ?? null);
  const [refresh, setRefresh] = useState(0);
  const [bookTitleLabel, setBookTitleLabel] = useState("");
  const loadedOnce = useRef(false);
  const [showCollab, setShowCollab] = useState(false);
  // Chunks que cambiaron en la última sincronización del equipo (las tarjetas afectadas se recargan).
  const [remote, setRemote] = useState<{ tick: number; changed: string[] }>({ tick: 0, changed: [] });

  const load = useCallback(async () => {
    try {
      const d = await api.getProject(id);
      setDetail(d);
      if (d.manifest.type.id !== "text") setMode("review");
      setBookTitleLabel(d.book_title || bookName(d.item));
      setChapter((c) => c || (initialChapter ?? (d.chapters.find((x) => x.id !== "front")?.id ?? "front")));
    } catch (e) {
      fb.error(e);
      // Solo la primera carga devuelve a Inicio; un fallo pasajero al recargar
      // (archivo bloqueado un instante) no debe sacar al usuario del proyecto.
      if (!loadedOnce.current) navigate({ name: "home" });
    }
    loadedOnce.current = true;
  }, [id]);
  useEffect(() => {
    load();
  }, [load, refresh]);

  const collab = useCollab(id, (changed, conflicts) => {
    setRemote((r) => ({ tick: r.tick + 1, changed }));
    load();
    if (conflicts.length) fb.toast(t("collab.remote_conflicts", { n: conflicts.length }), "error");
    else if (changed.length) fb.toast(t("collab.remote_changes", { n: changed.length }), "success");
  });

  // Autoguardado git cada 2 minutos y al salir.
  useEffect(() => {
    const iv = setInterval(() => api.commitProject(id).catch(() => {}), 120000);
    return () => {
      clearInterval(iv);
      api.commitProject(id).catch(() => {});
    };
  }, [id]);

  const sources = detail?.sources ?? [];
  const source: ResourceRow | undefined = sources[sourceIdx] ?? sources[0];
  const sourceReady = !!source && source.downloaded;
  const [downloading, setDownloading] = useState(false);
  const downloadCurrentSource = async () => {
    if (!source) return;
    setDownloading(true);
    try {
      await api.downloadSource(source.language_slug, source.project_slug, source.slug);
      fb.toast(t("common.downloaded"), "success");
      setRefresh((r) => r + 1);
    } catch (e) {
      fb.error(String(e).includes("403") || String(e).includes("anti-bot") ? t("library.cdn_blocked") : e);
    } finally {
      setDownloading(false);
    }
  };
  const chapters = detail?.chapters ?? [];
  const current = chapters.find((c) => c.id === chapter);
  const bookLabel = detail ? `${bookTitleLabel} · ${detail.manifest.target_language.name}` : "";
  const rtl = detail?.manifest.target_language.direction === "rtl";

  const chapterLabel = (c: string) => (c === "front" ? t("translate.book_title") : c.replace(/^0+/, "") || c);

  if (!detail) return <div className="screen"><Spinner /></div>;

  const conflictCount = chapters.reduce((n, c) => n + c.chunks.filter((k) => k.has_conflict).length, 0);
  const jumpToConflict = () => {
    setMode("review");
    const c = chapters.find((x) => x.chunks.some((k) => k.has_conflict));
    if (c) {
      setChapter(c.id);
      setFocus(c.chunks.find((k) => k.has_conflict)?.id ?? null);
    }
  };

  return (
    <div className="translate">
      <aside className="sidebar">
        <Logo size={34} className="logo" />
        <button className="nav-btn" title={t("translate.home")} onClick={() => navigate({ name: "home" })}><Home size={19} /><span>{t("translate.home")}</span></button>
        <span className="sep" />
        {!isHelps && (
          <>
            <button className={"nav-btn" + (mode === "read" ? " active" : "")} title={t("translate.read")} onClick={() => setMode("read")}><BookOpen size={19} /><span>{t("mode.read")}</span></button>
            <button className={"nav-btn" + (mode === "chunk" ? " active" : "")} title={t("translate.chunk")} onClick={() => setMode("chunk")}><LayoutList size={19} /><span>{t("mode.chunk")}</span></button>
          </>
        )}
        <button className={"nav-btn" + (mode === "review" ? " active" : "")} title={t("translate.review")} onClick={() => setMode("review")}><ListChecks size={19} /><span>{t("mode.review")}</span></button>
        {conflictCount > 0 && <button className="nav-btn danger" title={t("home.conflicts")} onClick={jumpToConflict}><AlertTriangle size={19} /><span className="count">{conflictCount}</span></button>}
        <span className="grow" />
        {!isHelps && <button className={"nav-btn" + (showSearch ? " active" : "")} title={t("translate.search")} onClick={() => setShowSearch((s) => !s)}><Search size={19} /><span>{t("common.search")}</span></button>}
        <button className="nav-btn" title={t("translate.publish")} onClick={() => navigate({ name: "publish", id })}><CheckCheck size={19} /><span>{t("nav.publish")}</span></button>
        <button className="nav-btn" title={t("translate.upload")} onClick={() => setShowExport(true)}><Upload size={19} /><span>{t("nav.upload")}</span></button>
        {!isHelps && <button className="nav-btn" title={t("translate.print")} onClick={() => setShowPrint(true)}><Printer size={19} /><span>{t("translate.print")}</span></button>}
        <button className="nav-btn" title={t("home.settings")} onClick={() => navigate({ name: "settings" })}><SettingsIcon size={19} /><span>{t("home.settings")}</span></button>
      </aside>

      <main className="main">
        <div className="topbar">
          <div style={{ minWidth: 0 }}>
            <div className="title ellipsis">{bookLabel}</div>
            <div className="row" style={{ gap: ".5rem" }}>
              <div className="progress-bar thin" style={{ width: 90 }}><div style={{ width: `${detail.item.total_chunks ? Math.round((detail.item.finished_chunks / detail.item.total_chunks) * 100) : 0}%` }} /></div>
              <span className="muted small">{detail.item.finished_chunks}/{detail.item.total_chunks} {t("translate.chunks_done")}</span>
            </div>
          </div>
          <span className="grow" />
          {conflictCount > 0 && <Button size="sm" variant="danger" onClick={jumpToConflict}><AlertTriangle size={14} /> {t("translate.conflicts_count", { n: conflictCount })}</Button>}
          <div className="row" style={{ gap: ".4rem" }}>
            {sources.length > 0 && (
              <div className="source-tabs">
                {sources.map((s, i) => (
                  <button key={s.language_slug + s.slug} className={"tab" + (i === sourceIdx ? " active" : "")} onClick={() => setSourceIdx(i)} title={`${s.language_name} - ${s.name}`}>
                    {s.language_slug.toUpperCase()} {s.slug.toUpperCase()} {!s.downloaded && "⬇"}
                  </button>
                ))}
              </div>
            )}
            <Button size="sm" variant="ghost" onClick={() => setShowSources(true)}><Plus size={14} /> {sources.length ? t("translate.add_source") : t("translate.edit_source")}</Button>
          </div>
          {mode !== "read" && !isHelps && (
            <Button size="sm" variant={blind ? "primary" : "ghost"} title={t("translate.blind")} onClick={() => setBlind((b) => !b)}>{blind ? <EyeOff size={14} /> : <Eye size={14} />}</Button>
          )}
          <CollabPill collab={collab} onClick={() => setShowCollab(true)} />
          {mode === "review" && !isHelps && <MarkAllButton id={id} onDone={() => setRefresh((r) => r + 1)} />}
        </div>

        {showSearch && <SearchBar id={id} onPick={(ch, chunk) => { setChapter(ch); setFocus(chunk); setMode("review"); }} onClose={() => setShowSearch(false)} />}

        <div className="chapter-nav">
          <span className="label">{t("translate.chapters")}</span>
          {chapters.map((c) => {
            const done = c.chunks.length > 0 && c.chunks.every((k) => k.finished);
            return (
              <button key={c.id} className={(c.id === chapter ? "active" : "") + (done ? " done" : "")} onClick={() => { setChapter(c.id); setFocus(null); }} title={c.title}>
                {chapterLabel(c.id)}
              </button>
            );
          })}
        </div>

        {!sourceReady && !isHelps && (
          <div className="source-banner">
            <BookOpen size={18} />
            <div className="grow">
              <strong>{source ? t("translate.source_not_downloaded", { name: `${source.language_name} · ${source.name}` }) : t("translate.no_source")}</strong>
              <div className="small muted">{t("translate.no_source_hint")}</div>
            </div>
            {source && <Button size="sm" variant="primary" busy={downloading} onClick={downloadCurrentSource}><Download size={14} /> {t("translate.download_this_book")}</Button>}
            {source && <DownloadLanguageButton language={source.language_slug} languageName={source.language_name} resource={source.slug} resourceName={source.name} onDone={() => setRefresh((r) => r + 1)} />}
            <Button size="sm" onClick={() => setShowSources(true)}>{t("translate.edit_source")}</Button>
          </div>
        )}
        {current ? (
          <ChapterView
            key={`${chapter}-${mode}-${source?.language_slug}-${source?.slug}-${refresh}`}
            detail={detail}
            chapter={current}
            mode={mode}
            source={source}
            blind={blind}
            rtl={!!rtl}
            focus={focus}
            onBegin={() => setMode("chunk")}
            onChanged={() => { load(); }}
            onResolve={jumpToConflict}
            collab={collab}
            remote={remote}
          />
        ) : null}
      </main>

      {showSources && <SourceSelectionDialog detail={detail} onClose={() => setShowSources(false)} onChanged={() => setRefresh((r) => r + 1)} />}
      {showExport && <ExportDialog project={detail.item} onClose={() => setShowExport(false)} onReview={(files) => { setShowExport(false); setMode("review"); const [ch, f] = files[0]?.split("/") ?? []; if (ch) { setChapter(ch); setFocus(f?.replace(".txt", "") ?? null); } setRefresh((r) => r + 1); }} />}
      {showPrint && <PrintDialog id={id} isObs={detail.manifest.resource.id === "obs"} onClose={() => setShowPrint(false)} />}
      {showCollab && <CollabDialog id={id} collab={collab} onClose={() => setShowCollab(false)} />}
      <span className="hidden">{formatDate(0, lang, "")}</span>
    </div>
  );
}

// ---------------------------------------------------------------- marcar todos

function MarkAllButton({ id, onDone }: { id: string; onDone: () => void }) {
  const { t } = useT();
  const fb = useFeedback();
  const [busy, setBusy] = useState(false);
  const run = async () => {
    const ok = await fb.confirm(t("translate.mark_all"), <Checklist t={t} />);
    if (!ok) return;
    setBusy(true);
    try {
      const r = await api.markAllChunks(id);
      fb.toast(t("translate.mark_all_result", { marked: r.marked, total: r.total }), "success");
      onDone();
    } catch (e) {
      fb.error(e);
    } finally {
      setBusy(false);
    }
  };
  return <Button size="sm" busy={busy} onClick={run}>{t("translate.mark_all")}</Button>;
}

// ---------------------------------------------------------------- búsqueda

function SearchBar({ id, onPick, onClose }: { id: string; onPick: (ch: string, chunk: string) => void; onClose: () => void }) {
  const { t } = useT();
  const [q, setQ] = useState("");
  const [inSource, setInSource] = useState(false);
  const [hits, setHits] = useState<{ chapter: string; chunk: string; snippet: string }[]>([]);
  const [idx, setIdx] = useState(0);
  const search = async () => {
    const h = await api.searchProject(id, q, inSource).catch(() => []);
    setHits(h);
    setIdx(0);
    if (h[0]) onPick(h[0].chapter, h[0].chunk);
  };
  const step = (d: number) => {
    if (!hits.length) return;
    const n = (idx + d + hits.length) % hits.length;
    setIdx(n);
    onPick(hits[n].chapter, hits[n].chunk);
  };
  return (
    <div className="search-bar">
      <input className="input grow" placeholder={inSource ? t("translate.search_source") : t("translate.search_target")} value={q} onChange={(e) => setQ(e.target.value)} onKeyDown={(e) => e.key === "Enter" && search()} autoFocus />
      <Switch on={inSource} onChange={setInSource} label={t("translate.source")} />
      <Button size="sm" onClick={search}>{t("common.search")}</Button>
      <Button size="sm" disabled={!hits.length} onClick={() => step(-1)}>‹</Button>
      <span className="small muted">{hits.length ? `${idx + 1}/${hits.length}` : "0"}</span>
      <Button size="sm" disabled={!hits.length} onClick={() => step(1)}>›</Button>
      <Button size="sm" variant="ghost" icon onClick={onClose}><X size={14} /></Button>
    </div>
  );
}

// ---------------------------------------------------------------- capítulo

function ChapterView(props: {
  detail: ProjectDetail;
  chapter: { id: string; title: string; chunks: ChunkInfo[] };
  mode: Mode;
  source?: ResourceRow;
  blind: boolean;
  rtl: boolean;
  focus: string | null;
  onBegin: () => void;
  onChanged: () => void;
  onResolve: () => void;
  collab: Collab;
  remote: { tick: number; changed: string[] };
}) {
  const { t } = useT();
  const { detail, chapter, mode, source } = props;
  const isHelps = detail.manifest.type.id !== "text";
  const isTw = detail.manifest.type.id === "tw";
  const [sourceChunks, setSourceChunks] = useState<SourceChunkView[] | null>(null);
  const [sourceTitle, setSourceTitle] = useState<{ book: string; chapter: string }>({ book: "", chapter: "" });
  const projectSlug = isTw ? "bible" : detail.manifest.project.id;

  useEffect(() => {
    if (!source || !source.downloaded || chapter.id === "front" || isTw) {
      setSourceChunks([]);
      return;
    }
    api.getSourceChapter(source.language_slug, projectSlug, source.slug, chapter.id)
      .then((list) => setSourceChunks([...list].sort((a, b) => Number(a.chunk) - Number(b.chunk))))
      .catch(() => setSourceChunks([]));
    // `downloaded` va en las dependencias: al terminar una descarga el capítulo se recarga solo.
  }, [source?.language_slug, source?.slug, source?.downloaded, chapter.id]);
  useEffect(() => {
    if (!source || !source.downloaded || isTw) return;
    api.getSourceInfo(source.language_slug, projectSlug, source.slug).then((i) => {
      const ci = i.chapters.indexOf(chapter.id);
      setSourceTitle({ book: i.book_title, chapter: ci >= 0 ? i.chapter_titles[ci] : "" });
    }).catch(() => {});
  }, [source?.language_slug, source?.slug, source?.downloaded, chapter.id]);

  const focusRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    focusRef.current?.scrollIntoView({ block: "start", behavior: "smooth" });
  }, [props.focus, sourceChunks]);

  if (sourceChunks === null) return <Spinner />;

  const sourceFor = (chunk: string) => sourceChunks.find((s) => s.chunk === chunk);

  if (mode === "read" && !isHelps) {
    return (
      <div className="cards">
        {chapter.chunks.some((k) => k.has_conflict) && (
          <div className="card pad row conflict-banner">
            <IconBadge tone="danger"><AlertTriangle size={18} /></IconBadge>
            <div className="grow">
              <strong>{t("translate.conflicts_banner")}</strong>
              <div className="small muted">{t("translate.conflicts_banner_desc")}</div>
            </div>
            <Button variant="danger" onClick={props.onResolve}>{t("translate.resolve_conflicts")}</Button>
          </div>
        )}
        <div className="chunk-card">
          <div className="read-card">
            <div className="pane-head"><span className="title">{t("translate.source")}</span><span className="grow" /><span className="ref">{source?.language_name} · {source?.name}</span></div>
            <h3>{chapter.id === "front" ? sourceTitle.book : sourceTitle.chapter || `${t("translate.chapter")} ${chapter.id.replace(/^0+/, "")}`}</h3>
            <div className="source-text">
              {chapter.id === "front" ? sourceTitle.book : sourceChunks.map((s) => <SourceText key={s.chunk} chunk={s} />)}
            </div>
          </div>
          <div className="read-card">
            <div className="pane-head"><span className="title">{t("translate.target")}</span><span className="grow" /><span className="ref">{detail.manifest.target_language.name}</span></div>
            <ReadTarget id={detail.item.id} chapter={chapter} rtl={props.rtl} />
            <div style={{ marginTop: "1rem" }}><Button variant="primary" onClick={props.onBegin}>{t("translate.begin")}</Button></div>
          </div>
        </div>
      </div>
    );
  }

  const rank = (id: string) => (id === "title" ? -2 : id === "reference" ? -1 : Number(id) || 0);
  const orderedChunks = [...chapter.chunks].sort((a, b) => rank(a.id) - rank(b.id) || a.id.localeCompare(b.id));
  return (
    <div className="cards">
      {orderedChunks.map((c) => (
        <div key={c.id} ref={props.focus === c.id ? focusRef : undefined}>
          <ChunkCard
            detail={detail}
            chapter={chapter.id}
            chunk={c}
            source={source}
            sourceChunk={sourceFor(c.id)}
            sourceBookTitle={sourceTitle.book}
            sourceChapterTitle={sourceTitle.chapter}
            review={mode === "review"}
            blind={props.blind}
            rtl={props.rtl}
            focused={props.focus === c.id}
            onChanged={props.onChanged}
            collab={props.collab}
            remote={props.remote}
          />
        </div>
      ))}
      {chapter.chunks.length === 0 && <p className="muted">{t("common.none")}</p>}
    </div>
  );
}

function ReadTarget({ id, chapter, rtl }: { id: string; chapter: { id: string; chunks: ChunkInfo[] }; rtl: boolean }) {
  const [texts, setTexts] = useState<Record<string, string>>({});
  useEffect(() => {
    Promise.all(chapter.chunks.map((c) => api.readChunk(id, chapter.id, c.id).then((d) => [c.id, d.text] as const))).then((all) => setTexts(Object.fromEntries(all))).catch(() => {});
  }, [id, chapter]);
  return (
    <div className={"target-text" + (rtl ? " rtl" : "")}>
      {chapter.chunks.map((c) => (
        <div key={c.id}>{c.id === "title" ? <h4>{texts[c.id]}</h4> : <UsfmText text={texts[c.id] ?? ""} />}</div>
      ))}
    </div>
  );
}

// ---------------------------------------------------------------- render de texto

export function SourceText({ chunk }: { chunk: SourceChunkView }) {
  if (!chunk.verses.length) return <p>{chunk.raw.replace(/<[^>]+>/g, "")}</p>;
  const out: ReactNode[] = [];
  let para: ReactNode[] = [];
  let k = 0;
  const flush = () => {
    if (para.length) out.push(<p key={k++}>{para}</p>);
    para = [];
  };
  if (chunk.preamble) out.push(<h4 key={k++}>{chunk.preamble}</h4>);
  for (const v of chunk.verses) {
    if (v.paragraphs_before.length) flush();
    para.push(<sup key={`s${v.number}`}>{v.number}</sup>, v.text, " ");
    v.footnotes.forEach((f, i) => para.push(<span key={`f${v.number}${i}`} className="fn" title={f}>✱</span>, " "));
  }
  flush();
  return <>{out}</>;
}

/** Muestra USFM de la traducción con versos en superíndice (sin edición). */
export function UsfmText({ text }: { text: string }) {
  const parts = text.replace(/\r\n/g, "\n").split(/(\\v\s+\d+[a-z]?(?:-\d+[a-z]?)?\s*)/);
  const out: ReactNode[] = [];
  for (let i = 0; i < parts.length; i++) {
    const p = parts[i];
    const m = /^\\v\s+(\S+)/.exec(p);
    if (m) out.push(<sup key={i}>{m[1]}</sup>);
    else if (p) out.push(<span key={i}>{p.replace(/\\f\s.*?\\f\*/g, " ✱ ").replace(/\\[a-z0-9]+\*?\s?/g, " ")}</span>);
  }
  return <>{out}</>;
}

// ---------------------------------------------------------------- tarjeta de chunk

function ChunkCard(props: {
  detail: ProjectDetail;
  chapter: string;
  chunk: ChunkInfo;
  source?: ResourceRow;
  sourceChunk?: SourceChunkView;
  sourceBookTitle: string;
  sourceChapterTitle: string;
  review: boolean;
  blind: boolean;
  rtl: boolean;
  focused: boolean;
  onChanged: () => void;
  collab: Collab;
  remote: { tick: number; changed: string[] };
}) {
  const { t, lang } = useT();
  const fb = useFeedback();
  const { detail, chapter, chunk, source } = props;
  const id = detail.item.id;
  const type = detail.manifest.type.id;
  const isTw = type === "tw";
  const isHelps = type !== "text";
  const isTitle = chunk.id === "title" || chunk.id === "reference" || chapter === "front";
  const [data, setData] = useState<ChunkData | null>(null);
  const [text, setText] = useState("");
  const [editing, setEditing] = useState(false);
  const [saved, setSaved] = useState(true);
  const [helps, setHelps] = useState<Helps | null>(null);
  const [helpTab, setHelpTab] = useState<"notes" | "words" | "questions">("notes");
  const [word, setWord] = useState<WordArticle | null>(null);
  const [history, setHistory] = useState<HistoryEntry[] | null>(null);
  const [sourceWord, setSourceWord] = useState<WordArticle | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Los títulos usan un área de texto simple; los chunks con versículos, el editor con fichas.
  const textRef = useRef<HTMLTextAreaElement>(null);
  const editorRef = useRef<VerseEditorHandle>(null);
  const caretNow = () => (isTitle ? textRef.current?.selectionStart : editorRef.current?.getCaret()) ?? text.length;
  const focusAt = (c: number) => {
    if (isTitle) {
      const el = textRef.current;
      if (el) { el.focus(); el.setSelectionRange(c, c); }
    } else {
      editorRef.current?.focus();
      editorRef.current?.setCaret(c);
    }
  };
  // Modo "Mover versículos": el texto se muestra como palabras y fichas arrastrables.
  const [arrange, setArrange] = useState(false);
  // Versículo de la barra elegido con un toque, a la espera de tocar dónde va.
  const [pickedNew, setPickedNew] = useState<number | null>(null);
  const [overChips, setOverChips] = useState(false);
  // Posición del cursor a restaurar tras una normalización que añadió espacios.
  const caretFix = useRef<number | null>(null);

  const reload = useCallback(() => api.readChunk(id, chapter, chunk.id).then((d) => { setData(d); setText(d.text); }).catch(fb.error), [id, chapter, chunk.id]);
  useEffect(() => {
    reload();
  }, [reload]);
  useEffect(() => {
    if (props.review && source?.downloaded && !isTitle && !isTw) {
      api.getHelps(source.language_slug, detail.manifest.project.id, source.slug, chapter, chunk.id).then((h) => {
        setHelps(h);
        if (h.notes.length === 0 && h.words.length) setHelpTab("words");
        else if (h.notes.length === 0 && h.questions.length) setHelpTab("questions");
      }).catch(() => setHelps(null));
    }
    if (isTw && source?.downloaded) api.getWord(source.language_slug, chunk.id).then(setSourceWord).catch(() => {});
  }, [props.review, source?.language_slug, source?.slug, chapter, chunk.id]);

  // Texto escrito que todavía no se ha enviado al disco (el guardado espera 500 ms tras la última tecla).
  const pending = useRef<string | null>(null);
  // Las escrituras de esta tarjeta van en serie: una lenta nunca puede aterrizar
  // después de otra más nueva y dejar en disco un texto viejo.
  const queue = useRef<Promise<unknown>>(Promise.resolve());
  const writeNow = (value: string) => {
    const p = queue.current.then(() => api.writeChunk(id, chapter, chunk.id, value));
    queue.current = p.catch(() => {});
    return p;
  };
  const save = (value: string) => {
    setText(value);
    setSaved(false);
    pending.current = value;
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(async () => {
      try {
        await writeNow(value);
        // Si no se escribió nada más mientras tanto, ya no hay texto pendiente.
        if (pending.current === value) pending.current = null;
        setSaved(true);
      } catch (e) {
        fb.error(e); // lo pendiente se conserva para reintentar al salir
      }
    }, 500);
  };
  // Al desmontar la tarjeta (cambio de capítulo, de modo o de proyecto) se guarda lo pendiente en vez de perderlo,
  // y se deja en el historial. El id, capítulo y chunk van fijados en la llamada: nunca se escribe sobre otro proyecto.
  useEffect(() => () => {
    if (timer.current) clearTimeout(timer.current);
    const value = pending.current;
    if (value !== null) {
      pending.current = null;
      writeNow(value).then(() => api.commitProject(id)).catch((e) => console.warn("guardado al salir:", e));
    }
  }, []);

  const toggleDone = async (finished: boolean) => {
    if (finished) {
      if (timer.current) { clearTimeout(timer.current); pending.current = null; await writeNow(text).catch(fb.error); }
      const ok = await fb.confirm(t("translate.checklist_title"), <Checklist t={t} />, { okLabel: t("common.confirm") });
      if (!ok) return;
    }
    try {
      const issues = await api.setChunkFinished(id, chapter, chunk.id, finished);
      if (issues.length) {
        fb.toast(`${t("translate.issues")} ${issues.map((i) => issueText(t, i)).join(" ")}`, "error");
        return;
      }
      setEditing(false);
      endCollab();
      await reload();
      props.onChanged();
    } catch (e) {
      fb.error(e);
    }
  };

  // ---- equipo: reserva del chunk mientras se edita y recarga cuando cambia en el servidor
  const collabKey = `${chapter}/${chunk.id}`;
  const claim = props.collab.enabled ? props.collab.claims.find((c) => c.key === collabKey && c.user !== props.collab.me && c.until * 1000 > Date.now()) : undefined;
  const editingRef = useRef(false);
  editingRef.current = editing;
  /** Avisa si otra persona tiene el chunk y reserva. `false` si el usuario desiste. */
  const beginCollab = async (ask: boolean) => {
    if (!props.collab.enabled) return true;
    if (claim && ask) {
      const ok = await fb.confirm(t("collab.claimed_title"), t("collab.claimed_confirm", { user: claim.user, ago: ago(claim.since, t) }), { okLabel: t("collab.edit_anyway") });
      if (!ok) return false;
    } else if (claim) {
      fb.toast(t("collab.claimed_by", { user: claim.user }), "error");
    }
    void props.collab.startEditing(collabKey);
    return true;
  };
  const endCollab = () => {
    if (props.collab.enabled) void props.collab.stopEditing(collabKey);
  };
  useEffect(() => () => { if (editingRef.current) endCollab(); }, []);
  useEffect(() => {
    if (props.remote.changed.includes(`${collabKey}.txt`)) {
      // Lo que llegó del servidor manda: se descarta lo no guardado de los últimos instantes.
      if (timer.current) clearTimeout(timer.current);
      pending.current = null;
      reload();
    }
  }, [props.remote.tick]);

  const startEdit = async () => {
    if (data?.finished) {
      const ok = await fb.confirm(t("translate.done"), t("translate.reopen"));
      if (!ok) return;
      try {
        await api.setChunkFinished(id, chapter, chunk.id, false);
      } catch (e) {
        fb.error(e);
        return;
      }
      await reload();
      props.onChanged();
    }
    if (!(await beginCollab(true))) return;
    setEditing(true);
    setTimeout(() => (isTitle ? textRef.current?.focus() : editorRef.current?.focus()), 50);
  };

  /** Lo escrito en el área de texto, separando lo que quede pegado a un marcador `\v`. */
  const onEditorChange = (e: ChangeEvent<HTMLTextAreaElement>) => {
    const el = e.target;
    const fixed = normalizeVerseMarkers(el.value, el.selectionStart ?? el.value.length);
    if (fixed.text !== el.value) caretFix.current = fixed.caret;
    save(fixed.text);
  };
  useEffect(() => {
    const c = caretFix.current;
    if (c !== null && textRef.current) {
      caretFix.current = null;
      textRef.current.setSelectionRange(c, c);
    }
  }, [text]);

  const insertFootnote = () => {
    const pos = caretNow();
    const note = " \\f + \\ft " + (window.prompt(t("translate.footnote")) ?? "").trim() + "\\f* ";
    if (note.trim() === "\\f + \\ft\\f*") return;
    save(text.slice(0, pos) + note + text.slice(pos));
    setTimeout(() => focusAt(pos + note.length), 50);
  };

  /** Versos ya marcados en el borrador (`\v 3` o rangos `\v 3-4`). */
  const versesInText = new Set<number>();
  for (const m of text.matchAll(/\\v\s+(\d+)(?:-(\d+))?/g)) {
    const a = Number(m[1]);
    const b = m[2] ? Number(m[2]) : a;
    for (let n = a; n <= b; n++) versesInText.add(n);
  }

  /** Inserta `\v N` en el cursor (o en la posición dada, o al final) con los espacios necesarios. */
  const insertVerse = (n: number, at?: number) => {
    if (versesInText.has(n)) return;
    const pos = at ?? caretNow();
    const r = insertMarkerAt(text, n, pos);
    save(r.text);
    setTimeout(() => focusAt(r.caret), 50);
  };
  const resolveConflict = async (option: string) => {
    await api.writeChunk(id, chapter, chunk.id, option).catch(fb.error);
    await reload();
    props.onChanged();
  };

  const openHistory = async () => {
    setHistory(await api.chunkHistory(id, chapter, chunk.id).catch(() => []));
  };
  const restore = async (entry: HistoryEntry) => {
    if (entry.current) {
      setHistory(null);
      return;
    }
    if (timer.current) clearTimeout(timer.current);
    pending.current = null;
    try {
      const restored = await api.restoreChunk(id, entry.id, chapter, chunk.id);
      setText(restored);
      setSaved(true);
      setHistory(null);
      await reload();
      props.onChanged();
      fb.toast(t("translate.restored_toast", { date: formatDate(entry.time, lang, "") }), "success");
    } catch (e) {
      fb.error(e);
    }
  };
  /** Texto legible del motivo de una versión ("auto save", "restore abc1234", mensajes de fusión…). */
  const historyLabel = (h: HistoryEntry, all: HistoryEntry[]) => {
    if (h.message === "auto save" || /GMT|UTC|^\d{4}-\d\d-\d\d/.test(h.message)) return t("translate.autosave");
    const m = /^restore ([0-9a-f]{7})/.exec(h.message);
    if (m) {
      const from = all.find((x) => x.id.startsWith(m[1]));
      return from ? t("translate.restored_from", { date: formatDate(from.time, lang, "") }) : t("translate.restored");
    }
    return h.message;
  };

  const title = chapter === "front" ? t("translate.book_title") : chunk.id === "title" ? t("translate.chapter_title") : chunk.id === "reference" ? t("translate.chapter_reference") : isTw ? sourceWord?.title || chunk.id : `${chapter.replace(/^0+/, "")}:${props.sourceChunk?.chunk_title || chunk.id.replace(/^0+/, "")}`;
  const showHelps = props.review && !isTitle && !isHelps && helps && (helps.has_notes || helps.has_words || helps.has_questions);
  const expected = data?.expected_verses ?? [];
  const placeholder = isTitle ? "" : expected.length ? expected.map((v) => `\\v ${v} `).join("") : t("translate.verses_hint");
  const canEdit = !data?.finished && (editing || !props.review || isHelps);
  const arrangeBtn = canEdit && !isTitle && !isHelps && !data?.has_conflict ? (
    <Button size="sm" variant={arrange ? "primary" : "ghost"} icon={!arrange} title={t("translate.arrange")} onClick={() => { setArrange((a) => !a); setPickedNew(null); }}>
      <Move size={14} />{arrange ? ` ${t("common.done")}` : null}
    </Button>
  ) : null;

  return (
    <div className={"chunk-card" + (showHelps ? " with-helps" : "")}>
      {/* Fuente */}
      <div className="pane">
        <div className="pane-head"><span className="title">{title}</span><span className="grow" />{source ? <span className="ref">{source.language_slug} {source.slug}</span> : null}</div>
        {props.blind && !props.review ? (
          <div className="muted" style={{ flex: 1, display: "grid", placeItems: "center" }}><EyeOff /> {t("translate.blind")}</div>
        ) : (
          <div className="source-text">
            {chapter === "front" ? props.sourceBookTitle : chunk.id === "title" ? props.sourceChapterTitle || `${t("translate.chapter")} ${chapter.replace(/^0+/, "")}` : isTw ? (
              sourceWord ? <><h4>{sourceWord.title}</h4><SimpleMarkdown text={sourceWord.body} /></> : <span className="muted">{chunk.id}</span>
            ) : props.sourceChunk ? <SourceText chunk={props.sourceChunk} /> : <span className="muted">—</span>}
            {isHelps && !isTw && source?.downloaded && !isTitle && <SourceHelps source={source} project={detail.manifest.project.id} chapter={chapter} chunk={chunk.id} type={type} />}
          </div>
        )}
      </div>

      {/* Traducción */}
      <div className={"pane target" + (data?.finished ? " done" : "") + (data?.has_conflict ? " conflict" : "")}>
        <div className="pane-head">
          <span className="title">{t("translate.target")}</span>
          {claim && <span className="badge warn claim-badge" title={`${t("collab.claimed_by", { user: claim.user })} · ${t("collab.claimed_since", { ago: ago(claim.since, t) })}`}><Users size={11} /> {claim.user}</span>}
          {!saved ? <span className="save-state"><span className="spinner" style={{ width: 10, height: 10, borderWidth: 2 }} /> {t("translate.saving")}</span> : data?.finished && !props.review ? <span className="badge ok"><Check size={11} /> {t("translate.done")}</span> : null}
          <span className="grow" />
          {data && !isTitle && !data.has_conflict && (
            props.review ? (
              <>
                {!data.finished && <Button size="sm" variant={editing ? "primary" : "ghost"} icon={!editing} title={t("translate.chunk")} onClick={() => { if (editing) { setEditing(false); endCollab(); } else startEdit(); }}>{editing ? t("common.done") : <Pencil size={14} />}</Button>}
                {editing && !isHelps && !arrange && <Button size="sm" variant="ghost" title={t("translate.add_footnote")} onClick={insertFootnote}>ƒ</Button>}
                {editing && arrangeBtn}
                <Button size="sm" variant="ghost" icon title={t("translate.history")} onClick={openHistory}><History size={14} /></Button>
                <Switch on={data.finished} onChange={toggleDone} label={<span className="small">{t("translate.done")}</span>} />
              </>
            ) : data.finished ? (
              <Button size="sm" variant="ghost" icon title={t("translate.chunk")} onClick={startEdit}><Pencil size={14} /></Button>
            ) : (
              arrangeBtn
            )
          )}
        </div>

        {!data ? (
          <Spinner />
        ) : data.has_conflict ? (
          <div className="col">
            <strong className="small row" style={{ color: "var(--danger)" }}><AlertTriangle size={14} /> {t("translate.conflict_title")}</strong>
            <span className="small muted">{t("translate.conflicts_banner_desc")}</span>
            {data.conflict_options.map((o, i) => {
              const origin = data.conflict_origins?.[i] ?? null;
              const times = (data.conflict_origins ?? []).map((c) => c?.time ?? 0);
              const newest = Math.max(...times);
              const distinct = new Set(times.filter((x) => x > 0)).size > 1;
              const isNewest = distinct && !!origin && origin.time === newest;
              const sideLabel = data.conflict_options.length === 2 ? (i === 0 ? t("translate.this_copy") : t("translate.other_copy")) : null;
              return (
              <div key={i} className={"conflict-option" + (isNewest ? " newest" : "")}>
                <div className="row wrap" style={{ marginBottom: ".45rem", rowGap: ".3rem" }}>
                  <span className="badge danger">{t("translate.option", { n: i + 1 })}</span>
                  {sideLabel && <span className="badge">{sideLabel}</span>}
                  {distinct && origin && <span className={"badge " + (isNewest ? "ok" : "warn")}>{isNewest ? t("translate.newest") : t("translate.older")}</span>}
                  <span className="grow" />
                  <Button size="sm" variant="primary" onClick={() => resolveConflict(o)}><Check size={14} /> {t("translate.keep")}</Button>
                </div>
                <div className="origin small muted" style={{ marginBottom: ".4rem" }}>
                  {origin ? (
                    <><History size={13} /> <strong>{origin.author || "—"}</strong> · {formatDate(origin.time, lang, "")}</>
                  ) : (
                    <><History size={13} /> {t("translate.unknown_origin")}</>
                  )}
                </div>
                <div className={"target-text" + (props.rtl ? " rtl" : "")} style={{ fontSize: "calc(var(--size-target) * 0.8)" }}><UsfmText text={o} /></div>
              </div>
              );
            })}
          </div>
        ) : isHelps && !isTitle ? (
          <HelpsEditor key={data.text} items={data.helps} type={type} readOnly={!canEdit} onChange={async (items) => { const txt = await api.writeHelps(id, chapter, chunk.id, items).catch((e) => { fb.error(e); return null; }); if (txt !== null) setText(txt); }} />
        ) : canEdit ? (
          <>
            {!isTitle && !isHelps && expected.length > 0 && (
              <div
                className={"verse-chips" + (overChips ? " drop-remove" : "")}
                title={overChips ? t("translate.drop_to_remove") : t("translate.verse_chips_hint")}
                // La barra también sirve para quitar un marcador: se arrastra hasta aquí desde el editor o desde "Mover versículos".
                onDragOver={(e) => { const ty = e.dataTransfer.types; if (ty.includes(MARKER_MIME) || ty.includes(VERSE_MIME)) { e.preventDefault(); e.dataTransfer.dropEffect = "move"; if (!overChips) setOverChips(true); } }}
                onDragLeave={() => setOverChips(false)}
                onDrop={(e) => {
                  setOverChips(false);
                  const n = e.dataTransfer.getData(VERSE_MIME);
                  const idx = e.dataTransfer.getData(MARKER_MIME);
                  if (n !== "") { e.preventDefault(); save(removeVerse(text, n)); }
                  else if (idx !== "") { e.preventDefault(); save(removeMarker(text, Number(idx))); }
                }}
              >
                <span className="small muted" style={{ marginRight: ".2rem" }}>{t("translate.verse_chips")}</span>
                {expected.map((v) => {
                  const used = versesInText.has(v);
                  return (
                    <button
                      key={v}
                      type="button"
                      className={"verse-chip" + (used ? " used" : "") + (pickedNew === v ? " picked" : "")}
                      draggable={!used}
                      disabled={used}
                      onDragStart={(e) => { e.dataTransfer.setData("text/plain", `\\v ${v} `); e.dataTransfer.effectAllowed = "copy"; }}
                      onClick={() => (arrange ? setPickedNew((p) => (p === v ? null : v)) : insertVerse(v))}
                      title={used ? t("translate.verse_used", { n: v }) : t("translate.verse_insert", { n: v })}
                    >
                      {v}
                    </button>
                  );
                })}
              </div>
            )}
            {arrange ? (
              <VerseArranger text={text} rtl={props.rtl} onChange={(v) => save(v)} pickedNew={pickedNew} onPlacedNew={() => setPickedNew(null)} />
            ) : isTitle ? (
              <textarea
                ref={textRef}
                className={"editor" + (props.rtl ? " rtl" : "")}
                value={text}
                placeholder={placeholder}
                onChange={onEditorChange}
                onFocus={() => { if (!props.review && !editing) { setEditing(true); void beginCollab(false); } }}
                spellCheck={false}
              />
            ) : (
              // Los versículos van como fichas dentro del texto: se arrastran a otra palabra
              // o a la barra de arriba para quitarlos; una ficha de la barra se suelta donde va.
              <VerseEditor
                ref={editorRef}
                text={text}
                rtl={props.rtl}
                placeholder={placeholder}
                onChange={save}
                onFocus={() => { if (!props.review && !editing) { setEditing(true); void beginCollab(false); } }}
              />
            )}
          </>
        ) : (
          <div className={"target-text" + (props.rtl ? " rtl" : "")} onDoubleClick={startEdit}>
            {isTitle ? text : <UsfmText text={text} />}
            {!text && <span className="muted">{placeholder}</span>}
          </div>
        )}
        {data && data.issues.length > 0 && props.review && !isTitle && text && (
          <div className="issue-text"><AlertTriangle size={14} style={{ marginTop: 2, flex: "none" }} /><span>{data.issues.map((i) => issueText(t, i)).join(" ")}</span></div>
        )}
      </div>

      {/* Recursos */}
      {showHelps && helps && (
        <div className="pane helps">
          <Tabs value={helpTab} onChange={setHelpTab} tabs={[
            ...(helps.has_notes ? [{ id: "notes" as const, label: `${t("translate.notes")} (${helps.notes.length})` }] : []),
            ...(helps.has_words ? [{ id: "words" as const, label: `${t("translate.words")} (${helps.words.length})` }] : []),
            ...(helps.has_questions ? [{ id: "questions" as const, label: `${t("translate.questions")} (${helps.questions.length})` }] : []),
          ]} />
          <div style={{ overflow: "auto", maxHeight: 420 }}>
            {helpTab === "notes" && helps.notes.map((n, i) => <div key={i} className="help-entry"><h4>{n.title}</h4><SimpleMarkdown text={n.body} onLink={(h) => openHelpLink(h, source, setWord)} /></div>)}
            {helpTab === "questions" && helps.questions.map((q, i) => <div key={i} className="help-entry"><h4>{q.title}</h4><SimpleMarkdown text={q.body} /></div>)}
            {helpTab === "words" && (
              <>
                {helps.words.map((w) => <div key={w.slug} className="help-entry"><h4><a href="#" onClick={(e) => { e.preventDefault(); setWord(w); }}>{w.title}</a></h4></div>)}
                <WordIndexButton language={source!.language_slug} onPick={setWord} />
              </>
            )}
            {helpTab === "notes" && helps.notes.length === 0 && <p className="muted small">{t("common.none")}</p>}
          </div>
        </div>
      )}
      {props.review && !isTitle && !isHelps && helps && !showHelps && <div className="pane helps muted small">{t("translate.no_helps")}</div>}

      {word && (
        <Dialog title={word.title} onClose={() => setWord(null)}>
          <SimpleMarkdown text={word.body} onLink={(h) => openHelpLink(h, source, setWord)} />
          {word.see_also.length > 0 && (
            <p className="small"><strong>{t("translate.see_also")}:</strong> {word.see_also.map((s) => <a key={s} href="#" style={{ marginRight: ".4rem" }} onClick={(e) => { e.preventDefault(); source && api.getWord(source.language_slug, s).then((w) => w && setWord(w)); }}>{s}</a>)}</p>
          )}
        </Dialog>
      )}
      {history && (
        <Dialog title={t("translate.history")} onClose={() => setHistory(null)}>
          <p className="muted small" style={{ margin: "0 0 .6rem" }}>{t("translate.history_hint")}</p>
          <div className="card list" style={{ maxHeight: 420, overflow: "auto" }}>
            {history.map((h) => (
              <div key={h.id} className={"list-item history-item" + (h.current ? " selected" : "")} onClick={() => restore(h)}>
                <div className="grow col" style={{ gap: ".15rem", minWidth: 0 }}>
                  <span className="small">
                    <strong>{formatDate(h.time, lang, "")}</strong>
                    {h.author && <span className="muted"> · {h.author}</span>}
                    <span className="muted"> · {historyLabel(h, history)}</span>
                  </span>
                  <span className="small muted history-preview">{h.preview || t("translate.empty_version")}</span>
                </div>
                {h.current ? <span className="badge success">{t("translate.current_version")}</span> : <span className="small muted">{t("translate.restore_version")}</span>}
              </div>
            ))}
            {history.length === 0 && <div className="list-item muted">{t("common.none")}</div>}
          </div>
        </Dialog>
      )}
    </div>
  );
}

function openHelpLink(href: string, source: ResourceRow | undefined, setWord: (w: WordArticle) => void) {
  // [t](../kt/priest.md) o [[:en:obe:kt:priest]] → palabra; rc://*/tw/dict/bible/kt/priest
  const m = /(?:\/|:)([a-z0-9_-]+)(?:\.md)?$/i.exec(href.replace(/\/01\.md$/, ""));
  if (m && source) api.getWord(source.language_slug, m[1].toLowerCase()).then((w) => w && setWord(w)).catch(() => {});
}

function WordIndexButton({ language, onPick }: { language: string; onPick: (w: WordArticle) => void }) {
  const { t } = useT();
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const [list, setList] = useState<{ slug: string; title: string }[]>([]);
  useEffect(() => {
    if (!open) return;
    let alive = true;
    const t = setTimeout(() => api.wordIndex(language, q).then((l) => alive && setList(l)).catch(() => alive && setList([])), 150);
    return () => { alive = false; clearTimeout(t); };
  }, [open, q, language]);
  return (
    <>
      <Button size="sm" block onClick={() => setOpen(true)}>{t("translate.word_index")}</Button>
      {open && (
        <Dialog title={t("translate.word_index")} narrow onClose={() => setOpen(false)}>
          <input className="input" placeholder={t("common.search")} value={q} onChange={(e) => setQ(e.target.value)} autoFocus />
          <div className="card list" style={{ maxHeight: 400, overflow: "auto", marginTop: ".5rem" }}>
            {list.slice(0, 300).map((w) => <div key={w.slug} className="list-item" onClick={() => { api.getWord(language, w.slug).then((x) => x && onPick(x)); setOpen(false); }}>{w.title}</div>)}
          </div>
        </Dialog>
      )}
    </>
  );
}

/** Para proyectos tN/tQ: muestra las notas/preguntas del idioma fuente bajo el texto fuente. */
function SourceHelps({ source, project, chapter, chunk, type }: { source: ResourceRow; project: string; chapter: string; chunk: string; type: string }) {
  const [helps, setHelps] = useState<Helps | null>(null);
  useEffect(() => {
    api.getHelps(source.language_slug, project, source.slug, chapter, chunk).then(setHelps).catch(() => {});
  }, [source.language_slug, source.slug, chapter, chunk]);
  if (!helps) return null;
  const list = type === "tq" ? helps.questions : helps.notes;
  return <div style={{ marginTop: ".8rem", borderTop: "1px solid var(--border)", paddingTop: ".5rem" }}>{list.map((n, i) => <div key={i} className="help-entry"><h4>{n.title}</h4><SimpleMarkdown text={n.body} /></div>)}</div>;
}

/** Editor de entradas título/cuerpo para tN, tQ y tW. */
function HelpsEditor({ items, type, readOnly, onChange }: { items: HelpItem[]; type: string; readOnly: boolean; onChange: (items: HelpItem[]) => void }) {
  const { t } = useT();
  const [list, setList] = useState<HelpItem[]>(items.length ? items : type === "tw" ? [{ title: "", body: "" }] : []);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pending = useRef<HelpItem[] | null>(null);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;
  const update = (next: HelpItem[]) => {
    setList(next);
    pending.current = next;
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => { pending.current = null; onChange(next); }, 500);
  };
  // Lo pendiente se entrega al desmontar en vez de perderse.
  useEffect(() => () => {
    if (timer.current) clearTimeout(timer.current);
    const next = pending.current;
    if (next !== null) { pending.current = null; onChangeRef.current(next); }
  }, []);
  const [titleLabel, bodyLabel] = type === "tq" ? [t("translate.question"), t("translate.answer")] : type === "tw" ? [t("translate.word_title"), t("translate.word_body")] : [t("translate.help_title"), t("translate.help_body")];
  return (
    <div className="col">
      {list.map((it, i) => (
        <div key={i} className="card pad col" style={{ gap: ".3rem" }}>
          <input className="input" placeholder={titleLabel} value={it.title} readOnly={readOnly} onChange={(e) => update(list.map((x, j) => (j === i ? { ...x, title: e.target.value } : x)))} />
          <textarea className="textarea" style={{ minHeight: 80 }} placeholder={bodyLabel} value={it.body} readOnly={readOnly} onChange={(e) => update(list.map((x, j) => (j === i ? { ...x, body: e.target.value } : x)))} />
          {!readOnly && type !== "tw" && <div className="row" style={{ justifyContent: "flex-end" }}><Button size="sm" variant="ghost" onClick={() => update(list.filter((_, j) => j !== i))}>{t("common.remove")}</Button></div>}
        </div>
      ))}
      {!readOnly && type !== "tw" && <Button size="sm" onClick={() => update([...list, { title: "", body: "" }])}>{t("translate.add_entry")}</Button>}
    </div>
  );
}
