// Ajustes: general, servidor, avanzado, legal.
import { useState, type ReactNode } from "react";
import { openPath } from "@tauri-apps/plugin-opener";
import { ArrowLeft, FileText, FolderOpen, HeartHandshake, ScrollText } from "lucide-react";
import { api } from "../api";
import { FONT_SIZES, useApp } from "../app-context";
import { useT } from "../i18n";
import { Button, OptionItem, Switch, Tabs, useFeedback } from "../components/ui";
import { LegalDialog } from "./Profile";

const FONTS = ["Noto Sans", "Noto Serif", "Segoe UI", "Arial", "Times New Roman", "Charis SIL", "Gentium Plus", "Andika", "Doulos SIL", "Scheherazade New", "Padauk", "Abyssinica SIL", "Noto Nastaliq Urdu"];

function Group({ title, children, footer }: { title: ReactNode; children: ReactNode; footer?: ReactNode }) {
  return (
    <section>
      <p className="eyebrow" style={{ margin: "0 0 .5rem .2rem" }}>{title}</p>
      <div className="card settings-group">
        {children}
        {footer}
      </div>
    </section>
  );
}

function Row({ label, hint, children }: { label: ReactNode; hint?: ReactNode; children: ReactNode }) {
  return (
    <div className="setting-row">
      <div className="text">
        <strong>{label}</strong>
        {hint ? <span>{hint}</span> : null}
      </div>
      <div className="control">{children}</div>
    </div>
  );
}

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
        <div className="container col" style={{ maxWidth: 820, gap: "1.3rem" }}>
          <Tabs
            segmented
            value={tab}
            onChange={setTab}
            tabs={[
              { id: "general", label: t("settings.general") },
              { id: "server", label: t("settings.server") },
              { id: "advanced", label: t("settings.advanced") },
              { id: "legal", label: t("settings.legal") },
            ]}
          />

          {tab === "general" && (
            <>
              <Group title={t("settings.appearance")}>
                <Row label={t("settings.theme")}>
                  <select className="select" value={settings.color_theme} onChange={(e) => set({ color_theme: e.target.value as "light" | "dark" | "system" })}>
                    <option value="light">{t("settings.theme.light")}</option>
                    <option value="dark">{t("settings.theme.dark")}</option>
                    <option value="system">{t("settings.theme.system")}</option>
                  </select>
                </Row>
                <Row label={t("settings.ui_language")}>
                  <select className="select" value={lang} onChange={(e) => set({ ui_language: e.target.value })}>
                    <option value="es">Español</option>
                    <option value="en">English</option>
                  </select>
                </Row>
                <Row label={t("settings.gl_mode")} hint={t("settings.gl_desc")}>
                  <Switch on={settings.gateway_language_mode} onChange={(v) => set({ gateway_language_mode: v })} />
                </Row>
                <Row label={t("settings.blind")}>
                  <Switch on={settings.blind_draft} onChange={(v) => set({ blind_draft: v })} />
                </Row>
              </Group>

              <Group
                title={t("settings.translation_text")}
                footer={
                  <div className="preview-text">
                    <div className="eyebrow">{t("settings.preview")}</div>
                    <p className="target-text" style={{ fontSize: "var(--size-target)", margin: 0 }}><sup>1</sup>En el principio creó Dios los cielos y la tierra.</p>
                  </div>
                }
              >
                <Row label={t("settings.font")}>
                  <select className="select" value={settings.translation_font} onChange={(e) => set({ translation_font: e.target.value })}>
                    {FONTS.map((f) => <option key={f}>{f}</option>)}
                  </select>
                </Row>
                <Row label={t("settings.font_size")}>
                  <select className="select" value={settings.translation_font_size} onChange={(e) => set({ translation_font_size: Number(e.target.value) })}>
                    {FONT_SIZES.map((s) => <option key={s.value} value={s.value}>{t(s.key)}</option>)}
                  </select>
                </Row>
              </Group>

              <Group title={t("settings.source_text")}>
                <Row label={t("settings.source_font")}>
                  <select className="select" value={settings.source_font} onChange={(e) => set({ source_font: e.target.value })}>
                    {FONTS.map((f) => <option key={f}>{f}</option>)}
                  </select>
                </Row>
                <Row label={t("settings.source_font_size")}>
                  <select className="select" value={settings.source_font_size} onChange={(e) => set({ source_font_size: Number(e.target.value) })}>
                    {FONT_SIZES.map((s) => <option key={s.value} value={s.value}>{t(s.key)}</option>)}
                  </select>
                </Row>
              </Group>
            </>
          )}

          {tab === "server" && (
            <>
              <Group title={t("settings.server")}>
                <Row label={t("settings.content_server")} hint={t("settings.restart_server")}>
                  <select className="select" value={settings.content_server} onChange={(e) => changeServer(e.target.value as "wacs" | "wacs_dev" | "dcs")}>
                    <option value="wacs">WACS (content.bibletranslationtools.org)</option>
                    <option value="wacs_dev">WACS DEV</option>
                    <option value="dcs">DCS (git.door43.org)</option>
                  </select>
                </Row>
              </Group>
              <Group title={t("settings.urls")}>
                {([
                  ["git_api_url", "Git Server API"],
                  ["media_server_url", "Media Server"],
                  ["reader_url", "Reader Server"],
                  ["create_account_url", "Account Creation URL"],
                  ["langnames_url", "Language URL"],
                  ["index_sqlite_url", "Index.sqlite URL"],
                  ["translation_manual_url", "Translation Manual URL"],
                ] as const).map(([key, label]) => (
                  <Row key={key} label={label}>
                    <input className="input" style={{ width: 360, maxWidth: "100%" }} value={settings[key]} onChange={(e) => set({ [key]: e.target.value })} spellCheck={false} />
                  </Row>
                ))}
              </Group>
            </>
          )}

          {tab === "advanced" && (
            <>
              <Group title={t("settings.advanced")}>
                <Row label={t("settings.backup_interval")}>
                  <select className="select" value={settings.backup_interval_minutes} onChange={(e) => set({ backup_interval_minutes: Number(e.target.value) })}>
                    {[1, 5, 10].map((m) => <option key={m} value={m}>{m} {t("settings.minutes")}</option>)}
                    <option value={-1}>{t("settings.disabled")}</option>
                  </select>
                </Row>
                <Row label={t("settings.tm_links")}>
                  <Switch on={settings.enable_tm_links} onChange={(v) => set({ enable_tm_links: v })} />
                </Row>
                <Row label={t("settings.data_dir")} hint={<code>{info?.data_dir}</code>}>
                  <Button size="sm" onClick={() => info && openPath(info.data_dir).catch(fb.error)}><FolderOpen size={14} /> {t("settings.open_folder")}</Button>
                </Row>
              </Group>
              <Group title={t("settings.about")}>
                <Row label={t("settings.version")}><span className="badge info">{info?.version}</span></Row>
                <Row label={t("settings.about")} hint={`${info?.platform} · ${info?.device}`}><span /></Row>
              </Group>
            </>
          )}

          {tab === "legal" && (
            <Group title={t("settings.legal")}>
              <OptionItem icon={<ScrollText size={18} />} title={t("profile.license")} description="Creative Commons Attribution-ShareAlike 4.0" onClick={() => setLegal("license")} />
              <OptionItem icon={<HeartHandshake size={18} />} tone="accent" title={t("profile.faith")} onClick={() => setLegal("faith")} />
              <OptionItem icon={<FileText size={18} />} tone="success" title={t("profile.guidelines")} onClick={() => setLegal("guidelines")} />
            </Group>
          )}
        </div>
      </div>
      {legal && <LegalDialog kind={legal} onClose={() => setLegal(null)} />}
    </div>
  );
}
