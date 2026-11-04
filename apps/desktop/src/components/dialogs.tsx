// Diálogos compartidos: detalles de proyecto, nuevo proyecto, fuentes,
// importar, exportar, biblioteca, respaldos, migración, comentarios.
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import { Archive, Check, Cloud, Database, Download, FileText, FileUp, FolderOpen, HardDriveDownload, Languages, Printer, RefreshCw, Server, Trash2, UploadCloud, UserPlus } from "lucide-react";
import {
  api,
  DEFAULT_PRINT,
  isTauri,
  resourceLabel,
  type CategoryRow,
  type ImportMode,
  type ImportOutcome,
  type Issue,
  type PrintRequest,
  type ProjectDetail,
  type ProjectItem,
  type ProjectRow,
  type Repo,
  type ResourceRow,
  type TargetLanguageRow,
  type UsfmImportSummary,
} from "../api";
import { useApp } from "../app-context";
import { formatDate, useT } from "../i18n";
import { Button, Dialog, Field, IconBadge, OptionItem, Spinner, Switch, Tabs, useDebounced, useFeedback } from "./ui";

export function issueText(t: (k: string, v?: Record<string, string | number>) => string, i: Issue): string {
  return "verse" in i ? t(`issue.${i.kind}`, { verse: i.verse }) : t(`issue.${i.kind}`);
}

export function importStatusText(t: (k: string) => string, o: ImportOutcome): string {
  const s = o.status;
  if (s.status === "failed") return `${t("import.status.failed")}: ${s.error}`;
  if (s.status === "conflicts") return `${t("import.status.conflicts")} (${s.files.join(", ")})`;
  return t(`import.status.${s.status}`);
}

export const bookName = (p: { project_id: string; project_name: string }, books?: ProjectRow[]) =>
  books?.find((b) => b.slug === p.project_id)?.name || p.project_name || p.project_id.toUpperCase();

// ---------------------------------------------------------------- colaboradores

export function ContributorsEditor({ id, translators, onChange }: { id: string; translators: string[]; onChange: () => void }) {
  const { t } = useT();
  const fb = useFeedback();
  const [adding, setAdding] = useState(false);
  const [name, setName] = useState("");
  const [agree, setAgree] = useState(false);

  const save = async (list: string[]) => {
    try {
      await api.setTranslators(id, list);
      onChange();
    } catch (e) {
      fb.error(e);
    }
  };
  return (
    <div className="col">
      {translators.map((name) => (
        <div key={name} className="row card" style={{ padding: ".5rem .8rem" }}>
          <span className="grow">{name}</span>
          <Button variant="ghost" icon size="sm" title={t("common.remove")} onClick={() => save(translators.filter((x) => x !== name))}>
            <Trash2 size={16} />
          </Button>
        </div>
      ))}
      {adding ? (
        <div className="card pad col">
          <Field label={t("details.contributor_name")}>
            <input className="input" value={name} onChange={(e) => setName(e.target.value)} autoFocus />
          </Field>
          <label className="checkbox small">
            <input type="checkbox" checked={agree} onChange={(e) => setAgree(e.target.checked)} />
            <span>{t("details.contributor_agree")}</span>
          </label>
          <div className="row" style={{ justifyContent: "flex-end" }}>
            <Button onClick={() => setAdding(false)}>{t("common.cancel")}</Button>
            <Button variant="primary" disabled={!name.trim() || !agree} onClick={() => { save([...translators, name.trim()]); setName(""); setAgree(false); setAdding(false); }}>
              {t("common.add")}
            </Button>
          </div>
        </div>
      ) : (
        <Button onClick={() => setAdding(true)}><UserPlus size={16} /> {t("details.add_contributor")}</Button>
      )}
    </div>
  );
}

// ---------------------------------------------------------------- detalles de proyecto

export function ProjectDetailsDialog({ item, onClose, onChanged, onDeleted }: { item: ProjectItem; onClose: () => void; onChanged: () => void; onDeleted: () => void }) {
  const { t, lang } = useT();
  const { navigate, settings } = useApp();
  const fb = useFeedback();
  const [detail, setDetail] = useState<ProjectDetail | null>(null);
  const [books, setBooks] = useState<ProjectRow[]>([]);
  const [showExport, setShowExport] = useState(false);
  const [showPrint, setShowPrint] = useState(false);
  const [title, setTitle] = useState("");

  const load = async () => {
    try {
      const d = await api.getProject(item.id);
      setDetail(d);
      setTitle(d.book_title);
    } catch (e) {
      fb.error(e);
    }
  };
  useEffect(() => {
    load();
    api.listBooks(lang === "es" ? "es-419" : "en").then(setBooks).catch(() => {});
  }, [item.id]);

  const changeResource = async (res: string) => {
    try {
      await api.changeResource(item.id, res);
      onChanged();
      onClose();
    } catch (e) {
      fb.error(e);
    }
  };

  const del = async () => {
    const ok = await fb.confirm(t("common.delete"), t("details.delete_confirm"), { danger: true, okLabel: t("common.delete") });
    if (!ok) return;
    try {
      const path = await api.deleteProject(item.id);
      fb.toast(`${t("details.deleted")} ${path}`, "success");
      onDeleted();
    } catch (e) {
      fb.error(e);
    }
  };

  const saveTitle = async () => {
    if (!detail || title === detail.book_title) return;
    await api.setBookTitle(item.id, title).catch(fb.error);
  };

  const p = detail?.item ?? item;
  return (
    <Dialog
      title={`${bookName(p, books)} - ${p.language_name}`}
      onClose={onClose}
      footer={
        <>
          <Button variant="danger" onClick={del}>{t("common.delete")}</Button>
          <span className="grow" />
          {p.type_id === "text" && <Button onClick={() => setShowPrint(true)}>{t("details.print")}</Button>}
          <Button onClick={() => navigate({ name: "publish", id: p.id })}>{t("details.publish")}</Button>
          <Button variant="primary" onClick={() => setShowExport(true)}>{t("details.backup")}</Button>
        </>
      }
    >
      {!detail ? (
        <Spinner />
      ) : (
        <dl className="kv">
          <dt>{t("details.project")}</dt>
          <dd>{bookName(p, books)} ({p.project_id})</dd>
          <dt>{t("details.book_title")}</dt>
          <dd><input className="input" value={title} onChange={(e) => setTitle(e.target.value)} onBlur={saveTitle} /></dd>
          <dt>{t("details.language")}</dt>
          <dd>{p.language_name} ({p.language_id}) {p.language_direction === "rtl" ? "· RTL" : ""}</dd>
          <dt>{t("details.resource")}</dt>
          <dd>
            {resourceLabel(p, t)}
            {p.type_id === "text" && p.resource_id !== "obs" && (
              <select className="select" style={{ width: "auto" }} value={p.resource_id} onChange={(e) => changeResource(e.target.value)}>
                {(settings.gateway_language_mode ? ["reg", "ulb", "udb"] : ["reg"]).concat(["reg", "ulb", "udb"].includes(p.resource_id) ? [] : [p.resource_id]).filter((v, i, a) => a.indexOf(v) === i).map((r) => (
                  <option key={r} value={r}>{t(`resource.${r}`)}</option>
                ))}
              </select>
            )}
          </dd>
          <dt>{t("details.progress")}</dt>
          <dd className="col" style={{ alignItems: "stretch" }}>
            <div className="progress-bar"><div style={{ width: `${Math.round(p.progress * 100)}%` }} /></div>
            <span className="small muted">{p.finished_chunks} / {p.total_chunks} {t("translate.chunks_done")}</span>
          </dd>
          <dt>{t("details.contributors")}</dt>
          <dd style={{ display: "block" }}><ContributorsEditor id={p.id} translators={detail.manifest.translators} onChange={load} /></dd>
          <dt>{t("home.last_backup")}</dt>
          <dd>{formatDate(p.last_backup, lang, t("home.never"))}</dd>
        </dl>
      )}
      {showExport && <ExportDialog project={p} onClose={() => setShowExport(false)} onReview={(files) => { setShowExport(false); navigate({ name: "translate", id: p.id, mode: "review", chapter: files[0]?.split("/")[0], chunk: files[0]?.split("/")[1]?.replace(".txt", "") }); }} />}
      {showPrint && <PrintDialog id={p.id} isObs={p.resource_id === "obs"} onClose={() => setShowPrint(false)} />}
    </Dialog>
  );
}

// ---------------------------------------------------------------- nuevo proyecto

export function NewProjectDialog({ onClose, onCreated }: { onClose: () => void; onCreated: (id: string) => void }) {
  const { t, lang } = useT();
  const { settings } = useApp();
  const fb = useFeedback();
  const [step, setStep] = useState<"language" | "project" | "type">("language");
  const [langQuery, setLangQuery] = useState("");
  const [langs, setLangs] = useState<TargetLanguageRow[]>([]);
  const [language, setLanguage] = useState<TargetLanguageRow | null>(null);
  const [books, setBooks] = useState<ProjectRow[]>([]);
  const [cats, setCats] = useState<CategoryRow[]>([]);
  const [bookQuery, setBookQuery] = useState("");
  const [book, setBook] = useState<ProjectRow | null>(null);
  const [existing, setExisting] = useState<ProjectItem[]>([]);
  const [busy, setBusy] = useState(false);
  const dq = useDebounced(langQuery, 200);
  const uiLang = lang === "es" ? "es-419" : "en";

  useEffect(() => {
    api.searchLanguages(dq, 80).then(setLangs).catch(fb.error);
  }, [dq]);
  useEffect(() => {
    api.listBooks(uiLang).then(setBooks).catch(fb.error);
    api.listCategories(uiLang).then(setCats).catch(() => {});
    api.listProjects().then(setExisting).catch(() => {});
  }, []);

  const exists = (type: string, resource: string) =>
    !!language && !!book && existing.some((p) => p.language_id === language.slug && p.project_id === book.slug && p.type_id === type && (type !== "text" || p.resource_id === resource));

  const create = async (type: string, resource: string) => {
    if (!language || !book) return;
    if (exists(type, resource)) {
      fb.toast(t("new.exists"), "error");
      return;
    }
    setBusy(true);
    try {
      const id = await api.createProject({ language: language.slug, project: book.slug, type, resource });
      onCreated(id);
    } catch (e) {
      fb.error(e);
    } finally {
      setBusy(false);
    }
  };

  const pickBook = (b: ProjectRow) => {
    setBook(b);
    const isTw = b.slug === "bible" || b.slug === "bible-obs";
    if (isTw) return create("tw", "");
    if (b.slug === "obs") return create("text", "obs");
    if (settings.gateway_language_mode) setStep("type");
    else create("text", "reg");
  };

  const filteredBooks = useMemo(() => {
    const q = bookQuery.trim().toLowerCase();
    return books.filter((b) => (settings.gateway_language_mode || !["bible", "bible-obs"].includes(b.slug)) && (!q || b.name.toLowerCase().includes(q) || b.slug.includes(q)));
  }, [books, bookQuery, settings.gateway_language_mode]);

  const catName = (slug: string) => cats.find((c) => c.slug === slug)?.name || (slug === "bible-ot" ? t("new.ot") : slug === "bible-nt" ? t("new.nt") : t("new.other"));
  const groups = useMemo(() => {
    const g: Record<string, ProjectRow[]> = {};
    for (const b of filteredBooks) (g[b.category || "other"] ??= []).push(b);
    const order = (c: string) => { const i = ["bible-ot", "bible-nt"].indexOf(c); return i === -1 ? 99 : i; };
    return Object.entries(g).sort(([a], [b]) => order(a) - order(b));
  }, [filteredBooks, cats]);

  return (
    <Dialog title={t("new.title")} onClose={onClose} wide>
      <ol className="steps">
        <li className={step === "language" ? "cur" : "done"}><span className="n">{step === "language" ? 1 : <Check size={13} />}</span>{t("new.step.language")}</li>
        <li className={step === "project" ? "cur" : step === "type" ? "done" : ""}><span className="n">{step === "type" ? <Check size={13} /> : 2}</span>{t("new.step.project")}</li>
        {settings.gateway_language_mode && <li className={step === "type" ? "cur" : ""}><span className="n">3</span>{t("new.step.type")}</li>}
      </ol>
      {step === "language" && (
        <div className="col">
          <input className="input" placeholder={t("new.search_language")} value={langQuery} onChange={(e) => setLangQuery(e.target.value)} autoFocus />
          <div className="card list" style={{ maxHeight: 420, overflow: "auto" }}>
            {langs.map((l) => (
              <div key={l.slug + (l.temporary ? "-t" : "")} className="list-item" onClick={() => { setLanguage(l); setStep("project"); }}>
                <div className="grow">
                  <strong>{l.name}</strong> <span className="muted small">{l.anglicized_name && l.anglicized_name !== l.name ? `(${l.anglicized_name})` : ""}</span>
                  <div className="small muted">{l.slug} · {l.region} {l.direction === "rtl" ? "· RTL" : ""} {l.temporary ? `· ${t("new.temporary")}` : ""}</div>
                </div>
              </div>
            ))}
            {langs.length === 0 && <div className="list-item muted">{t("common.none")}</div>}
          </div>
        </div>
      )}
      {step === "project" && language && (
        <div className="col">
          <div className="row">
            <span className="badge">{language.name} ({language.slug})</span>
            <Button size="sm" variant="ghost" onClick={() => setStep("language")}>{t("details.change")}</Button>
          </div>
          <input className="input" placeholder={t("new.search_book")} value={bookQuery} onChange={(e) => setBookQuery(e.target.value)} autoFocus />
          {busy && <Spinner />}
          <div style={{ maxHeight: 440, overflow: "auto" }}>
            {groups.map(([cat, list]) => (
              <div key={cat} style={{ marginBottom: ".8rem" }}>
                <h4 className="muted small" style={{ textTransform: "uppercase" }}>{catName(cat)}</h4>
                <div className="grid-books">
                  {list.map((b) => (
                    <button key={b.slug} className={"btn" + (exists("text", "reg") && book?.slug === b.slug ? " disabled" : "")} onClick={() => pickBook(b)} disabled={busy}>
                      <span className="ellipsis">{b.name}</span> <span className="muted small">{b.slug}</span>
                    </button>
                  ))}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
      {step === "type" && language && book && (
        <div className="col">
          <div className="row">
            <span className="badge">{language.name}</span>
            <span className="badge">{book.name}</span>
            <Button size="sm" variant="ghost" onClick={() => setStep("project")}>{t("details.change")}</Button>
          </div>
          <h3>{t("new.choose_type")}</h3>
          <div className="card list">
            {[
              { type: "text", resource: "ulb", label: t("resource.ulb") },
              { type: "text", resource: "udb", label: t("resource.udb") },
              { type: "text", resource: "reg", label: t("resource.reg") },
              { type: "tn", resource: "", label: t("type.notes") },
              { type: "tq", resource: "", label: t("type.questions") },
            ].map((o) => {
              const has = exists(o.type, o.resource);
              const needsText = (o.type === "tn" || o.type === "tq") && !existing.some((p) => p.language_id === language.slug && p.project_id === book.slug && p.type_id === "text");
              return (
                <div key={o.type + o.resource} className={"list-item" + (has || needsText ? " disabled" : "")} onClick={() => !has && !needsText && create(o.type, o.resource)}>
                  <span className="grow">{o.label}</span>
                  {has && <span className="badge">{t("new.exists")}</span>}
                </div>
              );
            })}
          </div>
        </div>
      )}
    </Dialog>
  );
}

// ---------------------------------------------------------------- selección de fuentes

export function SourceSelectionDialog({ detail, onClose, onChanged }: { detail: ProjectDetail; onClose: () => void; onChanged: () => void }) {
  const { t } = useT();
  const { settings } = useApp();
  const fb = useFeedback();
  const [rows, setRows] = useState<ResourceRow[] | null>(null);
  const [selected, setSelected] = useState<{ language: string; resource: string }[]>(detail.manifest.source_translations.map((s) => ({ language: s.language_id, resource: s.resource_id })));
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const projectSlug = detail.manifest.type.id === "tw" ? "bible" : detail.manifest.project.id;

  const load = () =>
    (detail.manifest.type.id === "tw" ? api.allResources("bible").then((r) => r.filter((x) => x.type === "dict")) : api.availableSources(projectSlug, settings.gateway_language_mode))
      .then(setRows)
      .catch(fb.error);
  useEffect(() => {
    load();
  }, []);

  const isSel = (r: ResourceRow) => selected.some((s) => s.language === r.language_slug && s.resource === r.slug);
  const toggle = (r: ResourceRow) => {
    if (isSel(r)) setSelected(selected.filter((s) => !(s.language === r.language_slug && s.resource === r.slug)));
    else if (selected.length >= 3) fb.toast(t("sources.max"), "error");
    else setSelected([...selected, { language: r.language_slug, resource: r.slug }]);
  };
  const download = async (r: ResourceRow) => {
    const ok = await fb.confirm(t("common.download"), t("sources.download_confirm"));
    if (!ok) return;
    setBusy(r.language_slug + r.slug);
    try {
      await api.downloadSource(r.language_slug, r.project_slug, r.slug);
      await load();
      fb.toast(t("common.downloaded"), "success");
    } catch (e) {
      fb.error(String(e).includes("403") || String(e).includes("anti-bot") ? t("library.cdn_blocked") : e);
    } finally {
      setBusy(null);
    }
  };
  const remove = async (r: ResourceRow) => {
    await api.deleteSource(r.language_slug, r.project_slug, r.slug).catch(fb.error);
    setSelected(selected.filter((s) => !(s.language === r.language_slug && s.resource === r.slug)));
    load();
  };
  const confirm = async () => {
    try {
      await api.setProjectSources(detail.item.id, selected);
      onChanged();
      onClose();
    } catch (e) {
      fb.error(e);
    }
  };

  const q = query.trim().toLowerCase();
  const match = (r: ResourceRow) => !q || r.language_name.toLowerCase().includes(q) || r.language_slug.includes(q) || r.name.toLowerCase().includes(q) || r.slug.includes(q);
  const label = (r: ResourceRow) => `${r.language_name} (${r.language_slug}) - ${r.name}`;
  const sel = (rows ?? []).filter((r) => isSel(r));
  const avail = (rows ?? []).filter((r) => !isSel(r) && r.downloaded && match(r));
  const online = (rows ?? []).filter((r) => !isSel(r) && !r.downloaded && match(r));

  return (
    <Dialog title={t("sources.title")} onClose={onClose} footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" onClick={confirm}>{t("common.confirm")}</Button></>}>
      {!rows ? (
        <Spinner />
      ) : (
        <div className="col">
          <input className="input" placeholder={t("common.search")} value={query} onChange={(e) => setQuery(e.target.value)} />
          <h4 className="muted small">{t("sources.selected")} ({sel.length}/3)</h4>
          <div className="card list">
            {sel.map((r) => (
              <div key={label(r)} className="list-item selected" onClick={() => toggle(r)}>
                <Check size={16} /> <span className="grow">{label(r)}</span>
                {!r.downloaded && <span className="badge warn">{t("common.download")}</span>}
              </div>
            ))}
            {sel.length === 0 && <div className="list-item muted">{t("common.none")}</div>}
          </div>
          <h4 className="muted small">{t("sources.available")}</h4>
          <div className="card list">
            {avail.map((r) => (
              <div key={label(r)} className="list-item" onClick={() => toggle(r)}>
                <span className="grow">{label(r)}</span>
                <Button size="sm" variant="ghost" icon title={t("common.remove")} onClick={() => remove(r)}><Trash2 size={14} /></Button>
              </div>
            ))}
            {avail.length === 0 && <div className="list-item muted">{t("common.none")}</div>}
          </div>
          <h4 className="muted small">{t("sources.online")}</h4>
          <div className="card list" style={{ maxHeight: 260, overflow: "auto" }}>
            {online.map((r) => (
              <div key={label(r)} className="list-item">
                <span className="grow">{label(r)} <span className="muted small">v{r.version}</span></span>
                <Button size="sm" busy={busy === r.language_slug + r.slug} onClick={() => download(r)}><Download size={14} /> {t("common.download")}</Button>
              </div>
            ))}
            {online.length === 0 && <div className="list-item muted small">{t("sources.none_online")}</div>}
          </div>
        </div>
      )}
    </Dialog>
  );
}

// ---------------------------------------------------------------- exportar / subir

export function ExportDialog({ project, onClose, onReview }: { project: ProjectItem; onClose: () => void; onReview?: (files: string[]) => void }) {
  const { t } = useT();
  const { profile, setProfile } = useApp();
  const fb = useFeedback();
  const [busy, setBusy] = useState<string | null>(null);
  const [result, setResult] = useState<ReactNode>(null);
  const [login, setLogin] = useState(false);
  const [showPrint, setShowPrint] = useState(false);
  const [user, setUser] = useState("");
  const [pass, setPass] = useState("");

  const run = async (kind: string, fn: () => Promise<ReactNode>) => {
    setBusy(kind);
    try {
      setResult(await fn());
    } catch (e) {
      fb.error(e);
    } finally {
      setBusy(null);
    }
  };

  const upload = () => {
    if (!profile.session) {
      setLogin(true);
      return;
    }
    run("upload", async () => {
      const r = await api.uploadProject(project.id);
      if (r.merge?.status === "conflicts") {
        return (
          <div className="col">
            <p>{t("export.upload_conflicts")}</p>
            <div className="row">
              <Button variant="primary" onClick={() => onReview?.(r.merge && "files" in r.merge ? r.merge.files : [])}>{t("publish.review")}</Button>
              <Button variant="danger" onClick={() => api.restoreBackupBranch(project.id).then(() => { fb.toast(t("common.done"), "success"); setResult(null); }).catch(fb.error)}>{t("export.restore_local")}</Button>
            </div>
          </div>
        );
      }
      const key = `export.push.${r.push.status}`;
      const ok = r.push.status === "ok" || r.push.status === "up_to_date";
      return (
        <div className="col">
          <p className={ok ? "" : "muted"}><strong>{t(key)}</strong>{r.push.status === "rejected" ? `: ${r.push.reason}` : ""}</p>
          {ok && (
            <>
              <p>{t("export.uploaded")} <a href="#" onClick={(e) => { e.preventDefault(); openUrl(r.url); }}>{r.url}</a></p>
              <div className="row"><Button size="sm" onClick={() => openUrl(r.url)}>{t("export.view_online")}</Button><Button size="sm" onClick={() => openUrl(r.repo_url)}>WACS</Button></div>
            </>
          )}
        </div>
      );
    });
  };

  const doLogin = async () => {
    setBusy("login");
    try {
      setProfile(await api.wacsLogin(user.trim(), pass));
      setLogin(false);
      fb.toast(t("common.done"), "success");
    } catch (e) {
      fb.error(e);
    } finally {
      setBusy(null);
    }
  };

  const saved = (path: string) => (
    <p>{t("export.saved_to")} <a href="#" onClick={(e) => { e.preventDefault(); openPath(path).catch(fb.error); }}>{path}</a></p>
  );

  return (
    <Dialog title={t("export.title")} onClose={onClose}>
      <div className="col">
        <div className="card list">
          <OptionItem icon={<UploadCloud size={18} />} title={t("export.upload")} description={`${t("export.upload_desc")} ${profile.session ? `· ${profile.session.user.username}` : `· ${t("export.need_login")}`}`} onClick={upload} busy={busy === "upload"} />
          {project.type_id === "text" && project.resource_id !== "obs" && (
            <OptionItem icon={<FileText size={18} />} tone="accent" title={t("export.usfm")} onClick={() => run("usfm", async () => saved(await api.exportUsfm(project.id)))} busy={busy === "usfm"} />
          )}
          {project.type_id === "text" && <OptionItem icon={<Printer size={18} />} tone="neutral" title={t("export.pdf")} onClick={() => setShowPrint(true)} />}
          <OptionItem icon={<Archive size={18} />} tone="success" title={t("export.tstudio")} onClick={() => run("tstudio", async () => saved(await api.exportTstudio(project.id)))} busy={busy === "tstudio"} />
        </div>
        {login && (
          <div className="card pad col">
            <h3>{t("profile.connect")}</h3>
            <Field label={t("profile.username")}><input className="input" value={user} onChange={(e) => setUser(e.target.value)} /></Field>
            <Field label={t("profile.password")}><input className="input" type="password" value={pass} onChange={(e) => setPass(e.target.value)} onKeyDown={(e) => e.key === "Enter" && doLogin()} /></Field>
            <div className="row" style={{ justifyContent: "flex-end" }}>
              <Button onClick={() => setLogin(false)}>{t("common.cancel")}</Button>
              <Button variant="primary" busy={busy === "login"} onClick={doLogin}>{t("common.continue")}</Button>
            </div>
          </div>
        )}
        {result && <div className="card pad">{result}</div>}
      </div>
      {showPrint && <PrintDialog id={project.id} isObs={project.resource_id === "obs"} onClose={() => setShowPrint(false)} />}
    </Dialog>
  );
}

const PRINT_FONTS = ["Arial", "Calibri", "Noto Sans", "Andika", "Noto Serif", "Georgia", "Times New Roman", "Cambria", "Charis SIL", "Gentium Plus"];
const PRINT_OPTS_KEY = "btt.printOptions.v2";

/** Opciones guardadas por el usuario sobre el formato predeterminado. */
function loadPrintOptions(): PrintRequest {
  try {
    const raw = localStorage.getItem(PRINT_OPTS_KEY);
    if (raw) return { ...DEFAULT_PRINT, ...(JSON.parse(raw) as Partial<PrintRequest>) };
  } catch {
    /* sin almacenamiento */
  }
  return { ...DEFAULT_PRINT };
}

export function PrintDialog({ id, isObs, onClose }: { id: string; isObs: boolean; onClose: () => void }) {
  const { t } = useT();
  const { settings } = useApp();
  const fb = useFeedback();
  const [opts, setOpts] = useState<PrintRequest>(loadPrintOptions);
  useEffect(() => {
    try {
      localStorage.setItem(PRINT_OPTS_KEY, JSON.stringify(opts));
    } catch {
      /* sin almacenamiento */
    }
  }, [opts]);
  const [html, setHtml] = useState<string | null>(null);
  const [busy, setBusy] = useState<"open" | "print" | null>(null);
  const frameRef = useRef<HTMLIFrameElement>(null);
  const debounced = useDebounced(opts, 250);
  // URL blob (mismo origen) para el iframe: funciona con la CSP de la app y permite print().
  const blobUrl = useMemo(() => (html === null ? null : URL.createObjectURL(new Blob([html], { type: "text/html" }))), [html]);
  useEffect(() => () => { if (blobUrl) URL.revokeObjectURL(blobUrl); }, [blobUrl]);

  useEffect(() => {
    let alive = true;
    api.renderPrintHtml(id, debounced).then((h) => alive && setHtml(h)).catch(fb.error);
    return () => {
      alive = false;
    };
  }, [id, debounced]);

  const set = (patch: Partial<PrintRequest>) => setOpts((o) => ({ ...o, ...patch }));

  const print = () => {
    const w = frameRef.current?.contentWindow;
    if (!w) return;
    setBusy("print");
    try {
      w.focus();
      w.print();
    } finally {
      setBusy(null);
    }
  };
  const openInBrowser = async () => {
    setBusy("open");
    try {
      await openPath(await api.exportPrintHtml(id, opts));
    } catch (e) {
      fb.error(e);
    } finally {
      setBusy(null);
    }
  };

  return (
    <Dialog
      title={t("print.title")}
      wide
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" busy={busy === "open"} onClick={openInBrowser}>{t("print.open_browser")}</Button>
          <span className="grow" />
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="primary" disabled={!html} busy={busy === "print"} onClick={print}><Printer size={16} /> {t("print.save_pdf")}</Button>
        </>
      }
    >
      <div className="print-layout">
        <div className="print-options col">
          <div className="print-group">
            <h4>{t("print.content")}</h4>
            <Switch on={opts.notes} onChange={(v) => set({ notes: v })} label={t("print.notes")} />
            <Switch on={opts.words} onChange={(v) => set({ words: v })} label={t("print.words")} />
            <Switch on={opts.include_incomplete} onChange={(v) => set({ include_incomplete: v })} label={t("print.incomplete")} />
            {isObs && <Switch on={opts.include_images} onChange={(v) => set({ include_images: v })} label={t("print.images")} />}
            <Switch on={opts.cover} onChange={(v) => set({ cover: v })} label={t("print.cover")} />
            <Switch on={opts.toc} onChange={(v) => set({ toc: v })} label={t("print.toc")} />
            <Switch on={opts.license} onChange={(v) => set({ license: v })} label={t("print.license")} />
            <Switch on={opts.page_numbers} onChange={(v) => set({ page_numbers: v })} label={t("print.page_numbers")} />
          </div>
          <div className="print-group">
            <h4>{t("print.format")}</h4>
            <Field label={t("print.font")}>
              <select className="select" value={opts.font_family} onChange={(e) => set({ font_family: e.target.value })}>
                {[...new Set([...PRINT_FONTS, settings.translation_font, opts.font_family].filter(Boolean))].map((f) => <option key={f}>{f}</option>)}
              </select>
            </Field>
            <div className="row">
              <Field label={t("print.font_size")}>
                <select className="select" value={opts.font_size_pt} onChange={(e) => set({ font_size_pt: Number(e.target.value) })}>
                  {[9, 10, 11, 12, 13, 14, 16].map((n) => <option key={n} value={n}>{n} pt</option>)}
                </select>
              </Field>
              <Field label={t("print.line_height")}>
                <select className="select" value={opts.line_height} onChange={(e) => set({ line_height: Number(e.target.value) })}>
                  {[1.15, 1.3, 1.5, 1.7, 2].map((n) => <option key={n} value={n}>{n}</option>)}
                </select>
              </Field>
            </div>
            <Field label={t("print.margins")}>
              <select className="select" value={opts.margin_cm} onChange={(e) => set({ margin_cm: Number(e.target.value) })}>
                {[1.5, 2, 2.5, 3].map((n) => <option key={n} value={n}>{n} cm</option>)}
              </select>
            </Field>
            <Field label={t("print.columns")}>
              <Tabs segmented value={String(opts.columns)} onChange={(v) => set({ columns: v === "2" ? 2 : 1 })} tabs={[{ id: "1", label: t("print.one_column") }, { id: "2", label: t("print.two_columns") }]} />
            </Field>
            <Field label={t("print.paragraphs")}>
              <Tabs segmented value={opts.paragraphs} onChange={(v) => set({ paragraphs: v })} tabs={[{ id: "spaced", label: t("print.para_spaced") }, { id: "indent", label: t("print.para_indent") }]} />
            </Field>
            <Field label={t("print.heading")}>
              <Tabs segmented value={opts.heading} onChange={(v) => set({ heading: v })} tabs={[{ id: "left", label: t("print.heading_left") }, { id: "centered", label: t("print.heading_centered") }]} />
            </Field>
            <Switch on={opts.verse_color} onChange={(v) => set({ verse_color: v })} label={t("print.verse_color")} />
          </div>
          <Button variant="ghost" onClick={() => setOpts({ ...DEFAULT_PRINT })}><RefreshCw size={14} /> {t("print.reset")}</Button>
          <p className="muted small" style={{ marginTop: ".4rem" }}>{t("print.hint")}</p>
        </div>
        <div className="print-preview">
          {blobUrl === null ? <Spinner label={t("print.rendering")} /> : <iframe ref={frameRef} title={t("print.preview")} src={blobUrl} />}
        </div>
      </div>
    </Dialog>
  );
}

// ---------------------------------------------------------------- importar

export function ImportDialog({ onClose, onImported }: { onClose: () => void; onImported: (outcomes: ImportOutcome[]) => void }) {
  const { t, lang } = useT();
  const { settings } = useApp();
  const fb = useFeedback();
  const [mode, setMode] = useState<"menu" | "server" | "usfm" | "backups">("menu");
  const [busy, setBusy] = useState(false);
  const [results, setResults] = useState<ImportOutcome[] | null>(null);
  const [existsFor, setExistsFor] = useState<null | ((m: ImportMode) => Promise<void>)>(null);

  const finish = (out: ImportOutcome[]) => {
    setResults(out);
    onImported(out);
  };

  const withExists = async (run: (m: ImportMode) => Promise<ImportOutcome[]>) => {
    setBusy(true);
    try {
      const out = await run("skip");
      if (out.some((o) => o.status.status === "exists")) {
        setExistsFor(() => async (m: ImportMode) => {
          setExistsFor(null);
          setBusy(true);
          try {
            finish(await run(m));
          } catch (e) {
            fb.error(e);
          } finally {
            setBusy(false);
          }
        });
      } else finish(out);
    } catch (e) {
      fb.error(e);
    } finally {
      setBusy(false);
    }
  };

  const importFile = async () => {
    const path = await openDialog({ multiple: false, filters: [{ name: "BTT Writer", extensions: ["tstudio", "zip"] }] });
    if (!path) return;
    await withExists((m) => api.importTstudio(path as string, m));
  };
  const importSource = async () => {
    const path = await openDialog({ multiple: false, filters: [{ name: "Resource container", extensions: ["tsrc"] }] });
    if (!path) return;
    setBusy(true);
    try {
      const slug = await api.importSourceContainer(path as string);
      fb.toast(`${t("common.downloaded")}: ${slug}`, "success");
    } catch (e) {
      fb.error(e);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog title={t("import.title")} onClose={onClose} wide={mode !== "menu"}>
      {busy && <Spinner />}
      {results ? (
        <div className="col">
          <h3>{t("import.result")}</h3>
          <div className="card list">
            {results.map((o) => (
              <div key={o.id} className="list-item"><strong className="grow">{o.id}</strong><span className={"badge " + (o.status.status === "failed" ? "danger" : o.status.status === "conflicts" ? "warn" : "ok")}>{importStatusText(t, o)}</span></div>
            ))}
          </div>
          <div className="row" style={{ justifyContent: "flex-end" }}><Button variant="primary" onClick={onClose}>{t("common.close")}</Button></div>
        </div>
      ) : mode === "menu" ? (
        <div className="card list">
          <OptionItem icon={<Cloud size={18} />} title={t("import.server")} description={t("import.server_desc")} onClick={() => setMode("server")} />
          <OptionItem icon={<FolderOpen size={18} />} tone="accent" title={t("import.file")} description=".tstudio" onClick={importFile} />
          <OptionItem icon={<FileUp size={18} />} tone="success" title={t("import.usfm")} description=".usfm · .sfm · .txt" onClick={() => setMode("usfm")} />
          <OptionItem icon={<Database size={18} />} tone="neutral" title={t("import.source")} description=".tsrc" onClick={importSource} />
          <OptionItem icon={<Archive size={18} />} tone="warning" title={t("import.backup")} onClick={() => setMode("backups")} />
        </div>
      ) : mode === "server" ? (
        <ServerImport glMode={settings.gateway_language_mode} onBack={() => setMode("menu")} run={(url) => withExists(async (m) => [await api.importFromWacs(url, m)])} />
      ) : mode === "usfm" ? (
        <UsfmImport uiLang={lang === "es" ? "es-419" : "en"} onBack={() => setMode("menu")} onDone={(o) => finish(o ? [o] : [])} />
      ) : (
        <BackupsList onBack={() => setMode("menu")} onPick={(path) => withExists((m) => api.importTstudio(path, m))} />
      )}
      {existsFor && (
        <Dialog title={t("import.exists")} narrow onClose={() => setExistsFor(null)} footer={<><Button onClick={() => setExistsFor(null)}>{t("common.cancel")}</Button><Button variant="danger" onClick={() => existsFor("overwrite")}>{t("import.overwrite")}</Button><Button variant="primary" onClick={() => existsFor("merge")}>{t("import.merge")}</Button></>}>
          <p>{t("import.exists_desc")}</p>
        </Dialog>
      )}
    </Dialog>
  );
}

function ServerImport({ glMode, onBack, run }: { glMode: boolean; onBack: () => void; run: (cloneUrl: string) => Promise<void> }) {
  const { t } = useT();
  const fb = useFeedback();
  const [user, setUser] = useState("");
  const [query, setQuery] = useState("");
  const [repos, setRepos] = useState<Repo[] | null>(null);
  const [busy, setBusy] = useState(false);
  const search = async () => {
    setBusy(true);
    try {
      setRepos(await api.searchWacs(user, query));
    } catch (e) {
      fb.error(e);
    } finally {
      setBusy(false);
    }
  };
  const supported = (r: Repo) => {
    const parts = r.name.split("_");
    if (parts.length < 3) return false;
    const type = parts[2];
    return type === "text" || (glMode && ["tn", "tq", "tw"].includes(type));
  };
  return (
    <div className="col">
      <div className="row wrap">
        <input className="input grow" placeholder={t("import.wacs_user")} value={user} onChange={(e) => setUser(e.target.value)} />
        <input className="input grow" placeholder={t("import.wacs_query")} value={query} onChange={(e) => setQuery(e.target.value)} onKeyDown={(e) => e.key === "Enter" && search()} />
        <Button variant="primary" busy={busy} onClick={search}>{t("common.search")}</Button>
      </div>
      {repos && (
        <div className="card list" style={{ maxHeight: 380, overflow: "auto" }}>
          {repos.map((r) => (
            <div key={r.id} className={"list-item" + (supported(r) ? "" : " disabled")} onClick={() => supported(r) && run(r.clone_url)}>
              <div className="grow"><strong>{r.name}</strong><div className="small muted">{r.full_name} {r.private ? "🔒" : ""}</div></div>
              {!supported(r) && <span className="badge">Unsupported</span>}
            </div>
          ))}
          {repos.length === 0 && <div className="list-item muted">{t("common.none")}</div>}
        </div>
      )}
      <div className="row"><Button onClick={onBack}>{t("common.back")}</Button></div>
    </div>
  );
}

function UsfmImport({ uiLang, onBack, onDone }: { uiLang: string; onBack: () => void; onDone: (o: ImportOutcome | null) => void }) {
  const { t } = useT();
  const fb = useFeedback();
  const [path, setPath] = useState<string>("");
  const [summary, setSummary] = useState<UsfmImportSummary | null>(null);
  const [langs, setLangs] = useState<TargetLanguageRow[]>([]);
  const [lq, setLq] = useState("");
  const [language, setLanguage] = useState<TargetLanguageRow | null>(null);
  const [books, setBooks] = useState<ProjectRow[]>([]);
  const [book, setBook] = useState("");
  const [busy, setBusy] = useState(false);
  const dq = useDebounced(lq, 200);
  useEffect(() => {
    api.searchLanguages(dq, 40).then(setLangs).catch(() => {});
  }, [dq]);
  useEffect(() => {
    api.listBooks(uiLang).then(setBooks).catch(() => {});
  }, []);

  const pick = async () => {
    const p = await openDialog({ multiple: false, filters: [{ name: "USFM", extensions: ["usfm", "txt", "sfm"] }] });
    if (!p) return;
    setPath(p as string);
    setBusy(true);
    try {
      const s = await api.importUsfm(p as string, "", "", "skip");
      setSummary(s);
      setBook(s.book_id);
    } catch (e) {
      fb.error(e);
    } finally {
      setBusy(false);
    }
  };
  const go = async (mode: ImportMode) => {
    if (!language || !book) return;
    setBusy(true);
    try {
      const s = await api.importUsfm(path, language.slug, book, mode);
      if (s.outcome?.status.status === "exists") {
        const ok = await fb.confirm(t("import.exists"), t("import.exists_desc"), { okLabel: t("import.merge") });
        if (ok) return go("merge");
        return;
      }
      onDone(s.outcome);
    } catch (e) {
      fb.error(e);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="col">
      <div className="row"><Button onClick={pick}>{t("import.usfm")}</Button><span className="muted small ellipsis grow">{path}</span></div>
      {busy && <Spinner />}
      {summary && (
        <div className="card pad col">
          <h4>{t("import.usfm_summary")}</h4>
          <div className="small">{summary.title || "—"} · {summary.chapters} cap. · {summary.verses} v.</div>
          {summary.warnings.map((w, i) => <div key={i} className="small muted">⚠ {w}</div>)}
          <Field label={t("import.usfm_book")}>
            <select className="select" value={book} onChange={(e) => setBook(e.target.value)}>
              <option value="">{t("import.usfm_missing_book")}</option>
              {books.map((b) => <option key={b.slug} value={b.slug}>{b.name} ({b.slug})</option>)}
            </select>
          </Field>
          <Field label={t("import.usfm_language")}>
            <input className="input" placeholder={t("new.search_language")} value={language ? `${language.name} (${language.slug})` : lq} onChange={(e) => { setLanguage(null); setLq(e.target.value); }} />
          </Field>
          {!language && lq && (
            <div className="card list" style={{ maxHeight: 200, overflow: "auto" }}>
              {langs.map((l) => <div key={l.slug} className="list-item" onClick={() => setLanguage(l)}>{l.name} <span className="muted small">({l.slug})</span></div>)}
            </div>
          )}
          <div className="row" style={{ justifyContent: "flex-end" }}>
            <Button variant="primary" disabled={!language || !book} busy={busy} onClick={() => go("skip")}>{t("import.title")}</Button>
          </div>
        </div>
      )}
      <div className="row"><Button onClick={onBack}>{t("common.back")}</Button></div>
    </div>
  );
}

function BackupsList({ onBack, onPick }: { onBack: () => void; onPick: (path: string) => void }) {
  const { t, lang } = useT();
  const [list, setList] = useState<{ name: string; path: string; modified: number }[] | null>(null);
  useEffect(() => {
    api.listBackups().then(setList).catch(() => setList([]));
  }, []);
  return (
    <div className="col">
      {!list ? <Spinner /> : (
        <div className="card list" style={{ maxHeight: 400, overflow: "auto" }}>
          {list.map((b) => <div key={b.path} className="list-item" onClick={() => onPick(b.path)}><strong className="grow">{b.name}</strong><span className="muted small">{formatDate(b.modified, lang, "")}</span></div>)}
          {list.length === 0 && <div className="list-item muted">{t("common.none")}</div>}
        </div>
      )}
      <div className="row"><Button onClick={onBack}>{t("common.back")}</Button></div>
    </div>
  );
}

// ---------------------------------------------------------------- biblioteca

export function UpdateLibraryDialog({ onClose }: { onClose: () => void }) {
  const { t } = useT();
  const fb = useFeedback();
  const [busy, setBusy] = useState<string | null>(null);
  const [msg, setMsg] = useState<string>("");
  const [downloads, setDownloads] = useState(false);
  const run = async (key: string, fn: () => Promise<string>) => {
    setBusy(key);
    setMsg("");
    try {
      setMsg(await fn());
    } catch (e) {
      const s = String(e);
      fb.error(s.includes("403") || s.includes("anti-bot") ? t("library.cdn_blocked") : e);
    } finally {
      setBusy(null);
    }
  };
  const importIndex = async () => {
    const p = await openDialog({ multiple: false, filters: [{ name: "SQLite", extensions: ["sqlite", "db"] }] });
    if (!p) return;
    await run("index_file", async () => { await api.importIndexFile(p as string); return t("common.done"); });
  };
  return (
    <Dialog title={t("library.title")} onClose={onClose}>
      <div className="card list">
        <OptionItem icon={<RefreshCw size={18} />} title={t("library.sources")} onClick={() => run("sources", async () => t("library.result_sources", { n: (await api.updateCatalog()).length }))} busy={busy === "sources"} />
        <OptionItem icon={<HardDriveDownload size={18} />} tone="success" title={t("library.download_sources")} onClick={() => setDownloads(true)} />
        <OptionItem icon={<Languages size={18} />} tone="accent" title={t("library.languages")} onClick={() => run("langs", async () => t("library.result_languages", { n: await api.updateLanguages() }))} busy={busy === "langs"} />
        <OptionItem icon={<Server size={18} />} tone="neutral" title={t("library.index_download")} onClick={() => run("index", async () => { await api.downloadIndex(); return t("common.done"); })} busy={busy === "index"} />
        <OptionItem icon={<Database size={18} />} tone="neutral" title={t("library.index_file")} onClick={importIndex} busy={busy === "index_file"} />
      </div>
      {msg && <p style={{ marginTop: "1rem" }} className="badge ok"><Check size={13} /> {msg}</p>}
      {downloads && <DownloadSourcesDialog onClose={() => setDownloads(false)} />}
    </Dialog>
  );
}

function DownloadSourcesDialog({ onClose }: { onClose: () => void }) {
  const { t, lang } = useT();
  const fb = useFeedback();
  const [rows, setRows] = useState<ResourceRow[] | null>(null);
  const [byBook, setByBook] = useState(false);
  const [filter, setFilter] = useState("");
  const [group, setGroup] = useState<string>("");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState("");
  const [books, setBooks] = useState<ProjectRow[]>([]);
  useEffect(() => {
    api.allResources("").then((r) => setRows(r.filter((x) => x.type === "book" && Number(x.checking_level) >= 3))).catch(fb.error);
    api.listBooks(lang === "es" ? "es-419" : "en").then(setBooks).catch(() => {});
  }, []);
  const key = (r: ResourceRow) => `${r.language_slug}|${r.project_slug}|${r.slug}`;
  const groups = useMemo(() => {
    const g = new Map<string, string>();
    for (const r of rows ?? []) {
      if (byBook) g.set(r.project_slug, books.find((b) => b.slug === r.project_slug)?.name || r.project_name || r.project_slug);
      else g.set(r.language_slug, `${r.language_name} (${r.language_slug})`);
    }
    return [...g.entries()].filter(([k, v]) => !filter || v.toLowerCase().includes(filter.toLowerCase()) || k.includes(filter.toLowerCase())).sort((a, b) => a[1].localeCompare(b[1]));
  }, [rows, byBook, filter, books]);
  const items = (rows ?? []).filter((r) => (byBook ? r.project_slug : r.language_slug) === group);
  const toggle = (k: string) => setSelected((s) => { const n = new Set(s); n.has(k) ? n.delete(k) : n.add(k); return n; });
  const download = async () => {
    setBusy(true);
    let done = 0;
    for (const k of selected) {
      const [l, p, r] = k.split("|");
      setProgress(`${++done}/${selected.size} ${k}`);
      try {
        await api.downloadSource(l, p, r);
      } catch (e) {
        fb.error(String(e).includes("403") || String(e).includes("anti-bot") ? t("library.cdn_blocked") : e);
        break;
      }
    }
    setBusy(false);
    setSelected(new Set());
    api.allResources("").then((r) => setRows(r.filter((x) => x.type === "book" && Number(x.checking_level) >= 3))).catch(() => {});
  };
  return (
    <Dialog title={t("library.download_sources")} wide onClose={onClose} footer={<><span className="muted small grow">{progress}</span><Button onClick={onClose}>{t("common.close")}</Button><Button variant="primary" busy={busy} disabled={selected.size === 0} onClick={download}>{t("library.download_selected")} ({selected.size})</Button></>}>
      {!rows ? <Spinner /> : (
        <div className="row" style={{ alignItems: "stretch", gap: "1rem", minHeight: 360 }}>
          <div className="col" style={{ flex: 1 }}>
            <Tabs value={byBook ? "book" : "lang"} onChange={(v) => { setByBook(v === "book"); setGroup(""); }} tabs={[{ id: "lang", label: t("library.by_language") }, { id: "book", label: t("library.by_book") }]} />
            <input className="input" placeholder={t("common.search")} value={filter} onChange={(e) => setFilter(e.target.value)} />
            <div className="card list" style={{ maxHeight: 360, overflow: "auto" }}>
              {groups.map(([k, label]) => <div key={k} className={"list-item" + (k === group ? " selected" : "")} onClick={() => setGroup(k)}>{label}</div>)}
            </div>
          </div>
          <div className="col" style={{ flex: 1 }}>
            <div className="row"><Button size="sm" onClick={() => setSelected(new Set([...selected, ...items.filter((r) => !r.downloaded).map(key)]))}>{t("library.select_all")}</Button></div>
            <div className="card list" style={{ maxHeight: 400, overflow: "auto" }}>
              {items.map((r) => (
                <label key={key(r)} className="list-item">
                  <input type="checkbox" disabled={r.downloaded} checked={r.downloaded || selected.has(key(r))} onChange={() => toggle(key(r))} />
                  <span className="grow">{byBook ? `${r.language_name} (${r.language_slug})` : books.find((b) => b.slug === r.project_slug)?.name || r.project_name} - {r.name}</span>
                  {r.downloaded && <span className="badge ok">{t("common.downloaded")}</span>}
                </label>
              ))}
              {items.length === 0 && <div className="list-item muted">{t("common.select")}</div>}
            </div>
          </div>
        </div>
      )}
    </Dialog>
  );
}

// ---------------------------------------------------------------- comentarios, respaldos, migración

export function FeedbackDialog({ onClose }: { onClose: () => void }) {
  const { t } = useT();
  const fb = useFeedback();
  const [email, setEmail] = useState("");
  const [msg, setMsg] = useState("");
  const send = async () => {
    try {
      const path = await api.sendFeedback(email, msg);
      fb.toast(t("feedback.saved", { path }), "success");
      onClose();
    } catch (e) {
      fb.error(e);
    }
  };
  return (
    <Dialog title={t("feedback.title")} onClose={onClose} footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" disabled={!msg.trim()} onClick={send}>{t("common.save")}</Button></>}>
      <div className="col">
        <Field label={t("feedback.email")}><input className="input" value={email} onChange={(e) => setEmail(e.target.value)} /></Field>
        <Field label={t("feedback.message")}><textarea className="textarea" value={msg} onChange={(e) => setMsg(e.target.value)} /></Field>
      </div>
    </Dialog>
  );
}

export function BackupsDialog({ onClose, onImported }: { onClose: () => void; onImported: () => void }) {
  const { t, lang } = useT();
  const fb = useFeedback();
  const [list, setList] = useState<{ name: string; path: string; modified: number }[]>([]);
  const [busy, setBusy] = useState(false);
  const load = () => api.listBackups().then(setList).catch(fb.error);
  useEffect(() => {
    load();
  }, []);
  const runNow = async () => {
    setBusy(true);
    try {
      const r = await api.runBackupNow();
      fb.toast(t("backups.result", { n: r.backed_up.length }), "success");
      load();
    } catch (e) {
      fb.error(e);
    } finally {
      setBusy(false);
    }
  };
  const imp = async (path: string) => {
    try {
      const out = await api.importTstudio(path, "merge");
      fb.toast(out.map((o) => `${o.id}: ${importStatusText(t, o)}`).join("\n"), out.some((o) => o.status.status === "failed") ? "error" : "success");
      onImported();
    } catch (e) {
      fb.error(e);
    }
  };
  return (
    <Dialog title={t("backups.title")} onClose={onClose} footer={<><Button busy={busy} onClick={runNow}>{t("backups.run_now")}</Button><Button variant="primary" onClick={onClose}>{t("common.close")}</Button></>}>
      <div className="card list" style={{ maxHeight: 420, overflow: "auto" }}>
        {list.map((b) => <div key={b.path} className="list-item"><strong className="grow ellipsis">{b.name}</strong><span className="muted small">{formatDate(b.modified, lang, "")}</span><Button size="sm" onClick={() => imp(b.path)}>{t("backups.import")}</Button></div>)}
        {list.length === 0 && <div className="list-item muted">{t("common.none")}</div>}
      </div>
    </Dialog>
  );
}

export function MigrateDialog({ roots, onClose, onDone }: { roots: string[]; onClose: () => void; onDone: () => void }) {
  const { t } = useT();
  const fb = useFeedback();
  const [busy, setBusy] = useState<string | null>(null);
  const [counts, setCounts] = useState<Record<string, number>>({});
  const [progress, setProgress] = useState<{ done: number; total: number; current: string } | null>(null);
  const [results, setResults] = useState<ImportOutcome[] | null>(null);

  useEffect(() => {
    roots.forEach((r) => api.inspectLegacy(r).then((i) => setCounts((c) => ({ ...c, [r]: i.projects }))).catch(() => {}));
  }, [roots]);
  useEffect(() => {
    if (!isTauri) return;
    const un = listen<{ done: number; total: number; current: string }>("migrate-progress", (e) => setProgress(e.payload));
    return () => {
      un.then((f) => f()).catch(() => {});
    };
  }, []);

  const rootLabel = (r: string) => (/BTT-Writer-2\.0/i.test(r) ? t("migrate.root_20") : /translationStudio/i.test(r) ? t("migrate.root_ts") : t("migrate.root_16"));

  const go = async (root: string) => {
    setBusy(root);
    setProgress(null);
    try {
      const out = await api.migrateLegacy(root);
      setResults(out);
      fb.toast(t("migrate.result", { n: out.filter((o) => o.status.status !== "failed").length }), "success");
      onDone();
    } catch (e) {
      fb.error(e);
    } finally {
      setBusy(null);
      setProgress(null);
    }
  };

  return (
    <Dialog title={results ? t("migrate.result_title") : t("migrate.title")} onClose={busy ? undefined : onClose} footer={results ? <Button variant="primary" onClick={onClose}>{t("common.close")}</Button> : undefined}>
      {results ? (
        <div className="col">
          {results.length === 0 && <p className="muted">{t("migrate.none_found")}</p>}
          <div className="card list" style={{ maxHeight: 420, overflow: "auto" }}>
            {results.map((o) => (
              <div key={o.id} className="list-item"><strong className="grow ellipsis">{o.id}</strong><span className={"badge " + (o.status.status === "failed" ? "danger" : o.status.status === "conflicts" ? "warn" : "ok")}>{importStatusText(t, o)}</span></div>
            ))}
          </div>
        </div>
      ) : (
        <div className="col">
          <p>{t("migrate.desc")}</p>
          <div className="card list">
            {roots.map((r) => (
              <div key={r} className="list-item" style={{ cursor: "default" }}>
                <IconBadge tone="accent"><Download size={18} /></IconBadge>
                <div className="grow" style={{ minWidth: 0 }}>
                  <strong>{rootLabel(r)}</strong> {counts[r] !== undefined && <span className="badge">{t("migrate.projects_found", { n: counts[r] })}</span>}
                  <div className="small muted ellipsis">{r}</div>
                </div>
                <Button size="sm" variant="primary" busy={busy === r} disabled={!!busy || counts[r] === 0} onClick={() => go(r)}>{t("migrate.start")}</Button>
              </div>
            ))}
          </div>
          {busy && progress && progress.total > 0 && (
            <div className="col" style={{ gap: ".3rem" }}>
              <div className="progress-bar"><div style={{ width: `${Math.round((progress.done / progress.total) * 100)}%` }} /></div>
              <span className="small muted">{t("migrate.running", { current: progress.current, done: progress.done, total: progress.total })}</span>
            </div>
          )}
        </div>
      )}
    </Dialog>
  );
}
