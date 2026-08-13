export type GameOrientation = "portrait" | "landscape";
export type GameDisplayMode = "native" | "compact" | "fit" | "large" | "max";
export type GameControlPreset = "classic";

export interface GameSettings {
  orientation: GameOrientation;
  displayMode: GameDisplayMode;
  controlPreset: GameControlPreset;
}

export interface GameRecord {
  id: string;
  name: string;
  fileName: string;
  archive: ArrayBuffer;
  cover?: Blob;
  createdAt: number;
  lastPlayedAt?: number;
  settings: GameSettings;
}

const DB_NAME = "wipi_player_library";
const DB_VERSION = 1;
const STORE_NAME = "games";

const openDatabase = (): Promise<IDBDatabase> =>
  new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION);

    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains(STORE_NAME)) {
        db.createObjectStore(STORE_NAME, { keyPath: "id" });
      }
    };

    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });

const transactionDone = (transaction: IDBTransaction): Promise<void> =>
  new Promise((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onerror = () => reject(transaction.error);
    transaction.onabort = () => reject(transaction.error);
  });

export const defaultGameSettings = (): GameSettings => ({
  orientation: "portrait",
  displayMode: "fit",
  controlPreset: "classic",
});

const normalizeSettings = (settings?: Partial<GameSettings>): GameSettings => {
  const defaults = defaultGameSettings();
  const orientation =
    settings?.orientation === "landscape" || settings?.orientation === "portrait"
      ? settings.orientation
      : defaults.orientation;

  const validDisplayModes: GameDisplayMode[] = ["native", "compact", "fit", "large", "max"];
  const displayMode = validDisplayModes.includes(settings?.displayMode as GameDisplayMode)
    ? (settings?.displayMode as GameDisplayMode)
    : defaults.displayMode;

  return {
    orientation,
    displayMode,
    controlPreset: "classic",
  };
};

const normalizeGame = (game: GameRecord): GameRecord => ({
  ...game,
  settings: normalizeSettings(game.settings),
});

export class GameLibrary {
  private constructor(private readonly db: IDBDatabase) {}

  public static async open(): Promise<GameLibrary> {
    return new GameLibrary(await openDatabase());
  }

  public async list(): Promise<GameRecord[]> {
    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction(STORE_NAME, "readonly");
      const request = transaction.objectStore(STORE_NAME).getAll();

      request.onsuccess = () => {
        const games = (request.result as GameRecord[])
          .map(normalizeGame)
          .sort((a, b) => {
            const aTime = a.lastPlayedAt ?? a.createdAt;
            const bTime = b.lastPlayedAt ?? b.createdAt;
            return bTime - aTime;
          });
        resolve(games);
      };
      request.onerror = () => reject(request.error);
    });
  }

  public async get(id: string): Promise<GameRecord | undefined> {
    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction(STORE_NAME, "readonly");
      const request = transaction.objectStore(STORE_NAME).get(id);
      request.onsuccess = () => {
        const result = request.result as GameRecord | undefined;
        resolve(result ? normalizeGame(result) : undefined);
      };
      request.onerror = () => reject(request.error);
    });
  }

  public async put(game: GameRecord): Promise<void> {
    const transaction = this.db.transaction(STORE_NAME, "readwrite");
    transaction.objectStore(STORE_NAME).put(normalizeGame(game));
    await transactionDone(transaction);
  }

  public async delete(id: string): Promise<void> {
    const transaction = this.db.transaction(STORE_NAME, "readwrite");
    transaction.objectStore(STORE_NAME).delete(id);
    await transactionDone(transaction);
  }
}

export const gameIdForArchive = async (data: ArrayBuffer): Promise<string> => {
  if (crypto?.subtle) {
    const digest = await crypto.subtle.digest("SHA-256", data);
    return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("");
  }

  let hash = 2166136261;
  for (const byte of new Uint8Array(data)) {
    hash ^= byte;
    hash = Math.imul(hash, 16777619);
  }
  return `fnv1a-${(hash >>> 0).toString(16).padStart(8, "0")}-${data.byteLength}`;
};

export const displayNameForFile = (fileName: string): string => {
  const withoutExtension = fileName.replace(/\.(zip|jar)$/i, "").trim();
  return withoutExtension || "Untitled Game";
};
