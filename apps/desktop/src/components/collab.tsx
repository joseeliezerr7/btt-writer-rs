// Trabajo en equipo: estado de sincronización del proyecto abierto, diálogo para
// activar/gestionar el equipo y diálogo para unirse a un proyecto compartido.
import { useCallback, useEffect, useRef, useState } from "react";
import { AlertTriangle, Check, CloudOff, ExternalLink, RefreshCw, Search, Trash2, UserPlus, Users, X } from "lucide-react";
import { api, type ClaimView, type CollabMembers, type CollabStatus, type ImportMode, type Repo, type SyncReport, type User } from "../api";
import { useApp } from "../app-context";
import { useT } from "../i18n";
import { Button, Dialog, EmptyState, Field, Spinner, useFeedback } from "./ui";
import { importStatusText, ServerLoginDialog } from "./dialogs";

/** Cada cuánto se sincroniza mientras el proyecto está abierto. */
const SYNC_EVERY_MS = 40_000;

export interface Collab {
  enabled: boolean;
  status: CollabStatus | null;
  syncing: boolean;
  offline: boolean;
  error: string | null;
  lastSync: number;
  claims: ClaimView[];
  me: string;
  sync: (push?: boolean) => Promise<SyncReport | null>;
  refresh: () => Promise<void>;
  setStatus: (s: CollabStatus) => void;
  /** Reserva el chunk antes de editarlo; `false` si otra persona lo tiene reservado. */
  startEditing: (key: string) => Promise<boolean>;
  stopEditing: (key: string) => Promise<void>;
}

/** Sincronización periódica del proyecto `id` mientras el equipo esté activo. */
export function useCollab(id: string, onRemoteChanges: (changed: string[], conflicts: string[]) => void): Collab {
  const [status, setStatus] = useState<CollabStatus | null>(null);
  const [syncing, setSyncing] = useState(false);
  const [offline, setOffline] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lastSync, setLastSync] = useState(0);
  const enabled = !!status?.config?.enabled;
  const enabledRef = useRef(enabled);
  enabledRef.current = enabled;
  const editing = useRef<string | null>(null);
  const busy = useRef(false);
  const onRemote = useRef(onRemoteChanges);
  onRemote.current = onRemoteChanges;

  const refresh = useCallback(async () => {
    try {
      const s = await api.collabStatus(id);
      setStatus(s);
      if (s.config?.last_sync) setLastSync(s.config.last_sync);
    } catch {
      /* sin equipo o sin proyecto: nada que mostrar */
    }
  }, [id]);

  const sync = useCallback(async (push = true): Promise<SyncReport | null> => {
    if (!enabledRef.current || busy.current) return null;
    busy.current = true;
    setSyncing(true);
    try {
      const r = await api.collabSync(id, editing.current, push);
      if (r.error === "busy") return r;
      setOffline(r.offline);
      setError(r.error);
      if (!r.error && !r.offline) setLastSync(r.at);
      setStatus((s) => (s ? { ...s, claims: r.claims, conflicts: r.conflicts } : s));
      if (r.changed.length || r.conflicts.length) onRemote.current(r.changed, r.conflicts);
      return r;
    } catch (e) {
      setError(String(e));
      return null;
    } finally {
      busy.current = false;
      setSyncing(false);
    }
  }, [id]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  useEffect(() => {
    if (!enabled) return;
    sync(true);
    const iv = setInterval(() => sync(true), SYNC_EVERY_MS);
    const onVisible = () => { if (document.visibilityState === "visible") sync(true); };
    const onOnline = () => sync(true);
    document.addEventListener("visibilitychange", onVisible);
    window.addEventListener("online", onOnline);
    return () => {
      clearInterval(iv);
      document.removeEventListener("visibilitychange", onVisible);
      window.removeEventListener("online", onOnline);
    };
  }, [enabled, sync]);

  // Al salir del proyecto: se libera la reserva y se sube lo pendiente.
  useEffect(() => () => {
    const key = editing.current;
    editing.current = null;
    if (!enabledRef.current) return;
    const done = key ? api.collabRelease(id, key).catch(() => {}) : Promise.resolve();
    done.then(() => api.collabSync(id, null, true)).catch(() => {});
  }, [id]);

  const startEditing = useCallback(async (key: string) => {
    if (!enabledRef.current) return true;
    editing.current = key;
    let ok = true;
    try {
      ok = await api.collabClaim(id, key);
    } catch {
      ok = true;
    }
    void sync(true);
    return ok;
  }, [id, sync]);

  const stopEditing = useCallback(async (key: string) => {
    if (!enabledRef.current) return;
    if (editing.current === key) editing.current = null;
    await api.collabRelease(id, key).catch(() => {});
    void sync(true);
  }, [id, sync]);

  return { enabled, status, syncing, offline, error, lastSync, claims: status?.claims ?? [], me: status?.me ?? "", sync, refresh, setStatus, startEditing, stopEditing };
}

/** "hace 20 s" / "hace 3 min". */
export function ago(secs: number, t: (k: string, v?: Record<string, string | number>) => string): string {
  const d = Math.max(0, Math.floor(Date.now() / 1000) - secs);
  if (d < 60) return t("collab.ago_seconds", { n: d });
  if (d < 3600) return t("collab.ago_minutes", { n: Math.floor(d / 60) });
  return t("collab.ago_hours", { n: Math.floor(d / 3600) });
}

/** Indicador del equipo en la barra superior del proyecto. */
export function CollabPill({ collab, onClick }: { collab: Collab; onClick: () => void }) {
  const { t } = useT();
  const [, tick] = useState(0);
  useEffect(() => {
    const iv = setInterval(() => tick((n) => n + 1), 10_000);
    return () => clearInterval(iv);
  }, []);
  if (!collab.enabled) {
    return (
      <Button size="sm" variant="ghost" title={t("collab.title")} onClick={onClick}><Users size={14} /> {t("collab.title")}</Button>
    );
  }
  const others = collab.claims.filter((c) => c.user !== collab.me).length;
  const tone = collab.syncing ? "busy" : collab.error ? "error" : collab.offline ? "offline" : "ok";
  const label = collab.syncing
    ? t("collab.syncing")
    : collab.error
      ? t("collab.error")
      : collab.offline
        ? t("collab.offline")
        : collab.lastSync
          ? t("collab.synced_ago", { ago: ago(collab.lastSync, t) })
          : t("collab.title");
  return (
    <button type="button" className={"collab-pill " + tone} onClick={onClick} title={collab.error || label}>
      {collab.offline && !collab.syncing ? <CloudOff size={14} /> : <Users size={14} />}
      <span className="dot" />
      <span className="ellipsis">{label}</span>
      {others > 0 && <span className="badge">{others}</span>}
    </button>
  );
}

/** Activar o gestionar el equipo del proyecto abierto. */
export function CollabDialog({ id, collab, onClose }: { id: string; collab: Collab; onClose: () => void }) {
  const { t } = useT();
  const { profile } = useApp();
  const fb = useFeedback();
  const [login, setLogin] = useState(false);
  const [members, setMembers] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [team, setTeam] = useState<CollabMembers | null>(null);
  const cfg = collab.status?.config;
  const enabled = !!cfg?.enabled;

  useEffect(() => {
    if (enabled && profile.session) api.collabMembers(id).then(setTeam).catch(() => setTeam(null));
  }, [enabled, id, profile.session?.user.username]);

  if (!profile.session) {
    return (
      <>
        <Dialog title={t("collab.title")} onClose={onClose} narrow footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" onClick={() => setLogin(true)}>{t("profile.connect_server")}</Button></>}>
          <p>{t("collab.need_login")}</p>
        </Dialog>
        {login && <ServerLoginDialog onClose={() => setLogin(false)} onDone={() => setLogin(false)} />}
      </>
    );
  }

  const enable = async () => {
    setBusy(true);
    try {
      const s = await api.collabEnable(id, members);
      collab.setStatus(s);
      fb.toast(t("collab.enabled_toast"), "success");
      onClose();
    } catch (e) {
      fb.error(e);
    } finally {
      setBusy(false);
    }
  };

  const disable = async () => {
    const ok = await fb.confirm(t("collab.disable"), t("collab.disable_confirm"), { danger: true, okLabel: t("collab.disable") });
    if (!ok) return;
    setBusy(true);
    try {
      const s = await api.collabDisable(id);
      collab.setStatus(s);
      fb.toast(t("collab.disabled_toast"), "success");
      onClose();
    } catch (e) {
      fb.error(e);
    } finally {
      setBusy(false);
    }
  };

  const addMember = async (name: string) => {
    if (!enabled) {
      if (!members.includes(name)) setMembers([...members, name]);
      return;
    }
    try {
      setTeam(await api.collabAddMember(id, name));
      fb.toast(t("collab.member_added", { user: name }), "success");
    } catch (e) {
      fb.error(e);
    }
  };
  const removeMember = async (name: string) => {
    if (!enabled) {
      setMembers(members.filter((m) => m !== name));
      return;
    }
    const ok = await fb.confirm(t("collab.remove_member"), t("collab.remove_member_confirm", { user: name }), { danger: true });
    if (!ok) return;
    try {
      setTeam(await api.collabRemoveMember(id, name));
    } catch (e) {
      fb.error(e);
    }
  };

  const list = enabled ? (team?.collaborators.map((u) => u.username) ?? []) : members;
  const canManage = !enabled || !!team?.can_manage;

  return (
    <Dialog
      title={<span className="row" style={{ gap: ".5rem" }}><Users size={18} /> {t("collab.title")}</span>}
      onClose={onClose}
      footer={
        enabled ? (
          <>
            <Button variant="danger" disabled={busy} onClick={disable}>{t("collab.disable")}</Button>
            <span className="grow" />
            <Button busy={collab.syncing} onClick={() => collab.sync(true)}><RefreshCw size={14} /> {t("collab.sync_now")}</Button>
            <Button variant="primary" onClick={onClose}>{t("common.close")}</Button>
          </>
        ) : (
          <>
            <Button onClick={onClose}>{t("common.cancel")}</Button>
            <Button variant="primary" busy={busy} onClick={enable}><Users size={14} /> {t("collab.enable")}</Button>
          </>
        )
      }
    >
      <div className="col" style={{ gap: ".9rem" }}>
        {enabled && cfg ? (
          <div className="card pad col" style={{ gap: ".4rem" }}>
            <div className="row" style={{ gap: ".5rem" }}>
              <span className={"collab-dot " + (collab.error ? "error" : collab.offline ? "offline" : "ok")} />
              <strong>{collab.syncing ? t("collab.syncing") : collab.error ? t("collab.error") : collab.offline ? t("collab.offline") : t("collab.up_to_date")}</strong>
              <span className="grow" />
              {collab.lastSync > 0 && <span className="small muted">{t("collab.synced_ago", { ago: ago(collab.lastSync, t) })}</span>}
            </div>
            {collab.error && <div className="small" style={{ color: "var(--danger)" }}>{collab.error}</div>}
            {collab.status && collab.status.conflicts.length > 0 && (
              <div className="small row" style={{ gap: ".4rem", color: "var(--danger)" }}><AlertTriangle size={14} /> {t("collab.conflicts_pending", { n: collab.status.conflicts.length })}</div>
            )}
            <div className="small muted row" style={{ gap: ".4rem" }}>
              {t("collab.repo")}: <a href={cfg.html_url} target="_blank" rel="noreferrer" className="row" style={{ gap: ".2rem" }}>{cfg.owner}/{cfg.repo} <ExternalLink size={12} /></a>
            </div>
          </div>
        ) : (
          <>
            <p style={{ margin: 0 }}>{t("collab.desc")}</p>
            <ul className="collab-how">
              <li>{t("collab.how1")}</li>
              <li>{t("collab.how2")}</li>
              <li>{t("collab.how3")}</li>
              <li>{t("collab.how4")}</li>
            </ul>
          </>
        )}

        <div>
          <div className="row" style={{ gap: ".5rem", marginBottom: ".4rem" }}>
            <strong>{t("collab.members")}</strong>
            <span className="grow" />
            {enabled && team && <span className="small muted">{t("collab.owner")}: {team.owner}</span>}
          </div>
          {enabled && !team ? <Spinner /> : null}
          <div className="col" style={{ gap: ".3rem" }}>
            {list.map((m) => (
              <div key={m} className="row member-row">
                <Users size={14} />
                <span className="grow">{m}{m === collab.me ? ` (${t("collab.you")})` : ""}</span>
                {canManage && <Button size="sm" variant="ghost" icon title={t("collab.remove_member")} onClick={() => removeMember(m)}><Trash2 size={14} /></Button>}
              </div>
            ))}
            {list.length === 0 && <div className="small muted">{t("collab.no_members")}</div>}
          </div>
          {canManage ? (
            <MemberPicker exclude={[collab.me, ...list]} onPick={addMember} />
          ) : (
            <div className="small muted" style={{ marginTop: ".4rem" }}>{t("collab.not_owner")}</div>
          )}
        </div>
      </div>
    </Dialog>
  );
}

/** Campo para añadir compañeros por su usuario de WACS, con sugerencias. */
function MemberPicker({ exclude, onPick }: { exclude: string[]; onPick: (username: string) => void }) {
  const { t } = useT();
  const [q, setQ] = useState("");
  const [hits, setHits] = useState<User[]>([]);
  useEffect(() => {
    if (q.trim().length < 2) {
      setHits([]);
      return;
    }
    const h = setTimeout(() => api.searchUsers(q.trim()).then((u) => setHits(u.filter((x) => !exclude.includes(x.username)))).catch(() => setHits([])), 300);
    return () => clearTimeout(h);
  }, [q]);
  const pick = (name: string) => {
    const n = name.trim();
    if (!n) return;
    onPick(n);
    setQ("");
    setHits([]);
  };
  return (
    <div style={{ marginTop: ".6rem" }}>
      <Field label={t("collab.add_member")} hint={t("collab.add_member_hint")}>
        <div className="row" style={{ gap: ".4rem" }}>
          <input className="input grow" value={q} placeholder={t("collab.username")} onChange={(e) => setQ(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") pick(q); }} />
          <Button onClick={() => pick(q)} disabled={!q.trim()}><UserPlus size={14} /> {t("collab.add")}</Button>
        </div>
      </Field>
      {hits.length > 0 && (
        <div className="menu static">
          {hits.map((u) => (
            <button key={u.id} type="button" onClick={() => pick(u.username)}><UserPlus size={14} />{u.username}{u.full_name ? <span className="muted"> · {u.full_name}</span> : null}</button>
          ))}
        </div>
      )}
    </div>
  );
}

/** Unirse a un proyecto que alguien compartió en WACS. */
export function JoinCollabDialog({ existingIds, onClose, onJoined }: { existingIds: string[]; onClose: () => void; onJoined: (id: string) => void }) {
  const { t } = useT();
  const { profile, settings } = useApp();
  const fb = useFeedback();
  const [login, setLogin] = useState(false);
  const [repos, setRepos] = useState<Repo[] | null>(null);
  const [q, setQ] = useState("");
  const [busy, setBusy] = useState<string | null>(null);

  useEffect(() => {
    if (!profile.session) return;
    api.collabSharedRepos().then(setRepos).catch((e) => { fb.error(e); setRepos([]); });
  }, [profile.session?.user.username]);

  if (!profile.session) {
    return (
      <>
        <Dialog title={t("collab.join")} onClose={onClose} narrow footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" onClick={() => setLogin(true)}>{t("profile.connect_server")}</Button></>}>
          <p>{t("collab.need_login")}</p>
        </Dialog>
        {login && <ServerLoginDialog onClose={() => setLogin(false)} onDone={() => setLogin(false)} />}
      </>
    );
  }

  const me = profile.session.user.username;
  const join = async (cloneUrl: string, id: string) => {
    let mode: ImportMode = "merge";
    if (existingIds.includes(id)) {
      const ok = await fb.confirm(t("collab.join"), t("collab.join_merge_confirm", { id }), { okLabel: t("collab.join_merge") });
      if (!ok) return;
      mode = "merge";
    }
    setBusy(cloneUrl);
    try {
      const o = await api.collabJoin(cloneUrl, mode);
      if (o.status.status === "failed") {
        fb.error(importStatusText(t, o));
        return;
      }
      fb.toast(t("collab.joined", { id: o.id }), "success");
      onJoined(o.id);
      onClose();
    } catch (e) {
      fb.error(e);
    } finally {
      setBusy(null);
    }
  };

  const typed = q.trim();
  const direct = /^[\w.-]+\/[\w.-]+$/.test(typed) ? typed : null;
  const directUrl = direct ? `${settings.data_server_url.replace(/\/+$/, "")}/${direct}.git` : null;
  const filtered = (repos ?? []).filter((r) => !typed || r.full_name.toLowerCase().includes(typed.toLowerCase()));
  const shared = filtered.filter((r) => r.owner.username !== me);
  const mine = filtered.filter((r) => r.owner.username === me);

  return (
    <Dialog title={<span className="row" style={{ gap: ".5rem" }}><Users size={18} /> {t("collab.join")}</span>} onClose={onClose} footer={<Button onClick={onClose}>{t("common.close")}</Button>}>
      <div className="col" style={{ gap: ".8rem" }}>
        <p style={{ margin: 0 }} className="small muted">{t("collab.join_desc")}</p>
        <div className="row" style={{ gap: ".4rem" }}>
          <Search size={16} className="muted" />
          <input className="input grow" value={q} placeholder={t("collab.join_search")} onChange={(e) => setQ(e.target.value)} autoFocus />
          {q && <Button variant="ghost" icon size="sm" onClick={() => setQ("")}><X size={14} /></Button>}
        </div>
        {directUrl && !filtered.some((r) => r.full_name.toLowerCase() === direct!.toLowerCase()) && (
          <div className="list-item" onClick={() => join(directUrl, direct!.split("/")[1])}>
            <Users size={16} />
            <div className="grow"><strong>{direct}</strong><div className="small muted">{t("collab.join_direct")}</div></div>
            <Button size="sm" variant="primary" busy={busy === directUrl}>{t("collab.join_btn")}</Button>
          </div>
        )}
        {repos === null ? (
          <Spinner />
        ) : shared.length === 0 && mine.length === 0 && !directUrl ? (
          <EmptyState icon={<Users size={24} />} title={t("collab.no_shared")} description={t("collab.no_shared_desc")} />
        ) : shared.length + mine.length === 0 ? null : (
          <div className="card list">
            {shared.length > 0 && <div className="card-title">{t("collab.shared_with_me")}</div>}
            {shared.map((r) => <RepoRow key={r.id} repo={r} joined={existingIds.includes(r.name)} busy={busy === r.clone_url} onJoin={() => join(r.clone_url, r.name)} />)}
            {mine.length > 0 && <div className="card-title">{t("collab.my_repos")}</div>}
            {mine.map((r) => <RepoRow key={r.id} repo={r} joined={existingIds.includes(r.name)} busy={busy === r.clone_url} onJoin={() => join(r.clone_url, r.name)} />)}
          </div>
        )}
      </div>
    </Dialog>
  );
}

function RepoRow({ repo, joined, busy, onJoin }: { repo: Repo; joined: boolean; busy: boolean; onJoin: () => void }) {
  const { t } = useT();
  return (
    <div className="list-item" onClick={onJoin}>
      <Users size={16} />
      <div className="grow">
        <strong>{repo.name}</strong>
        <div className="small muted">{t("collab.owner")}: {repo.owner.username}{repo.updated_at ? ` · ${repo.updated_at.slice(0, 10)}` : ""}</div>
      </div>
      {joined ? <span className="badge ok"><Check size={12} /> {t("collab.on_device")}</span> : null}
      <Button size="sm" variant={joined ? "default" : "primary"} busy={busy}>{joined ? t("collab.join_merge") : t("collab.join_btn")}</Button>
    </div>
  );
}
