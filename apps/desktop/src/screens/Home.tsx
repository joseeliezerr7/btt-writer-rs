// Inicio: lista de proyectos agrupada por idioma, ordenación, menú de opciones y diálogos.
import { useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { AlertTriangle, Archive, BookOpen, CheckCircle2, ChevronRight, Download, FolderClosed, FolderInput, FolderOpen, Info, Layers, LogOut, MessageSquare, MoreVertical, Plus, RefreshCw, Settings as SettingsIcon } from "lucide-react";
import { api, isTauri, resourceLabel, type ProjectItem, type ProjectRow } from "../api";
import { useApp } from "../app-context";
import { formatDate, useT } from "../i18n";
import { Avatar, Button, EmptyState, Logo, Monogram, Spinner, Stat, Switch, useFeedback } from "../components/ui";
import { BackupsDialog, bookName, FeedbackDialog, ImportDialog, MigrateDialog, NewProjectDialog, ProjectDetailsDialog, UpdateLibraryDialog } from "../components/dialogs";

const BIBLE_ORDER = "gen exo lev num deu jos jdg rut 1sa 2sa 1ki 2ki 1ch 2ch ezr neh est job psa pro ecc sng isa jer lam ezk dan hos jol amo oba jon mic nam hab zep hag zec mal mat mrk luk jhn act rom 1co 2co gal eph php col 1th 2th 1ti 2ti tit phm heb jas 1pe 2pe 1jn 2jn 3jn jud rev".split(" ");

type DialogKind = null | "new" | "import" | "library" | "feedback" | "backups" | "migrate";

const LS_GROUP = "btt.groupByLanguage";
const LS_COLLAPSED = "btt.collapsedLanguages";

function readLs<T>(key: string, fallback: T): T {
  try {
    const v = localStorage.getItem(key);
    return v === null ? fallback : (JSON.parse(v) as T);
  } catch {
    return fallback;
  }
}
function writeLs(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* sin almacenamiento */
  }
}

export default function HomeScreen() {
  const { t, lang } = useT();
  const { profile, settings, updateSettings, reloadSettings, navigate, info, setProfile } = useApp();
  const fb = useFeedback();
  const [projects, setProjects] = useState<ProjectItem[] | null>(null);
  const [books, setBooks] = useState<ProjectRow[]>([]);
  const [dialog, setDialog] = useState<DialogKind>(null);
  const [details, setDetails] = useState<ProjectItem | null>(null);
  const [menu, setMenu] = useState(false);
  const [grouped, setGrouped] = useState<boolean>(() => readLs(LS_GROUP, true));
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>(() => readLs(LS_COLLAPSED, {}));
  const menuRef = useRef<HTMLDivElement>(null);

  const load = async () => {
    try {
      setProjects(await api.listProjects());
    } catch (e) {
      fb.error(e);
      setProjects([]);
    }
  };
  useEffect(() => {
    load();
    api.listBooks(lang === "es" ? "es-419" : "en").then(setBooks).catch(() => {});
    if (!isTauri) return;
    const un = listen<string[]>("backup-done", () => load());
    const unFailed = listen<[string, string][]>("backup-failed", (ev) => {
      const detail = ev.payload.map(([pid, e]) => `${pid}: ${e}`).join(" · ");
      fb.toast(`${t("home.backup_failed")} ${detail}`, "error");
    });
    return () => {
      un.then((f) => f()).catch(() => {});
      unFailed.then((f) => f()).catch(() => {});
    };
  }, [lang]);
  useEffect(() => {
    const close = (e: MouseEvent) => {
      if (!menuRef.current?.contains(e.target as Node)) setMenu(false);
    };
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, []);

  const sorted = useMemo(() => {
    const list = [...(projects ?? [])];
    const bookKey = (p: ProjectItem) => (settings.book_order === "bible" ? String(BIBLE_ORDER.indexOf(p.project_id) === -1 ? 99 : BIBLE_ORDER.indexOf(p.project_id)).padStart(3, "0") : bookName(p, books).toLowerCase());
    list.sort((a, b) => {
      if (settings.sort_by === "language") return a.language_name.localeCompare(b.language_name) || bookKey(a).localeCompare(bookKey(b));
      if (settings.sort_by === "progress") return b.progress - a.progress || bookKey(a).localeCompare(bookKey(b));
      return bookKey(a).localeCompare(bookKey(b)) || a.language_name.localeCompare(b.language_name);
    });
    return list;
  }, [projects, settings.sort_by, settings.book_order, books]);

  /** Carpetas por idioma destino, ordenadas por nombre; dentro, el orden elegido. */
  const groups = useMemo(() => {
    const map = new Map<string, { id: string; name: string; items: ProjectItem[] }>();
    for (const p of sorted) {
      const g = map.get(p.language_id) ?? { id: p.language_id, name: p.language_name, items: [] };
      g.items.push(p);
      map.set(p.language_id, g);
    }
    return [...map.values()].sort((a, b) => a.name.localeCompare(b.name));
  }, [sorted]);

  const stats = useMemo(() => {
    const all = projects ?? [];
    return {
      total: all.length,
      finished: all.filter((p) => p.progress >= 1).length,
      inProgress: all.filter((p) => p.progress > 0 && p.progress < 1).length,
      conflicts: all.filter((p) => p.has_conflicts).length,
      languages: new Set(all.map((p) => p.language_id)).size,
    };
  }, [projects]);

  const toggleGroup = (id: string) => {
    const next = { ...collapsed, [id]: !collapsed[id] };
    setCollapsed(next);
    writeLs(LS_COLLAPSED, next);
  };
  const setGroupedPersist = (v: boolean) => {
    setGrouped(v);
    writeLs(LS_GROUP, v);
  };

  const logout = async () => {
    await api.logout();
    setProfile(await api.getProfile());
    navigate({ name: "profile" });
  };

  const openProject = (p: ProjectItem) => {
    updateSettings({ last_project: p.id }).catch(() => {});
    navigate({ name: "translate", id: p.id });
  };

  const legacyRoots = (info?.legacy_roots ?? []).filter((r) => !settings.migrated_from.includes(r));
  const userName = profile.full_name || profile.session?.user.username || "";

  const row = (p: ProjectItem, i: number) => {
    const pct = Math.round(p.progress * 100);
    const idx = BIBLE_ORDER.indexOf(p.project_id);
    const hueKey = p.type_id !== "text" ? "helps-" + p.type_id : p.resource_id === "obs" ? "obs" : idx >= 0 && idx < 39 ? "ot" : "nt";
    return (
      <div key={p.id} className="project-row" onClick={() => openProject(p)} style={{ animationDelay: `${Math.min(i, 12) * 25}ms` }}>
        <Monogram text={p.project_id} hueKey={hueKey} />
        <div style={{ minWidth: 0 }}>
          <div className="name ellipsis">{bookName(p, books)}</div>
          <div className="meta ellipsis">{p.id} · {formatDate(p.modified_at, lang, "")}</div>
        </div>
        <div className="cell-type"><span className="badge">{resourceLabel(p, t)}</span></div>
        <div className="cell-lang lang ellipsis">{p.language_name}<span className="code">{p.language_id}</span></div>
        <div className="row" style={{ gap: ".6rem" }}>
          <div className="progress-inline">
            <div className="progress-bar thin"><div style={{ width: `${pct}%` }} /></div>
            <span className="pct" style={{ color: pct >= 100 ? "var(--success)" : undefined }}>{pct}%</span>
          </div>
          {p.has_conflicts && <span title={t("home.conflicts")} className="badge danger"><AlertTriangle size={12} /></span>}
        </div>
        <div onClick={(e) => e.stopPropagation()}>
          <Button variant="ghost" icon title={t("home.info")} onClick={() => setDetails(p)}><Info size={18} /></Button>
        </div>
      </div>
    );
  };

  const listHead = (
    <div className="project-list-head">
      <span />
      <span>{t("home.col.project")}</span>
      <span>{t("home.col.type")}</span>
      <span>{t("home.col.language")}</span>
      <span>{t("home.col.progress")}</span>
      <span />
    </div>
  );

  return (
    <div className="screen">
      <div className="topbar">
        <div className="brand">
          <Logo size={30} />
          <div>
            <div className="name">{t("app.name")}</div>
            <div className="sub">{t("home.brand_sub")}</div>
          </div>
        </div>
        <span className="grow" />
        <span className="chip" title={t("profile.current_user")}>
          <Avatar name={userName} size={24} />
          <span className="ellipsis" style={{ maxWidth: 180 }}>{userName}</span>
        </span>
        <Button variant="primary" onClick={() => setDialog("new")}><Plus size={16} /> {t("home.new")}</Button>
        <div style={{ position: "relative" }} ref={menuRef}>
          <Button variant="ghost" icon onClick={() => setMenu((m) => !m)} title={t("home.more")}><MoreVertical size={18} /></Button>
          {menu && (
            <div className="menu" onClick={() => setMenu(false)}>
              <button onClick={() => setDialog("library")}><RefreshCw size={16} />{t("home.update")}</button>
              <button onClick={() => setDialog("import")}><FolderInput size={16} />{t("home.import")}</button>
              <button onClick={() => setDialog("backups")}><Archive size={16} />{t("home.backups")}</button>
              {legacyRoots.length > 0 && <button onClick={() => setDialog("migrate")}><Download size={16} />{t("home.migrate")}</button>}
              <button onClick={() => setDialog("feedback")}><MessageSquare size={16} />{t("home.feedback")}</button>
              <hr />
              <button onClick={() => navigate({ name: "settings" })}><SettingsIcon size={16} />{t("home.settings")}</button>
              <button onClick={logout}><LogOut size={16} />{t("profile.logout")}</button>
            </div>
          )}
        </div>
      </div>

      <div className="content">
        <div className="container">
          {projects === null ? (
            <Spinner label={t("app.loading")} />
          ) : projects.length === 0 ? (
            <div className="card elevated" style={{ marginTop: "2rem" }}>
              <EmptyState icon={<BookOpen size={28} />} title={t("home.empty")} description={t("home.empty_desc")}>
                <Button variant="primary" onClick={() => setDialog("new")}><Plus size={16} /> {t("home.new")}</Button>
                <Button onClick={() => setDialog("import")}><FolderInput size={16} /> {t("home.import")}</Button>
                {legacyRoots.length > 0 && <Button onClick={() => setDialog("migrate")}><Download size={16} /> {t("home.migrate")}</Button>}
              </EmptyState>
            </div>
          ) : (
            <>
              <div className="page-head">
                <div>
                  <h1>{t("home.title")}</h1>
                  <p>{t("home.subtitle", { n: stats.total })}</p>
                </div>
              </div>

              <div className="stats" style={{ marginBottom: "1.2rem" }}>
                <Stat icon={<Layers size={20} />} value={stats.total} label={t("home.stats.projects")} />
                <Stat icon={<BookOpen size={20} />} value={stats.inProgress} label={t("home.stats.in_progress")} tone="accent" />
                <Stat icon={<CheckCircle2 size={20} />} value={stats.finished} label={t("home.stats.finished")} tone="success" />
                {stats.conflicts > 0 && <Stat icon={<AlertTriangle size={20} />} value={stats.conflicts} label={t("home.stats.conflicts")} tone="danger" />}
              </div>

              <div className="toolbar">
                <span className="small muted">{t("home.sort")}</span>
                <select className="select" value={settings.sort_by} onChange={(e) => updateSettings({ sort_by: e.target.value }).catch(fb.error)}>
                  <option value="project">{t("home.sort.project")}</option>
                  <option value="language">{t("home.sort.language")}</option>
                  <option value="progress">{t("home.sort.progress")}</option>
                </select>
                <select className="select" value={settings.book_order} onChange={(e) => updateSettings({ book_order: e.target.value }).catch(fb.error)}>
                  <option value="bible">{t("home.book_order")}</option>
                  <option value="alpha">{t("home.alpha_order")}</option>
                </select>
                <span className="grow" />
                <Switch on={grouped} onChange={setGroupedPersist} label={<span className="small">{t("home.group_by_language")}</span>} />
              </div>

              {grouped ? (
                groups.map((g) => {
                  const open = !collapsed[g.id];
                  const avg = g.items.reduce((s, p) => s + p.progress, 0) / g.items.length;
                  const pct = Math.round(avg * 100);
                  const conflicts = g.items.filter((p) => p.has_conflicts).length;
                  return (
                    <div key={g.id} className={"card lang-group" + (open ? " open" : "")}>
                      <button type="button" className="lang-group-head" onClick={() => toggleGroup(g.id)} aria-expanded={open}>
                        <ChevronRight size={18} className="chev" />
                        {open ? <FolderOpen size={20} style={{ color: "var(--accent)" }} /> : <FolderClosed size={20} style={{ color: "var(--accent)" }} />}
                        <span className="ellipsis"><span className="lname">{g.name}</span><span className="lcode">{g.id}</span></span>
                        <span className="lmeta">
                          {conflicts > 0 && <span className="badge danger"><AlertTriangle size={12} /> {conflicts}</span>}
                          <span className="badge">{g.items.length === 1 ? t("home.project_count_one") : t("home.projects_count", { n: g.items.length })}</span>
                          <span className="progress-inline">
                            <span className="progress-bar thin" style={{ display: "block" }}><div style={{ width: `${pct}%` }} /></span>
                            <span className="pct" style={{ color: pct >= 100 ? "var(--success)" : undefined }}>{pct}%</span>
                          </span>
                        </span>
                      </button>
                      {open && <div className="project-list">{g.items.map(row)}</div>}
                    </div>
                  );
                })
              ) : (
                <div className="card">
                  {listHead}
                  <div className="project-list">{sorted.map(row)}</div>
                </div>
              )}
            </>
          )}
        </div>
      </div>

      {dialog === "new" && <NewProjectDialog onClose={() => setDialog(null)} onCreated={(id) => { setDialog(null); load(); navigate({ name: "translate", id }); }} />}
      {dialog === "import" && <ImportDialog onClose={() => setDialog(null)} onImported={() => load()} />}
      {dialog === "library" && <UpdateLibraryDialog onClose={() => setDialog(null)} />}
      {dialog === "feedback" && <FeedbackDialog onClose={() => setDialog(null)} />}
      {dialog === "backups" && <BackupsDialog onClose={() => setDialog(null)} onImported={load} />}
      {dialog === "migrate" && <MigrateDialog roots={legacyRoots} onClose={() => setDialog(null)} onDone={() => { load(); reloadSettings().catch(() => {}); }} />}
      {details && <ProjectDetailsDialog item={details} onClose={() => setDetails(null)} onChanged={load} onDeleted={() => { setDetails(null); load(); }} />}
    </div>
  );
}
