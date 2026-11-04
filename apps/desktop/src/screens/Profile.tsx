// Pantalla de perfil: cuenta del servidor, cuenta sin conexión y términos de uso.
import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ArrowRight, Cloud, Settings as SettingsIcon, UserPlus, UserRound } from "lucide-react";
import { api } from "../api";
import { useApp } from "../app-context";
import { useT } from "../i18n";
import { Button, Dialog, Field, IconBadge, Logo, useFeedback } from "../components/ui";

type Step = "index" | "online" | "offline" | "privacy" | "terms";

export default function ProfileScreen() {
  const { t } = useT();
  const { settings, profile, setProfile, navigate } = useApp();
  const fb = useFeedback();
  const [step, setStep] = useState<Step>(profile.full_name || profile.session ? "terms" : "index");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [name, setName] = useState(profile.full_name);
  const [busy, setBusy] = useState(false);
  const [legal, setLegal] = useState<null | "license" | "guidelines" | "faith">(null);

  const loginOnline = async () => {
    setBusy(true);
    try {
      const p = await api.wacsLogin(username.trim(), password);
      setProfile(p);
      setStep("terms");
    } catch (e) {
      fb.error(e);
    } finally {
      setBusy(false);
    }
  };

  const offlineContinue = async () => {
    const p = await api.saveProfile({ ...profile, full_name: name.trim() });
    setProfile(p);
    setStep("privacy");
  };

  const agree = async () => {
    const p = await api.saveProfile({ ...profile, terms_accepted_version: 3, privacy_accepted: true });
    setProfile(p);
    navigate({ name: "home" });
  };

  const decline = async () => {
    await api.logout();
    setProfile(await api.getProfile());
    setStep("index");
  };

  return (
    <div className="screen auth">
      <div className="topbar" style={{ background: "transparent", borderBottom: 0, backdropFilter: "none" }}>
        <div className="brand">
          <Logo size={28} />
          <div className="name">{t("app.name")}</div>
        </div>
        <span className="grow" />
        <Button variant="ghost" onClick={() => navigate({ name: "settings" })}>
          <SettingsIcon size={16} /> {t("home.settings")}
        </Button>
      </div>
      <div className="content">
        <div className="container col" style={{ maxWidth: 900, alignItems: "center", width: "100%" }}>
          {step === "index" && (
            <>
              <div className="auth-hero">
                <Logo size={64} className="logo" />
                <h1>{t("app.welcome")}</h1>
                <p>{t("profile.tagline")}</p>
              </div>
              <p className="eyebrow" style={{ marginBottom: ".8rem" }}>{t("profile.choose")}</p>
              <div className="profile-options">
                <button type="button" className="option-card" onClick={() => setStep("online")}>
                  <IconBadge size="lg"><Cloud size={24} /></IconBadge>
                  <h3>{t("profile.login_server")}</h3>
                  <p>{t("profile.login_server_desc")}</p>
                  <span className="go">{t("common.continue")} <ArrowRight size={14} /></span>
                </button>
                <button type="button" className="option-card" onClick={() => openUrl(settings.create_account_url)}>
                  <IconBadge size="lg" tone="accent"><UserPlus size={24} /></IconBadge>
                  <h3>{t("profile.create_server")}</h3>
                  <p>{t("profile.create_server_desc")}</p>
                  <span className="go">{t("common.open")} <ArrowRight size={14} /></span>
                </button>
                <button type="button" className="option-card" onClick={() => setStep("offline")}>
                  <IconBadge size="lg" tone="success"><UserRound size={24} /></IconBadge>
                  <h3>{t("profile.offline")}</h3>
                  <p>{t("profile.offline_desc")}</p>
                  <span className="go">{t("common.continue")} <ArrowRight size={14} /></span>
                </button>
              </div>
            </>
          )}

          {step === "online" && (
            <div className="card elevated auth-card col">
              <div className="row" style={{ gap: ".8rem", marginBottom: ".4rem" }}>
                <IconBadge><Cloud size={20} /></IconBadge>
                <h2 style={{ margin: 0 }}>{t("profile.login_server")}</h2>
              </div>
              <p className="muted small" style={{ marginTop: 0 }}>{t("profile.login_server_desc")}</p>
              <Field label={t("profile.username")}>
                <input className="input" value={username} onChange={(e) => setUsername(e.target.value)} autoFocus autoComplete="username" />
              </Field>
              <Field label={t("profile.password")}>
                <input className="input" type="password" value={password} autoComplete="current-password" onChange={(e) => setPassword(e.target.value)} onKeyDown={(e) => e.key === "Enter" && loginOnline()} />
              </Field>
              <div className="row" style={{ justifyContent: "flex-end", marginTop: ".4rem" }}>
                <Button variant="ghost" onClick={() => setStep("index")}>{t("common.cancel")}</Button>
                <Button variant="primary" busy={busy} disabled={!username || !password} onClick={loginOnline}>
                  {t("common.continue")} <ArrowRight size={16} />
                </Button>
              </div>
            </div>
          )}

          {step === "offline" && (
            <div className="card elevated auth-card col">
              <div className="row" style={{ gap: ".8rem", marginBottom: ".4rem" }}>
                <IconBadge tone="success"><UserRound size={20} /></IconBadge>
                <h2 style={{ margin: 0 }}>{t("profile.offline")}</h2>
              </div>
              <Field label={t("profile.name")} hint={t("profile.name_public")}>
                <input className="input" value={name} onChange={(e) => setName(e.target.value)} autoFocus onKeyDown={(e) => e.key === "Enter" && name.trim() && offlineContinue()} />
              </Field>
              <div className="row" style={{ justifyContent: "flex-end", marginTop: ".4rem" }}>
                <Button variant="ghost" onClick={() => setStep("index")}>{t("common.cancel")}</Button>
                <Button variant="primary" disabled={!name.trim()} onClick={offlineContinue}>
                  {t("common.continue")} <ArrowRight size={16} />
                </Button>
              </div>
            </div>
          )}

          {step === "privacy" && (
            <Dialog title={t("profile.privacy_title")} narrow footer={<>
              <Button variant="ghost" onClick={() => setStep("index")}>{t("common.cancel")}</Button>
              <Button variant="primary" onClick={() => setStep("terms")}>{t("common.continue")}</Button>
            </>}>
              <p>{t("profile.privacy_body")}</p>
            </Dialog>
          )}

          {step === "terms" && (
            <div className="card elevated auth-card wide col">
              <h2>{t("profile.terms_title")}</h2>
              <p>{t("profile.terms_body")}</p>
              <div className="legal-links">
                <Button size="sm" onClick={() => setLegal("license")}>{t("profile.license")}</Button>
                <Button size="sm" onClick={() => setLegal("guidelines")}>{t("profile.guidelines")}</Button>
                <Button size="sm" onClick={() => setLegal("faith")}>{t("profile.faith")}</Button>
              </div>
              <div className="row" style={{ justifyContent: "flex-end", marginTop: ".3rem" }}>
                <Button variant="ghost" onClick={decline}>{t("profile.no_thanks")}</Button>
                <Button variant="primary" onClick={agree}>{t("profile.agree")}</Button>
              </div>
            </div>
          )}

          {legal && <LegalDialog kind={legal} onClose={() => setLegal(null)} />}
        </div>
      </div>
    </div>
  );
}

const LEGAL: Record<"license" | "guidelines" | "faith", { title: string; body: string[] }> = {
  license: {
    title: "Creative Commons Attribution-ShareAlike 4.0 International",
    body: [
      "Eres libre de compartir (copiar y redistribuir el material en cualquier medio o formato) y adaptar (remezclar, transformar y crear a partir del material) para cualquier propósito, incluso comercial.",
      "Atribución: debes dar crédito adecuado, indicar si se hicieron cambios y enlazar la licencia.",
      "CompartirIgual: si remezclas o transformas el material, debes distribuir tu contribución bajo la misma licencia.",
      "Texto completo: https://creativecommons.org/licenses/by-sa/4.0/",
    ],
  },
  guidelines: {
    title: "Pautas de traducción",
    body: [
      "La traducción debe ser exacta (comunicar fielmente el significado del original), clara (fácil de entender para la audiencia) y natural (usar el lenguaje como lo hablan las personas).",
      "La traducción debe ser fiel a la iglesia (afirmando la Declaración de fe), autoritativa (basada en el texto original en hebreo, arameo y griego), histórica (comunicando con precisión los acontecimientos y hechos históricos) e igual (comunicando la misma intención que el texto fuente, incluidas las expresiones de sentimientos y actitudes).",
      "El equipo de traducción debe trabajar en colaboración y someter su trabajo a revisión de la comunidad y de los líderes de la iglesia local.",
    ],
  },
  faith: {
    title: "Declaración de fe",
    body: [
      "Creemos que la Biblia es la única Palabra de Dios inspirada, inerrante, suficiente y autoritativa.",
      "Creemos que hay un solo Dios, eternamente existente en tres personas: Padre, Hijo y Espíritu Santo.",
      "Creemos en la deidad de Jesucristo, su nacimiento virginal, su vida sin pecado, sus milagros, su muerte expiatoria, su resurrección corporal y su regreso en poder y gloria.",
      "Creemos que la salvación es por gracia mediante la fe en Jesucristo, y en la resurrección de los salvos y de los perdidos.",
      "Creemos en la unidad espiritual de los creyentes en nuestro Señor Jesucristo y en la importancia de la iglesia local.",
      "Texto completo: https://wycliffeassociates.org/statement-of-faith/",
    ],
  },
};

export function LegalDialog({ kind, onClose }: { kind: "license" | "guidelines" | "faith"; onClose: () => void }) {
  const { t } = useT();
  const doc = LEGAL[kind];
  return (
    <Dialog title={doc.title} onClose={onClose} footer={<Button variant="primary" onClick={onClose}>{t("common.close")}</Button>}>
      {doc.body.map((p, i) => (
        <p key={i}>{p}</p>
      ))}
    </Dialog>
  );
}
