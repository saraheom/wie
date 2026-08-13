export interface GameSaveSources {
  databases: string[];
  filesystemAids: string[];
}

interface SaveEntry {
  key: IDBValidKey;
  dataBase64: string;
}

interface SaveStoreSnapshot {
  name: string;
  entries: SaveEntry[];
}

interface SaveDatabaseSnapshot {
  name: string;
  stores: SaveStoreSnapshot[];
}

export interface SaveBackup {
  id: string;
  format: "wipi-player-save-v1";
  gameId: string;
  gameName: string;
  createdAt: number;
  sources: GameSaveSources;
  databases: SaveDatabaseSnapshot[];
}

const VAULT_DB = "wipi_player_save_vault";
const VAULT_VERSION = 1;
const BACKUPS_STORE = "backups";
const FILESYSTEM_DB = "wie_filesystem";

const openDatabase = (name: string): Promise<IDBDatabase> =>
  new Promise((resolve, reject) => {
    const request = indexedDB.open(name);
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
    request.onblocked = () => reject(new Error(`IndexedDB open blocked: ${name}`));
  });

const openVault = (): Promise<IDBDatabase> =>
  new Promise((resolve, reject) => {
    const request = indexedDB.open(VAULT_DB, VAULT_VERSION);
    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains(BACKUPS_STORE)) {
        const store = db.createObjectStore(BACKUPS_STORE, { keyPath: "id" });
        store.createIndex("gameId", "gameId", { unique: false });
      }
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });

const transactionDone = (transaction: IDBTransaction): Promise<void> =>
  new Promise((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onerror = () => reject(transaction.error);
    transaction.onabort = () => reject(transaction.error ?? new Error("IndexedDB transaction aborted"));
  });

const bytesToBase64 = (value: unknown): string => {
  let bytes: Uint8Array;
  if (value instanceof Uint8Array) bytes = value;
  else if (value instanceof ArrayBuffer) bytes = new Uint8Array(value);
  else if (ArrayBuffer.isView(value)) bytes = new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  else throw new Error(`Unsupported save value type: ${Object.prototype.toString.call(value)}`);

  let binary = "";
  const chunk = 0x8000;
  for (let offset = 0; offset < bytes.length; offset += chunk) {
    binary += String.fromCharCode(...bytes.subarray(offset, Math.min(offset + chunk, bytes.length)));
  }
  return btoa(binary);
};

const base64ToBytes = (value: string): Uint8Array => {
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
};

const requestValue = <T>(request: IDBRequest<T>): Promise<T> =>
  new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });

const keyMatchesFilesystemAid = (key: IDBValidKey, aids: Set<string>): boolean =>
  Array.isArray(key) && typeof key[0] === "string" && aids.has(key[0]);

const snapshotDatabase = async (
  dbName: string,
  filesystemAids: Set<string>
): Promise<SaveDatabaseSnapshot | undefined> => {
  const db = await openDatabase(dbName);
  try {
    const stores: SaveStoreSnapshot[] = [];
    for (const storeName of Array.from(db.objectStoreNames)) {
      const tx = db.transaction(storeName, "readonly");
      const store = tx.objectStore(storeName);
      const keysRequest = store.getAllKeys();
      const valuesRequest = store.getAll();
      const [keys, values] = await Promise.all([requestValue(keysRequest), requestValue(valuesRequest)]);
      await transactionDone(tx);
      const entries: SaveEntry[] = [];

      for (let index = 0; index < keys.length; index++) {
        const key = keys[index];
        if (dbName === FILESYSTEM_DB && !keyMatchesFilesystemAid(key, filesystemAids)) continue;
        const value = values[index];
        if (value === undefined) continue;
        entries.push({ key, dataBase64: bytesToBase64(value) });
      }

      stores.push({ name: storeName, entries });
    }
    return { name: dbName, stores };
  } finally {
    db.close();
  }
};

const clearSnapshotScope = async (dbName: string, sources: GameSaveSources): Promise<void> => {
  const db = await openDatabase(dbName);
  try {
    const aids = new Set(sources.filesystemAids);
    for (const storeName of Array.from(db.objectStoreNames)) {
      if (dbName !== FILESYSTEM_DB) {
        const tx = db.transaction(storeName, "readwrite");
        tx.objectStore(storeName).clear();
        await transactionDone(tx);
        continue;
      }

      // Read the shared filesystem keys first, then delete only this game's AID namespace.
      const readTx = db.transaction(storeName, "readonly");
      const keys = await requestValue(readTx.objectStore(storeName).getAllKeys());
      await transactionDone(readTx);

      const deleteTx = db.transaction(storeName, "readwrite");
      const deleteStore = deleteTx.objectStore(storeName);
      for (const key of keys) {
        if (keyMatchesFilesystemAid(key, aids)) deleteStore.delete(key);
      }
      await transactionDone(deleteTx);
    }
  } finally {
    db.close();
  }
};

export const normalizeSaveSources = (sources?: Partial<GameSaveSources>): GameSaveSources => ({
  databases: Array.from(new Set((sources?.databases ?? []).filter((name) => name.startsWith("wie_") && name !== FILESYSTEM_DB))),
  filesystemAids: Array.from(new Set((sources?.filesystemAids ?? []).filter(Boolean))),
});

export const hasSaveSources = (sources: GameSaveSources): boolean =>
  sources.databases.length > 0 || sources.filesystemAids.length > 0;

export const createSaveBackup = async (
  gameId: string,
  gameName: string,
  sources: GameSaveSources
): Promise<SaveBackup> => {
  const normalized = normalizeSaveSources(sources);
  const names = [...normalized.databases];
  if (normalized.filesystemAids.length > 0) names.push(FILESYSTEM_DB);

  const snapshots: SaveDatabaseSnapshot[] = [];
  for (const name of Array.from(new Set(names))) {
    const snapshot = await snapshotDatabase(name, new Set(normalized.filesystemAids));
    if (snapshot) snapshots.push(snapshot);
  }

  const createdAt = Date.now();
  const backup: SaveBackup = {
    id: `${gameId}-${createdAt}-${crypto.randomUUID?.() ?? Math.random().toString(16).slice(2)}`,
    format: "wipi-player-save-v1",
    gameId,
    gameName,
    createdAt,
    sources: normalized,
    databases: snapshots,
  };

  const vault = await openVault();
  try {
    const tx = vault.transaction(BACKUPS_STORE, "readwrite");
    tx.objectStore(BACKUPS_STORE).put(backup);
    await transactionDone(tx);
  } finally {
    vault.close();
  }
  return backup;
};

export const listSaveBackups = async (gameId: string): Promise<SaveBackup[]> => {
  const vault = await openVault();
  try {
    const tx = vault.transaction(BACKUPS_STORE, "readonly");
    const index = tx.objectStore(BACKUPS_STORE).index("gameId");
    const rows = await requestValue(index.getAll(IDBKeyRange.only(gameId))) as SaveBackup[];
    await transactionDone(tx);
    return rows.sort((a, b) => b.createdAt - a.createdAt);
  } finally {
    vault.close();
  }
};

export const deleteSaveBackup = async (id: string): Promise<void> => {
  const vault = await openVault();
  try {
    const tx = vault.transaction(BACKUPS_STORE, "readwrite");
    tx.objectStore(BACKUPS_STORE).delete(id);
    await transactionDone(tx);
  } finally {
    vault.close();
  }
};

export const restoreSaveBackup = async (backup: SaveBackup): Promise<void> => {
  if (backup.format !== "wipi-player-save-v1") throw new Error("Unsupported save backup format");

  for (const snapshot of backup.databases) {
    await clearSnapshotScope(snapshot.name, backup.sources);
    const db = await openDatabase(snapshot.name);
    try {
      for (const storeSnapshot of snapshot.stores) {
        if (!db.objectStoreNames.contains(storeSnapshot.name)) continue;
        const tx = db.transaction(storeSnapshot.name, "readwrite");
        const store = tx.objectStore(storeSnapshot.name);
        for (const entry of storeSnapshot.entries) {
          store.put(base64ToBytes(entry.dataBase64), entry.key);
        }
        await transactionDone(tx);
      }
    } finally {
      db.close();
    }
  }
};

export const eraseGameSaveData = async (sources: GameSaveSources): Promise<void> => {
  const normalized = normalizeSaveSources(sources);
  for (const name of normalized.databases) await clearSnapshotScope(name, normalized);
  if (normalized.filesystemAids.length > 0) await clearSnapshotScope(FILESYSTEM_DB, normalized);
};

export const exportSaveBackup = async (backup: SaveBackup): Promise<void> => {
  const text = JSON.stringify(backup, null, 2);
  const safeName = backup.gameName.replace(/[^\p{L}\p{N}._-]+/gu, "-").replace(/^-+|-+$/g, "") || "game";
  const stamp = new Date(backup.createdAt).toISOString().replace(/[:.]/g, "-");
  const fileName = `${safeName}-save-${stamp}.wipisave.json`;
  const file = new File([text], fileName, { type: "application/json" });

  const navigatorWithShare = navigator as Navigator & { canShare?: (data: ShareData) => boolean };
  if (navigator.share && (!navigatorWithShare.canShare || navigatorWithShare.canShare({ files: [file] }))) {
    await navigator.share({ title: `${backup.gameName} save backup`, files: [file] });
    return;
  }

  const url = URL.createObjectURL(file);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = fileName;
  anchor.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
};

export const parseImportedSaveBackup = async (file: File): Promise<SaveBackup> => {
  const parsed = JSON.parse(await file.text()) as SaveBackup;
  if (parsed?.format !== "wipi-player-save-v1" || !parsed.gameId || !Array.isArray(parsed.databases)) {
    throw new Error("This is not a WIPI Player save backup file.");
  }
  parsed.sources = normalizeSaveSources(parsed.sources);
  return parsed;
};

export const storeImportedSaveBackup = async (backup: SaveBackup, gameId: string, gameName: string): Promise<SaveBackup> => {
  const imported: SaveBackup = {
    ...backup,
    id: `${gameId}-${Date.now()}-${crypto.randomUUID?.() ?? Math.random().toString(16).slice(2)}`,
    gameId,
    gameName,
    createdAt: Date.now(),
  };
  const vault = await openVault();
  try {
    const tx = vault.transaction(BACKUPS_STORE, "readwrite");
    tx.objectStore(BACKUPS_STORE).put(imported);
    await transactionDone(tx);
  } finally {
    vault.close();
  }
  return imported;
};
