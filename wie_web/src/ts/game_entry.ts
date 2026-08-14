import { GameRecord, normalizeImportedGame } from "./game_library";

interface PortableGameEntryV1 {
  format: "wipi-player-game-v1";
  exportedAt: number;
  game: {
    id: string;
    name: string;
    fileName: string;
    archiveBase64: string;
    coverBase64?: string;
    coverType?: string;
    createdAt: number;
    lastPlayedAt?: number;
    favorite?: boolean;
    settings: GameRecord["settings"];
  };
}

const bytesToBase64 = (bytes: Uint8Array): string => {
  let binary = "";
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  return btoa(binary);
};

const base64ToBytes = (value: string): Uint8Array => {
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  return bytes;
};

const base64ToArrayBuffer = (value: string): ArrayBuffer => {
  const bytes = base64ToBytes(value);
  const copy = new Uint8Array(bytes.byteLength);
  copy.set(bytes);
  return copy.buffer;
};

const blobToBase64 = async (blob: Blob): Promise<string> => bytesToBase64(new Uint8Array(await blob.arrayBuffer()));

export const exportGameEntry = async (game: GameRecord): Promise<void> => {
  const portable: PortableGameEntryV1 = {
    format: "wipi-player-game-v1",
    exportedAt: Date.now(),
    game: {
      id: game.id,
      name: game.name,
      fileName: game.fileName,
      archiveBase64: bytesToBase64(new Uint8Array(game.archive)),
      coverBase64: game.cover ? await blobToBase64(game.cover) : undefined,
      coverType: game.cover?.type,
      createdAt: game.createdAt,
      lastPlayedAt: game.lastPlayedAt,
      favorite: game.favorite,
      settings: game.settings,
    },
  };

  const safeName = game.name.replace(/[^\p{L}\p{N}._-]+/gu, "-").replace(/^-+|-+$/g, "") || "game";
  const file = new File([JSON.stringify(portable)], `${safeName}.wipigame.json`, { type: "application/json" });
  const shareNavigator = navigator as Navigator & { canShare?: (data: ShareData) => boolean };

  if (navigator.share && (!shareNavigator.canShare || shareNavigator.canShare({ files: [file] }))) {
    await navigator.share({ title: `${game.name} — WIPI Player`, files: [file] });
    return;
  }

  const url = URL.createObjectURL(file);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = file.name;
  anchor.click();
  setTimeout(() => URL.revokeObjectURL(url), 1500);
};

export const parseGameEntry = async (file: File): Promise<GameRecord> => {
  const parsed = JSON.parse(await file.text()) as PortableGameEntryV1;
  if (parsed?.format !== "wipi-player-game-v1" || !parsed.game?.id || !parsed.game.archiveBase64) {
    throw new Error("This is not a WIPI Player game-entry file.");
  }

  const archiveBuffer = base64ToArrayBuffer(parsed.game.archiveBase64);
  const cover = parsed.game.coverBase64
    ? new Blob([base64ToArrayBuffer(parsed.game.coverBase64)], { type: parsed.game.coverType || "image/jpeg" })
    : undefined;

  return normalizeImportedGame({
    id: parsed.game.id,
    name: parsed.game.name,
    fileName: parsed.game.fileName,
    archive: archiveBuffer,
    cover,
    createdAt: parsed.game.createdAt || Date.now(),
    lastPlayedAt: parsed.game.lastPlayedAt,
    favorite: Boolean(parsed.game.favorite),
    settings: parsed.game.settings,
    saveSources: { databases: [], filesystemAids: [] },
  } as GameRecord);
};
