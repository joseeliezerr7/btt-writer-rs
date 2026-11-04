import { useCallback, useEffect, useMemo, useState } from "react";
import { api, type AppInfo, type Profile, type Settings } from "./api";
import { AppContext, spToPx, type View } from "./app-context";
import { I18nContext, translate, type Lang } from "./i18n";
import { FeedbackProvider, Spinner } from "./components/ui";
import ProfileScreen from "./screens/Profile";
import HomeScreen from "./screens/Home";
import TranslateScreen from "./screens/Translate";
import SettingsScreen from "./screens/Settings";
import PublishScreen from "./screens/Publish";

const DEFAULT_SETTINGS: Settings = {
  color_theme: "system",
  ui_language: "es",
  gateway_language_mode: false,
  translation_font: "Noto Sans",
  translation_font_size: 25,
  source_font: "Noto Sans",
  source_font_size: 25,
  content_server: "wacs",
  git_api_url: "",
  data_server_url: "",
  media_server_url: "",
  reader_url: "",
  create_account_url: "",
  langnames_url: "",
  index_sqlite_url: "",
  translation_manual_url: "",
  enable_tm_links: true,
  backup_interval_minutes: 5,
  logging_level: "info",
  sort_by: "project",
  book_order: "bible",
  blind_draft: false,
  check_hardware: true,
  last_project: "",
  migrated_from: [],
};

const DEFAULT_PROFILE: Profile = { full_name: "", email: "", session: null, terms_accepted_version: 0, privacy_accepted: false };

export default function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS);
  const [profile, setProfile] = useState<Profile>(DEFAULT_PROFILE);
  const [view, setView] = useState<View>({ name: "loading" });
  const [fatal, setFatal] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const [i, s, p] = await Promise.all([api.appInfo(), api.getSettings(), api.getProfile()]);
        setInfo(i);
        setSettings(s);
        setProfile(p);
        const loggedIn = (p.full_name || p.session) && p.terms_accepted_version >= 3;
        setView(loggedIn ? { name: "home" } : { name: "profile" });
      } catch (e) {
        setFatal(String(e));
      }
    })();
  }, []);

  // Tema y tipografía
  useEffect(() => {
    const root = document.documentElement;
    const apply = () => {
      const dark = settings.color_theme === "dark" || (settings.color_theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
      root.dataset.theme = dark ? "dark" : "light";
    };
    apply();
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    mq.addEventListener("change", apply);
    root.style.setProperty("--font-text", `'${settings.translation_font}', 'Noto Sans', 'Segoe UI', system-ui, sans-serif`);
    root.style.setProperty("--size-target", `${spToPx(settings.translation_font_size)}px`);
    root.style.setProperty("--size-source", `${spToPx(settings.source_font_size)}px`);
    return () => mq.removeEventListener("change", apply);
  }, [settings]);

  const lang: Lang = settings.ui_language === "en" ? "en" : "es";
  const i18n = useMemo(() => ({ lang, t: (k: string, vars?: Record<string, string | number>) => translate(lang, k, vars) }), [lang]);

  const updateSettings = useCallback(
    async (patch: Partial<Settings>) => {
      const next = { ...settings, ...patch };
      setSettings(next);
      const saved = await api.saveSettings(next);
      setSettings(saved);
    },
    [settings],
  );

  const reloadProfile = useCallback(async () => setProfile(await api.getProfile()), []);
  const reloadSettings = useCallback(async () => setSettings(await api.getSettings()), []);

  const ctx = useMemo(
    () => ({ info, settings, profile, view, navigate: setView, updateSettings, setProfile, reloadProfile, reloadSettings }),
    [info, settings, profile, view, updateSettings, reloadProfile, reloadSettings],
  );

  if (fatal) {
    return (
      <div className="welcome">
        <div className="card">
          <h2>BTT Writer</h2>
          <p className="muted">{fatal}</p>
        </div>
      </div>
    );
  }

  return (
    <I18nContext.Provider value={i18n}>
      <FeedbackProvider labels={{ ok: i18n.t("common.ok"), cancel: i18n.t("common.cancel") }}>
        <AppContext.Provider value={ctx}>
          {view.name === "loading" ? (
            <div className="welcome">
              <div className="card">
                <h1>{i18n.t("app.welcome")}</h1>
                <Spinner label={i18n.t("app.loading")} />
              </div>
            </div>
          ) : view.name === "profile" ? (
            <ProfileScreen />
          ) : view.name === "home" ? (
            <HomeScreen />
          ) : view.name === "translate" ? (
            <TranslateScreen key={view.id} id={view.id} initialChapter={view.chapter} initialChunk={view.chunk} initialMode={view.mode} />
          ) : view.name === "settings" ? (
            <SettingsScreen />
          ) : (
            <PublishScreen id={view.id} />
          )}
        </AppContext.Provider>
      </FeedbackProvider>
    </I18nContext.Provider>
  );
}
