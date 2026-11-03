// Pantalla de perfil: cuenta del servidor, cuenta sin conexión y términos de uso.
import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Cloud, UserPlus, UserRound } from "lucide-react";
import { api } from "../api";
import { useApp } from "../app-context";
import { useT } from "../i18n";
import { Button, Dialog, Field, useFeedback } from "../components/ui";

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
    <div className="screen">
      <div className="topbar">
        <h1>{t("app.name")}</h1>
        <span className="grow" />
        <Button variant="ghost" onClick={() => navigate({ name: "settings" })}>
          {t("home.settings")}
        </Button>
      </div>
      <div className="content">
        <div className="container" style={{ maxWidth: 820, paddingTop: "2rem" }}>
          {step === "index" && (
            <>
              <h2 className="center">{t("profile.title")}</h2>
              <div className="profile-options" style={{ marginTop: "1.5rem" }}>
                <div className="card" onClick={() => setStep("online")}>
                  <Cloud size={28} />
                  <h3>{t("profile.login_server")}</h3>
                  <p className="muted small">{t("profile.login_server_desc")}</p>
                </div>
                <div className="card" onClick={() => openUrl(settings.create_account_url)}>
                  <UserPlus size={28} />
                  <h3>{t("profile.create_server")}</h3>
                  <p className="muted small">{t("profile.create_server_desc")}</p>
                </div>
                <div className="card" onClick={() => setStep("offline")}>
                  <UserRound size={28} />
                  <h3>{t("profile.offline")}</h3>
                  <p className="muted small">{t("profile.offline_desc")}</p>
                </div>
              </div>
            </>
          )}

          {step === "online" && (
            <div className="card pad col" style={{ maxWidth: 460, margin: "0 auto" }}>
              <h2>{t("profile.login_server")}</h2>
              <Field label={t("profile.username")}>
                <input className="input" value={username} onChange={(e) => setUsername(e.target.value)} autoFocus />
              </Field>
              <Field label={t("profile.password")}>
                <input className="input" type="password" value={password} onChange={(e) => setPassword(e.target.value)} onKeyDown={(e) => e.key === "Enter" && loginOnline()} />
              </Field>
              <div className="row" style={{ justifyContent: "flex-end" }}>
                <Button onClick={() => setStep("index")}>{t("common.cancel")}</Button>
                <Button variant="primary" busy={busy} disabled={!username || !password} onClick={loginOnline}>
                  {t("common.continue")}
                </Button>
              </div>
            </div>
          )}

          {step === "offline" && (
            <div className="card pad col" style={{ maxWidth: 460, margin: "0 auto" }}>
              <h2>{t("profile.offline")}</h2>
              <Field label={t("profile.name")} hint={t("profile.name_public")}>
                <input className="input" value={name} onChange={(e) => setName(e.target.value)} autoFocus onKeyDown={(e) => e.key === "Enter" && name.trim() && offlineContinue()} />
              </Field>
              <div className="row" style={{ justifyContent: "flex-end" }}>
                <Button onClick={() => setStep("index")}>{t("common.cancel")}</Button>
                <Button variant="primary" disabled={!name.trim()} onClick={offlineContinue}>
                  {t("common.continue")}
                </Button>
              </div>
            </div>
          )}

          {step === "privacy" && (
            <Dialog title={t("profile.privacy_title")} narrow footer={<>
              <Button onClick={() => setStep("index")}>{t("common.cancel")}</Button>
              <Button variant="primary" onClick={() => setStep("terms")}>{t("common.continue")}</Button>
            </>}>
              <p>{t("profile.privacy_body")}</p>
            </Dialog>
          )}

          {step === "terms" && (
            <div className="card pad col" style={{ maxWidth: 640, margin: "0 auto" }}>
              <h2>{t("profile.terms_title")}</h2>
              <p>{t("profile.terms_body")}</p>
              <div className="row wrap">
                <Button size="sm" onClick={() => setLegal("license")}>{t("profile.license")}</Button>
                <Button size="sm" onClick={() => setLegal("guidelines")}>{t("profile.guidelines")}</Button>
                <Button size="sm" onClick={() => setLegal("faith")}>{t("profile.faith")}</Button>
              </div>
              <div className="row" style={{ justifyContent: "flex-end", marginTop: ".5rem" }}>
                <Button onClick={decline}>{t("profile.no_thanks")}</Button>
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
