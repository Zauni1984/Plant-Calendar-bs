// Storage for browser-guest mode: the garden of someone using the web app
// without an account lives only in this browser, in IndexedDB.
//
// Nothing here leaves the device. Signing in offers to move it into the
// account (POST /me/import-guest); otherwise it stays until the user discards
// it or clears the site data.

const DB_NAME = "plants-calendar-guest";
const DB_VERSION = 1;
export const STORES = {
  plants: "plant_id",
  tasks: "task_id",
  logs: "log_id",
  locations: "location_id",
  seasons: "season_id",
};

let dbPromise = null;
// Used when IndexedDB is unavailable (some private modes, blocked storage):
// the app still works for the visit, it just forgets everything on reload.
let memory = null;

function request(req) {
  return new Promise((resolve, reject) => {
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

function openDb() {
  if (dbPromise) return dbPromise;
  dbPromise = new Promise((resolve) => {
    let req;
    try {
      req = indexedDB.open(DB_NAME, DB_VERSION);
    } catch {
      resolve(null);
      return;
    }
    req.onupgradeneeded = () => {
      const db = req.result;
      for (const [name, keyPath] of Object.entries(STORES)) {
        if (!db.objectStoreNames.contains(name)) db.createObjectStore(name, { keyPath });
      }
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => resolve(null);
    req.onblocked = () => resolve(null);
  }).then((db) => {
    if (!db) {
      console.warn("IndexedDB unavailable: guest data is kept for this visit only");
      memory = Object.fromEntries(Object.keys(STORES).map((s) => [s, new Map()]));
    }
    return db;
  });
  return dbPromise;
}

/** True when guest data survives a reload (IndexedDB works). */
export async function isPersistent() {
  return !!(await openDb());
}

export async function all(store) {
  const db = await openDb();
  if (!db) return [...memory[store].values()].map((v) => structuredClone(v));
  return request(db.transaction(store).objectStore(store).getAll());
}

export async function get(store, id) {
  const db = await openDb();
  if (!db) return structuredClone(memory[store].get(id) ?? null);
  return (await request(db.transaction(store).objectStore(store).get(id))) ?? null;
}

export async function put(store, value) {
  const db = await openDb();
  if (!db) {
    memory[store].set(value[STORES[store]], structuredClone(value));
    return value;
  }
  const tx = db.transaction(store, "readwrite");
  tx.objectStore(store).put(value);
  await new Promise((resolve, reject) => {
    tx.oncomplete = resolve;
    tx.onerror = () => reject(tx.error);
    tx.onabort = () => reject(tx.error);
  });
  return value;
}

export async function remove(store, id) {
  const db = await openDb();
  if (!db) {
    memory[store].delete(id);
    return;
  }
  const tx = db.transaction(store, "readwrite");
  tx.objectStore(store).delete(id);
  await new Promise((resolve, reject) => {
    tx.oncomplete = resolve;
    tx.onerror = () => reject(tx.error);
  });
}

/** Everything, for the move into an account. */
export async function snapshot() {
  const out = {};
  for (const store of Object.keys(STORES)) out[store] = await all(store);
  return out;
}

export async function plantCount() {
  return (await all("plants")).filter((p) => p.status !== "deleted").length;
}

export async function clearAll() {
  const db = await openDb();
  if (!db) {
    for (const map of Object.values(memory)) map.clear();
    return;
  }
  const names = Object.keys(STORES);
  const tx = db.transaction(names, "readwrite");
  for (const name of names) tx.objectStore(name).clear();
  await new Promise((resolve, reject) => {
    tx.oncomplete = resolve;
    tx.onerror = () => reject(tx.error);
  });
}
