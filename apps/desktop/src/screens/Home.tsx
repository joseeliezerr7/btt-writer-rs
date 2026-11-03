// Inicio: lista de proyectos, ordenación, menú de opciones y diálogos.
import { useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { AlertTriangle, Info, MoreVertical, Plus } from "lucide-react";
import { api, resourceLabel, type ProjectItem, type ProjectRow } from "../api";
import { useApp } from "../app-context";
import { formatDate, useT } from "../i18n";
import { Button, ProgressRing, Spinner, useFeedback } from "../components/ui";
import { BackupsDialog, bookName, FeedbackDialog, ImportDialog, MigrateDialog, NewProjectDialog, ProjectDetailsDialog, UpdateLibraryDialog } from "../components/dialogs";

const BIBLE_ORDER = "gen exo lev num deu jos jdg rut 1sa 2sa 1ki 2ki 1ch 2ch ezr neh est job psa pro ecc sng isa jer lam ezk dan hos jol amo oba jon mic nam hab zep hag zec mal mat mrk luk jhn act rom 1co 2co gal eph php col 1th 2th 1ti 2ti tit phm heb jas 1pe 2pe 1jn 2jn 3jn jud rev".split(" ");

type DialogKind = null | "new" | "import" | "library" | "feedback" | "backups" | "migrate";

export default function HomeScreen() {
  const { t, lang } = useT();
  const { profile, settings, updateSettings, navigate, info, setProfile } = useApp();
  const fb = useFeedback();
  const [projects, setProjects] = useState<ProjectItem[] | null>(null);
  const [books, setBooks] = useState<ProjectRow[]>([]);
  const [dialog, setDialog] = useState<DialogKind>(null);
  const [details, setDetails] = useState<ProjectItem | null>(null);
  const [menu, setMenu] = useState(false);
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
    const un = listen<string[]>("backup-done", () => load());
    return () => {
      un.then((f) => f());
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

  return (
    <div className="screen">
      <div className="topbar">
        <h1>{t("home.title")}</h1>
        <span className="muted small">{t("profile.current_user")}: <strong>{profile.full_name || profile.session?.user.username}</strong></span>
        <span className="grow" />
        <Button variant="primary" onClick={() => setDialog("new")}><Plus size={16} /> {t("home.new")}</Button>
        <div style={{ position: "relative" }} ref={menuRef}>
          <Button variant="ghost" icon onClick={() => setMenu((m) => !m)} title={t("home.more")}><MoreVertical size={18} /></Button>
          {menu && (
            <div className="menu" onClick={() => setMenu(false)}>
              <button onClick={() => setDialog("library")}>{t("home.update")}</button>
              <button onClick={() => setDialog("import")}>{t("home.import")}</button>
              <button onClick={() => setDialog("backups")}>{t("home.backups")}</button>
              {legacyRoots.length > 0 && <button onClick={() => setDialog("migrate")}>{t("home.migrate")}</button>}
              <button onClick={() => setDialog("feedback")}>{t("home.feedback")}</button>
              <hr />
              <button onClick={() => navigate({ name: "settings" })}>{t("home.settings")}</button>
              <button onClick={logout}>{t("profile.logout")}</button>
            </div>
          )}
        </div>
      </div>

      <div className="content">
        <div className="container">
          {projects === null ? (
            <Spinner />
          ) : projects.length === 0 ? (
            <div className="welcome">
              <div className="card">
                <h2>{t("home.empty")}</h2>
                <p className="muted">{t("home.empty_desc")}</p>
                <Button variant="primary" onClick={() => setDialog("new")}><Plus size={16} /> {t("home.new")}</Button>
                {legacyRoots.length > 0 && <div style={{ marginTop: "1rem" }}><Button onClick={() => setDialog("migrate")}>{t("home.migrate")}</Button></div>}
              </div>
            </div>
          ) : (
            <>
              <div className="row wrap small muted" style={{ marginBottom: ".6rem" }}>
                <span>{t("home.sort")}:</span>
                <select className="select" style={{ width: "auto" }} value={settings.sort_by} onChange={(e) => updateSettings({ sort_by: e.target.value })}>
                  <option value="project">{t("home.sort.project")}</option>
                  <option value="language">{t("home.sort.language")}</option>
                  <option value="progress">{t("home.sort.progress")}</option>
                </select>
                <select className="select" style={{ width: "auto" }} value={settings.book_order} onChange={(e) => updateSettings({ book_order: e.target.value })}>
                  <option value="bible">{t("home.book_order")}</option>
                  <option value="alpha">{t("home.alpha_order")}</option>
                </select>
              </div>
              <div className="card">
                <table className="project-table">
                  <thead>
                    <tr>
                      <th>{t("home.col.project")}</th>
                      <th>{t("home.col.type")}</th>
                      <th>{t("home.col.language")}</th>
                      <th>{t("home.col.progress")}</th>
                      <th></th>
                    </tr>
                  </thead>
                  <tbody>
                    {sorted.map((p) => (
                      <tr key={p.id} onClick={() => openProject(p)}>
                        <td>
                          <strong>{bookName(p, books)}</strong>
                          <div className="small muted">{p.id} · {formatDate(p.modified_at, lang, "")}</div>
                        </td>
                        <td>{resourceLabel(p, t)}</td>
                        <td>
                          <span className="target-text" style={{ fontSize: "1rem" }}>{p.language_name}</span> <span className="muted small">({p.language_id})</span>
                        </td>
                        <td>
                          <div className="row">
                            <ProgressRing value={p.progress} />
                            {p.has_conflicts && <span title={t("home.conflicts")} className="badge danger"><AlertTriangle size={12} /></span>}
                          </div>
                        </td>
                        <td onClick={(e) => e.stopPropagation()}>
                          <Button variant="ghost" icon title={t("home.info")} onClick={() => setDetails(p)}><Info size={18} /></Button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </>
          )}
        </div>
      </div>

      {dialog === "new" && <NewProjectDialog onClose={() => setDialog(null)} onCreated={(id) => { setDialog(null); load(); navigate({ name: "translate", id }); }} />}
      {dialog === "import" && <ImportDialog onClose={() => setDialog(null)} onImported={() => load()} />}
      {dialog === "library" && <UpdateLibraryDialog onClose={() => setDialog(null)} />}
      {dialog === "feedback" && <FeedbackDialog onClose={() => setDialog(null)} />}
      {dialog === "backups" && <BackupsDialog onClose={() => setDialog(null)} onImported={load} />}
      {dialog === "migrate" && <MigrateDialog roots={legacyRoots} onClose={() => setDialog(null)} onDone={() => { load(); updateSettings({}).catch(() => {}); }} />}
      {details && <ProjectDetailsDialog item={details} onClose={() => setDetails(null)} onChanged={load} onDeleted={() => { setDetails(null); load(); }} />}
    </div>
  );
}
