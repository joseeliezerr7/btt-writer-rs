// Inicio: lista de proyectos agrupada por idioma, búsqueda, selección múltiple,
// menú de cuenta, ordenación, menú de opciones y diálogos.
import { useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { AlertTriangle, Archive, BookOpen, CheckCircle2, CheckSquare, ChevronDown, ChevronRight, Cloud, CloudOff, Download, ExternalLink, FolderClosed, FolderInput, FolderOpen, Info, Layers, LogOut, MessageSquare, MoreVertical, Pencil, Plus, RefreshCw, Search, Settings as SettingsIcon, Square, Trash2, UserRound, X } from "lucide-react";
import { api, isTauri, resourceLabel, type ProjectItem, type ProjectRow } from "../api";
import { useApp } from "../app-context";
import { formatDate, useT } from "../i18n";
import { Avatar, Button, Dialog, EmptyState, Field, Logo, Monogram, Spinner, Stat, Switch, useFeedback } from "../components/ui";
import { BackupsDialog, bookName, FeedbackDialog, ImportDialog, MigrateDialog, NewProjectDialog, ProjectDetailsDialog, ServerLoginDialog, UpdateLibraryDialog } from "../components/dialogs";

const BIBLE_ORDER = "gen exo lev num deu jos jdg rut 1sa 2sa 1ki 2ki 1ch 2ch ezr neh est job psa pro ecc sng isa jer lam ezk dan hos jol amo oba jon mic nam hab zep hag zec mal mat mrk luk jhn act rom 1co 2co gal eph php col 1th 2th 1ti 2ti tit phm heb jas 1pe 2pe 1jn 2jn 3jn jud rev".split(" ");

type DialogKind = null | "new" | "import" | "library" | "feedback" | "backups" | "migrate" | "login" | "rename";

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
  const [userMenu, setUserMenu] = useState(false);
  const [grouped, setGrouped] = useState<boolean>(() => readLs(LS_GROUP, true));
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>(() => readLs(LS_COLLAPSED, {}));
  const [query, setQuery] = useState("");
  const [selecting, setSelecting] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [deleting, setDeleting] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const userMenuRef = useRef<HTMLDivElement>(null);

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
      if (!userMenuRef.current?.contains(e.target as Node)) setUserMenu(false);
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

  /** Búsqueda por nombre del libro, código, idioma o identificador del proyecto. */
  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return sorted;
    const terms = q.split(/\s+/);
    return sorted.filter((p) => {
      const hay = `${bookName(p, books)} ${p.project_id} ${p.id} ${p.language_name} ${p.language_id} ${resourceLabel(p, t)}`.toLowerCase();
      return terms.every((term) => hay.includes(term));
    });
  }, [sorted, query, books]);

  /** Carpetas por idioma destino, ordenadas por nombre; dentro, el orden elegido. */
  const groups = useMemo(() => {
    const map = new Map<string, { id: string; name: string; items: ProjectItem[] }>();
    for (const p of filtered) {
      const g = map.get(p.language_id) ?? { id: p.language_id, name: p.language_name, items: [] };
      g.items.push(p);
      map.set(p.language_id, g);
    }
    return [...map.values()].sort((a, b) => a.name.localeCompare(b.name));
  }, [filtered]);

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
  /** Cierra la sesión del servidor pero conserva la cuenta local (nombre y términos aceptados). */
  const disconnect = async () => {
    try {
      await api.logout();
      setProfile(await api.saveProfile({ ...profile, session: null }));
      fb.toast(t("profile.disconnected"), "success");
    } catch (e) {
      fb.error(e);
    }
  };

  const openProject = (p: ProjectItem) => {
    updateSettings({ last_project: p.id }).catch(() => {});
    navigate({ name: "translate", id: p.id });
  };

  // ---- selección múltiple
  const toggleSelected = (id: string) =>
    setSelected((s) => {
      const n = new Set(s);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });
  const stopSelecting = () => {
    setSelecting(false);
    setSelected(new Set());
  };
  const selectAllVisible = () => setSelected(new Set(filtered.map((p) => p.id)));
  const deleteSelected = async () => {
    const items = (projects ?? []).filter((p) => selected.has(p.id));
    if (items.length === 0) return;
    const ok = await fb.confirm(
      items.length === 1 ? t("home.delete_one_title") : t("home.delete_many_title", { n: items.length }),
      <div className="col" style={{ gap: ".5rem" }}>
        <p style={{ margin: 0 }}>{t("home.delete_many_body")}</p>
        <ul className="delete-list">
          {items.map((p) => (
            <li key={p.id}><strong>{bookName(p, books)}</strong> · {p.language_name} <span className="muted small">({p.id})</span></li>
          ))}
        </ul>
      </div>,
      { danger: true, okLabel: t("common.delete") },
    );
    if (!ok) return;
    setDeleting(true);
    let done = 0;
    for (const p of items) {
      try {
        await api.deleteProject(p.id);
        done++;
      } catch (e) {
        fb.toast(t("home.delete_failed", { id: p.id, error: String((e as Error)?.message ?? e) }), "error");
      }
    }
    setDeleting(false);
    if (done > 0) fb.toast(t("home.deleted_many", { n: done }), "success");
    stopSelecting();
    load();
  };

  const legacyRoots = (info?.legacy_roots ?? []).filter((r) => !settings.migrated_from.includes(r));
  const userName = profile.full_name || profile.session?.user.username || "";
  const serverUrl = settings.data_server_url || settings.git_api_url.replace(/\/api\/v\d+\/?$/, "");

  const row = (p: ProjectItem, i: number) => {
    const pct = Math.round(p.progress * 100);
    const idx = BIBLE_ORDER.indexOf(p.project_id);
    const hueKey = p.type_id !== "text" ? "helps-" + p.type_id : p.resource_id === "obs" ? "obs" : idx >= 0 && idx < 39 ? "ot" : "nt";
    const isSel = selected.has(p.id);
    return (
      <div
        key={p.id}
        className={"project-row" + (isSel ? " selected" : "")}
        onClick={() => (selecting ? toggleSelected(p.id) : openProject(p))}
        style={{ animationDelay: `${Math.min(i, 12) * 25}ms` }}
        aria-selected={selecting ? isSel : undefined}
      >
        {selecting ? (
          <span className={"select-box" + (isSel ? " on" : "")}>{isSel ? <CheckSquare size={22} /> : <Square size={22} />}</span>
        ) : (
          <Monogram text={p.project_id} hueKey={hueKey} />
        )}
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
          {!selecting && <Button variant="ghost" icon title={t("home.info")} onClick={() => setDetails(p)}><Info size={18} /></Button>}
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
        <div style={{ position: "relative" }} ref={userMenuRef}>
          <button type="button" className={"chip user-chip" + (userMenu ? " open" : "")} title={t("profile.current_user")} onClick={() => setUserMenu((m) => !m)} aria-haspopup="menu" aria-expanded={userMenu}>
            <Avatar name={userName} size={24} />
            <span className="ellipsis" style={{ maxWidth: 180 }}>{userName}</span>
            {profile.session ? <Cloud size={14} className="user-state online" /> : <CloudOff size={14} className="user-state" />}
            <ChevronDown size={14} />
          </button>
          {userMenu && (
            <div className="menu user-menu" role="menu" onClick={() => setUserMenu(false)}>
              <div className="menu-head">
                <Avatar name={userName} size={38} />
                <div style={{ minWidth: 0 }}>
                  <div className="ellipsis" style={{ fontWeight: 650 }}>{userName}</div>
                  <div className="small muted ellipsis">
                    {profile.session ? `${t("profile.menu_server")} · ${profile.session.user.username}` : t("profile.menu_local")}
                  </div>
                </div>
              </div>
              <hr />
              {profile.session ? (
                <>
                  {serverUrl && <button onClick={() => openUrl(`${serverUrl}/${profile.session!.user.username}`).catch(fb.error)}><ExternalLink size={16} />{t("profile.open_wacs")}</button>}
                  <button onClick={disconnect}><CloudOff size={16} />{t("profile.disconnect_server")}</button>
                </>
              ) : (
                <button onClick={() => setDialog("login")}><Cloud size={16} />{t("profile.connect_server")}</button>
              )}
              <button onClick={() => setDialog("rename")}><Pencil size={16} />{t("profile.rename")}</button>
              <hr />
              <button onClick={() => navigate({ name: "settings" })}><SettingsIcon size={16} />{t("home.settings")}</button>
              <button onClick={logout}><LogOut size={16} />{t("profile.switch_account")}</button>
            </div>
          )}
        </div>
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
                <label className="search-field">
                  <Search size={16} />
                  <input className="input" type="text" placeholder={t("home.search")} value={query} onChange={(e) => setQuery(e.target.value)} aria-label={t("home.search")} />
                  {query && <button type="button" className="clear" onClick={() => setQuery("")} aria-label={t("common.cancel")}><X size={14} /></button>}
                </label>
                <select className="select" value={settings.sort_by} onChange={(e) => updateSettings({ sort_by: e.target.value }).catch(fb.error)} title={t("home.sort")}>
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
                <Button variant={selecting ? "primary" : "default"} size="sm" onClick={() => (selecting ? stopSelecting() : setSelecting(true))}>
                  <CheckSquare size={15} /> {selecting ? t("common.cancel") : t("home.select")}
                </Button>
              </div>

              {selecting && (
                <div className="selection-bar card">
                  <strong>{selected.size === 1 ? t("home.selected_one") : t("home.selected", { n: selected.size })}</strong>
                  <Button size="sm" variant="ghost" onClick={selectAllVisible}>{t("home.select_all")}</Button>
                  <span className="grow" />
                  <Button size="sm" variant="danger" disabled={selected.size === 0} busy={deleting} onClick={deleteSelected}><Trash2 size={15} /> {t("home.delete_selected")}{selected.size ? ` (${selected.size})` : ""}</Button>
                  <Button size="sm" variant="ghost" icon title={t("common.cancel")} onClick={stopSelecting}><X size={16} /></Button>
                </div>
              )}

              {filtered.length === 0 ? (
                <div className="card"><EmptyState icon={<Search size={24} />} title={t("home.no_results")} /></div>
              ) : grouped ? (
                groups.map((g) => {
                  const open = !collapsed[g.id] || !!query.trim();
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
                  <div className="project-list">{filtered.map(row)}</div>
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
      {dialog === "login" && <ServerLoginDialog onClose={() => setDialog(null)} />}
      {dialog === "rename" && <RenameDialog onClose={() => setDialog(null)} />}
      {details && <ProjectDetailsDialog item={details} onClose={() => setDetails(null)} onChanged={load} onDeleted={() => { setDetails(null); load(); }} />}
    </div>
  );
}

/** Cambia el nombre o seudónimo de la cuenta local. */
function RenameDialog({ onClose }: { onClose: () => void }) {
  const { t } = useT();
  const { profile, setProfile } = useApp();
  const fb = useFeedback();
  const [name, setName] = useState(profile.full_name || profile.session?.user.username || "");
  const [busy, setBusy] = useState(false);
  const save = async () => {
    if (!name.trim()) return;
    setBusy(true);
    try {
      setProfile(await api.saveProfile({ ...profile, full_name: name.trim() }));
      onClose();
    } catch (e) {
      fb.error(e);
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog title={t("profile.rename")} narrow onClose={onClose} footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" busy={busy} disabled={!name.trim()} onClick={save}>{t("common.save")}</Button></>}>
      <Field label={t("profile.name")} hint={t("profile.name_public")}>
        <div className="row"><UserRound size={18} className="muted" /><input className="input grow" value={name} autoFocus onChange={(e) => setName(e.target.value)} onKeyDown={(e) => e.key === "Enter" && save()} /></div>
      </Field>
    </Dialog>
  );
}
