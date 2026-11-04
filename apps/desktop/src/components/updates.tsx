// Aviso de versión nueva: comprobación automática al arrancar y botón manual en Ajustes.
import { useEffect, useRef } from "react";
import { isTauri } from "../api";
import { useT } from "../i18n";
import { checkForUpdate, relaunchApp } from "../updater";
import { useFeedback } from "./ui";

/** Devuelve una función que busca actualizaciones y guía al usuario (confirmar, descargar, reiniciar). */
export function useUpdateCheck() {
  const fb = useFeedback();
  const { t } = useT();
  return async (manual: boolean) => {
    if (!isTauri) {
      if (manual) fb.toast(t("update.not_available_here"));
      return;
    }
    try {
      const update = await checkForUpdate();
      if (!update) {
        if (manual) fb.toast(t("update.up_to_date"), "success");
        return;
      }
      const ok = await fb.confirm(
        t("update.available_title", { version: update.version }),
        <div className="col">
          <p>{t("update.available_body", { current: update.currentVersion })}</p>
          {update.notes && <pre className="muted small" style={{ whiteSpace: "pre-wrap", maxHeight: 180, overflow: "auto" }}>{update.notes}</pre>}
        </div>,
        { okLabel: t("update.install_now"), cancelLabel: t("update.later") },
      );
      if (!ok) return;
      fb.toast(t("update.downloading"));
      await update.install();
      const restart = await fb.confirm(t("update.installed_title"), t("update.installed_body"), { okLabel: t("update.restart"), cancelLabel: t("update.later") });
      if (restart) await relaunchApp();
    } catch (e) {
      if (manual) fb.error(e);
      else console.warn("comprobación de actualizaciones:", e);
    }
  };
}

/** Comprueba una vez, unos segundos después de arrancar, sin molestar si no hay nada nuevo. */
export function AutoUpdateCheck() {
  const run = useUpdateCheck();
  const started = useRef(false);
  useEffect(() => {
    if (started.current) return;
    started.current = true;
    const id = setTimeout(() => run(false), 6000);
    return () => clearTimeout(id);
  }, []);
  return null;
}
