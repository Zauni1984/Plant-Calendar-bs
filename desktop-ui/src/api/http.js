// HTTP client for the browser build.
//
// Session model
//   - The access token (short-lived JWT) lives only in this module's memory.
//     A page reload drops it on purpose; it is re-obtained from the refresh
//     cookie, so nothing long-lived is ever reachable from JavaScript.
//   - The refresh token is an httpOnly cookie set by the backend for clients
//     that send `X-Client: web`. This code can neither read nor leak it.

const API_BASE = String(import.meta.env?.VITE_API_BASE_URL || "http://localhost:8080").replace(/\/+$/, "");

let accessToken = null;
let sessionInfo = null; // { user_id, auth_mode } of the signed-in user
let refreshInFlight = null;

export class ApiError extends Error {
  constructor(status, message, body) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.body = body;
  }
}

function rawFetch(path, { method = "GET", body, binary, contentType, auth = true } = {}) {
  const headers = { "X-Client": "web" };
  let payload;
  if (binary !== undefined) {
    payload = binary;
    headers["Content-Type"] = contentType || "application/octet-stream";
  } else if (body !== undefined) {
    payload = JSON.stringify(body);
    headers["Content-Type"] = "application/json";
  }
  if (auth && accessToken) headers.Authorization = `Bearer ${accessToken}`;
  return fetch(API_BASE + path, { method, headers, body: payload, credentials: "include" });
}

/** Fetch with one transparent refresh-and-retry on 401; throws ApiError on failure. */
async function authedFetch(path, opts = {}) {
  const hadSession = !!accessToken;
  let r = await rawFetch(path, opts);
  // Only an expired token is worth refreshing. Signed out, the cookie is gone
  // too, and a refresh attempt would just be a second guaranteed 401.
  if (r.status === 401 && opts.auth !== false && hadSession && (await refreshSession())) {
    r = await rawFetch(path, opts);
  }
  if (!r.ok) throw await readError(r);
  return r;
}

/** Upload a Blob/File as the request body; resolves with the parsed JSON reply. */
export async function apiUpload(path, blob, contentType) {
  const r = await authedFetch(path, { method: "POST", binary: blob, contentType });
  return r.json();
}

/** GET binary content (photos, archives) as a Blob. */
export async function apiBlob(path) {
  const r = await authedFetch(path);
  return r.blob();
}

function adoptAuthResponse(data) {
  accessToken = data?.access_token || null;
  sessionInfo = accessToken ? { user_id: data.user_id, auth_mode: data.auth_mode } : null;
  return sessionInfo;
}

/**
 * Exchange the refresh cookie for a fresh access token.
 * Concurrent callers share one request: the backend rotates refresh tokens,
 * so two parallel refreshes would race and the loser would be logged out.
 */
export function refreshSession() {
  if (!refreshInFlight) {
    refreshInFlight = (async () => {
      try {
        const r = await rawFetch("/auth/refresh", { method: "POST", body: {}, auth: false });
        if (!r.ok) return adoptAuthResponse(null);
        return adoptAuthResponse(await r.json());
      } catch {
        return adoptAuthResponse(null);
      }
    })().finally(() => { refreshInFlight = null; });
  }
  return refreshInFlight;
}

async function readError(r) {
  let body = null;
  let message = r.statusText || `HTTP ${r.status}`;
  try {
    body = await r.json();
    message = body?.message || body?.error || message;
  } catch {
    try { message = (await r.text()) || message; } catch { /* keep statusText */ }
  }
  return new ApiError(r.status, message, body);
}

/** Authenticated JSON request with one transparent refresh-and-retry on 401. */
export async function api(path, opts = {}) {
  const r = await authedFetch(path, opts);
  if (r.status === 204) return null;
  const type = r.headers.get("content-type") || "";
  return type.includes("application/json") ? r.json() : r.text();
}

/** Sign in or register; on success the session is held in memory + cookie. */
export async function authenticate(kind, email, password) {
  const r = await rawFetch(`/auth/${kind}`, { method: "POST", body: { email, password }, auth: false });
  if (!r.ok) throw await readError(r);
  return adoptAuthResponse(await r.json());
}

export async function logout() {
  try {
    if (accessToken) await rawFetch("/auth/logout", { method: "POST", body: {} });
  } finally {
    adoptAuthResponse(null);
  }
}

export const getSession = () => sessionInfo;
export const hasSession = () => !!accessToken;
export const API_BASE_URL = API_BASE;
