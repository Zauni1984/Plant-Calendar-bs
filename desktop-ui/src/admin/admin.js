// Sowling admin area (admin.sowling.com): statistics, accounts, invoices,
// audit log. Sign-in with password + authenticator code; the admin token lives
// only in memory (a reload means signing in again) and expires after 30 minutes.

import "./admin.css";

const API = String(import.meta.env?.VITE_API_BASE_URL || "http://localhost:8080").replace(/\/+$/, "");
const root = document.getElementById("admin");

let token = null;
let expiresAt = 0;
let expiryTimer = null;
let tab = "overview";
const charts = [];
let Chart = null;

const PLAN_NAMES = { free: "Free", basic: "Basic", pro: "Pro", nursery: "Gärtnerei" };
const ACTIONS = {
  login: "Anmeldung", disable: "Konto deaktiviert", enable: "Konto aktiviert", delete: "Konto gelöscht",
  invoice_retry: "Rechnung neu angestoßen", set_plan: "Tarif geändert", voucher_create: "Gutscheine erstellt", voucher_revoke: "Gutscheine gesperrt",
};

const EYE_OPEN = '<svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/></svg>';
const EYE_SHUT = '<svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 3l18 18"/><path d="M10.6 5.1A10.4 10.4 0 0 1 12 5c6.5 0 10 7 10 7a17.6 17.6 0 0 1-3.2 4.2M6.6 6.6C3.9 8.3 2 12 2 12s3.5 7 10 7c1.7 0 3.2-.5 4.5-1.2"/><path d="M9.9 9.9a3 3 0 0 0 4.2 4.2"/></svg>';

const esc = (v) => String(v ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
const fmtDate = (v) => (v ? new Date(v).toLocaleDateString("de-DE") : "–");
// "Redeem until" is the end of a day in UTC; show that day, not the next one in CET.
const fmtDateUtc = (v) => (v ? new Date(v).toLocaleDateString("de-DE", { timeZone: "UTC" }) : "–");
const fmtDateTime = (v) => (v ? new Date(v).toLocaleString("de-DE", { dateStyle: "short", timeStyle: "short" }) : "–");
const fmtInt = (n) => new Intl.NumberFormat("de-DE").format(n ?? 0);
const fmtEuro = (cents) => new Intl.NumberFormat("de-DE", { style: "currency", currency: "EUR" }).format((cents ?? 0) / 100);

// --- API ------------------------------------------------------------------------

async function call(path, { method = "GET", body } = {}) {
  const headers = {};
  if (token) headers.Authorization = `Bearer ${token}`;
  if (body !== undefined) headers["Content-Type"] = "application/json";
  const r = await fetch(API + path, { method, headers, body: body === undefined ? undefined : JSON.stringify(body) });
  if (r.status === 401 && token) {
    signOut("Sitzung abgelaufen. Bitte neu anmelden.");
    throw new Error("Sitzung abgelaufen");
  }
  if (!r.ok) {
    let message = `HTTP ${r.status}`;
    try { const j = await r.json(); message = j.message || j.error || message; } catch (_) {}
    throw new Error(message);
  }
  return r.status === 204 ? null : r.json();
}

function toast(text) {
  const el = document.createElement("div");
  el.className = "toast";
  el.setAttribute("role", "status");
  el.textContent = text;
  document.body.appendChild(el);
  setTimeout(() => el.remove(), 3500);
}

// --- login ------------------------------------------------------------------------

function renderLogin(message = "") {
  destroyCharts();
  root.innerHTML = `
    <form class="login" id="loginForm" autocomplete="on">
      <h1>Sowling Admin</h1>
      <p>Anmeldung mit Admin-Konto und Code aus der Authenticator-App.</p>
      <label>E-Mail<input class="input" name="email" type="email" autocomplete="username" required /></label>
      <label>Passwort<span class="pw-field"><input class="input" name="password" type="password" autocomplete="current-password" required /><button type="button" class="pw-toggle" id="pwToggle" aria-label="Passwort anzeigen" title="Passwort anzeigen" aria-pressed="false">${EYE_OPEN}</button></span></label>
      <label>Code aus der App<input class="input code" name="code" inputmode="numeric" autocomplete="one-time-code" pattern="[0-9 ]{6,7}" maxlength="7" required /></label>
      <button class="btn btn--primary" type="submit">Anmelden</button>
      <div class="error" id="loginError" role="alert">${esc(message)}</div>
    </form>`;
  const form = document.getElementById("loginForm");
  form.email.focus();
  const pwToggle = document.getElementById("pwToggle");
  pwToggle.addEventListener("click", () => {
    const show = form.password.type === "password";
    form.password.type = show ? "text" : "password";
    pwToggle.innerHTML = show ? EYE_SHUT : EYE_OPEN;
    const label = show ? "Passwort verbergen" : "Passwort anzeigen";
    pwToggle.setAttribute("aria-label", label);
    pwToggle.title = label;
    pwToggle.setAttribute("aria-pressed", String(show));
    form.password.focus();
  });
  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    const button = form.querySelector("button");
    button.disabled = true;
    document.getElementById("loginError").textContent = "";
    try {
      const res = await call("/admin/login", { method: "POST", body: { email: form.email.value, password: form.password.value, code: form.code.value } });
      token = res.token;
      expiresAt = new Date(res.expires_at).getTime();
      clearTimeout(expiryTimer);
      expiryTimer = setTimeout(() => signOut("Sitzung nach 30 Minuten beendet. Bitte neu anmelden."), Math.max(0, expiresAt - Date.now()));
      renderShell();
    } catch (err) {
      document.getElementById("loginError").textContent =
        String(err.message).includes("429") || String(err.message).toLowerCase().includes("too many")
          ? "Zu viele Versuche. Bitte 15 Minuten warten."
          : "Anmeldung fehlgeschlagen: E-Mail, Passwort oder Code stimmt nicht.";
      form.code.value = "";
      button.disabled = false;
    }
  });
}

function signOut(message) {
  token = null;
  clearTimeout(expiryTimer);
  renderLogin(message);
}

// --- shell ------------------------------------------------------------------------

const TABS = [
  ["overview", "Übersicht"],
  ["users", "Konten"],
  ["vouchers", "Gutscheine"],
  ["invoices", "Rechnungen"],
  ["audit", "Protokoll"],
];

function renderShell() {
  root.innerHTML = `
    <header class="top">
      <div class="brand"><img src="/icons/icon-192.png" alt="" />Sowling <span>Admin</span></div>
      <nav class="tabs" role="tablist">
        ${TABS.map(([id, label]) => `<button class="tab" role="tab" data-tab="${id}" aria-selected="${id === tab}">${label}</button>`).join("")}
      </nav>
      <div class="right"><span id="sessionInfo"></span><button class="btn btn--sm" id="logout">Abmelden</button></div>
    </header>
    <main id="view"></main>
    <dialog id="dialog"></dialog>`;
  root.querySelector(".tabs").addEventListener("click", (e) => {
    const id = e.target.closest("[data-tab]")?.dataset.tab;
    if (!id || id === tab) return;
    tab = id;
    root.querySelectorAll(".tab").forEach((b) => b.setAttribute("aria-selected", String(b.dataset.tab === tab)));
    showTab();
  });
  document.getElementById("logout").addEventListener("click", () => signOut(""));
  const info = document.getElementById("sessionInfo");
  const tick = () => {
    if (!token) return;
    const min = Math.max(0, Math.round((expiresAt - Date.now()) / 60000));
    info.textContent = `Sitzung noch ${min} min`;
  };
  tick();
  setInterval(tick, 30000);
  showTab();
}

function showTab() {
  destroyCharts();
  const view = document.getElementById("view");
  view.innerHTML = `<div class="empty">Lädt…</div>`;
  ({ overview: renderOverview, users: renderUsers, vouchers: renderVouchers, invoices: renderInvoices, audit: renderAudit })[tab](view).catch((err) => {
    view.innerHTML = `<div class="empty">Fehler: ${esc(err.message)}</div>`;
  });
}

// --- overview ------------------------------------------------------------------------

function destroyCharts() {
  while (charts.length) charts.pop().destroy();
}

async function loadChart() {
  if (!Chart) Chart = (await import("chart.js/auto")).default;
  Chart.defaults.color = "#8fb8a7";
  Chart.defaults.borderColor = "rgba(107,255,188,0.08)";
  Chart.defaults.font.family = 'system-ui, -apple-system, "Segoe UI", Roboto, sans-serif';
  return Chart;
}

const PALETTE = ["#6bffbc", "#64b5f6", "#ffd76e", "#ff8fb1", "#b39dff", "#8fb8a7"];

function kpi(label, value, sub = "", warn = false) {
  return `<div class="kpi${warn ? " kpi--warn" : ""}"><div class="label">${esc(label)}</div><div class="value">${esc(value)}</div>${sub ? `<div class="sub">${esc(sub)}</div>` : ""}</div>`;
}

async function renderOverview(view) {
  const [stats, ChartJs] = await Promise.all([call("/admin/stats"), loadChart()]);
  const t = stats.totals;
  view.innerHTML = `
    <div class="kpis">
      ${kpi("Konten", fmtInt(t.accounts), `+${fmtInt(t.new_7d)} in 7 Tagen · +${fmtInt(t.new_30d)} in 30 Tagen`)}
      ${kpi("Aktiv (30 Tage)", fmtInt(t.active_30d), `${fmtInt(t.active_7d)} in den letzten 7 Tagen`)}
      ${kpi("Zahlende Abos", fmtInt(t.paying), `${fmtInt(t.voucher_active)} mit aktivem Gutschein`)}
      ${kpi("Umsatz 30 Tage", fmtEuro(t.revenue_30d_cents), `gesamt ${fmtEuro(t.revenue_total_cents)} (brutto)`)}
      ${kpi("Desktop", fmtInt(t.desktop_devices), `Geräte · ${fmtInt(t.desktop_purchases)} Einmalkäufe`)}
      ${kpi("Pflanzen", fmtInt(t.plants), `${fmtInt(t.plants_active)} aktiv · ${fmtInt(t.photos)} Fotos`)}
      ${kpi("Gutscheine eingelöst", fmtInt(t.vouchers_redeemed), `${fmtInt(t.push_subscriptions)} Push-Abos`)}
      ${kpi("Rechnungsprobleme", fmtInt(t.invoice_problems), `${fmtInt(t.disabled)} Konten deaktiviert`, t.invoice_problems > 0)}
    </div>
    <div class="charts">
      <div class="card card--wide"><h2>Neue Konten pro Tag (90 Tage)</h2><div class="chart-box"><canvas id="cSignups"></canvas></div></div>
      <div class="card card--narrow"><h2>Tarife</h2><div class="chart-box"><canvas id="cPlans"></canvas></div></div>
      <div class="card card--half"><h2>Umsatz pro Monat (brutto)</h2><div class="chart-box"><canvas id="cRevenue"></canvas></div></div>
      <div class="card card--half"><h2>Aktive Konten pro Tag (30 Tage)</h2><div class="chart-box"><canvas id="cActive"></canvas></div></div>
    </div>
    <p class="mono" style="margin-top:12px;">Stand ${esc(fmtDateTime(stats.generated_at))} · Aktiv = Anmeldung oder Sitzungsverlängerung an diesem Tag (zuletzt gesehen)</p>`;

  const day = (s) => new Date(s + "T00:00:00").toLocaleDateString("de-DE", { day: "2-digit", month: "2-digit" });
  const common = { responsive: true, maintainAspectRatio: false, plugins: { legend: { display: false } }, interaction: { mode: "index", intersect: false } };
  const grid = { grid: { color: "rgba(107,255,188,0.06)" } };

  charts.push(new ChartJs(document.getElementById("cSignups"), {
    type: "line",
    data: {
      labels: stats.signups_per_day.map(([d]) => day(d)),
      datasets: [{ data: stats.signups_per_day.map(([, n]) => n), borderColor: PALETTE[0], backgroundColor: "rgba(107,255,188,0.15)", fill: true, tension: 0.3, pointRadius: 0, borderWidth: 2 }],
    },
    options: { ...common, scales: { x: { ...grid, ticks: { maxTicksLimit: 10 } }, y: { ...grid, beginAtZero: true, ticks: { precision: 0 } } } },
  }));

  const plans = stats.plans.filter(([, n]) => n > 0);
  charts.push(new ChartJs(document.getElementById("cPlans"), {
    type: "doughnut",
    data: {
      labels: plans.map(([p]) => PLAN_NAMES[p] || p),
      datasets: [{ data: plans.map(([, n]) => n), backgroundColor: PALETTE, borderColor: "#0e1c17", borderWidth: 3 }],
    },
    options: { responsive: true, maintainAspectRatio: false, cutout: "62%", plugins: { legend: { position: "bottom", labels: { boxWidth: 10, padding: 12 } } } },
  }));

  charts.push(new ChartJs(document.getElementById("cRevenue"), {
    type: "bar",
    data: {
      labels: stats.revenue_per_month.map(([m]) => new Date(m + "-01T00:00:00").toLocaleDateString("de-DE", { month: "short", year: "2-digit" })),
      datasets: [{ data: stats.revenue_per_month.map(([, c]) => c / 100), backgroundColor: "rgba(100,181,246,0.7)", borderRadius: 6 }],
    },
    options: {
      ...common,
      plugins: { legend: { display: false }, tooltip: { callbacks: { label: (ctx) => fmtEuro(ctx.raw * 100) } } },
      scales: { x: grid, y: { ...grid, beginAtZero: true, ticks: { callback: (v) => fmtEuro(v * 100) } } },
    },
  }));

  charts.push(new ChartJs(document.getElementById("cActive"), {
    type: "bar",
    data: {
      labels: stats.active_per_day.map(([d]) => day(d)),
      datasets: [{ data: stats.active_per_day.map(([, n]) => n), backgroundColor: "rgba(107,255,188,0.55)", borderRadius: 4 }],
    },
    options: { ...common, scales: { x: { ...grid, ticks: { maxTicksLimit: 10 } }, y: { ...grid, beginAtZero: true, ticks: { precision: 0 } } } },
  }));
}

// --- accounts ------------------------------------------------------------------------

let userQuery = "";
let userOffset = 0;

function planCell(u) {
  const parts = [`<strong>${esc(PLAN_NAMES[u.plan] || u.plan)}</strong>`];
  if (u.stripe_plan) parts.push(`<span class="badge">Stripe ${esc(PLAN_NAMES[u.stripe_plan] || u.stripe_plan)}</span>`);
  if (u.voucher_plan && (!u.voucher_until || new Date(u.voucher_until) > new Date())) {
    parts.push(`<span class="badge badge--warn">Gutschein/manuell ${esc(PLAN_NAMES[u.voucher_plan] || u.voucher_plan)}${u.voucher_until ? ` bis ${esc(fmtDate(u.voucher_until))}` : " ∞"}</span>`);
  }
  return parts.join(" ");
}

async function renderUsers(view) {
  view.innerHTML = `
    <div class="toolbar">
      <input class="input" id="userSearch" type="search" placeholder="E-Mail oder Konto-ID suchen" value="${esc(userQuery)}" />
      <span class="mono" id="userCount"></span>
    </div>
    <div class="table-wrap"><table>
      <thead><tr><th>E-Mail</th><th>Tarif</th><th>Registriert</th><th>Zuletzt aktiv</th><th class="num">Pflanzen</th><th class="num">Rechn.</th><th>Status</th><th></th></tr></thead>
      <tbody id="userRows"><tr><td colspan="8" class="empty">Lädt…</td></tr></tbody>
    </table></div>
    <div class="pager"><button class="btn btn--sm" id="prevPage">Zurück</button><button class="btn btn--sm" id="nextPage">Weiter</button></div>`;
  let timer;
  document.getElementById("userSearch").addEventListener("input", (e) => {
    clearTimeout(timer);
    timer = setTimeout(() => { userQuery = e.target.value; userOffset = 0; loadUsers(); }, 300);
  });
  document.getElementById("prevPage").addEventListener("click", () => { userOffset = Math.max(0, userOffset - 50); loadUsers(); });
  document.getElementById("nextPage").addEventListener("click", () => { userOffset += 50; loadUsers(); });
  document.getElementById("userRows").addEventListener("click", onUserAction);
  await loadUsers();
}

let lastUsers = [];

async function loadUsers() {
  const rows = document.getElementById("userRows");
  lastUsers = await call(`/admin/users?q=${encodeURIComponent(userQuery)}&offset=${userOffset}`);
  document.getElementById("userCount").textContent = lastUsers.length ? `${userOffset + 1}–${userOffset + lastUsers.length}` : "";
  document.getElementById("prevPage").disabled = userOffset === 0;
  document.getElementById("nextPage").disabled = lastUsers.length < 50;
  rows.innerHTML = lastUsers.length
    ? lastUsers.map((u) => `
      <tr>
        <td>${esc(u.email)}${u.is_admin ? ' <span class="badge">Admin</span>' : ""}<div class="mono">${esc(u.user_id)}</div></td>
        <td>${planCell(u)}</td>
        <td>${esc(fmtDate(u.created_at))}</td>
        <td>${esc(fmtDate(u.last_seen_at))}</td>
        <td class="num">${fmtInt(u.plants)}</td>
        <td class="num">${fmtInt(u.invoices)}</td>
        <td>${u.disabled_at
          ? `<span class="badge badge--off">deaktiviert</span><div class="mono">${esc(fmtDate(u.disabled_at))}${u.disabled_reason ? ` · ${esc(u.disabled_reason)}` : ""}</div>`
          : '<span class="badge badge--muted">aktiv</span>'}</td>
        <td class="actions">
          ${u.disabled_at
            ? `<button class="btn btn--sm" data-act="enable" data-id="${esc(u.user_id)}">Aktivieren</button>`
            : `<button class="btn btn--sm" data-act="disable" data-id="${esc(u.user_id)}">Deaktivieren</button>`}
          <button class="btn btn--sm" data-act="plan" data-id="${esc(u.user_id)}">Tarif</button>
          <button class="btn btn--sm btn--danger" data-act="delete" data-id="${esc(u.user_id)}">Löschen</button>
        </td>
      </tr>`).join("")
    : `<tr><td colspan="8" class="empty">Keine Konten gefunden.</td></tr>`;
}

function dialog(html, onConfirm) {
  const dlg = document.getElementById("dialog");
  dlg.innerHTML = html;
  dlg.showModal();
  const form = dlg.querySelector("form");
  dlg.querySelector("[data-cancel]").addEventListener("click", () => dlg.close());
  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    const button = form.querySelector('button[type="submit"]');
    button.disabled = true;
    try {
      await onConfirm(form);
      dlg.close();
    } catch (err) {
      form.querySelector(".error").textContent = err.message;
      button.disabled = false;
    }
  });
}

async function onUserAction(e) {
  const btn = e.target.closest("[data-act]");
  if (!btn) return;
  const user = lastUsers.find((u) => u.user_id === btn.dataset.id);
  if (!user) return;
  if (btn.dataset.act === "enable") {
    await call(`/admin/users/${user.user_id}/enable`, { method: "POST" });
    toast(`${user.email} ist wieder aktiv.`);
    return loadUsers();
  }
  if (btn.dataset.act === "disable") {
    dialog(`
      <form>
        <h3>Konto deaktivieren</h3>
        <p><strong>${esc(user.email)}</strong> wird überall abgemeldet und kann sich nicht mehr anmelden. Daten bleiben erhalten; du kannst das Konto jederzeit wieder aktivieren. Ein laufendes Stripe-Abo wird dadurch <strong>nicht</strong> gekündigt.</p>
        <label class="mono">Grund (intern, optional)<input class="input" name="reason" maxlength="500" /></label>
        <div class="error" role="alert"></div>
        <div class="row"><button type="button" class="btn" data-cancel>Abbrechen</button><button type="submit" class="btn btn--primary">Deaktivieren</button></div>
      </form>`, async (form) => {
      await call(`/admin/users/${user.user_id}/disable`, { method: "POST", body: { reason: form.reason.value } });
      toast(`${user.email} ist deaktiviert.`);
      await loadUsers();
    });
    return;
  }
  if (btn.dataset.act === "plan") {
    const stripe = user.stripe_plan ? PLAN_NAMES[user.stripe_plan] || user.stripe_plan : null;
    dialog(`
      <form>
        <h3>Tarif ändern</h3>
        <p><strong>${esc(user.email)}</strong> hat jetzt <strong>${esc(PLAN_NAMES[user.plan] || user.plan)}</strong>.
        Der neue Tarif gilt wie ein Gutschein und ersetzt einen bestehenden Gutschein. „Free“ entfernt ihn.</p>
        ${stripe ? `<p class="warn-note">Stripe-Abo ${esc(stripe)} läuft: Das Konto behält mindestens diesen Tarif. Tiefer herabstufen geht nur nach Kündigung im Stripe-Dashboard.</p>` : ""}
        <div class="form-grid">
          <label class="mono">Tarif${planSelect("plan", user.plan === "free" ? "basic" : user.plan, true)}</label>
          <label class="mono">Dauer${durationSelect("duration")}</label>
          <label class="mono" data-days hidden>Tage<input class="input" name="days" type="number" min="1" max="3650" value="30" /></label>
        </div>
        <div class="error" role="alert"></div>
        <div class="row"><button type="button" class="btn" data-cancel>Abbrechen</button><button type="submit" class="btn btn--primary">Speichern</button></div>
      </form>`, async (form) => {
      const body = form.plan.value === "free" ? { plan: "free" } : { plan: form.plan.value, ...durationBody(form) };
      const res = await call(`/admin/users/${user.user_id}/plan`, { method: "POST", body });
      const now = PLAN_NAMES[res.plan] || res.plan;
      toast(res.plan !== body.plan ? `${user.email} hat ${now} (Stripe-Abo).` : `${user.email} hat jetzt ${now}.`);
      await loadUsers();
    });
    const dlg = document.getElementById("dialog");
    wireDuration(dlg);
    const toggleDuration = () => {
      const free = dlg.querySelector('[name="plan"]').value === "free";
      dlg.querySelector('[name="duration"]').disabled = free;
      dlg.querySelector('[name="days"]').disabled = free;
    };
    dlg.querySelector('[name="plan"]').addEventListener("change", toggleDuration);
    return;
  }
  if (btn.dataset.act === "delete") {
    dialog(`
      <form>
        <h3>Konto endgültig löschen</h3>
        <p>Löscht <strong>${esc(user.email)}</strong> mit allen Pflanzen, Fotos, Lizenzen ohne Schlüssel und dem Stripe-Kundeneintrag (wie „Konto löschen“ in der App). <strong>Nicht umkehrbar.</strong> Rechnungen bleiben wegen der Aufbewahrungspflicht erhalten.</p>
        <label class="mono">Zur Bestätigung die E-Mail-Adresse eintippen<input class="input" name="confirm" autocomplete="off" required /></label>
        <div class="error" role="alert"></div>
        <div class="row"><button type="button" class="btn" data-cancel>Abbrechen</button><button type="submit" class="btn btn--danger">Endgültig löschen</button></div>
      </form>`, async (form) => {
      if (form.confirm.value.trim().toLowerCase() !== String(user.email).toLowerCase()) throw new Error("Die E-Mail-Adresse stimmt nicht überein.");
      await call(`/admin/users/${user.user_id}`, { method: "DELETE", body: { confirm_email: form.confirm.value.trim() } });
      toast(`${user.email} wurde gelöscht.`);
      await loadUsers();
    });
  }
}

// --- shared form parts -----------------------------------------------------------------

function planSelect(name, selected, withFree = false) {
  const plans = withFree ? ["free", "basic", "pro", "nursery"] : ["basic", "pro", "nursery"];
  return `<select class="input" name="${name}">${plans.map((p) => `<option value="${p}"${p === selected ? " selected" : ""}>${PLAN_NAMES[p]}</option>`).join("")}</select>`;
}

function durationSelect(name) {
  return `<select class="input" name="${name}">
    <option value="m1">1 Monat</option><option value="m3" selected>3 Monate</option><option value="m6">6 Monate</option>
    <option value="m12">12 Monate</option><option value="unlimited">unbegrenzt</option><option value="days">Tage angeben…</option>
  </select>`;
}

// Shows the "days" field only when "Tage angeben…" is chosen.
function wireDuration(scope) {
  const select = scope.querySelector('[name="duration"]');
  const days = scope.querySelector("[data-days]");
  const update = () => { days.hidden = select.value !== "days"; };
  select.addEventListener("change", update);
  update();
}

function durationBody(form) {
  const v = form.duration.value;
  if (v === "unlimited") return { unlimited: true };
  if (v === "days") return { days: Number(form.days.value) };
  return { months: Number(v.slice(1)) };
}

function durationLabel(days) {
  if (days == null) return "unbegrenzt";
  const months = { 30: 1, 91: 3, 183: 6, 365: 12 }[days];
  return months ? `${months} Monat${months > 1 ? "e" : ""}` : `${days} Tage`;
}

// --- vouchers ------------------------------------------------------------------------

async function renderVouchers(view) {
  view.innerHTML = `
    <div class="card" style="margin-bottom:16px;">
      <h2>Gutscheine erstellen</h2>
      <form id="voucherForm">
        <div class="form-grid">
          <label class="mono">Tarif${planSelect("plan", "pro")}</label>
          <label class="mono">Dauer nach Einlösen${durationSelect("duration")}</label>
          <label class="mono" data-days hidden>Tage<input class="input" name="days" type="number" min="1" max="3650" value="30" /></label>
          <label class="mono">Art
            <select class="input" name="kind">
              <option value="single">Einzelcodes (je 1× einlösbar)</option>
              <option value="shared">Ein Code für viele (z. B. Flyer)</option>
            </select>
          </label>
          <label class="mono" data-single>Anzahl Codes<input class="input" name="count" type="number" min="1" max="10000" value="1" /></label>
          <label class="mono" data-shared hidden>Code-Text<input class="input" name="code" maxlength="40" placeholder="z. B. FRUEHLING26" autocomplete="off" /></label>
          <label class="mono" data-shared hidden>Höchstens Konten (leer = beliebig viele)<input class="input" name="max" type="number" min="1" /></label>
          <label class="mono">Einlösbar bis (optional)<input class="input" name="until" type="date" /></label>
          <label class="mono">Notiz (intern)<input class="input" name="note" maxlength="200" placeholder="z. B. Hanfjack Bestellungen" /></label>
          <label class="mono" data-single>Per E-Mail senden an (optional, nur bei 1 Code)<input class="input" name="email" type="email" /></label>
        </div>
        <div class="error" role="alert"></div>
        <button type="submit" class="btn btn--primary">Erstellen</button>
      </form>
      <div id="voucherResult"></div>
    </div>
    <h2>Bisherige Gutscheine</h2>
    <div class="table-wrap"><table>
      <thead><tr><th>Erstellt</th><th>Code</th><th>Tarif</th><th>Dauer</th><th class="num">Eingelöst</th><th>Einlösbar bis</th><th>Notiz</th><th>Status</th><th></th></tr></thead>
      <tbody id="voucherRows"><tr><td colspan="9" class="empty">Lädt…</td></tr></tbody>
    </table></div>`;
  const form = document.getElementById("voucherForm");
  wireDuration(form);
  const toggleKind = () => {
    const shared = form.kind.value === "shared";
    form.querySelectorAll("[data-shared]").forEach((el) => { el.hidden = !shared; });
    form.querySelectorAll("[data-single]").forEach((el) => { el.hidden = shared; });
  };
  form.kind.addEventListener("change", toggleKind);
  toggleKind();
  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    const button = form.querySelector('button[type="submit"]');
    const error = form.querySelector(".error");
    error.textContent = "";
    const shared = form.kind.value === "shared";
    const body = { plan: form.plan.value, ...durationBody(form), note: form.note.value, redeem_until: form.until.value || null };
    if (shared) {
      if (!form.code.value.trim()) { error.textContent = "Bitte einen Code-Text eingeben."; return; }
      body.code = form.code.value.trim();
      if (form.max.value) body.max_redemptions = Number(form.max.value);
    } else {
      body.count = Number(form.count.value || 1);
      if (form.email.value.trim()) {
        if (body.count !== 1) { error.textContent = "Per E-Mail lässt sich nur 1 Code verschicken."; return; }
        body.email = form.email.value.trim();
      }
    }
    button.disabled = true;
    try {
      const res = await call("/admin/vouchers", { method: "POST", body });
      showCodes(res, shared);
      toast(`${res.codes.length} Code${res.codes.length > 1 ? "s" : ""} erstellt.`);
      await loadVouchers();
    } catch (err) {
      error.textContent = voucherError(err.message);
    } finally {
      button.disabled = false;
    }
  });
  document.getElementById("voucherRows").addEventListener("click", onVoucherAction);
  await loadVouchers();
}

function voucherError(message) {
  if (message.includes("exists already")) return "Diesen Code gibt es schon. Bitte einen anderen Text wählen.";
  if (message.includes("code:")) return "Code-Text: 4 bis 32 Buchstaben oder Ziffern, Bindestriche erlaubt, keine Umlaute.";
  if (message.includes("SMTP")) return "E-Mail ist auf dem Server nicht eingerichtet.";
  if (message.includes("past")) return "Das Datum „Einlösbar bis“ liegt in der Vergangenheit.";
  return message;
}

function showCodes(res, shared) {
  const box = document.getElementById("voucherResult");
  const text = res.codes.join("\n");
  box.innerHTML = `
    <div class="codes">
      <p>${shared
        ? "Der Code ist gespeichert und bleibt in der Liste unten sichtbar."
        : "<strong>Diese Codes werden nur jetzt angezeigt.</strong> Gespeichert ist nur eine Prüfsumme. Bitte jetzt kopieren oder herunterladen."}${res.emailed ? " Der Code wurde per E-Mail verschickt." : ""}</p>
      <textarea class="input mono" readonly rows="${Math.min(12, res.codes.length + 1)}">${esc(text)}</textarea>
      <div class="row" style="justify-content:flex-start;">
        <button class="btn btn--sm" type="button" data-copy>Kopieren</button>
        <button class="btn btn--sm" type="button" data-download>Als Textdatei herunterladen</button>
      </div>
    </div>`;
  box.querySelector("[data-copy]").addEventListener("click", async () => {
    try { await navigator.clipboard.writeText(text); toast("Kopiert."); }
    catch (_) { box.querySelector("textarea").select(); toast("Bitte mit Strg+C kopieren."); }
  });
  box.querySelector("[data-download]").addEventListener("click", () => {
    const url = URL.createObjectURL(new Blob([text + "\n"], { type: "text/plain" }));
    const a = document.createElement("a");
    a.href = url;
    a.download = `sowling-gutscheine-${res.batch_id.slice(0, 8)}.txt`;
    a.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  });
}

let lastBatches = [];

async function loadVouchers() {
  lastBatches = await call("/admin/vouchers");
  const rows = document.getElementById("voucherRows");
  rows.innerHTML = lastBatches.length
    ? lastBatches.map((b) => {
      const revokedAll = b.revoked === b.codes;
      const expired = b.redeem_until && new Date(b.redeem_until) < new Date();
      const usedUp = b.kind === "shared" ? b.max_redemptions != null && b.redeemed >= b.max_redemptions : b.redeemed >= b.codes;
      const status = revokedAll ? ["gesperrt", "badge--off"] : expired ? ["abgelaufen", "badge--muted"] : usedUp ? ["aufgebraucht", "badge--muted"] : ["aktiv", ""];
      const redeemed = b.kind === "shared" ? `${fmtInt(b.redeemed)}${b.max_redemptions != null ? ` / ${fmtInt(b.max_redemptions)}` : ""}` : `${fmtInt(b.redeemed)} / ${fmtInt(b.codes)}`;
      return `
        <tr>
          <td>${esc(fmtDate(b.created_at))}</td>
          <td>${b.kind === "shared" ? `<strong>${esc(b.shared_code)}</strong>` : `${fmtInt(b.codes)} Einzelcode${b.codes > 1 ? "s" : ""}`}</td>
          <td>${esc(PLAN_NAMES[b.plan] || b.plan)}</td>
          <td>${esc(durationLabel(b.duration_days))}</td>
          <td class="num">${redeemed}</td>
          <td>${esc(fmtDateUtc(b.redeem_until))}</td>
          <td>${esc(b.note || "")}</td>
          <td><span class="badge ${status[1]}">${status[0]}</span>${b.revoked > 0 && !revokedAll ? `<div class="mono">${fmtInt(b.revoked)} gesperrt</div>` : ""}</td>
          <td class="actions">${revokedAll ? "" : `<button class="btn btn--sm btn--danger" data-revoke="${esc(b.batch_id)}">Sperren</button>`}</td>
        </tr>`;
    }).join("")
    : `<tr><td colspan="9" class="empty">Noch keine Gutscheine.</td></tr>`;
}

function onVoucherAction(e) {
  const id = e.target.closest("[data-revoke]")?.dataset.revoke;
  const batch = lastBatches.find((b) => b.batch_id === id);
  if (!batch) return;
  const label = batch.kind === "shared" ? `den Code ${batch.shared_code}` : `${batch.codes} Einzelcode${batch.codes > 1 ? "s" : ""}${batch.note ? ` („${batch.note}“)` : ""}`;
  dialog(`
    <form>
      <h3>Gutscheine sperren</h3>
      <p>Sperrt ${esc(label)}. Danach lässt sich nichts mehr einlösen. Wer schon eingelöst hat, behält den Tarif bis zum Ende der Laufzeit.</p>
      <div class="error" role="alert"></div>
      <div class="row"><button type="button" class="btn" data-cancel>Abbrechen</button><button type="submit" class="btn btn--danger">Sperren</button></div>
    </form>`, async () => {
    await call(`/admin/vouchers/${batch.batch_id}/revoke`, { method: "POST" });
    toast("Gesperrt.");
    await loadVouchers();
  });
}

// --- invoices ------------------------------------------------------------------------

const STATUS = { pending: ["wartet", "badge--warn"], created: ["erstellt, Versand offen", "badge--warn"], failed: ["fehlgeschlagen", "badge--off"] };

async function renderInvoices(view) {
  const rows = await call("/admin/invoices/problems");
  view.innerHTML = `
    <p class="mono" style="margin-top:0;">Rechnungen, die noch nicht verschickt sind. Der Server versucht es automatisch erneut (bis zu 12 Mal mit wachsenden Pausen); „Erneut versuchen“ startet sofort und setzt den Zähler zurück.</p>
    <div class="table-wrap"><table>
      <thead><tr><th>Bezahlt</th><th>Art</th><th>Kunde</th><th class="num">Betrag</th><th>Status</th><th>Letzter Fehler</th><th></th></tr></thead>
      <tbody id="invoiceRows">${rows.length
        ? rows.map((r) => `
          <tr>
            <td>${esc(fmtDateTime(r.paid_at))}<div class="mono">${esc(r.stripe_ref)}</div></td>
            <td>${r.kind === "desktop" ? "Desktop-Kauf" : "Abo"}</td>
            <td>${esc(r.email || "–")}</td>
            <td class="num">${esc(fmtEuro(r.amount_cents))}</td>
            <td><span class="badge ${STATUS[r.status]?.[1] || ""}">${esc(STATUS[r.status]?.[0] || r.status)}</span><div class="mono">${r.attempts} Versuche${r.status !== "failed" ? ` · nächster ${esc(fmtDateTime(r.next_attempt_at))}` : ""}</div></td>
            <td class="mono" style="max-width:360px; white-space:normal;">${esc(r.last_error || "–")}</td>
            <td class="actions"><button class="btn btn--sm" data-retry="${esc(r.invoice_id)}">Erneut versuchen</button></td>
          </tr>`).join("")
        : `<tr><td colspan="7" class="empty">Alles erledigt: Keine offenen Rechnungen.</td></tr>`}</tbody>
    </table></div>`;
  document.getElementById("invoiceRows").addEventListener("click", async (e) => {
    const id = e.target.closest("[data-retry]")?.dataset.retry;
    if (!id) return;
    await call(`/admin/invoices/${id}/retry`, { method: "POST" });
    toast("Wird in der nächsten Minute erneut versucht.");
    showTab();
  });
}

// --- audit -----------------------------------------------------------------------------

async function renderAudit(view) {
  const rows = await call("/admin/audit");
  view.innerHTML = `
    <div class="table-wrap"><table>
      <thead><tr><th>Zeit</th><th>Admin</th><th>Aktion</th><th>Betroffen</th><th>Details</th></tr></thead>
      <tbody>${rows.length
        ? rows.map((r) => `
          <tr><td>${esc(fmtDateTime(r.at))}</td><td>${esc(r.admin_email || "–")}</td><td>${esc(ACTIONS[r.action] || r.action)}</td><td>${esc(r.target || "–")}</td><td class="mono">${esc(r.detail || "")}</td></tr>`).join("")
        : `<tr><td colspan="5" class="empty">Noch keine Einträge.</td></tr>`}</tbody>
    </table></div>`;
}

renderLogin();
