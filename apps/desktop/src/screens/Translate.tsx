// Pantalla de traducción: modos lectura / chunk / revisión, fuentes, recursos,
// búsqueda, historial, conflictos y edición de ayudas (tN/tQ/tW).
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { AlertTriangle, BookOpen, CheckCheck, Eye, EyeOff, History, Home, LayoutList, ListChecks, Printer, Search, Settings as SettingsIcon, Upload, X } from "lucide-react";
import {
  api,
  type ChunkData,
  type ChunkInfo,
  type HelpItem,
  type Helps,
  type ProjectDetail,
  type ResourceRow,
  type SourceChunkView,
  type WordArticle,
} from "../api";
import { useApp } from "../app-context";
import { formatDate, useT } from "../i18n";
import { Button, Dialog, SimpleMarkdown, Spinner, Switch, Tabs, useFeedback } from "../components/ui";
import { bookName, ExportDialog, issueText, PrintDialog, SourceSelectionDialog } from "../components/dialogs";

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

  const load = useCallback(async () => {
    try {
      const d = await api.getProject(id);
      setDetail(d);
      if (d.manifest.type.id !== "text") setMode("review");
      setBookTitleLabel(d.book_title || bookName(d.item));
      setChapter((c) => c || (initialChapter ?? (d.chapters.find((x) => x.id !== "front")?.id ?? "front")));
    } catch (e) {
      fb.error(e);
      navigate({ name: "home" });
    }
  }, [id]);
  useEffect(() => {
    load();
  }, [load, refresh]);

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
  const chapters = detail?.chapters ?? [];
  const current = chapters.find((c) => c.id === chapter);
  const bookLabel = detail ? `${bookTitleLabel} · ${detail.manifest.target_language.name}` : "";
  const rtl = detail?.manifest.target_language.direction === "rtl";

  const chapterLabel = (c: string) => (c === "front" ? t("translate.book_title") : c.replace(/^0+/, "") || c);

  if (!detail) return <div className="screen"><Spinner /></div>;

  const conflictCount = chapters.reduce((n, c) => n + c.chunks.filter((k) => k.has_conflict).length, 0);

  return (
    <div className="translate">
      <aside className="sidebar">
        <Button variant="ghost" icon title={t("translate.home")} onClick={() => navigate({ name: "home" })}><Home size={18} /><span>{t("translate.home")}</span></Button>
        {!isHelps && (
          <>
            <Button icon className={mode === "read" ? "active" : ""} title={t("translate.read")} onClick={() => setMode("read")}><BookOpen size={18} /><span>{t("translate.read").split(" ")[1] ?? "Read"}</span></Button>
            <Button icon className={mode === "chunk" ? "active" : ""} title={t("translate.chunk")} onClick={() => setMode("chunk")}><LayoutList size={18} /><span>Chunk</span></Button>
          </>
        )}
        <Button icon className={mode === "review" ? "active" : ""} title={t("translate.review")} onClick={() => setMode("review")}><ListChecks size={18} /><span>{t("translate.review").split(" ")[1] ?? "Review"}</span></Button>
        {conflictCount > 0 && <Button icon variant="danger" title={t("home.conflicts")} onClick={() => { setMode("review"); const c = chapters.find((x) => x.chunks.some((k) => k.has_conflict)); if (c) { setChapter(c.id); setFocus(c.chunks.find((k) => k.has_conflict)?.id ?? null); } }}><AlertTriangle size={18} /><span>{conflictCount}</span></Button>}
        <span className="grow" />
        {!isHelps && <Button variant="ghost" icon title={t("translate.search")} onClick={() => setShowSearch((s) => !s)}><Search size={18} /></Button>}
        <Button variant="ghost" icon title={t("translate.publish")} onClick={() => navigate({ name: "publish", id })}><CheckCheck size={18} /></Button>
        <Button variant="ghost" icon title={t("translate.upload")} onClick={() => setShowExport(true)}><Upload size={18} /></Button>
        {!isHelps && <Button variant="ghost" icon title={t("translate.print")} onClick={() => setShowPrint(true)}><Printer size={18} /></Button>}
        <Button variant="ghost" icon title={t("home.settings")} onClick={() => navigate({ name: "settings" })}><SettingsIcon size={18} /></Button>
      </aside>

      <main className="main">
        <div className="topbar" style={{ padding: ".4rem .8rem" }}>
          <strong className="ellipsis">{bookLabel}</strong>
          <span className="muted small">{detail.item.finished_chunks}/{detail.item.total_chunks} {t("translate.chunks_done")}</span>
          <span className="grow" />
          <div className="row" style={{ gap: ".2rem" }}>
            {sources.map((s, i) => (
              <button key={s.language_slug + s.slug} className={"tab" + (i === sourceIdx ? " active" : "")} onClick={() => setSourceIdx(i)} title={`${s.language_name} - ${s.name}`}>
                {s.language_slug.toUpperCase()} {s.slug.toUpperCase()} {!s.downloaded && "⬇"}
              </button>
            ))}
            <Button size="sm" onClick={() => setShowSources(true)}>{sources.length ? t("translate.add_source") : t("translate.edit_source")}</Button>
          </div>
          {mode !== "read" && !isHelps && (
            <Button size="sm" variant={blind ? "primary" : "ghost"} title={t("translate.blind")} onClick={() => setBlind((b) => !b)}>{blind ? <EyeOff size={14} /> : <Eye size={14} />}</Button>
          )}
          {mode === "review" && !isHelps && <MarkAllButton id={id} onDone={() => setRefresh((r) => r + 1)} />}
        </div>

        {showSearch && <SearchBar id={id} onPick={(ch, chunk) => { setChapter(ch); setFocus(chunk); setMode("review"); }} onClose={() => setShowSearch(false)} />}

        <div className="chapter-nav">
          {chapters.map((c) => {
            const done = c.chunks.length > 0 && c.chunks.every((k) => k.finished);
            return (
              <button key={c.id} className={"btn sm" + (c.id === chapter ? " active" : "") + (done ? " done" : "")} onClick={() => { setChapter(c.id); setFocus(null); }} title={c.title}>
                {chapterLabel(c.id)}
              </button>
            );
          })}
        </div>

        {!sourceReady && !isHelps ? (
          <div className="welcome">
            <div className="card">
              <h2>{t("translate.no_source")}</h2>
              <Button variant="primary" onClick={() => setShowSources(true)}>{t("translate.edit_source")}</Button>
            </div>
          </div>
        ) : current ? (
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
            onChanged={() => setRefresh((r) => r + 1)}
          />
        ) : null}
      </main>

      {showSources && <SourceSelectionDialog detail={detail} onClose={() => setShowSources(false)} onChanged={() => setRefresh((r) => r + 1)} />}
      {showExport && <ExportDialog project={detail.item} onClose={() => setShowExport(false)} onReview={(files) => { setShowExport(false); setMode("review"); const [ch, f] = files[0]?.split("/") ?? []; if (ch) { setChapter(ch); setFocus(f?.replace(".txt", "") ?? null); } setRefresh((r) => r + 1); }} />}
      {showPrint && <PrintDialog id={id} isObs={detail.manifest.resource.id === "obs"} onClose={() => setShowPrint(false)} />}
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
    const ok = await fb.confirm(t("translate.mark_all"), t("translate.checklist_body"));
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
    api.getSourceChapter(source.language_slug, projectSlug, source.slug, chapter.id).then(setSourceChunks).catch(() => setSourceChunks([]));
  }, [source?.language_slug, source?.slug, chapter.id]);
  useEffect(() => {
    if (!source || !source.downloaded || isTw) return;
    api.getSourceInfo(source.language_slug, projectSlug, source.slug).then((i) => {
      const ci = i.chapters.indexOf(chapter.id);
      setSourceTitle({ book: i.book_title, chapter: ci >= 0 ? i.chapter_titles[ci] : "" });
    }).catch(() => {});
  }, [source?.language_slug, source?.slug, chapter.id]);

  const focusRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    focusRef.current?.scrollIntoView({ block: "start", behavior: "smooth" });
  }, [props.focus, sourceChunks]);

  if (sourceChunks === null) return <Spinner />;

  const sourceFor = (chunk: string) => sourceChunks.find((s) => s.chunk === chunk);

  if (mode === "read" && !isHelps) {
    return (
      <div className="cards">
        <div className="chunk-card">
          <div className="read-card">
            <div className="pane-head"><span className="title">{t("translate.source")}</span> {source?.language_name} · {source?.name}</div>
            <h3>{chapter.id === "front" ? sourceTitle.book : sourceTitle.chapter || `${t("translate.chapter")} ${chapter.id.replace(/^0+/, "")}`}</h3>
            <div className="source-text">
              {chapter.id === "front" ? sourceTitle.book : sourceChunks.map((s) => <SourceText key={s.chunk} chunk={s} />)}
            </div>
          </div>
          <div className="read-card">
            <div className="pane-head"><span className="title">{t("translate.target")}</span> {detail.manifest.target_language.name}</div>
            <ReadTarget id={detail.item.id} chapter={chapter} rtl={props.rtl} />
            <div style={{ marginTop: "1rem" }}><Button variant="primary" onClick={props.onBegin}>{t("translate.begin")}</Button></div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="cards">
      {chapter.chunks.map((c) => (
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
}) {
  const { t } = useT();
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
  const [history, setHistory] = useState<{ id: string; time: number; message: string }[] | null>(null);
  const [sourceWord, setSourceWord] = useState<WordArticle | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const textRef = useRef<HTMLTextAreaElement>(null);

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

  const save = (value: string) => {
    setText(value);
    setSaved(false);
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(async () => {
      try {
        await api.writeChunk(id, chapter, chunk.id, value);
        setSaved(true);
      } catch (e) {
        fb.error(e);
      }
    }, 500);
  };
  useEffect(() => () => { if (timer.current) clearTimeout(timer.current); }, []);

  const toggleDone = async (finished: boolean) => {
    if (finished) {
      if (timer.current) { clearTimeout(timer.current); await api.writeChunk(id, chapter, chunk.id, text).catch(fb.error); }
      const ok = await fb.confirm(t("translate.checklist_title"), t("translate.checklist_body"), { okLabel: t("common.confirm") });
      if (!ok) return;
    }
    try {
      const issues = await api.setChunkFinished(id, chapter, chunk.id, finished);
      if (issues.length) {
        fb.toast(`${t("translate.issues")} ${issues.map((i) => issueText(t, i)).join(" ")}`, "error");
        return;
      }
      setEditing(false);
      await reload();
      props.onChanged();
    } catch (e) {
      fb.error(e);
    }
  };

  const startEdit = async () => {
    if (data?.finished) {
      const ok = await fb.confirm(t("translate.done"), t("translate.reopen"));
      if (!ok) return;
      await api.setChunkFinished(id, chapter, chunk.id, false).catch(fb.error);
      await reload();
      props.onChanged();
    }
    setEditing(true);
    setTimeout(() => textRef.current?.focus(), 50);
  };

  const insertFootnote = () => {
    const el = textRef.current;
    if (!el) return;
    const pos = el.selectionStart ?? text.length;
    const note = " \\f + \\ft " + (window.prompt(t("translate.footnote")) ?? "").trim() + "\\f* ";
    if (note.trim() === "\\f + \\ft\\f*") return;
    save(text.slice(0, pos) + note + text.slice(pos));
  };

  const resolveConflict = async (option: string) => {
    await api.writeChunk(id, chapter, chunk.id, option).catch(fb.error);
    await reload();
    props.onChanged();
  };

  const openHistory = async () => {
    setHistory(await api.chunkHistory(id, chapter, chunk.id).catch(() => []));
  };
  const restore = async (commit: string) => {
    const old = await api.chunkAtCommit(id, commit, chapter, chunk.id).catch((e) => { fb.error(e); return null; });
    if (old === null) return;
    save(old);
    setHistory(null);
  };

  const title = chapter === "front" ? t("translate.book_title") : chunk.id === "title" ? t("translate.chapter_title") : chunk.id === "reference" ? t("translate.chapter_reference") : isTw ? sourceWord?.title || chunk.id : `${chapter.replace(/^0+/, "")}:${props.sourceChunk?.chunk_title || chunk.id.replace(/^0+/, "")}`;
  const showHelps = props.review && !isTitle && !isHelps && helps && (helps.has_notes || helps.has_words || helps.has_questions);
  const expected = data?.expected_verses ?? [];
  const placeholder = isTitle ? "" : expected.length ? expected.map((v) => `\\v ${v} `).join("") : t("translate.verses_hint");
  const canEdit = !data?.finished && (editing || !props.review);

  return (
    <div className={"chunk-card" + (showHelps ? " with-helps" : "")}>
      {/* Fuente */}
      <div className="pane">
        <div className="pane-head"><span className="title">{title}</span><span className="grow" />{source ? `${source.language_slug} ${source.slug}` : ""}</div>
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
          {!saved && <span className="small muted">…</span>}
          <span className="grow" />
          {data && !isTitle && !data.has_conflict && (
            props.review ? (
              <>
                {!data.finished && <Button size="sm" variant={editing ? "primary" : "ghost"} onClick={() => (editing ? setEditing(false) : startEdit())}>{editing ? t("common.done") : "✎"}</Button>}
                {editing && !isHelps && <Button size="sm" variant="ghost" title={t("translate.add_footnote")} onClick={insertFootnote}>ƒ</Button>}
                <Button size="sm" variant="ghost" icon title={t("translate.history")} onClick={openHistory}><History size={14} /></Button>
                <Switch on={data.finished} onChange={toggleDone} label={<span className="small">{t("translate.done")}</span>} />
              </>
            ) : data.finished ? (
              <Button size="sm" onClick={startEdit}>{t("translate.done")} ✓</Button>
            ) : null
          )}
        </div>

        {!data ? (
          <Spinner />
        ) : data.has_conflict ? (
          <div className="col">
            <strong className="small" style={{ color: "var(--danger)" }}>{t("translate.conflict_title")}</strong>
            {data.conflict_options.map((o, i) => (
              <div key={i} className="conflict-option" onClick={() => resolveConflict(o)} title={t("translate.keep")}>{o}</div>
            ))}
          </div>
        ) : isHelps && !isTitle ? (
          <HelpsEditor items={data.helps} type={type} readOnly={!canEdit} onChange={async (items) => { const txt = await api.writeHelps(id, chapter, chunk.id, items).catch((e) => { fb.error(e); return null; }); if (txt !== null) setText(txt); }} />
        ) : canEdit ? (
          <textarea
            ref={textRef}
            className={"editor" + (props.rtl ? " rtl" : "")}
            value={text}
            placeholder={placeholder}
            onChange={(e) => save(e.target.value)}
            onFocus={() => { if (!props.review) setEditing(true); }}
            spellCheck={false}
          />
        ) : (
          <div className={"target-text" + (props.rtl ? " rtl" : "")} onDoubleClick={startEdit}>
            {isTitle ? text : <UsfmText text={text} />}
            {!text && <span className="muted">{placeholder}</span>}
          </div>
        )}
        {data && data.issues.length > 0 && props.review && !isTitle && text && (
          <div className="small" style={{ color: "var(--warning)", marginTop: ".4rem" }}>{data.issues.map((i) => issueText(t, i)).join(" ")}</div>
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
        <Dialog title={t("translate.history")} narrow onClose={() => setHistory(null)}>
          <div className="card list" style={{ maxHeight: 360, overflow: "auto" }}>
            {history.map((h) => <div key={h.id} className="list-item" onClick={() => restore(h.id)}><span className="grow small">{new Date(h.time * 1000).toLocaleString()} · {h.message}</span><span className="small muted">{t("translate.restore_version")}</span></div>)}
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
    if (open) api.wordIndex(language, q).then(setList).catch(() => setList([]));
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
  const update = (next: HelpItem[]) => {
    setList(next);
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => onChange(next), 500);
  };
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
