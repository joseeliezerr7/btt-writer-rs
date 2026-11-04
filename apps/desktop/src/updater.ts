// Actualización automática (solo en la app de escritorio): consulta latest.json en GitHub Releases,
// descarga el instalador firmado y reinicia la app.
import { isTauri } from "./api";

export interface UpdateInfo {
  version: string;
  currentVersion: string;
  notes: string;
  date: string;
  /** Descarga e instala; `onProgress` recibe el porcentaje (0-100) cuando se conoce el tamaño. */
  install: (onProgress?: (percent: number | null) => void) => Promise<void>;
}

/** Devuelve la actualización disponible, o null si ya estamos al día (o no es la app nativa). */
export async function checkForUpdate(): Promise<UpdateInfo | null> {
  if (!isTauri) return null;
  const { check } = await import("@tauri-apps/plugin-updater");
  const update = await check({ timeout: 15000 });
  if (!update) return null;
  return {
    version: update.version,
    currentVersion: update.currentVersion,
    notes: update.body ?? "",
    date: update.date ?? "",
    install: async (onProgress) => {
      let total = 0;
      let received = 0;
      await update.downloadAndInstall((ev) => {
        if (ev.event === "Started") {
          total = ev.data.contentLength ?? 0;
          onProgress?.(total ? 0 : null);
        } else if (ev.event === "Progress") {
          received += ev.data.chunkLength;
          onProgress?.(total ? Math.min(100, Math.round((received / total) * 100)) : null);
        } else if (ev.event === "Finished") {
          onProgress?.(100);
        }
      });
    },
  };
}

/** Reinicia la app (tras instalar una actualización). */
export async function relaunchApp(): Promise<void> {
  if (!isTauri) return;
  const { relaunch } = await import("@tauri-apps/plugin-process");
  await relaunch();
}
