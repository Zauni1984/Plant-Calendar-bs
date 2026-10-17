// Browser implementations of the Tauri commands the frontend calls.
//
// Every handler receives the exact argument object main.js passes to
// invoke() — camelCase for direct parameters, `{ payload: {...} }` with
// snake_case fields where the Rust command takes a struct — and returns the
// shape the Rust command returns. That is what lets main.js run unchanged in
// both builds.
//
// Coverage
//   server-backed .......... account, plants, tasks, logs, locations (with
//                            coordinates), seasons, consents, subscription,
//                            export, climate settings, weather history,
//                            photos (paid plans; converted to WebP server-side)
//   browser-side ........... forecasts (Open-Meteo is called directly; the
//                            current reading is then posted to the history),
//                            reminders, file import/export (download/upload)
//   desktop-only ........... Bluetooth, Tuya, device management — browsers
//                            cannot reach LAN devices, and iOS has no Web
//                            Bluetooth at all
//   not on the web ......... local profiles (a desktop multi-user feature)
//   guest mode ............. without an account, plants/tasks/logs/locations/
//                            seasons live in this browser (guest-api.js) and
//                            can later be moved into an account

import { api, apiBlob, apiUpload, authenticate, logout, refreshSession, getSession, hasSession, API_BASE_URL } from "./http.js";
import { takeUpload, takeUploadFile, triggerDownload } from "./web-files.js";
import { guestApi, exportForImport } from "./guest-api.js";
import * as guestStore from "./guest-store.js";

export const DESKTOP_ONLY = "desktop-only";

const desktopOnly = (feature) => () => {
  throw `${DESKTOP_ONLY}: ${feature} ist nur in der Desktop-App verfügbar.`;
};
const notYet = (feature) => () => {
  throw `${feature} ist in der Web-Version noch nicht verfügbar.`;
};
const requireSession = () => {
  if (!hasSession()) throw "Bitte melde dich an, um diese Funktion zu nutzen.";
};
/** List reads degrade to an empty list while signed out, so the UI can render. */
const listOrEmpty = (fn) => async (args) => (hasSession() ? fn(args) : []);

// --- guest mode ------------------------------------------------------------------
// Chosen explicitly ("try without an account"). A session always wins: once
// signed in, the data endpoints go to the server again.
const GUEST_MODE_KEY = "pc_guest_mode";
function isGuest() {
  if (hasSession()) return false;
  try { return localStorage.getItem(GUEST_MODE_KEY) === "1"; } catch { return false; }
}
function setGuestFlag(on) {
  try { on ? localStorage.setItem(GUEST_MODE_KEY, "1") : localStorage.removeItem(GUEST_MODE_KEY); } catch { /* storage unavailable */ }
}
/** Garden data: the account's, or this browser's in guest mode. */
const dataApi = (path, opts) => (isGuest() ? guestApi(path, opts) : api(path, opts));
const requireData = () => {
  if (!hasSession() && !isGuest()) throw "Bitte melde dich an, um diese Funktion zu nutzen.";
};
const dataListOrEmpty = (fn) => async (args) => (hasSession() || isGuest() ? fn(args) : []);

const enc = encodeURIComponent;
const lower = (v, fallback) => (v == null || v === "" ? fallback : String(v).toLowerCase());

// --- helpers mirroring the Rust side ------------------------------------------

/** Same as the desktop's parse_due_at: accept RFC3339 or YYYY-MM-DD, else default. */
function parseDueAt(raw, fallback) {
  if (!raw) return fallback.toISOString();
  const d = /^\d{4}-\d{2}-\d{2}$/.test(raw) ? new Date(`${raw}T00:00:00`) : new Date(raw);
  return Number.isNaN(d.getTime()) ? fallback.toISOString() : d.toISOString();
}

/** Same as resolve_repeat_until: explicit date (end of that day) or due + 4 weeks. */
function resolveRepeatUntil(raw, dueAtIso, repeatHours) {
  if (raw) {
    const d = /^\d{4}-\d{2}-\d{2}$/.test(raw) ? new Date(`${raw}T23:59:59`) : new Date(raw);
    return Number.isNaN(d.getTime()) ? null : d.toISOString();
  }
  if (repeatHours && repeatHours > 0) {
    return new Date(new Date(dueAtIso).getTime() + 28 * 24 * 3600 * 1000).toISOString();
  }
  return null;
}

const METRIC_FIELDS = ["ph", "ec", "temp_c", "rh", "harvest_kg", "harvest_count", "harvest_quality", "water_liters", "water_method"];

/** The desktop sends a metrics object only when at least one metric is set. */
function metricsFrom(p) {
  if (!METRIC_FIELDS.some((f) => p[f] != null && p[f] !== "")) return null;
  const m = { vpd: null };
  for (const f of METRIC_FIELDS) m[f] = p[f] ?? null;
  return m;
}

function sessionView(me) {
  const s = getSession();
  const now = new Date().toISOString();
  // Tokens are deliberately left blank: the frontend logs this object, and the
  // access token has no business appearing in a debug panel.
  return {
    session: {
      session_key: "web",
      user_id: s?.user_id ?? null,
      auth_mode: s?.auth_mode ?? null,
      access_token: "",
      refresh_token: "",
      backend_url: API_BASE_URL,
      created_at: now,
      updated_at: now,
    },
    me,
  };
}

// Consents given before sign-in are kept here and uploaded after sign-in, so
// the consent dialog on a first visit works for anonymous visitors too.
const PENDING_CONSENTS_KEY = "pc_web_pending_consents";
const POLICY_VERSION = "1.0";

function readPendingConsents() {
  try { return JSON.parse(localStorage.getItem(PENDING_CONSENTS_KEY) || "{}"); } catch { return {}; }
}
function writePendingConsents(value) {
  try { localStorage.setItem(PENDING_CONSENTS_KEY, JSON.stringify(value)); } catch { /* storage unavailable */ }
}
async function flushPendingConsents() {
  const pending = readPendingConsents();
  for (const [consent_type, granted] of Object.entries(pending)) {
    await api("/me/consents", { method: "POST", body: { consent_type, granted, policy_version: POLICY_VERSION } });
  }
  try { localStorage.removeItem(PENDING_CONSENTS_KEY); } catch { /* storage unavailable */ }
}

async function signIn(kind, payload) {
  await authenticate(kind, payload.email, payload.password);
  await flushPendingConsents().catch(() => {});
  return sessionView(await api("/me"));
}

// --- per-browser settings (until the server stores them) ----------------------

const CLIMATE_KEY = "pc_web_climate_config";
const SENT_REMINDERS_KEY = "pc_web_sent_reminders";

function readJson(key, fallback) {
  try { return JSON.parse(localStorage.getItem(key) || "null") ?? fallback; } catch { return fallback; }
}
function writeJson(key, value) {
  try { localStorage.setItem(key, JSON.stringify(value)); } catch { /* storage unavailable */ }
}

// --- weather: Open-Meteo answers browsers directly (CORS enabled) -------------

/** Data minimisation, as on the desktop: ~1 km precision is enough for weather. */
const roundCoord = (v) => Math.round(Number(v) * 100) / 100;

const WEATHER_BASIC =
  "current=temperature_2m,relative_humidity_2m,precipitation,wind_speed_10m" +
  "&daily=temperature_2m_max,temperature_2m_min,precipitation_sum&timezone=auto&forecast_days=7";

const WEATHER_EXTENDED =
  "current=temperature_2m,relative_humidity_2m,apparent_temperature,precipitation,rain,showers,snowfall," +
  "weather_code,cloud_cover,pressure_msl,surface_pressure,wind_speed_10m,wind_direction_10m,wind_gusts_10m,is_day,uv_index" +
  "&hourly=temperature_2m,relative_humidity_2m,apparent_temperature,dew_point_2m,precipitation_probability," +
  "precipitation,rain,showers,snowfall,weather_code,pressure_msl,cloud_cover,visibility,wind_speed_10m,wind_gusts_10m," +
  "wind_direction_10m,uv_index,soil_temperature_0cm,soil_temperature_6cm,soil_moisture_0_to_1cm,soil_moisture_1_to_3cm," +
  "soil_moisture_3_to_9cm" +
  "&daily=weather_code,temperature_2m_max,temperature_2m_min,apparent_temperature_max,apparent_temperature_min," +
  "sunrise,sunset,daylight_duration,sunshine_duration,uv_index_max,precipitation_sum,rain_sum,showers_sum,snowfall_sum," +
  "precipitation_hours,precipitation_probability_max,wind_speed_10m_max,wind_gusts_10m_max,wind_direction_10m_dominant," +
  "shortwave_radiation_sum,et0_fao_evapotranspiration" +
  "&timezone=auto&forecast_days=7";

/**
 * Build a history reading from an extended Open-Meteo response — the same
 * field mapping the desktop uses in fetch_and_log_weather (soil, visibility
 * and dew point come from the first hourly slot).
 */
function readingFrom(data, locationId, lat, lon) {
  const c = data?.current || {};
  const h = data?.hourly || {};
  const first = (key) => (Array.isArray(h[key]) && h[key].length ? h[key][0] : null);
  const num = (v) => (typeof v === "number" && Number.isFinite(v) ? v : null);
  const visibility = num(first("visibility"));
  return {
    location_id: locationId || null,
    latitude: Number(lat),
    longitude: Number(lon),
    temp_c: num(c.temperature_2m),
    humidity: num(c.relative_humidity_2m),
    wind_kmh: num(c.wind_speed_10m),
    precipitation_mm: num(c.precipitation),
    weather_code: Number.isInteger(c.weather_code) ? c.weather_code : null,
    apparent_temp_c: num(c.apparent_temperature),
    uv_index: num(c.uv_index),
    cloud_cover: num(c.cloud_cover),
    pressure_hpa: num(c.pressure_msl) ?? num(c.surface_pressure),
    dew_point_c: num(first("dew_point_2m")),
    visibility_km: visibility == null ? null : visibility / 1000,
    wind_gusts_kmh: num(c.wind_gusts_10m),
    wind_direction_deg: num(c.wind_direction_10m),
    soil_temp_c: num(first("soil_temperature_0cm")),
    soil_moisture: num(first("soil_moisture_0_to_1cm")),
    is_day: c.is_day == null ? null : c.is_day !== 0,
  };
}

async function openMeteo(lat, lon, query) {
  const url = `https://api.open-meteo.com/v1/forecast?latitude=${roundCoord(lat)}&longitude=${roundCoord(lon)}&${query}`;
  const r = await fetch(url);
  if (!r.ok) throw `Wetterdienst nicht erreichbar (${r.status})`;
  return r.json();
}

// --- photos -------------------------------------------------------------------

/** Photos are stored at most 1000x1000; shrinking before upload saves mobile data. */
const UPLOAD_MAX_EDGE = 1000;

/**
 * Shrink camera JPEGs before upload to spare mobile data. Purely a bandwidth
 * saving: the server decodes and re-encodes every upload to WebP regardless.
 * Only JPEGs are touched — re-encoding a PNG as JPEG would flatten
 * transparency. Any failure falls back to the original file.
 */
async function shrinkForUpload(file) {
  try {
    if (file.type !== "image/jpeg" || typeof createImageBitmap !== "function") return file;
    // "from-image" applies the EXIF rotation, so the pixels come out upright
    // and the server receives no orientation tag to apply twice.
    const bmp = await createImageBitmap(file, { imageOrientation: "from-image" });
    const scale = Math.min(1, UPLOAD_MAX_EDGE / Math.max(bmp.width, bmp.height));
    const canvas = document.createElement("canvas");
    canvas.width = Math.round(bmp.width * scale);
    canvas.height = Math.round(bmp.height * scale);
    canvas.getContext("2d").drawImage(bmp, 0, 0, canvas.width, canvas.height);
    bmp.close?.();
    const blob = await new Promise((resolve) => canvas.toBlob(resolve, "image/jpeg", 0.9));
    return blob && blob.size < file.size ? blob : file;
  } catch {
    return file;
  }
}

const photoUrls = new Map();

async function photoObjectUrl(photoPath) {
  if (!photoUrls.has(photoPath)) {
    // Cache the promise, not the result, so a gallery rendering the same photo
    // twice does not download it twice.
    photoUrls.set(photoPath, apiBlob(photoPath).then((blob) => URL.createObjectURL(blob)));
  }
  try {
    return await photoUrls.get(photoPath);
  } catch (err) {
    photoUrls.delete(photoPath);
    throw err;
  }
}

function forgetPhotoUrl(photoPath) {
  const pending = photoUrls.get(photoPath);
  photoUrls.delete(photoPath);
  pending?.then((url) => URL.revokeObjectURL(url)).catch(() => {});
}

// --- reminders: computed from the user's tasks while the tab is open ----------

async function dueReminders(withinMinutes = 30) {
  if (!hasSession() && !isGuest()) return { count: 0, reminders: [] };
  const sent = readJson(SENT_REMINDERS_KEY, {});
  const tasks = await dataApi("/tasks");
  const now = Date.now();
  const horizon = now + withinMinutes * 60 * 1000;
  const reminders = tasks
    .filter((t) => t.status === "open" && t.notification_enabled)
    .filter((t) => new Date(t.due_at).getTime() <= horizon)
    .filter((t) => sent[t.task_id] !== t.due_at)
    .map((t) => {
      const due = new Date(t.due_at).getTime();
      return {
        task_id: t.task_id,
        plant_id: t.plant_id,
        title: t.title,
        category: t.category,
        due_at: t.due_at,
        overdue: due < now,
        minutes_until_due: Math.round((due - now) / 60000),
      };
    });
  return { count: reminders.length, reminders };
}

// --- web push: reminders from the server while the app is closed -------------
//
// The browser subscribes at its push service with our VAPID public key and
// hands the subscription to the server, which then sends an encrypted message
// when a task falls due. src/sw.js shows it. Permission must already be
// granted (main.js asks from the click handler, which Safari requires).

let pushPublicKey; // undefined = not asked yet, null = push off on the server

async function serverPushKey() {
  if (pushPublicKey !== undefined) return pushPublicKey;
  try {
    pushPublicKey = (await api("/push/public-key", { auth: false })).public_key || null;
  } catch (err) {
    // 404: push is not configured on this server; remember that instead of
    // asking again on every check. Network errors are retried next time.
    if (err?.status === 404) pushPublicKey = null;
    return null;
  }
  return pushPublicKey;
}

function pushSupportedHere() {
  return typeof window !== "undefined" && "serviceWorker" in navigator && "PushManager" in window && "Notification" in window;
}

async function currentPushSubscription() {
  if (!pushSupportedHere()) return null;
  const reg = await navigator.serviceWorker.getRegistration();
  return reg ? reg.pushManager.getSubscription() : null;
}

function base64UrlToBytes(b64) {
  const padded = (b64 + "===".slice((b64.length + 3) % 4)).replace(/-/g, "+").replace(/_/g, "/");
  return Uint8Array.from(atob(padded), (c) => c.charCodeAt(0));
}

async function registerPushSubscription(sub, lang) {
  await api("/push/subscriptions", {
    method: "POST",
    body: { ...sub.toJSON(), lang, utc_offset_minutes: -new Date().getTimezoneOffset() },
  });
}

async function forgetPushSubscription() {
  const sub = await currentPushSubscription();
  if (!sub) return;
  try { await api("/push/subscriptions", { method: "DELETE", body: { endpoint: sub.endpoint } }); } catch (_) {}
  try { await sub.unsubscribe(); } catch (_) {}
}

async function pushStatus() {
  const supported = pushSupportedHere() && !!(await navigator.serviceWorker.getRegistration());
  const serverEnabled = !!(await serverPushKey());
  const permission = typeof Notification === "undefined" ? "unsupported" : Notification.permission;
  const sub = supported ? await currentPushSubscription() : null;
  return { supported, server_enabled: serverEnabled, permission, subscribed: !!sub && permission === "granted" };
}

async function enablePush(lang) {
  if (!pushSupportedHere()) throw "push not supported";
  if (Notification.permission !== "granted") throw "notification permission not granted";
  const key = await serverPushKey();
  if (!key) throw "push is not available on this server";
  const reg = await Promise.race([
    navigator.serviceWorker.ready,
    new Promise((_, reject) => setTimeout(() => reject("service worker not ready"), 10000)),
  ]);
  const options = { userVisibleOnly: true, applicationServerKey: base64UrlToBytes(key) };
  let sub;
  try {
    sub = await reg.pushManager.subscribe(options);
  } catch (err) {
    // An old subscription made with a different server key blocks a new one.
    const old = await reg.pushManager.getSubscription();
    if (!old) throw err;
    await old.unsubscribe();
    sub = await reg.pushManager.subscribe(options);
  }
  await registerPushSubscription(sub, lang);
  return "subscribed";
}

export const webCommands = {
  // --- session ---------------------------------------------------------------
  bootstrap_local_user: async () => ((await refreshSession()) ? "web session restored" : "no web session"),
  get_sync_status: async () => {
    const s = getSession();
    return {
      session_active: hasSession(),
      guest_mode: isGuest(),
      user_id: s?.user_id ?? null,
      auth_mode: s?.auth_mode ?? null,
      backend_url: API_BASE_URL,
      dirty_count: 0,
      dirty_breakdown: { plants: 0, tasks: 0, logs: 0, deletes: 0 },
      last_sync_at: null,
    };
  },
  auth_login: ({ payload }) => signIn("login", payload),
  auth_register: ({ payload }) => signIn("register", payload),
  auth_fetch_me: () => api("/me"),
  // A shared browser must not keep receiving the previous user's reminders.
  web_logout: async () => { await forgetPushSubscription(); await logout(); return "logged out"; },
  // The server is the source of truth in the browser; there is nothing to sync.
  sync_now: async () => ({
    pushed_plants: 0, pushed_tasks: 0, pushed_logs: 0, pushed_deleted: 0,
    pulled_plants: 0, pulled_tasks: 0, pulled_logs: 0, pulled_deleted: 0,
  }),

  // --- desktop app license (web account card) ---------------------------------
  web_desktop_license: async () => (hasSession() ? api("/licenses/me") : null),
  web_desktop_checkout: async () => {
    requireSession();
    const r = await api("/licenses/checkout", { method: "POST", body: {} });
    // Stripe hosts the payment page; development builds grant directly.
    if (r?.checkout_url) window.location.assign(r.checkout_url);
    return r;
  },

  // --- guest mode --------------------------------------------------------------
  web_guest_start: async () => { setGuestFlag(true); return "guest mode"; },
  web_guest_status: async () => {
    const plants = await guestStore.plantCount();
    return { guest: isGuest(), has_data: plants > 0, plants, persistent: await guestStore.isPersistent() };
  },
  // Moves this browser's garden into the signed-in account, then forgets it
  // locally. The server writes all of it in one transaction or nothing.
  web_guest_import: async () => {
    requireSession();
    const report = await api("/me/import-guest", { method: "POST", body: await exportForImport() });
    await guestStore.clearAll();
    setGuestFlag(false);
    return report;
  },
  web_guest_discard: async () => { await guestStore.clearAll(); setGuestFlag(false); return "discarded"; },
  web_guest_leave: async () => { setGuestFlag(false); return "left guest mode"; },

  // --- account ---------------------------------------------------------------
  update_profile_smart: ({ payload }) =>
    api("/me", { method: "PATCH", body: { locale: payload.locale ?? null, timezone: payload.timezone ?? null } }),
  change_password_smart: async ({ payload }) => {
    await api("/me/change-password", {
      method: "POST",
      body: { current_password: payload.current_password, new_password: payload.new_password },
    });
    return "password changed";
  },
  delete_account_smart: async () => {
    requireSession();
    await api("/me", { method: "DELETE" });
    await logout();
    return "account deleted";
  },
  export_account_json: async ({ path }) => {
    requireData();
    // A guest's export is simply everything this browser holds.
    const data = isGuest()
      ? { schema_version: 1, exported_at: new Date().toISOString(), guest: true, ...(await exportForImport()) }
      : await api("/me/export");
    triggerDownload(path, JSON.stringify(data, null, 2), "application/json");
    return path;
  },

  // --- plants ----------------------------------------------------------------
  list_current_plants: dataListOrEmpty(() => dataApi("/plants")),
  create_plant_smart: ({ payload }) => {
    requireData();
    return dataApi("/plants", {
      method: "POST",
      body: {
        name: String(payload.name || "").trim(),
        color_tag: payload.color_tag || "#0B3D2E",
        phase: lower(payload.phase, "veg"),
        phase_week: payload.phase_week ?? null,
        location_id: payload.location_id || null,
        lifecycle: payload.lifecycle ? lower(payload.lifecycle) : null,
        plant_year: payload.plant_year ?? null,
        catalog_id: payload.catalog_id || null,
        pot_size_liters: payload.pot_size_liters ?? null,
        soil_type: payload.soil_type || null,
      },
    });
  },
  update_plant_smart: ({ payload }) => {
    requireData();
    // The server resolves the target status from status/archived itself.
    return dataApi(`/plants/${enc(payload.plant_id)}`, {
      method: "PATCH",
      body: {
        name: String(payload.name || "").trim(),
        color_tag: payload.color_tag,
        phase: lower(payload.phase, "veg"),
        status: payload.status ? lower(payload.status) : null,
        archived: payload.archived ?? null,
        phase_week: payload.phase_week ?? null,
        location_id: payload.location_id || null,
        lifecycle: payload.lifecycle ? lower(payload.lifecycle) : null,
        plant_year: payload.plant_year ?? null,
        pot_size_liters: payload.pot_size_liters ?? null,
        soil_type: payload.soil_type || null,
      },
    });
  },
  delete_plant_smart: async ({ plantId }) => {
    requireData();
    await dataApi(`/plants/${enc(plantId)}`, { method: "DELETE" });
    return "plant deleted";
  },
  get_plant_catalog: () => api("/catalog", { auth: false }),

  // --- tasks -----------------------------------------------------------------
  list_current_tasks: dataListOrEmpty(({ plantId } = {}) => dataApi(plantId ? `/plants/${enc(plantId)}/tasks` : "/tasks")),
  create_task_smart: ({ payload }) => {
    requireData();
    const due_at = parseDueAt(payload.due_at, new Date(Date.now() + 24 * 3600 * 1000));
    return dataApi(`/plants/${enc(payload.plant_id)}/tasks`, {
      method: "POST",
      body: {
        title: String(payload.title || "").trim(),
        category: lower(payload.category, "water"),
        due_at,
        repeat_interval_hours: payload.repeat_interval_hours ?? null,
        repeat_until: resolveRepeatUntil(payload.repeat_until, due_at, payload.repeat_interval_hours),
        notification_enabled: payload.notification_enabled ?? true,
      },
    });
  },
  update_task_smart: ({ payload }) => {
    requireData();
    return dataApi(`/tasks/${enc(payload.task_id)}`, {
      method: "PATCH",
      body: {
        title: payload.title ?? null,
        status: payload.status ? lower(payload.status) : null,
        due_at: payload.due_at ? parseDueAt(payload.due_at, new Date()) : null,
        repeat_interval_hours: payload.repeat_interval_hours ?? null,
        notification_enabled: payload.notification_enabled ?? null,
      },
    });
  },
  update_task_status_smart: ({ payload }) => {
    requireData();
    return dataApi(`/tasks/${enc(payload.task_id)}`, { method: "PATCH", body: { status: lower(payload.status, "open") } });
  },
  delete_task_smart: async ({ taskId }) => {
    requireData();
    await dataApi(`/tasks/${enc(taskId)}`, { method: "DELETE" });
    return "task deleted";
  },

  // --- logs ------------------------------------------------------------------
  list_current_logs: dataListOrEmpty(({ plantId } = {}) => dataApi(plantId ? `/plants/${enc(plantId)}/logs` : "/logs")),
  create_log_smart: ({ payload }) => {
    requireData();
    return dataApi(`/plants/${enc(payload.plant_id)}/logs`, {
      method: "POST",
      body: {
        log_type: lower(payload.log_type, "note"),
        created_at: payload.created_at ? parseDueAt(payload.created_at, new Date()) : null,
        text: payload.text ?? null,
        metrics: metricsFrom(payload),
      },
    });
  },
  update_log_smart: ({ payload }) => {
    requireData();
    return dataApi(`/logs/${enc(payload.log_id)}`, {
      method: "PATCH",
      body: {
        log_type: payload.log_type ? lower(payload.log_type) : null,
        text: payload.text ?? null,
        metrics: metricsFrom(payload),
      },
    });
  },
  delete_log_smart: async ({ logId }) => {
    requireData();
    await dataApi(`/logs/${enc(logId)}`, { method: "DELETE" });
    return "log deleted";
  },

  // --- locations -------------------------------------------------------------
  // The server rounds coordinates to ~1 km before storing them.
  list_locations: dataListOrEmpty(() => dataApi("/locations")),
  create_location: ({ name, locationType, description, latitude, longitude }) => {
    requireData();
    return dataApi("/locations", {
      method: "POST",
      body: {
        name,
        location_type: locationType ?? null,
        description: description ?? null,
        latitude: latitude ?? null,
        longitude: longitude ?? null,
      },
    });
  },
  update_location: ({ locationId, name, locationType, description, latitude, longitude }) => {
    requireData();
    return dataApi(`/locations/${enc(locationId)}`, {
      method: "PATCH",
      body: {
        name,
        location_type: locationType ?? null,
        description: description ?? null,
        latitude: latitude ?? null,
        longitude: longitude ?? null,
      },
    });
  },
  delete_location: async ({ locationId }) => {
    requireData();
    await dataApi(`/locations/${enc(locationId)}`, { method: "DELETE" });
    return "location deleted";
  },

  // --- seasons (perennials) --------------------------------------------------
  list_plant_seasons: dataListOrEmpty(({ plantId }) => dataApi(`/plants/${enc(plantId)}/seasons`)),
  create_plant_season: ({ plantId, year, notes }) => {
    requireData();
    return dataApi(`/plants/${enc(plantId)}/seasons`, { method: "POST", body: { year, notes: notes ?? null } });
  },
  start_new_season: ({ plantId }) => {
    requireData();
    return dataApi(`/plants/${enc(plantId)}/seasons`, {
      method: "POST",
      body: { year: new Date().getFullYear(), notes: null },
    });
  },
  update_plant_season: ({ payload }) => {
    requireData();
    const { season_id, ...rest } = payload;
    return dataApi(`/seasons/${enc(season_id)}`, { method: "PATCH", body: rest });
  },
  delete_plant_season: async ({ seasonId }) => {
    requireData();
    await dataApi(`/seasons/${enc(seasonId)}`, { method: "DELETE" });
    return "season deleted";
  },

  // --- consents (GDPR) -------------------------------------------------------
  list_consents_smart: async () => {
    if (hasSession()) return api("/me/consents");
    const pending = readPendingConsents();
    return Object.entries(pending).map(([consent_type, granted]) => ({
      consent_type, granted, policy_version: POLICY_VERSION,
    }));
  },
  set_consent_smart: async ({ consentType, granted }) => {
    if (hasSession()) {
      return api("/me/consents", {
        method: "POST",
        body: { consent_type: consentType, granted: !!granted, policy_version: POLICY_VERSION },
      });
    }
    writePendingConsents({ ...readPendingConsents(), [consentType]: !!granted });
    return { consent_type: consentType, granted: !!granted, policy_version: POLICY_VERSION };
  },

  // --- subscription ----------------------------------------------------------
  // Signed out there is no account to ask about; answer like the server would
  // for a fresh Free account (SubscriptionStatusSummary::empty_for_plan).
  subscription_status_smart: () => (hasSession() || isGuest() ? dataApi("/subscription/status") : Promise.resolve({
    plan: "free", plant_limit: 1, active_plants: 0, ended_plants: 0, archived_plants: 0,
    inactive_plants: 0, visible_plants: 0, grandfathered_active_plants: 0,
    remaining_slots: 1, over_limit_by: 0, can_create: true, can_reactivate: true,
  })),
  subscription_history_smart: () => (hasSession() ? api("/subscription/history") : Promise.resolve([])),
  subscription_checkout_smart: ({ plan }) => {
    requireSession();
    return api("/subscription/checkout", { method: "POST", body: { plan: lower(plan) } });
  },
  // A test hook for the desktop; it must never exist in a public web build.
  subscription_webhook_stub_smart: notYet("Der Webhook-Test"),

  // --- reminders -------------------------------------------------------------
  list_due_notification_candidates: ({ withinMinutes } = {}) => dueReminders(withinMinutes ?? 30),
  mark_notification_sent: async ({ payload }) => {
    const sent = readJson(SENT_REMINDERS_KEY, {});
    sent[payload.task_id] = payload.due_at;
    writeJson(SENT_REMINDERS_KEY, sent);
    return "marked";
  },
  // The desktop fires OS notifications from Rust; the browser does it from main.js.
  check_and_send_notifications: async () => 0,
  web_push_status: () => pushStatus(),
  web_push_enable: ({ lang } = {}) => enablePush(lang),
  web_push_disable: async () => { await forgetPushSubscription(); return "unsubscribed"; },
  // Refreshes language/UTC offset (and the owning account) of an existing subscription.
  web_push_sync: async ({ lang } = {}) => {
    if (!hasSession() || typeof Notification === "undefined" || Notification.permission !== "granted") return "skipped";
    const sub = await currentPushSubscription();
    if (!sub || !(await serverPushKey())) return "skipped";
    await registerPushSubscription(sub, lang);
    return "synced";
  },
  web_push_test: async () => {
    const sub = await currentPushSubscription();
    return api("/push/test", { method: "POST", body: { endpoint: sub?.endpoint ?? null } });
  },

  // --- weather & climate -----------------------------------------------------
  fetch_weather: ({ lat, lon }) => openMeteo(lat, lon, WEATHER_BASIC),
  fetch_and_log_weather: async ({ locationId, lat, lon }) => {
    const data = await openMeteo(lat, lon, WEATHER_EXTENDED);
    // Logging is a side effect of viewing the forecast: a failure must not
    // hide the forecast itself.
    if (hasSession()) {
      api("/weather/history", { method: "POST", body: readingFrom(data, locationId, lat, lon) })
        .catch((err) => console.warn("weather history not stored", err));
    }
    return data;
  },
  search_city: async ({ query }) => {
    const r = await fetch(`https://geocoding-api.open-meteo.com/v1/search?name=${enc(query)}&count=5&language=de`);
    if (!r.ok) throw `Ortssuche nicht erreichbar (${r.status})`;
    return r.json();
  },
  get_climate_config: async () => {
    if (!hasSession()) return readJson(CLIMATE_KEY, null);
    const remote = await api("/me/climate");
    if (remote) return remote;
    // One-time hand-over: settings saved in this browser before they were
    // stored per account move into the account instead of being lost.
    const local = readJson(CLIMATE_KEY, null);
    if (local?.latitude != null && local?.longitude != null) {
      const saved = await api("/me/climate", { method: "PUT", body: local });
      try { localStorage.removeItem(CLIMATE_KEY); } catch { /* storage unavailable */ }
      return saved;
    }
    return null;
  },
  save_climate_config: async ({ config }) => {
    if (!hasSession()) { writeJson(CLIMATE_KEY, config); return null; }
    await api("/me/climate", { method: "PUT", body: config });
    return null;
  },
  list_weather_history: listOrEmpty(({ locationId, hours, days } = {}) => {
    const q = new URLSearchParams();
    if (locationId) q.set("location_id", locationId);
    if (hours != null) q.set("hours", String(hours));
    else if (days != null) q.set("days", String(days));
    return api(`/weather/history?${q}`);
  }),
  prune_weather_history_cmd: async ({ days } = {}) => {
    if (!hasSession()) return 0;
    const r = await api(`/weather/history?older_than_days=${enc(days ?? 365)}`, { method: "DELETE" });
    return r?.deleted ?? 0;
  },

  // --- files -----------------------------------------------------------------
  write_export_file: async ({ path, content }) => { triggerDownload(path, content); return null; },
  read_import_file: async ({ path }) => takeUpload(path),
  export_all_csv: notYet("Der CSV-Export"),
  import_plants_csv: notYet("Der CSV-Import"),
  resolve_doc_path: async ({ filename }) => `/docs/${enc(filename)}`,
  open_external_path: async ({ path }) => { window.open(path, "_blank", "noopener"); return null; },

  // --- photos (paid plans; the server converts every upload to WebP) --------
  list_plant_photos: listOrEmpty(({ plantId }) => api(`/plants/${enc(plantId)}/photos`)),
  list_log_photos: listOrEmpty(({ logId }) => api(`/logs/${enc(logId)}/photos`)),
  add_photo: async ({ plantId, logId, seasonId, sourcePath, caption }) => {
    requireSession();
    const file = await shrinkForUpload(takeUploadFile(sourcePath));
    const q = new URLSearchParams();
    if (logId) q.set("log_id", logId);
    if (seasonId) q.set("season_id", seasonId);
    if (caption) q.set("caption", caption);
    const query = q.toString();
    return apiUpload(`/plants/${enc(plantId)}/photos${query ? `?${query}` : ""}`, file, file.type || "application/octet-stream");
  },
  delete_photo: async ({ photoId }) => {
    requireSession();
    await api(`/photos/${enc(photoId)}`, { method: "DELETE" });
    forgetPhotoUrl(`/photos/${photoId}`);
    return "photo deleted";
  },
  // The gallery sets <img src> from this. Photos need the Authorization header,
  // which an <img> tag cannot send, so the bytes are fetched here and handed
  // over as an object URL.
  get_photo_path: async ({ photoPath }) => photoObjectUrl(photoPath),
  web_photo_status: async () => (hasSession() ? api("/photos/status") : null),
  web_photo_archive: async () => {
    requireSession();
    triggerDownload("pflanzenkalender-fotos.zip", await apiBlob("/photos/archive"));
    return null;
  },

  // --- local profiles (a desktop multi-user feature) -------------------------
  list_profiles: async () => [],
  get_active_profile: async () => null,
  create_profile: notYet("Profile"),
  switch_profile: notYet("Profile"),
  delete_profile_local: notYet("Profile"),

  // --- hardware: desktop only ------------------------------------------------
  list_devices: async () => [],
  list_device_readings: async () => [],
  prune_old_readings: async () => null,
  ble_scan: desktopOnly("Bluetooth"),
  ble_read_telemetry: desktopOnly("Bluetooth"),
  ble_debug_read: desktopOnly("Bluetooth"),
  lc_read_status: desktopOnly("Bluetooth"),
  tuya_read_status: desktopOnly("Tuya"),
  save_device: desktopOnly("Geräteverwaltung"),
  link_device: desktopOnly("Geräteverwaltung"),
  remove_device: desktopOnly("Geräteverwaltung"),
  create_auto_log_from_reading: desktopOnly("Geräteverwaltung"),
};
