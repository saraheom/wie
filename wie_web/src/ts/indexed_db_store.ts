export class IndexedDBStore {
  private db: IDBDatabase;
  private store_name: string;
  private static pendingWrites = new Set<Promise<void>>();

  private static trackWrite(promise: Promise<void>): Promise<void> {
    IndexedDBStore.pendingWrites.add(promise);
    void promise.then(
      () => IndexedDBStore.pendingWrites.delete(promise),
      () => IndexedDBStore.pendingWrites.delete(promise)
    );
    return promise;
  }

  public static getPendingWriteCount(): number {
    return IndexedDBStore.pendingWrites.size;
  }

  public static async flushPendingWrites(): Promise<void> {
    while (IndexedDBStore.pendingWrites.size > 0) {
      const writes = Array.from(IndexedDBStore.pendingWrites);
      await Promise.allSettled(writes);
      if (writes.length === 0) break;
    }
  }

  private constructor(db: IDBDatabase, store_name: string) {
    this.db = db;
    this.store_name = store_name;
  }

  private notifyStorageAccess(key?: IDBValidKey): void {
    if (!this.db.name.startsWith("wie_")) return;
    window.dispatchEvent(
      new CustomEvent("wie-save-storage-access", {
        detail: { dbName: this.db.name, storeName: this.store_name, key },
      })
    );
  }

  public static open(db_name: string, store_name: string): Promise<IndexedDBStore> {
    return new Promise((resolve, reject) => {
      const request = indexedDB.open(db_name);

      request.onupgradeneeded = (event) => {
        const db = (event.target as IDBOpenDBRequest).result;
        if (!db.objectStoreNames.contains(store_name)) {
          db.createObjectStore(store_name);
        }
      };

      request.onsuccess = (event) => {
        const db = (event.target as IDBOpenDBRequest).result;
        const store = new IndexedDBStore(db, store_name);
        if (db_name.startsWith("wie_") && db_name !== "wie_filesystem") {
          store.notifyStorageAccess();
        }
        resolve(store);
      };

      request.onerror = (event) => {
        reject((event.target as IDBOpenDBRequest).error);
      };
    });
  }

  public get_all_keys(): Promise<IDBValidKey[]> {
    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction(this.store_name, "readonly");
      const store = transaction.objectStore(this.store_name);
      const request = store.getAllKeys();

      request.onsuccess = () => {
        resolve(request.result);
      };

      request.onerror = () => {
        reject(request.error);
      };
    });
  }

  public get(key: IDBValidKey): Promise<Uint8Array | undefined> {
    this.notifyStorageAccess(key);
    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction(this.store_name, "readonly");
      const store = transaction.objectStore(this.store_name);
      const request = store.get(key);

      request.onsuccess = () => {
        resolve(request.result as Uint8Array | undefined);
      };

      request.onerror = () => {
        reject(request.error);
      };
    });
  }

  public set(key: IDBValidKey, data: Uint8Array): Promise<void> {
    this.notifyStorageAccess(key);
    const startedAt = performance.now();
    const write = new Promise<void>((resolve, reject) => {
      const transaction = this.db.transaction(this.store_name, "readwrite");
      const store = transaction.objectStore(this.store_name);
      const request = store.put(data, key);

      request.onerror = () => reject(request.error ?? new Error("IndexedDB put failed"));
      transaction.oncomplete = () => {
        window.dispatchEvent(new CustomEvent("wie-save-write-committed", {
          detail: { dbName: this.db.name, storeName: this.store_name, key, bytes: data.byteLength, ms: Math.round(performance.now() - startedAt) },
        }));
        resolve();
      };
      transaction.onerror = () => reject(transaction.error ?? new Error("IndexedDB write transaction failed"));
      transaction.onabort = () => reject(transaction.error ?? new Error("IndexedDB write transaction aborted"));
    });
    return IndexedDBStore.trackWrite(write);
  }

  public delete(key: IDBValidKey): Promise<void> {
    this.notifyStorageAccess(key);
    const write = new Promise<void>((resolve, reject) => {
      const transaction = this.db.transaction(this.store_name, "readwrite");
      const store = transaction.objectStore(this.store_name);
      const request = store.delete(key);

      request.onerror = () => reject(request.error ?? new Error("IndexedDB delete failed"));
      transaction.oncomplete = () => resolve();
      transaction.onerror = () => reject(transaction.error ?? new Error("IndexedDB delete transaction failed"));
      transaction.onabort = () => reject(transaction.error ?? new Error("IndexedDB delete transaction aborted"));
    });
    return IndexedDBStore.trackWrite(write);
  }
}
