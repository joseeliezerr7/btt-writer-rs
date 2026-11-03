// Ajustes: general, servidor, avanzado, legal.
import { useState } from "react";
import { openPath } from "@tauri-apps/plugin-opener";
import { ArrowLeft } from "lucide-react";
import { api } from "../api";
import { FONT_SIZES, useApp } from "../app-context";
import { useT } from "../i18n";
import { Button, Field, Switch, Tabs, useFeedback } from "../components/ui";
import { LegalDialog } from "./Profile";

const FONTS = ["Noto Sans", "Noto Serif", "Segoe UI", "Arial", "Times New Roman", "Charis SIL", "Gentium Plus", "Andika", "Doulos SIL", "Scheherazade New", "Padauk", "Abyssinica SIL", "Noto Nastaliq Urdu"];

export default function SettingsScreen() {
  const { t, lang } = useT();
  const { settings, updateSettings, info, navigate, profile, setProfile } = useApp();
  const fb = useFeedback();
  const [tab, setTab] = useState<"general" | "server" | "advanced" | "legal">("general");
  const [legal, setLegal] = useState<null | "license" | "guidelines" | "faith">(null);

  const set = (patch: Parameters<typeof updateSettings>[0]) => updateSettings(patch).catch(fb.error);

  const changeServer = async (server: "wacs" | "wacs_dev" | "dcs") => {
    if (server === settings.content_server) return;
    if (profile.session) {
      const ok = await fb.confirm(t("settings.content_server"), t("settings.restart_server"));
      if (!ok) return;
      await api.logout();
      setProfile(await api.getProfile());
    }
    await set({ content_server: server });
  };

  return (
    <div className="screen">
      <div className="topbar">
        <Button variant="ghost" icon onClick={() => navigate(profile.full_name || profile.session ? { name: "home" } : { name: "profile" })} title={t("common.back")}>
          <ArrowLeft size={18} />
        </Button>
        <h1>{t("settings.title")}</h1>
      </div>
      <div className="content">
        <div className="container" style={{ maxWidth: 760 }}>
          <Tabs
            value={tab}
            onChange={setTab}
            tabs={[
              { id: "general", label: t("settings.general") },
              { id: "server", label: t("settings.server") },
              { id: "advanced", label: t("settings.advanced") },
              { id: "legal", label: t("settings.legal") },
            ]}
          />
          <div className="card pad col" style={{ marginTop: "1rem", gap: "1rem" }}>
            {tab === "general" && (
              <>
                <Field label={t("settings.theme")}>
                  <select className="select" value={settings.color_theme} onChange={(e) => set({ color_theme: e.target.value as "light" | "dark" | "system" })}>
                    <option value="light">{t("settings.theme.light")}</option>
                    <option value="dark">{t("settings.theme.dark")}</option>
                    <option value="system">{t("settings.theme.system")}</option>
                  </select>
                </Field>
                <Field label={t("settings.ui_language")}>
                  <select className="select" value={lang} onChange={(e) => set({ ui_language: e.target.value })}>
                    <option value="es">Español</option>
                    <option value="en">English</option>
                  </select>
                </Field>
                <Switch on={settings.gateway_language_mode} onChange={(v) => set({ gateway_language_mode: v })} label={<span>{t("settings.gl_mode")}<br /><span className="small muted">{t("settings.gl_desc")}</span></span>} />
                <Switch on={settings.blind_draft} onChange={(v) => set({ blind_draft: v })} label={t("settings.blind")} />
                <div className="row wrap" style={{ gap: "1rem" }}>
                  <Field label={t("settings.font")}>
                    <select className="select" value={settings.translation_font} onChange={(e) => set({ translation_font: e.target.value })}>
                      {FONTS.map((f) => <option key={f}>{f}</option>)}
                    </select>
                  </Field>
                  <Field label={t("settings.font_size")}>
                    <select className="select" value={settings.translation_font_size} onChange={(e) => set({ translation_font_size: Number(e.target.value) })}>
                      {FONT_SIZES.map((s) => <option key={s.value} value={s.value}>{t(s.key)}</option>)}
                    </select>
                  </Field>
                </div>
                <div className="row wrap" style={{ gap: "1rem" }}>
                  <Field label={t("settings.source_font")}>
                    <select className="select" value={settings.source_font} onChange={(e) => set({ source_font: e.target.value })}>
                      {FONTS.map((f) => <option key={f}>{f}</option>)}
                    </select>
                  </Field>
                  <Field label={t("settings.source_font_size")}>
                    <select className="select" value={settings.source_font_size} onChange={(e) => set({ source_font_size: Number(e.target.value) })}>
                      {FONT_SIZES.map((s) => <option key={s.value} value={s.value}>{t(s.key)}</option>)}
                    </select>
                  </Field>
                </div>
                <p className="target-text" style={{ fontSize: "var(--size-target)", margin: 0 }}>\v 1 En el principio creó Dios los cielos y la tierra.</p>
              </>
            )}

            {tab === "server" && (
              <>
                <Field label={t("settings.content_server")} hint={t("settings.restart_server")}>
                  <select className="select" value={settings.content_server} onChange={(e) => changeServer(e.target.value as "wacs" | "wacs_dev" | "dcs")}>
                    <option value="wacs">WACS (content.bibletranslationtools.org)</option>
                    <option value="wacs_dev">WACS DEV</option>
                    <option value="dcs">DCS (git.door43.org)</option>
                  </select>
                </Field>
                {([
                  ["git_api_url", "Git Server API"],
                  ["media_server_url", "Media Server"],
                  ["reader_url", "Reader Server"],
                  ["create_account_url", "Account Creation URL"],
                  ["langnames_url", "Language URL"],
                  ["index_sqlite_url", "Index.sqlite URL"],
                  ["translation_manual_url", "Translation Manual URL"],
                ] as const).map(([key, label]) => (
                  <Field key={key} label={label}>
                    <input className="input" value={settings[key]} onChange={(e) => set({ [key]: e.target.value })} />
                  </Field>
                ))}
              </>
            )}

            {tab === "advanced" && (
              <>
                <Field label={t("settings.backup_interval")}>
                  <select className="select" value={settings.backup_interval_minutes} onChange={(e) => set({ backup_interval_minutes: Number(e.target.value) })}>
                    {[1, 5, 10].map((m) => <option key={m} value={m}>{m} {t("settings.minutes")}</option>)}
                    <option value={-1}>{t("settings.disabled")}</option>
                  </select>
                </Field>
                <Switch on={settings.enable_tm_links} onChange={(v) => set({ enable_tm_links: v })} label={t("settings.tm_links")} />
                <Field label={t("settings.data_dir")}>
                  <div className="row">
                    <code className="grow ellipsis small">{info?.data_dir}</code>
                    <Button size="sm" onClick={() => info && openPath(info.data_dir).catch(fb.error)}>{t("settings.open_folder")}</Button>
                  </div>
                </Field>
                <div className="muted small">
                  {t("settings.version")}: {info?.version} · {info?.platform} · {info?.device}
                </div>
              </>
            )}

            {tab === "legal" && (
              <div className="col">
                <Button onClick={() => setLegal("license")}>{t("profile.license")}</Button>
                <Button onClick={() => setLegal("faith")}>{t("profile.faith")}</Button>
                <Button onClick={() => setLegal("guidelines")}>{t("profile.guidelines")}</Button>
              </div>
            )}
          </div>
        </div>
      </div>
      {legal && <LegalDialog kind={legal} onClose={() => setLegal(null)} />}
    </div>
  );
}
