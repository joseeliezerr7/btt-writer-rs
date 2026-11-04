// Contexto global: ajustes, perfil, info de la app y navegación.
import { createContext, useContext } from "react";
import type { AppInfo, Profile, Settings } from "./api";

export type View =
  | { name: "loading" }
  | { name: "profile" }
  | { name: "home" }
  | { name: "translate"; id: string; chapter?: string; chunk?: string; mode?: "read" | "chunk" | "review" }
  | { name: "settings" }
  | { name: "publish"; id: string };

export interface AppCtx {
  info: AppInfo | null;
  settings: Settings;
  profile: Profile;
  view: View;
  navigate: (v: View) => void;
  updateSettings: (patch: Partial<Settings>) => Promise<void>;
  setProfile: (p: Profile) => void;
  reloadProfile: () => Promise<void>;
  reloadSettings: () => Promise<void>;
}

export const AppContext = createContext<AppCtx>(null as unknown as AppCtx);
export const useApp = () => useContext(AppContext);

export const FONT_SIZES: { value: number; key: string }[] = [
  { value: 15, key: "settings.size.small" },
  { value: 25, key: "settings.size.normal" },
  { value: 35, key: "settings.size.medium" },
  { value: 45, key: "settings.size.large" },
];

/** Convierte el tamaño "sp" del original a píxeles razonables de pantalla. */
export const spToPx = (sp: number) => Math.round(12 + sp * 0.45);
