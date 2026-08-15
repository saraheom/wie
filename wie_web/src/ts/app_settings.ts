import { GameDisplayMode, GameOrientation } from "./game_library";

export type LibrarySortMode = "recent" | "name" | "favorites";
export type AppLanguage = "en";
export type LibrarySortDirection = "asc" | "desc";

export interface AppSettings {
  librarySort: LibrarySortMode;
  librarySortDirection: LibrarySortDirection;
  defaultOrientation: GameOrientation;
  defaultDisplayMode: GameDisplayMode;
  language: AppLanguage;
}

const STORAGE_KEY = "wipi_player_app_settings_v1";

export const defaultAppSettings = (): AppSettings => ({
  librarySort: "recent",
  librarySortDirection: "desc",
  defaultOrientation: "portrait",
  defaultDisplayMode: "fit",
  language: "en",
});

export const loadAppSettings = (): AppSettings => {
  const defaults = defaultAppSettings();
  try {
    const parsed = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}") as Partial<AppSettings>;
    return {
      librarySort:
        parsed.librarySort === "name" || parsed.librarySort === "favorites" || parsed.librarySort === "recent"
          ? parsed.librarySort
          : defaults.librarySort,
      librarySortDirection: parsed.librarySortDirection === "asc" ? "asc" : "desc",
      defaultOrientation: parsed.defaultOrientation === "landscape" ? "landscape" : "portrait",
      defaultDisplayMode:
        parsed.defaultDisplayMode === "native" ||
        parsed.defaultDisplayMode === "compact" ||
        parsed.defaultDisplayMode === "fit" ||
        parsed.defaultDisplayMode === "large" ||
        parsed.defaultDisplayMode === "max"
          ? parsed.defaultDisplayMode
          : defaults.defaultDisplayMode,
      language: "en",
    };
  } catch {
    return defaults;
  }
};

export const saveAppSettings = (settings: AppSettings) => {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
};
