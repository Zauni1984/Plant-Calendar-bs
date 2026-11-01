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
const ACTIONS = { login: "Anmeldung", disable: "Konto deaktiviert", enable: "Konto aktiviert", delete: "Konto gelöscht", invoice_retry: "Rechnung neu angestoßen" };

const esc = (v) => String(v ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
const fmtDate = (v) => (v ? new Date(v).toLocaleDateString("de-DE") : "–");
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
      <label>Passwort<input class="input" name="password" type="password" autocomplete="current-password" required /></label>
      <label>Code aus der App<input class="input code" name="code" inputmode="numeric" autocomplete="one-time-code" pattern="[0-9 ]{6,7}" maxlength="7" required /></label>
      <button class="btn btn--primary" type="submit">Anmelden</button>
      <div class="error" id="loginError" role="alert">${esc(message)}</div>
    </form>`;
  const form = document.getElementById("loginForm");
  form.email.focus();
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
  ({ overview: renderOverview, users: renderUsers, invoices: renderInvoices, audit: renderAudit })[tab](view).catch((err) => {
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
    parts.push(`<span class="badge badge--warn">Gutschein ${esc(PLAN_NAMES[u.voucher_plan] || u.voucher_plan)}${u.voucher_until ? ` bis ${esc(fmtDate(u.voucher_until))}` : " ∞"}</span>`);
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
