// The REST endpoints of the backend, answered from the browser's guest store.
//
// web-commands.js talks to "the API" the same way in both modes; in guest
// mode the data endpoints land here instead of on the server. Each handler
// mirrors the server's rules (backend/src/api/mod.rs, db/repos.rs,
// shared/src/rules.rs) so nothing behaves differently once the data has been
// moved into an account: Free plan with one active plant, status
// transitions, recurring tasks, coordinate rounding, sort orders.

import { ApiError } from "./http.js";
import * as store from "./guest-store.js";

/** A guest is on the Free plan. */
const PLANT_LIMIT = 1;
const MAX_REPEAT_INTERVAL_HOURS = 8760;
const MAX_LOG_TEXT_LEN = 5000;

const now = () => new Date().toISOString();
const uuid = () => crypto.randomUUID();
const fail = (status, message) => { throw new ApiError(status, message, { message }); };
const tooLong = (s, max) => !s || !String(s).trim() || String(s).length > max;
const byDesc = (key) => (a, b) => String(b[key]).localeCompare(String(a[key]));
const byAsc = (key) => (a, b) => String(a[key]).localeCompare(String(b[key]));

// --- shared rules ---------------------------------------------------------------

const visible = (plants) => plants.filter((p) => p.status !== "deleted");
const activeCount = (plants) => visible(plants).filter((p) => p.status === "active").length;

function statusSummary(plants) {
  const v = visible(plants);
  const active = activeCount(plants);
  const ended = v.filter((p) => p.status === "ended").length;
  const archived = v.filter((p) => p.status === "archived").length;
  const over = Math.max(0, active - PLANT_LIMIT);
  return {
    plan: "free",
    plant_limit: PLANT_LIMIT,
    active_plants: active,
    ended_plants: ended,
    archived_plants: archived,
    inactive_plants: ended + archived,
    visible_plants: v.length,
    grandfathered_active_plants: over,
    remaining_slots: Math.max(0, PLANT_LIMIT - active),
    over_limit_by: over,
    can_create: active < PLANT_LIMIT,
    can_reactivate: active < PLANT_LIMIT,
  };
}

/** shared::resolve_requested_plant_status (is_active is never sent by the web). */
function resolveStatus(current, requested, archived) {
  if (requested) return requested;
  if (archived === true) return "archived";
  if (current === "archived" && archived === false) return "active";
  return current;
}

const withFlags = (plant) => ({ ...plant, is_active: plant.status === "active", archived: plant.status === "archived" });

/** next_recurring_due_after: first slot strictly after now. */
function nextDue(currentIso, hours) {
  if (!hours || hours <= 0 || hours > MAX_REPEAT_INTERVAL_HOURS) return null;
  const step = hours * 3600 * 1000;
  let next = new Date(currentIso).getTime() + step;
  const t = Date.now();
  if (next <= t) next += (Math.floor((t - next) / step) + 1) * step;
  return new Date(next).toISOString();
}

/** repos::Coords::parse: both or neither, in range, rounded to ~1 km. */
function parseCoords(lat, lon) {
  if (lat == null && lon == null) return null;
  if (lat == null || lon == null) fail(400, "latitude and longitude must be given together");
  const a = Number(lat), o = Number(lon);
  if (!Number.isFinite(a) || !Number.isFinite(o) || a < -90 || a > 90 || o < -180 || o > 180) {
    fail(400, "latitude must be -90..90 and longitude -180..180");
  }
  const round = (v) => Math.round(v * 100) / 100;
  return { latitude: round(a), longitude: round(o) };
}

async function plantOr404(id) {
  const plant = await store.get("plants", id);
  if (!plant || plant.status === "deleted") fail(404, "plant not found");
  return plant;
}

async function rowOr404(kind, id, label) {
  const row = await store.get(kind, id);
  if (!row) fail(404, `${label} not found`);
  return row;
}

// --- plants ---------------------------------------------------------------------

async function listPlants() {
  return visible(await store.all("plants")).sort(byDesc("updated_at"));
}

async function createPlant(b) {
  if (tooLong(b.name, 120)) fail(400, "plant name must be 1-120 characters");
  if (tooLong(b.color_tag, 30)) fail(400, "invalid color tag");
  if (activeCount(await store.all("plants")) >= PLANT_LIMIT) fail(403, "plan limit reached");
  const t = now();
  return store.put("plants", withFlags({
    plant_id: uuid(),
    user_id: null,
    name: b.name,
    color_tag: b.color_tag,
    start_date: t,
    phase: b.phase || "veg",
    phase_week: b.phase_week ?? 1,
    status: "active",
    location_id: b.location_id ?? null,
    lifecycle: b.lifecycle || "annual",
    plant_year: b.plant_year ?? null,
    catalog_id: b.catalog_id ?? null,
    pot_size_liters: b.pot_size_liters ?? null,
    soil_type: b.soil_type ?? null,
    created_at: t,
    updated_at: t,
  }));
}

async function updatePlant(id, b) {
  const current = await plantOr404(id);
  const target = resolveStatus(current.status, b.status, b.archived);
  if ((current.status === "archived" || current.status === "ended") && target === "active") {
    const others = (await store.all("plants")).filter((p) => p.plant_id !== id);
    if (activeCount(others) >= PLANT_LIMIT) fail(403, "plan limit reached");
  }
  return store.put("plants", withFlags({
    ...current,
    name: b.name ?? current.name,
    color_tag: b.color_tag ?? current.color_tag,
    phase: b.phase ?? current.phase,
    status: target,
    phase_week: b.phase_week ?? current.phase_week,
    location_id: b.location_id ?? current.location_id,
    lifecycle: b.lifecycle ?? current.lifecycle,
    plant_year: b.plant_year ?? current.plant_year,
    pot_size_liters: b.pot_size_liters ?? current.pot_size_liters,
    soil_type: b.soil_type ?? current.soil_type,
    updated_at: now(),
  }));
}

async function deletePlant(id) {
  const plant = await store.get("plants", id);
  // The server keeps a "deleted" row for sync; a guest has nothing to sync,
  // so the plant and everything attached to it simply go.
  if (!plant) return null;
  for (const kind of ["tasks", "logs", "seasons"]) {
    for (const row of await store.all(kind)) if (row.plant_id === id) await store.remove(kind, row[store.STORES[kind]]);
  }
  await store.remove("plants", id);
  return null;
}

// --- tasks ----------------------------------------------------------------------

async function listTasks(plantId) {
  const tasks = await store.all("tasks");
  if (plantId) return tasks.filter((t) => t.plant_id === plantId).sort(byAsc("due_at"));
  return tasks.sort(byDesc("updated_at"));
}

function checkRepeat(hours) {
  if (hours != null && (hours <= 0 || hours > MAX_REPEAT_INTERVAL_HOURS)) {
    fail(400, `repeat_interval_hours must be 1-${MAX_REPEAT_INTERVAL_HOURS}`);
  }
}

async function createTask(plantId, b) {
  if (tooLong(b.title, 120)) fail(400, "task title must be 1-120 characters");
  checkRepeat(b.repeat_interval_hours);
  if (b.repeat_until && new Date(b.repeat_until) < new Date(b.due_at)) fail(400, "repeat_until must not be before due_at");
  await plantOr404(plantId);
  const t = now();
  return store.put("tasks", {
    task_id: uuid(),
    plant_id: plantId,
    title: b.title,
    category: b.category || "water",
    due_at: new Date(b.due_at).toISOString(),
    repeat_interval_hours: b.repeat_interval_hours ?? null,
    repeat_until: b.repeat_until ?? null,
    notification_enabled: b.notification_enabled ?? true,
    status: "open",
    created_at: t,
    updated_at: t,
  });
}

async function updateTask(id, b) {
  const task = await rowOr404("tasks", id, "task");
  const repeatHours = b.repeat_interval_hours ?? task.repeat_interval_hours;
  const repeatUntil = b.repeat_until ?? task.repeat_until;
  let dueAt = b.due_at ?? null;
  let status = b.status ?? task.status;
  // Completing a recurring task rolls it to its next date instead.
  if (b.status === "done" && !dueAt) {
    const next = nextDue(task.due_at, repeatHours);
    if (next && (!repeatUntil || new Date(next) <= new Date(repeatUntil))) {
      dueAt = next;
      status = "open";
    }
  }
  return store.put("tasks", {
    ...task,
    title: b.title ?? task.title,
    status,
    due_at: dueAt ? new Date(dueAt).toISOString() : task.due_at,
    repeat_interval_hours: repeatHours,
    repeat_until: repeatUntil,
    notification_enabled: b.notification_enabled ?? task.notification_enabled,
    updated_at: now(),
  });
}

// --- logs -----------------------------------------------------------------------

async function listLogs(plantId) {
  const logs = await store.all("logs");
  if (plantId) return logs.filter((l) => l.plant_id === plantId).sort(byDesc("created_at"));
  return logs.sort(byDesc("updated_at"));
}

async function createLog(plantId, b) {
  if (b.text && b.text.length > MAX_LOG_TEXT_LEN) fail(400, "log text too long (max 5000 characters)");
  await plantOr404(plantId);
  return store.put("logs", {
    log_id: uuid(),
    plant_id: plantId,
    log_type: b.log_type || "note",
    text: b.text ?? null,
    metrics: b.metrics ?? null,
    action: null,
    photo_refs: null,
    created_at: b.created_at ? new Date(b.created_at).toISOString() : now(),
    updated_at: now(),
  });
}

async function updateLog(id, b) {
  const log = await rowOr404("logs", id, "log");
  return store.put("logs", {
    ...log,
    log_type: b.log_type ?? log.log_type,
    text: b.text ?? log.text,
    metrics: b.metrics ?? log.metrics,
    updated_at: now(),
  });
}

// --- locations ------------------------------------------------------------------

async function createLocation(b) {
  if (tooLong(b.name, 120)) fail(400, "location name must be 1-120 characters");
  const c = parseCoords(b.latitude, b.longitude);
  const t = now();
  return store.put("locations", {
    location_id: uuid(),
    user_id: null,
    name: b.name.trim(),
    location_type: b.location_type || "custom",
    description: b.description ?? null,
    latitude: c?.latitude ?? null,
    longitude: c?.longitude ?? null,
    created_at: t,
    updated_at: t,
  });
}

async function updateLocation(id, b) {
  if (b.name != null && tooLong(b.name, 120)) fail(400, "location name must be 1-120 characters");
  const c = parseCoords(b.latitude, b.longitude);
  const loc = await rowOr404("locations", id, "location");
  return store.put("locations", {
    ...loc,
    name: b.name ?? loc.name,
    location_type: b.location_type ?? loc.location_type,
    description: b.description ?? loc.description,
    latitude: c ? c.latitude : loc.latitude,
    longitude: c ? c.longitude : loc.longitude,
    updated_at: now(),
  });
}

async function deleteLocation(id) {
  for (const p of await store.all("plants")) {
    if (p.location_id === id) await store.put("plants", { ...p, location_id: null });
  }
  await store.remove("locations", id);
  return null;
}

// --- seasons --------------------------------------------------------------------

async function createSeason(plantId, b) {
  await plantOr404(plantId);
  const t = now();
  return store.put("seasons", {
    season_id: uuid(),
    plant_id: plantId,
    year: b.year,
    started_at: t,
    ended_at: null,
    notes: b.notes ?? null,
    harvest_kg: null,
    harvest_count: null,
    pruning_done: false,
    created_at: t,
    updated_at: t,
  });
}

async function updateSeason(id, b) {
  const s = await rowOr404("seasons", id, "season");
  return store.put("seasons", {
    ...s,
    notes: b.notes ?? s.notes,
    harvest_kg: b.harvest_kg ?? s.harvest_kg,
    harvest_count: b.harvest_count ?? s.harvest_count,
    pruning_done: b.pruning_done ?? s.pruning_done,
    updated_at: now(),
  });
}

// --- routing --------------------------------------------------------------------

const ROUTES = [
  ["GET", /^\/plants$/, () => listPlants()],
  ["POST", /^\/plants$/, (_, b) => createPlant(b)],
  ["GET", /^\/plants\/([^/]+)$/, ([id]) => plantOr404(id)],
  ["PATCH", /^\/plants\/([^/]+)$/, ([id], b) => updatePlant(id, b)],
  ["DELETE", /^\/plants\/([^/]+)$/, ([id]) => deletePlant(id)],
  ["GET", /^\/plants\/([^/]+)\/tasks$/, ([id]) => listTasks(id)],
  ["POST", /^\/plants\/([^/]+)\/tasks$/, ([id], b) => createTask(id, b)],
  ["GET", /^\/tasks$/, () => listTasks(null)],
  ["PATCH", /^\/tasks\/([^/]+)$/, ([id], b) => updateTask(id, b)],
  ["DELETE", /^\/tasks\/([^/]+)$/, ([id]) => store.remove("tasks", id).then(() => null)],
  ["GET", /^\/plants\/([^/]+)\/logs$/, ([id]) => listLogs(id)],
  ["POST", /^\/plants\/([^/]+)\/logs$/, ([id], b) => createLog(id, b)],
  ["GET", /^\/logs$/, () => listLogs(null)],
  ["PATCH", /^\/logs\/([^/]+)$/, ([id], b) => updateLog(id, b)],
  ["DELETE", /^\/logs\/([^/]+)$/, ([id]) => store.remove("logs", id).then(() => null)],
  ["GET", /^\/locations$/, async () => (await store.all("locations")).sort(byAsc("name"))],
  ["POST", /^\/locations$/, (_, b) => createLocation(b)],
  ["PATCH", /^\/locations\/([^/]+)$/, ([id], b) => updateLocation(id, b)],
  ["DELETE", /^\/locations\/([^/]+)$/, ([id]) => deleteLocation(id)],
  ["GET", /^\/plants\/([^/]+)\/seasons$/, async ([id]) =>
    (await store.all("seasons")).filter((s) => s.plant_id === id).sort((a, b) => b.year - a.year)],
  ["POST", /^\/plants\/([^/]+)\/seasons$/, ([id], b) => createSeason(id, b)],
  ["PATCH", /^\/seasons\/([^/]+)$/, ([id], b) => updateSeason(id, b)],
  ["DELETE", /^\/seasons\/([^/]+)$/, ([id]) => store.remove("seasons", id).then(() => null)],
  ["GET", /^\/subscription\/status$/, async () => statusSummary(await store.all("plants"))],
];

/** Same contract as http.js api(): resolves with the JSON body, throws ApiError. */
export async function guestApi(path, { method = "GET", body } = {}) {
  const clean = path.split("?")[0];
  for (const [m, re, handler] of ROUTES) {
    if (m !== method) continue;
    const match = clean.match(re);
    if (match) return handler(match.slice(1).map(decodeURIComponent), body || {});
  }
  fail(404, `not available without an account: ${method} ${clean}`);
}

/** Guest data in the shape POST /me/import-guest expects. */
export async function exportForImport() {
  const s = await store.snapshot();
  return {
    locations: s.locations,
    plants: visible(s.plants),
    tasks: s.tasks,
    logs: s.logs,
    seasons: s.seasons,
  };
}
