// Runs both inside the Tauri desktop app and as a plain web app: transport.js
// forwards to the Rust process or to the HTTP backend depending on the build.
import { invoke, convertFileSrc, openDialog as tauriOpenDialog, saveDialog as tauriSaveDialog, IS_TAURI, DESKTOP_ONLY } from "./api/transport.js";

// ── i18n: locales ──
// German is the fallback for every missing key, so it ships in the main
// bundle. The other nine are separate chunks loaded on demand: bundling all
// ten put ~660 KB of JSON into startup, of which a visitor needs one.
import localeDe from './locales/de.json';
const LOCALE_LOADERS = import.meta.glob(['./locales/*.json', '!./locales/de.json'], { import: 'default' });

// ── Theme system: restore saved theme before first paint ──
const THEMES = [
  { id: 'forest', label: 'Forest', preview: ['#091310','#0b3d2e','#6bffbc'] },
  { id: 'ocean', label: 'Ocean', preview: ['#0a1220','#1a3a6b','#64b5f6'] },
  { id: 'sunset', label: 'Sunset', preview: ['#1a1008','#6b3a1a','#ffb347'] },
  { id: 'lavender', label: 'Lavender', preview: ['#12101a','#4a2d7a','#b388ff'] },
  { id: 'rose', label: 'Rose', preview: ['#1a0e14','#6b2a4a','#ff80ab'] },
  { id: 'midnight', label: 'Midnight', preview: ['#0a0a0e','#2a2a4a','#a0c4ff'] },
  { id: 'earth', label: 'Earth', preview: ['#141008','#5a3e20','#d4a574'] },
  { id: 'arctic', label: 'Arctic', preview: ['#f0f4f8','#2d6a4f','#1b7a4a'] },
  { id: 'mono', label: 'Mono', preview: ['#000000','#ffffff','#ffffff'] },
];
let currentTheme = localStorage.getItem('plants_calendar_theme') || 'forest';
{
  const saved = localStorage.getItem('plants_calendar_theme');
  if (saved) document.documentElement.setAttribute('data-theme', saved);
}

// ── i18n System ──
const LANGUAGES = ['de', 'en', 'fr', 'la', 'es', 'pt', 'it', 'nl', 'pl', 'tr'];
const LANGUAGE_LABELS = {
  de: 'Deutsch', en: 'English', fr: 'Fran\u00e7ais', la: 'Latina',
  es: 'Espa\u00f1ol', pt: 'Portugu\u00eas', it: 'Italiano', nl: 'Nederlands',
  pl: 'Polski', tr: 'T\u00fcrk\u00e7e',
};
const fallbackLocale = localeDe;

/** Resolve a language's strings; German needs no request. */
async function loadLocale(lang) {
  if (lang === 'de') return localeDe;
  const loader = LOCALE_LOADERS[`./locales/${lang}.json`];
  if (!loader) return null;
  try {
    return await loader();
  } catch (err) {
    // A failed chunk load (offline, deploy in progress) must not break the
    // app: German stays usable for every key.
    console.warn(`locale ${lang} could not be loaded`, err);
    return null;
  }
}

let currentLang = localStorage.getItem('plants_calendar_lang') || 'de';
if (!LANGUAGES.includes(currentLang)) currentLang = 'de';
// Top-level await: the first render already needs the right strings.
let localeData = (await loadLocale(currentLang)) || fallbackLocale;
// index.html ships lang="de"; correct it before first paint for the saved language.
try { document.documentElement.setAttribute('lang', currentLang); } catch (_) {}

function t(key, ...args) {
  const str = localeData[key] || fallbackLocale[key] || key;
  return args.reduce((s, arg, i) => s.replace(`{${i}}`, String(arg)), str);
}

/**
 * BCP-47 tag for Intl / toLocaleString, derived from the active UI language.
 * Dates used to be formatted with a hardcoded localeTag() everywhere, so they
 * stayed German no matter which language the user picked.
 */
function localeTag() {
  switch (currentLang) {
    case 'de': return 'de-DE';
    case 'en': return 'en-GB';
    case 'la': return 'la';       // no CLDR data; Intl falls back to the default locale
    default: return currentLang;
  }
}

async function setLanguage(lang) {
  if (!LANGUAGES.includes(lang)) return;
  const data = await loadLocale(lang);
  if (!data) return; // keep the current language rather than half-switching
  currentLang = lang;
  localStorage.setItem('plants_calendar_lang', lang);
  localeData = data;
  // Keep the document language in sync for screen readers and hyphenation.
  document.documentElement.setAttribute('lang', lang);
  applyTheme(currentTheme);
}

// ── End i18n System ──

function getViewLabels() {
  return {
    calendar: t('nav.calendar'),
    plants: t('nav.plants'),
    locations: t('nav.locations'),
    tips: t('nav.tips'),
    weather: t('nav.weather'),
    devices: t('nav.devices'),
    billing: t('nav.billing'),
    settings: t('nav.settings'),
  };
}

function getMonthName(idx) {
  return t('month.' + idx);
}

function getWeekdayHeaders() {
  return [t('day.mo'), t('day.tu'), t('day.we'), t('day.th'), t('day.fr'), t('day.sa'), t('day.su')];
}

function renderFab() {
  const menu = document.getElementById('fabMenu');
  if (!menu) return;
  menu.innerHTML = `
    <button class="fab-option" data-fab-action="log">
      <span class="fab-option-icon">&#128221;</span>
      <span>${t('fab.log')}</span>
    </button>
    <button class="fab-option" data-fab-action="task">
      <span class="fab-option-icon">&#9989;</span>
      <span>${t('fab.task')}</span>
    </button>
    <button class="fab-option" data-fab-action="plant">
      <span class="fab-option-icon">&#127793;</span>
      <span>${t('fab.plant')}</span>
    </button>
  `;
}

function renderOnboardingOverlayText() {
  const overlay = document.getElementById('onboardingOverlay');
  if (!overlay) return;
  const h2 = overlay.querySelector('h2');
  const p = overlay.querySelector('p');
  const input = overlay.querySelector('#onboardPlantName');
  const btn = overlay.querySelector('#onboardSubmit');
  const colorLabel = overlay.querySelector('label[for="onboardPlantColor"],label.form-label');
  if (h2) h2.textContent = t('onboard.welcome');
  if (p) p.textContent = t('onboard.firstPlant');
  if (input) input.placeholder = t('onboard.plantName');
  if (btn) btn.textContent = t('onboard.submit');
}

function getThemeColors() {
  const s = getComputedStyle(document.documentElement);
  return {
    bg: s.getPropertyValue('--bg').trim(),
    onPrimary: s.getPropertyValue('--on-primary').trim() || '#ffffff',
    accent: s.getPropertyValue('--accent').trim(),
    accentDim: s.getPropertyValue('--accent-dim').trim(),
    accentRgb: s.getPropertyValue('--accent-rgb').trim(),
    text: s.getPropertyValue('--text').trim(),
    textMuted: s.getPropertyValue('--text-muted').trim(),
    textDim: s.getPropertyValue('--text-dim').trim(),
    textFaint: s.getPropertyValue('--text-faint').trim(),
    surface: s.getPropertyValue('--surface').trim(),
    cardSolid: s.getPropertyValue('--card-solid').trim(),
    cardHover: s.getPropertyValue('--card-hover').trim(),
    inputBg: s.getPropertyValue('--input-bg').trim(),
    border: s.getPropertyValue('--border').trim(),
    borderHover: s.getPropertyValue('--border-hover').trim(),
    borderActive: s.getPropertyValue('--border-active').trim(),
    danger: s.getPropertyValue('--danger').trim(),
    dangerBg: s.getPropertyValue('--danger-bg').trim(),
    warning: s.getPropertyValue('--warning').trim(),
    warningBg: s.getPropertyValue('--warning-bg').trim(),
    info: s.getPropertyValue('--info').trim(),
    infoBg: s.getPropertyValue('--info-bg').trim(),
    success: s.getPropertyValue('--success').trim(),
    successBg: s.getPropertyValue('--success-bg').trim(),
    primary: s.getPropertyValue('--primary').trim(),
    primaryHover: s.getPropertyValue('--primary-hover').trim(),
    badgeBg: s.getPropertyValue('--badge-bg').trim(),
    overlayBg: s.getPropertyValue('--overlay-bg').trim(),
  };
}
let TC = getThemeColors();

/**
 * Fields that update_plant_smart overwrites unconditionally on the Rust side.
 * Every caller must resend them, otherwise they are silently reset to NULL.
 */
function plantExtraFields(plant) {
  return {
    location_id: plant.location_id ?? null,
    lifecycle: plant.lifecycle ?? null,
    pot_size_liters: plant.pot_size_liters ?? null,
    soil_type: plant.soil_type ?? null,
  };
}

function applyTheme(themeId) {
  currentTheme = themeId;
  document.documentElement.setAttribute('data-theme', themeId);
  localStorage.setItem('plants_calendar_theme', themeId);
  TC = getThemeColors();
  // Re-render all views that use TC colors
  try {
    renderAppNav();
    renderCalendarToolbar();
    renderPlantContextBar();
    renderTimelinePanel();
    renderPlants();
    renderTasks();
    renderLogs();
    renderDetailPanel();
    renderReminderPanel();
    renderSettingsPanel();
    if (typeof renderLocationsView === 'function') renderLocationsView();
    if (typeof renderTipsView === 'function') renderTipsView();
    if (typeof renderSensorWidget === 'function') renderSensorWidget();
    if (typeof renderDeviceManagementPanel === 'function') renderDeviceManagementPanel();
    if (typeof renderPhotoGallery === 'function') renderPhotoGallery();
    if (typeof renderGrowthAnalytics === 'function') renderGrowthAnalytics();
    if (typeof renderCareInsights === 'function') renderCareInsights();
    if (typeof renderWeatherDashboard === 'function' && currentView === 'weather') renderWeatherDashboard();
    renderFab();
    renderOnboardingOverlayText();
  } catch (_) {}
}

const root = document.querySelector("#app");

root.innerHTML = `
<div class="app-shell">
  <header class="app-toolbar" id="appToolbar">
    <div class="app-brand">Plants Calendar<span id="profileIndicator" style="margin-left:8px;display:inline-flex;align-items:center;gap:4px;vertical-align:middle;"></span></div>
    <nav class="app-nav" id="appNav"></nav>
    <div class="plant-context-bar" id="plantContextBar"></div>
  </header>

  <div class="app-content" id="appContent">
    <!-- Calendar View (default) -->
    <div id="calendarView" class="app-view app-view--active">
      <div class="cal-toolbar" id="calendarToolbar"></div>
      <div id="vacationBanner"></div>
      <div id="timelinePanel"></div>
      <div class="day-detail" id="dayDetailPanel"></div>
    </div>

    <!-- Plants View -->
    <div id="plantsView" class="app-view">
      <div class="plants-toolbar" id="plantsToolbar">
        <input type="text" id="plantSearch" class="form-input" placeholder="${t('plants.search')}" />
        <select id="plantPhaseFilter" class="form-select">
          <option value="all">${t('plants.allPhases')}</option>
          <option value="seed">${t('phase.seed')}</option><option value="veg">${t('phase.veg')}</option>
          <option value="flower">${t('phase.flower')}</option><option value="harvest">${t('phase.harvest')}</option>
          <option value="dry">${t('phase.dry')}</option><option value="cure">${t('phase.cure')}</option>
          <option value="custom">${t('phase.custom')}</option>
        </select>
        <select id="plantStateFilter" class="form-select">
          <option value="all">${t('plants.allStates')}</option>
          <option value="active">${t('plant.active')}</option><option value="ended">${t('plant.ended')}</option>
          <option value="inactive">${t('plants.inactive')}</option><option value="archived">${t('plant.archived')}</option>
        </select>
        <select id="plantSort" class="form-select">
          <option value="updated_desc">${t('plants.lastUpdated')}</option>
          <option value="name_asc">${t('plants.nameAZ')}</option>
          <option value="phase">${t('plant.phase')}</option><option value="week_desc">${t('plant.week')}</option>
        </select>
        <button id="resetPlantFilters" class="btn btn--ghost btn--sm">${t('plants.resetFilters')}</button>
        <span id="plantsFilterStatus" style="color:${TC.textDim};font-size:12px;margin-left:auto;"></span>
      </div>
      <div id="selectedPlantLabel" style="color:${TC.textMuted};font-size:13px;margin-bottom:10px;"></div>
      <div id="plantsList" class="plants-grid"></div>
      <div id="detailPanel" class="view-card" style="margin-top:14px;"></div>
      <div id="reminderPanel" class="view-card" style="margin-top:10px;"></div>
      <div id="photoGallery" class="view-card" style="margin-top:10px;"></div>
      <div id="analyticsPanel" style="margin-top:14px;"></div>
      <div id="careInsightsPanel" style="margin-top:14px;"></div>
      <div class="tasks-logs-grid" style="margin-top:14px;">
        <div><div class="section-title">${t('nav.tasksSection')}</div><div id="tasksList"></div></div>
        <div><div class="section-title">${t('nav.logsSection')}</div><div id="logsList"></div></div>
      </div>
      <div id="phaseTaskSetBar" style="margin-top:10px;"></div>
    </div>

    <!-- Locations View -->
    <div id="locationsView" class="app-view">
      <div id="locationsList"></div>
    </div>

    <!-- Tips View -->
    <div id="tipsView" class="app-view">
      <div id="seasonAlerts"></div>
      <div id="tipsContent"></div>
    </div>

    <!-- Weather View -->
    <div id="weatherView" class="app-view">
      <div id="weatherDashboard"></div>
    </div>

    <!-- Devices View -->
    <div id="devicesView" class="app-view">
      <div id="sensorWidget" style="display:none;"></div>
      <div id="deviceManagementPanel" class="view-card" style="margin-top:14px;"></div>
    </div>

    <!-- Billing View -->
    <div id="billingView" class="app-view">
      <div id="subscriptionPanel" class="view-card"></div>
      <div id="mvpReadinessPanel" class="view-card" style="margin-top:14px;"></div>
    </div>

    <!-- Settings View -->
    <div id="settingsView" class="app-view">
      <div class="view-card">
        <h3>${t('account.title')}</h3>
        <div id="authStatusDisplay" style="margin-bottom:10px;"></div>
        <div style="display:flex;gap:6px;flex-wrap:wrap;">
          <button id="openAuthModal" class="btn btn--primary btn--sm">${t('account.signInOrRegister')}</button>
          ${IS_TAURI ? `<button id="bootstrap" class="btn btn--sm">${t('account.startGuest')}</button>
          <button id="syncNow" class="btn btn--sm">${t('account.syncNow')}</button>` : `<button id="webLogout" class="btn btn--sm" style="display:none;">${t('account.signOut')}</button>`}
          <button id="me" class="btn btn--sm">${t('account.fetchProfile')}</button>
        </div>
      </div>
      <!-- Hidden inputs for auth (used by handlers) -->
      <div style="display:none;">
        <input id="backendUrl" value="http://localhost:8080" />
        <input id="email" /><input id="password" type="password" />
      </div>
      <div id="settingsPanel" class="view-card" style="margin-top:14px;"></div>
      <div id="climateSettingsPanel" class="view-card" style="margin-top:14px;"></div>
      <div id="liveStatusCard" class="view-card" style="margin-top:14px;">
        <h3>${t('account.liveStatus')}</h3>
        <div id="status"></div>
      </div>
      <div id="onboardingInfoPanel" class="view-card" style="margin-top:14px;"></div>
    </div>
  </div>

  <!-- Onboarding Overlay -->
  <div id="onboardingOverlay" class="onboarding-overlay" style="display:none;">
    <div class="onboarding-card">
      <h2>${t('onboard.welcome')}</h2>
      <p>${t('onboard.firstPlant')}</p>
      <div class="form-group">
        <input id="onboardPlantName" class="form-input" placeholder="${t('onboard.plantName')}" />
      </div>
      <div class="form-row" style="margin-bottom:12px;">
        <div>
          <label class="form-label">${t('onboard.color')}</label>
          <input id="onboardPlantColor" type="color" value="#0B3D2E" class="form-input" />
        </div>
        <div>
          <label class="form-label">${t('onboard.phase')}</label>
          <select id="onboardPlantPhase" class="form-select">
            <option value="seed">${t('phase.seed')}</option>
            <option value="veg" selected>${t('phase.veg')}</option>
            <option value="flower">${t('phase.flower')}</option>
            <option value="harvest">${t('phase.harvest')}</option>
            <option value="dry">${t('phase.dry')}</option>
          </select>
        </div>
      </div>
      <div id="onboardError" class="form-error" style="display:none;"></div>
      <button id="onboardSubmit" class="btn btn--primary" style="width:100%;padding:12px;">${t('onboard.submit')}</button>
    </div>
  </div>

  <!-- FAB -->
  <div class="fab-container" id="fabContainer">
    <div class="fab-menu" id="fabMenu">
      <button class="fab-option" data-fab-action="log">
        <span class="fab-option-icon">&#128221;</span>
        <span>Log erstellen</span>
      </button>
      <button class="fab-option" data-fab-action="task">
        <span class="fab-option-icon">&#9989;</span>
        <span>Task erstellen</span>
      </button>
      <button class="fab-option" data-fab-action="plant">
        <span class="fab-option-icon">&#127793;</span>
        <span>Pflanze anlegen</span>
      </button>
    </div>
    <button class="fab-main" id="fabMain">+</button>
  </div>

  <!-- Modal -->
  <div id="modalOverlay" class="modal-overlay" style="display:none;">
    <div class="modal-content">
      <div class="modal-header">
        <h3 id="modalTitle"></h3>
        <button class="modal-close" id="modalClose">&times;</button>
      </div>
      <div class="modal-body" id="modalBody"></div>
      <div class="modal-footer" id="modalFooter"></div>
    </div>
  </div>

  <!-- Loading Overlay -->
  <div id="loadingOverlay" class="loading-overlay" style="display:none;">
    <div class="loading-spinner"></div>
    <div class="loading-text">${t('common.loading')}</div>
  </div>

  <!-- Debug -->
  <pre id="output" style="display:none;position:fixed;bottom:0;left:0;right:0;max-height:200px;overflow:auto;background:#000;color:#0f0;font-size:11px;padding:8px;z-index:300;"></pre>
</div>
`;

const $ = (id) => document.getElementById(id);

// ── GDPR consent (Phase C2) ───────────────────────────────────────────────
// Consent type literals — must match `CONSENT_TYPES` in src-tauri/src/main.rs.
const CONSENT_TYPES = ["cloud_sync", "weather", "devices", "billing_stripe"];
const CONSENT_POLICY_VERSION = "1.0";
let consents = {}; // { cloud_sync: bool, weather: bool, devices: bool, billing_stripe: bool }
let consentsLoaded = false;
let consentModalShown = false;
const consentMissingToastShownThisSession = new Set();

async function loadConsents() {
  try {
    const records = await invoke("list_consents_smart");
    const latestByType = {};
    for (const r of Array.isArray(records) ? records : []) {
      const key = r.consent_type;
      if (!key) continue;
      if (!latestByType[key] || new Date(r.updated_at) > new Date(latestByType[key].updated_at)) {
        latestByType[key] = r;
      }
    }
    consents = {};
    for (const ct of CONSENT_TYPES) {
      consents[ct] = !!(latestByType[ct] && latestByType[ct].granted);
    }
    consentsLoaded = true;
  } catch (_) {
    consents = {};
    for (const ct of CONSENT_TYPES) consents[ct] = false;
    consentsLoaded = false;
  }
}

function hasConsent(type) {
  return !!consents[type];
}

async function setConsent(type, granted) {
  if (!CONSENT_TYPES.includes(type)) return;
  try {
    await invoke("set_consent_smart", { consentType: type, granted: !!granted });
    consents[type] = !!granted;
  } catch (err) {
    console.warn("set_consent_smart failed", err);
  }
}

function consentHasAnyRecord() {
  // Returns true if consentsLoaded and at least one record was actually fetched
  // (i.e. the user has made a choice before). We treat "no records at all" as
  // first-run so we show the modal.
  return consentsLoaded && CONSENT_TYPES.some(ct => ct in consents && consents[ct] !== undefined);
}

function notifyConsentMissing(consentType, messageKey) {
  const toastKey = `missing:${consentType}`;
  if (consentMissingToastShownThisSession.has(toastKey)) return;
  consentMissingToastShownThisSession.add(toastKey);
  try {
    // Simple toast: use the status output box + log + a mild alert fallback.
    const msg = t(messageKey);
    const statusEl = document.getElementById('status');
    if (statusEl) statusEl.textContent = msg;
    if (typeof logOut === 'function') logOut(msg);
    console.info('[consent]', msg);
  } catch (_) {}
}

async function openPrivacyDoc() {
  const filename = currentLang === 'de' ? 'PRIVACY_POLICY_DE.md' : 'PRIVACY_POLICY_EN.md';
  try {
    const abs = await invoke("resolve_doc_path", { filename });
    await invoke("open_external_path", { path: abs });
  } catch (err) {
    try { window.open(`../docs/${filename}`, '_blank'); } catch (_) {}
  }
}

async function openImprintDoc() {
  try {
    const abs = await invoke("resolve_doc_path", { filename: 'IMPRESSUM_DE.md' });
    await invoke("open_external_path", { path: abs });
  } catch (err) {
    try { window.open(`../docs/IMPRESSUM_DE.md`, '_blank'); } catch (_) {}
  }
}

function renderConsentModal() {
  if (consentModalShown) return;
  consentModalShown = true;
  const overlay = document.createElement('div');
  overlay.id = 'consentOverlay';
  overlay.style.cssText = 'position:fixed;inset:0;background:rgba(0,0,0,0.78);z-index:10000;display:flex;align-items:center;justify-content:center;padding:20px;';
  const rows = consentRows();
  overlay.innerHTML = `
    <div style="max-width:520px;width:100%;background:${TC.cardSolid || '#0b1a14'};border:1px solid rgba(${TC.accentRgb},0.2);border-radius:16px;padding:24px;color:${TC.text};max-height:90vh;overflow:auto;">
      <h2 style="margin:0 0 8px 0;color:${TC.accent};font-size:20px;">${escapeHtml(t('consent.title'))}</h2>
      <p style="margin:0 0 16px 0;color:${TC.textMuted};font-size:13px;line-height:1.5;">${escapeHtml(t('consent.intro'))}</p>
      <div style="display:grid;gap:12px;">
        ${rows.map(r => `
          <label style="display:flex;gap:12px;align-items:flex-start;padding:12px;border-radius:10px;background:${TC.surface};border:1px solid rgba(${TC.accentRgb},0.08);cursor:pointer;">
            <input type="checkbox" data-consent-toggle="${r.key}" style="margin-top:3px;" />
            <div>
              <div style="font-weight:600;">${escapeHtml(t(r.t))}</div>
              <div style="font-size:12px;color:${TC.textMuted};margin-top:2px;">${escapeHtml(t(r.d))}</div>
            </div>
          </label>
        `).join('')}
      </div>
      <div style="margin-top:14px;display:flex;gap:10px;flex-wrap:wrap;align-items:center;">
        <button type="button" id="consentOpenPolicy" class="btn btn--ghost btn--sm">${escapeHtml(t('consent.policyLink'))}</button>
        <div style="flex:1;"></div>
        <button type="button" id="consentSave" class="btn btn--primary">${escapeHtml(t('consent.save'))}</button>
      </div>
    </div>
  `;
  document.body.appendChild(overlay);
  overlay.querySelector('#consentOpenPolicy')?.addEventListener('click', openPrivacyDoc);
  overlay.querySelector('#consentSave')?.addEventListener('click', async () => {
    const checks = overlay.querySelectorAll('[data-consent-toggle]');
    for (const cb of checks) {
      await setConsent(cb.dataset.consentToggle, cb.checked);
    }
    overlay.remove();
    try { renderSettingsPanel(); } catch (_) {}
  });
}

/**
 * Consent choices offered in this build.
 *
 * In the browser, the server is where the data lives — the app cannot work
 * without it — so storing it there is contract performance (GDPR Art. 6(1)(b)),
 * not consent. Offering "cloud sync" as an opt-out would misdescribe that.
 * Device integrations do not exist in the browser at all.
 */
function consentRows() {
  const all = [
    { key: 'cloud_sync', t: 'consent.cloudSync', d: 'consent.cloudSyncDesc' },
    { key: 'weather', t: 'consent.weather', d: 'consent.weatherDesc' },
    { key: 'devices', t: 'consent.devices', d: 'consent.devicesDesc' },
    { key: 'billing_stripe', t: 'consent.billing', d: 'consent.billingDesc' },
  ];
  return IS_TAURI ? all : all.filter((r) => r.key !== 'cloud_sync' && r.key !== 'devices');
}

async function maybeShowConsentModal() {
  if (!consentsLoaded) await loadConsents();
  // If no record for any of the four types, this is first-run → show modal.
  // We key off the "were any records returned from the backend/local store"
  // heuristic: if all four are false AND we have no row in the local store,
  // show the modal. For simplicity, we check the raw list length.
  try {
    const records = await invoke("list_consents_smart");
    const recorded = new Set((Array.isArray(records) ? records : []).map(r => r.consent_type));
    const missingAll = CONSENT_TYPES.every(ct => !recorded.has(ct));
    if (missingAll) renderConsentModal();
  } catch (_) {}
}

function renderConsentToggles(containerId) {
  const el = document.getElementById(containerId);
  if (!el) return;
  const rows = consentRows();
  el.innerHTML = rows.map(r => `
    <label style="display:flex;gap:12px;align-items:flex-start;padding:10px;border-radius:10px;background:${TC.surface};border:1px solid rgba(${TC.accentRgb},0.06);cursor:pointer;">
      <input type="checkbox" data-settings-consent="${r.key}" ${hasConsent(r.key) ? 'checked' : ''} />
      <div>
        <div style="font-weight:600;">${escapeHtml(t(r.t))}</div>
        <div style="font-size:12px;color:${TC.textMuted};margin-top:2px;">${escapeHtml(t(r.d))}</div>
      </div>
    </label>
  `).join('');
  el.querySelectorAll('[data-settings-consent]').forEach(cb => {
    cb.addEventListener('change', async () => {
      const type = cb.dataset.settingsConsent;
      await setConsent(type, cb.checked);
      if (!cb.checked) {
        try { const s = document.getElementById('status'); if (s) s.textContent = t('consent.revokedToast'); } catch (_) {}
      }
    });
  });
}

const RETENTION_OPTIONS = [
  { days: 30, key: 'retention.30d' },
  { days: 90, key: 'retention.90d' },
  { days: 180, key: 'retention.180d' },
  { days: 365, key: 'retention.365d' },
  { days: 730, key: 'retention.2y' },
  { days: 1825, key: 'retention.5y' },
];

function getStoredRetentionDays() {
  const raw = parseInt(localStorage.getItem('plants_calendar_retention_days') || '365', 10);
  return Number.isFinite(raw) && raw > 0 ? raw : 365;
}

async function setRetentionDays(days) {
  localStorage.setItem('plants_calendar_retention_days', String(days));
  try {
    await invoke("prune_weather_history_cmd", { days });
  } catch (err) {
    console.warn("prune_weather_history_cmd failed", err);
  }
}

async function handleExportJson() {
  try {
    const path = await tauriSaveDialog({
      defaultPath: `plants-calendar-export-${new Date().toISOString().slice(0, 10)}.json`,
      filters: [{ name: 'JSON', extensions: ['json'] }],
    });
    if (!path) return;
    const saved = await invoke("export_account_json", { path });
    try { const s = document.getElementById('status'); if (s) s.textContent = t('settings.exportSaved', saved); } catch (_) {}
  } catch (err) {
    console.error("export_account_json failed", err);
    try { const s = document.getElementById('status'); if (s) s.textContent = String(err); } catch (_) {}
  }
}
// ── End GDPR consent ──────────────────────────────────────────────────────

function escapeHtml(str) {
  if (str == null) return '';
  return String(str)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

function sanitizeColorTag(color) {
  if (color && /^#[0-9a-fA-F]{3,8}$/.test(color)) return color;
  return '#888888';
}

// Convert #rgb or #rrggbb to "r,g,b" string for use in rgba().
function hexToRgb(hex) {
  if (!hex || typeof hex !== 'string') return '136,136,136';
  let h = hex.replace('#', '');
  if (h.length === 3) h = h.split('').map(c => c + c).join('');
  if (h.length < 6) return '136,136,136';
  const r = parseInt(h.slice(0, 2), 16);
  const g = parseInt(h.slice(2, 4), 16);
  const b = parseInt(h.slice(4, 6), 16);
  if ([r, g, b].some(Number.isNaN)) return '136,136,136';
  return `${r},${g},${b}`;
}

/**
 * Calculate moon phase for a given date.
 * Returns { phase: 0-7, illumination: 0-1, name: string, emoji: string, gardenTip: string }
 * Phases: 0=New, 1=Waxing Crescent, 2=First Quarter, 3=Waxing Gibbous,
 *         4=Full, 5=Waning Gibbous, 6=Last Quarter, 7=Waning Crescent
 */

// ── Notification Sounds ─────────────────────────────────────
// Short tones generated via Web Audio API — no external files needed.
let notifSoundEnabled = localStorage.getItem('plants_calendar_sound') !== 'off';

// Shared AudioContext — created once, reused for all sounds.
// Needs to be resumed after first user gesture (browser policy).
let _audioCtx = null;
function getAudioContext() {
  if (!_audioCtx) {
    _audioCtx = new (window.AudioContext || window.webkitAudioContext)();
  }
  if (_audioCtx.state === 'suspended') {
    _audioCtx.resume();
  }
  return _audioCtx;
}
// Resume on first user interaction
document.addEventListener('click', () => { try { getAudioContext(); } catch(_){} }, { once: true });

function playNotificationSound(type = 'default') {
  if (!notifSoundEnabled) return;
  try {
    const ctx = getAudioContext();
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.connect(gain);
    gain.connect(ctx.destination);

    if (type === 'success') {
      osc.type = 'sine';
      osc.frequency.setValueAtTime(660, ctx.currentTime);
      osc.frequency.setValueAtTime(880, ctx.currentTime + 0.1);
      gain.gain.setValueAtTime(0.15, ctx.currentTime);
      gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.3);
      osc.start(ctx.currentTime);
      osc.stop(ctx.currentTime + 0.3);
    } else if (type === 'warning') {
      osc.type = 'triangle';
      osc.frequency.setValueAtTime(520, ctx.currentTime);
      gain.gain.setValueAtTime(0.12, ctx.currentTime);
      gain.gain.setValueAtTime(0.001, ctx.currentTime + 0.08);
      gain.gain.setValueAtTime(0.12, ctx.currentTime + 0.15);
      gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.25);
      osc.start(ctx.currentTime);
      osc.stop(ctx.currentTime + 0.25);
    } else if (type === 'reminder') {
      osc.type = 'sine';
      osc.frequency.setValueAtTime(784, ctx.currentTime);
      gain.gain.setValueAtTime(0.18, ctx.currentTime);
      gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.5);
      osc.start(ctx.currentTime);
      osc.stop(ctx.currentTime + 0.5);
    } else {
      osc.type = 'sine';
      osc.frequency.setValueAtTime(700, ctx.currentTime);
      gain.gain.setValueAtTime(0.1, ctx.currentTime);
      gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.2);
      osc.start(ctx.currentTime);
      osc.stop(ctx.currentTime + 0.2);
    }
  } catch (_) {}
}

function getMoonPhase(date) {
  // Reference: known new moon on 2000-01-06 18:14 UTC
  const knownNew = new Date('2000-01-06T18:14:00Z').getTime();
  const synodicMonth = 29.53058770576; // days
  const msPerDay = 86400000;

  const daysSinceNew = (date.getTime() - knownNew) / msPerDay;
  const cycles = daysSinceNew / synodicMonth;
  const phase = (cycles - Math.floor(cycles)); // 0.0 to 1.0

  const phaseIndex = Math.round(phase * 8) % 8;
  const illumination = 0.5 * (1 - Math.cos(2 * Math.PI * phase));

  const emojis = ['\u{1F311}', '\u{1F312}', '\u{1F313}', '\u{1F314}', '\u{1F315}', '\u{1F316}', '\u{1F317}', '\u{1F318}'];


  // Zodiac sign (simplified — 2.5 days per sign through the cycle)
  const zodiacIndex = Math.floor((phase * 12)) % 12;
  const zodiacSymbols = ['\u2648', '\u2649', '\u264A', '\u264B', '\u264C', '\u264F', '\u264E', '\u264F', '\u2650', '\u2651', '\u2652', '\u2653'];
  // Element groups for gardening
  // Element group per sign, cycling fire -> earth -> air -> water.
  const elementKeys = ['fire', 'earth', 'air', 'water'];

  return {
    phase: phaseIndex,
    phaseFloat: phase,
    illumination,
    name: t(`moon.phase.${phaseIndex}`),
    emoji: emojis[phaseIndex],
    gardenTip: t(`moon.tip.${phaseIndex}`),
    zodiac: `${zodiacSymbols[zodiacIndex]} ${t(`zodiac.${zodiacIndex}`)}`,
    element: t(`moon.element.${elementKeys[zodiacIndex % 4]}`),
  };
}

/**
 * Simplified Chinese lunar calendar date.
 * Returns { year: number, month: number, day: number, animal: string, element: string }
 */
function getChineseDate(date) {
  // Lunar New Year dates (approximate)
  const lunarNewYears = [
    { year: 2024, start: new Date('2024-02-10'), animal: 'dragon', element: 'wood' },
    { year: 2025, start: new Date('2025-01-29'), animal: 'snake', element: 'wood' },
    { year: 2026, start: new Date('2026-02-17'), animal: 'horse', element: 'fire' },
    { year: 2027, start: new Date('2027-02-06'), animal: 'goat', element: 'fire' },
    { year: 2028, start: new Date('2028-01-26'), animal: 'monkey', element: 'earth' },
  ];

  // Find current Chinese year
  let cYear = lunarNewYears[0];
  for (const lny of lunarNewYears) {
    if (date >= lny.start) cYear = lny;
  }

  // Approximate lunar month and day (synodic month = 29.53 days)
  const daysSinceNewYear = Math.floor((date - cYear.start) / 86400000);
  const lunarMonth = Math.floor(daysSinceNewYear / 29.53) + 1;
  const lunarDay = Math.floor(daysSinceNewYear % 29.53) + 1;

  return {
    year: cYear.year,
    month: Math.max(1, Math.min(12, lunarMonth)),
    day: Math.max(1, Math.min(30, lunarDay)),
    animal: cYear.animal,
    element: cYear.element,
    animalLabel: t(`cal.chinese.animal.${cYear.animal}`),
    elementLabel: t(`cal.chinese.element.${cYear.element}`),
  };
}

function getPersianDate(date) {
  const gy = date.getFullYear();
  const gm = date.getMonth() + 1;
  const gd = date.getDate();
  const g_d_m = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
  let gy2 = (gm > 2) ? (gy + 1) : gy;
  let days = 355666 + (365 * gy) + Math.floor((gy2 + 3) / 4) - Math.floor((gy2 + 99) / 100) + Math.floor((gy2 + 399) / 400) + gd + g_d_m[gm - 1];
  let jy = -1595 + (33 * Math.floor(days / 12053));
  days %= 12053;
  jy += 4 * Math.floor(days / 1461);
  days %= 1461;
  if (days > 365) {
    jy += Math.floor((days - 1) / 365);
    days = (days - 1) % 365;
  }
  let jm, jd;
  if (days < 186) {
    jm = 1 + Math.floor(days / 31);
    jd = 1 + (days % 31);
  } else {
    jm = 7 + Math.floor((days - 186) / 30);
    jd = 1 + ((days - 186) % 30);
  }
  const persianMonths = ['Farvardin','Ordibehesht','Khordad','Tir','Mordad','Shahrivar','Mehr','Aban','Azar','Dey','Bahman','Esfand'];
  return { year: jy, month: jm, day: jd, monthName: persianMonths[jm - 1] };
}

function getJulianDate(date) {
  const gy = date.getFullYear();
  const century = Math.floor(gy / 100);
  const correction = century - Math.floor(century / 4) - 2;
  const julian = new Date(date);
  julian.setDate(julian.getDate() - correction);
  const monthNames = ['Januar','Februar','M\u00e4rz','April','Mai','Juni','Juli','August','September','Oktober','November','Dezember'];
  return {
    year: julian.getFullYear(),
    month: julian.getMonth() + 1,
    day: julian.getDate(),
    monthName: monthNames[julian.getMonth()],
    correction: correction,
  };
}

function getIslamicDate(date) {
  const gy = date.getFullYear();
  const gm = date.getMonth() + 1;
  const gd = date.getDate();
  const a = Math.floor((14 - gm) / 12);
  const y = gy + 4800 - a;
  const m = gm + 12 * a - 3;
  let jdn = gd + Math.floor((153 * m + 2) / 5) + 365 * y + Math.floor(y / 4) - Math.floor(y / 100) + Math.floor(y / 400) - 32045;
  const l = jdn - 1948440 + 10632;
  const n = Math.floor((l - 1) / 10631);
  const l2 = l - 10631 * n + 354;
  const j = Math.floor((10985 - l2) / 5316) * Math.floor((50 * l2) / 17719) + Math.floor(l2 / 5670) * Math.floor((43 * l2) / 15238);
  const l3 = l2 - Math.floor((30 - j) / 15) * Math.floor((17719 * j) / 50) - Math.floor(j / 16) * Math.floor((15238 * j) / 43) + 29;
  const im = Math.floor((24 * l3) / 709);
  const id = l3 - Math.floor((709 * im) / 24);
  const iy = 30 * n + j - 30;
  const islamicMonths = ['Muharram','Safar','Rabi al-Awwal','Rabi al-Thani','Jumada al-Ula','Jumada al-Thani','Rajab','Shaban','Ramadan','Shawwal','Dhu al-Qadah','Dhu al-Hijjah'];
  return { year: iy, month: im, day: id, monthName: islamicMonths[im - 1] };
}

function getJewishDate(date) {
  const gy = date.getFullYear();
  const gm = date.getMonth() + 1;
  const gd = date.getDate();
  const a = Math.floor((14 - gm) / 12);
  const y = gy + 4800 - a;
  const m = gm + 12 * a - 3;
  let jdn = gd + Math.floor((153 * m + 2) / 5) + 365 * y + Math.floor(y / 4) - Math.floor(y / 100) + Math.floor(y / 400) - 32045;
  const refjdn = 2450739;
  const refyear = 5758;
  let approxYear = refyear + Math.floor((jdn - refjdn) / 365.25);
  const hebrewMonths = ['Tischri','Cheschwan','Kislew','Tevet','Schwat','Adar','Nisan','Ijjar','Siwan','Tammus','Aw','Elul'];
  const dayOfYear = Math.floor((jdn - refjdn) % 365.25);
  const hm = Math.floor(dayOfYear / 29.5) % 12;
  const hd = Math.floor(dayOfYear % 29.5) + 1;
  return {
    year: approxYear,
    month: hm + 1,
    day: Math.max(1, Math.min(30, hd)),
    monthName: hebrewMonths[hm],
  };
}

function getBauernkalenderInfo(date) {
  const month = date.getMonth() + 1;
  const day = date.getDate();
  // Dates that have a lostag entry; the name/saying live in the locale files
  // so the farmer's calendar is translated like the rest of the UI.
  const lostagDates = new Set(['1-1', '1-6', '1-20', '2-2', '2-14', '2-24', '3-17', '3-19', '3-21', '4-23', '4-25', '5-1', '5-11', '5-12', '5-13', '5-14', '5-15', '6-11', '6-21', '6-24', '6-27', '7-2', '7-15', '7-20', '8-10', '8-15', '8-24', '9-1', '9-8', '9-22', '9-29', '10-4', '10-16', '10-28', '11-1', '11-2', '11-11', '11-25', '12-4', '12-21', '12-25', '12-31']);
  const monthWisdom = month >= 1 && month <= 12 ? t(`bauern.monthWisdom.${month}`) : '';
  const key = `${month}-${day}`;
  const lostag = lostagDates.has(key)
    ? { name: t(`bauern.${key}.name`), spruch: t(`bauern.${key}.spruch`) }
    : null;
  // Phenological season per month, resolved through the locale.
  const phenoSeason = month >= 1 && month <= 12 ? t(`pheno.season.${month}`) : '';
  return {
    lostag,
    monthWisdom,
    phenoSeason,
    spruch: lostag ? lostag.spruch : monthWisdom,
    name: lostag ? lostag.name : null,
  };
}

function plantAge(plant) {
  if (!plant.start_date) return null;
  const start = new Date(plant.start_date);
  const now = new Date();
  const years = now.getFullYear() - start.getFullYear();
  return years;
}

function isPerennial(plant) {
  return plant.lifecycle === 'perennial';
}
const output = $("output");
const statusBox = $("status");
const subscriptionPanel = $("subscriptionPanel");
const plantsList = $("plantsList");
const tasksList = $("tasksList");
const logsList = $("logsList");
const selectedPlantLabel = $("selectedPlantLabel");
const plantsFilterStatus = $("plantsFilterStatus");
const phaseTaskSetBar = $("phaseTaskSetBar");
const detailPanel = $("detailPanel");
const reminderPanel = $("reminderPanel");
const timelinePanel = $("timelinePanel");
const settingsPanel = $("settingsPanel");
const onboardingPanel = $("onboardingInfoPanel");
const mvpReadinessPanel = $("mvpReadinessPanel");

function showLoading(message) {
  if (!message) message = t('misc.loading');
  const el = document.getElementById('loadingOverlay');
  if (!el) return;
  const textEl = el.querySelector('.loading-text');
  if (textEl) textEl.textContent = message;
  el.style.display = '';
}
function hideLoading() {
  const el = document.getElementById('loadingOverlay');
  if (el) el.style.display = 'none';
}

let selectedPlantId = null;
let plants = [];
let tasks = [];
let logs = [];
let allTasks = [];
let allLogs = [];
let plantCatalog = [];
let climateConfig = null;
let weatherData = null;
let editingPlantId = null;
let editingTaskId = null;
let editingLogId = null;
let timelineMode = "month";
let timelineFocusKey = new Date().toISOString().slice(0, 10);
let calendarMode = localStorage.getItem('plants_calendar_mode') || 'standard';
// 'standard', 'moon', 'chinese', 'persian', 'julian', 'islamic', 'jewish', 'bauern'
let reminderPermissionState = typeof Notification === "undefined" ? "unsupported" : Notification.permission;
let homeTaskFilter = "all";
let homeTaskGrouping = "flat";
let homeTaskSort = "due";
let homeLogPlantFilter = "all";
let homeLogRangeFilter = "30d";
let homeLogTypeFilter = "all";
let homeMonthFocusKey = new Date().toISOString().slice(0, 10);
let plantSearchQuery = "";
let plantPhaseFilter = "all";
let plantStateFilter = "all";
let plantSort = "updated_desc";
let currentPlan = "free";
let currentPlantLimit = 1;
let currentPlanSource = "guest";
let checkoutState = { status: "idle", plan: null, message: "", checkoutUrl: null, updatedAt: null };

const CHECKOUT_HISTORY_STORAGE_KEY = "bs_checkout_history";
let checkoutHistory = [];
let subscriptionHistory = [];
let subscriptionHistorySource = "local";
let subscriptionStatusSummary = null;


let locations = [];
let devices = [];
let latestReadings = {};
let plantPhotos = [];
let sensorChartInstance = null;

// --- Vacation Mode ---
let vacationMode = JSON.parse(localStorage.getItem('plants_calendar_vacation') || 'null');
// { active: true, returnDate: "2026-05-01", multiplier: 1.5 }

// --- Local Profiles (multi-user) ---
let profiles = [];
let activeProfile = null; // LocalProfile or null

function saveVacationMode() {
  localStorage.setItem('plants_calendar_vacation', JSON.stringify(vacationMode));
}
function disableVacationMode() {
  vacationMode = null;
  saveVacationMode();
  renderCalendarToolbar();
  renderSettingsPanel();
  renderPlants();
}

// --- Smart Watering Calculator ---
function calculateWateringNeed(plant) {
  const entry = catalogById(plant.catalog_id);
  const location = locations.find(l => l.location_id === plant.location_id);

  // Base interval from catalog water need
  let baseHours = 48;
  if (entry) {
    if (entry.water === 'high') baseHours = 24;
    else if (entry.water === 'low') baseHours = 96;
    else baseHours = 48;
  }

  // Adjust for pot size (smaller pots dry faster)
  if (plant.pot_size_liters) {
    if (plant.pot_size_liters < 3) baseHours *= 0.6;
    else if (plant.pot_size_liters < 10) baseHours *= 0.8;
    else if (plant.pot_size_liters > 30) baseHours *= 1.3;
  }

  // Adjust for soil type
  if (plant.soil_type === 'kokos') baseHours *= 0.8;
  if (plant.soil_type === 'hydro') baseHours *= 0.5;
  if (plant.soil_type === 'steinwolle') baseHours *= 0.6;

  // Adjust for weather
  if (weatherData && weatherData.current) {
    const temp = weatherData.current.temperature_2m;
    const humidity = weatherData.current.relative_humidity_2m;
    const rain = weatherData.current.precipitation;

    if (temp > 30) baseHours *= 0.6;
    else if (temp > 25) baseHours *= 0.8;
    else if (temp < 10) baseHours *= 1.5;

    if (humidity > 70) baseHours *= 1.2;
    else if (humidity < 30) baseHours *= 0.7;

    if (rain > 5 && location && location.location_type === 'outdoor') baseHours *= 3.0;
    else if (rain > 2 && location && location.location_type === 'outdoor') baseHours *= 2.0;
  }

  // Adjust for indoor/outdoor
  if (location && (location.location_type === 'indoor' || location.location_type === 'growzelt')) {
    baseHours *= 0.9;
  }

  // Adjust for season
  const month = new Date().getMonth() + 1;
  if (month >= 6 && month <= 8) baseHours *= 0.7;
  if (month >= 11 || month <= 2) baseHours *= 1.4;

  // Vacation mode multiplier
  if (vacationMode && vacationMode.active) {
    baseHours *= (vacationMode.multiplier || 1.5);
  }

  // Find last watering — check water_liters OR water_method OR log_type=watering
  const lastWatering = allLogs
    .filter(l => l.plant_id === plant.plant_id && (
      (l.metrics && (l.metrics.water_liters || l.metrics.water_method)) ||
      l.log_type === 'watering' || l.log_type === 'Watering'
    ))
    .sort((a, b) => new Date(b.created_at) - new Date(a.created_at))[0];

  const hoursSinceWatering = lastWatering
    ? (Date.now() - new Date(lastWatering.created_at).getTime()) / 3600000
    : Infinity;

  const hoursUntilWater = Math.max(0, baseHours - hoursSinceWatering);

  let urgency = 'ok';
  let reason = '';
  if (hoursUntilWater <= 0) { urgency = 'overdue'; reason = 'Giessen ueberfaellig!'; }
  else if (hoursUntilWater < 6) { urgency = 'soon'; reason = 'In ~' + Math.round(hoursUntilWater) + 'h giessen'; }
  else if (hoursUntilWater < 24) { urgency = 'ok'; reason = 'Naechstes Giessen in ~' + Math.round(hoursUntilWater) + 'h'; }
  else { urgency = 'ok'; reason = 'Naechstes Giessen in ~' + Math.round(hoursUntilWater / 24) + ' Tagen'; }

  return { hoursUntilWater, urgency, reason, baseHours: Math.round(baseHours), hoursSinceWatering: Math.round(hoursSinceWatering) };
}

// --- Light Meter ---
async function measureLight() {
  try {
    const stream = await navigator.mediaDevices.getUserMedia({ video: { facingMode: 'environment' } });
    const video = document.createElement('video');
    video.srcObject = stream;
    await video.play();

    await new Promise(r => setTimeout(r, 1500));

    const canvas = document.createElement('canvas');
    canvas.width = video.videoWidth;
    canvas.height = video.videoHeight;
    const ctx = canvas.getContext('2d');
    ctx.drawImage(video, 0, 0);

    const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);
    const pixels = imageData.data;

    let totalBrightness = 0;
    for (let i = 0; i < pixels.length; i += 4) {
      const r = pixels[i], g = pixels[i+1], b = pixels[i+2];
      totalBrightness += (r * 0.299 + g * 0.587 + b * 0.114);
    }
    const avgBrightness = totalBrightness / (pixels.length / 4);

    const estimatedLux = Math.round(avgBrightness * avgBrightness / 6.5);

    stream.getTracks().forEach(t => t.stop());

    let lightLevel, recommendation;
    if (estimatedLux < 500) { lightLevel = t('sensor.light.low'); recommendation = t('sensor.light.rec.low'); }
    else if (estimatedLux < 2000) { lightLevel = t('sensor.light.mid'); recommendation = t('sensor.light.rec.mid'); }
    else if (estimatedLux < 10000) { lightLevel = t('sensor.light.high'); recommendation = t('sensor.light.rec.high'); }
    else { lightLevel = t('sensor.light.full'); recommendation = t('sensor.light.rec.full'); }

    return { lux: estimatedLux, brightness: Math.round(avgBrightness), lightLevel, recommendation };
  } catch (err) {
    throw new Error('Kamera nicht verfuegbar: ' + err.message);
  }
}

async function handleLightMeter() {
  const resultEl = document.getElementById('lightMeterResult');
  if (resultEl) resultEl.innerHTML = '<div style="color:' + TC.textMuted + ';">' + t('sensor.light.measuring') + '</div>';
  try {
    const result = await measureLight();
    if (resultEl) {
      const luxColor = result.lux < 500 ? TC.textDim : result.lux < 2000 ? TC.warning : result.lux < 10000 ? TC.accent : TC.info;
      resultEl.innerHTML = '<div style="padding:14px;border-radius:12px;background:' + TC.surface + ';border:1px solid rgba(' + TC.accentRgb + ',0.15);">'
        + '<div style="font-size:24px;font-weight:700;color:' + luxColor + ';">~' + result.lux + ' Lux</div>'
        + '<div style="font-size:14px;color:' + TC.text + ';margin-top:6px;">' + escapeHtml(result.lightLevel) + '</div>'
        + '<div style="font-size:12px;color:' + TC.textMuted + ';margin-top:4px;">' + t('sensor.light.brightness', result.brightness) + '</div>'
        + '<div style="margin-top:10px;padding:8px 12px;border-radius:8px;background:rgba(' + TC.accentRgb + ',0.08);color:' + TC.accent + ';font-size:13px;">' + escapeHtml(result.recommendation) + '</div>'
        + '</div>';
    }
  } catch (err) {
    if (resultEl) resultEl.innerHTML = '<div style="color:' + TC.danger + ';">' + escapeHtml(err.message) + '</div>';
  }
}

let currentView = 'calendar';
const views = ['calendar', 'plants', 'locations', 'tips', 'weather', 'devices', 'billing', 'settings'];
// viewLabels replaced by getViewLabels() — see i18n system above

function switchPageView(viewKey) {
  currentView = views.includes(viewKey) ? viewKey : 'calendar';
  applyPageView();
  if (currentView === 'weather') renderWeatherDashboard();
}

function applyPageView() {
  views.forEach(v => {
    const el = document.getElementById(v + 'View');
    if (el) el.classList.toggle('app-view--active', v === currentView);
  });
  renderAppNav();
  renderPlantContextBar();
}

function renderAppNav() {
  const nav = document.getElementById('appNav');
  if (!nav) return;
  const vl = getViewLabels();
  nav.innerHTML = views.map(v =>
    `<button class="app-nav-btn ${v === currentView ? 'app-nav-btn--active' : ''}" data-view="${v}">${vl[v]}</button>`
  ).join('');
  nav.querySelectorAll('[data-view]').forEach(btn => {
    btn.addEventListener('click', () => switchPageView(btn.dataset.view));
  });
}

function renderPlantContextBar() {
  const bar = document.getElementById('plantContextBar');
  if (!bar) return;
  if (!plants.length) {
    bar.innerHTML = `<span style="color:${TC.textDim};">${t('plant.empty')}</span>`;
    return;
  }
  const selected = plants.find(p => p.plant_id === selectedPlantId);
  bar.innerHTML = `
    <select class="plant-context-select" id="plantContextSelect">
      <option value="">${t('plant.allOption')}</option>
      ${plants.filter(p => plantStatusValue(p) !== 'deleted').map(p =>
        `<option value="${p.plant_id}" ${p.plant_id === selectedPlantId ? 'selected' : ''}>${escapeHtml(p.name)}</option>`
      ).join('')}
    </select>
    ${selected ? `<span class="plant-context-dot" style="background:${selected.color_tag}"></span>` : ''}
  `;
  document.getElementById('plantContextSelect')?.addEventListener('change', async (e) => {
    selectedPlantId = e.target.value || null;
    await loadTasks();
    await loadLogs();
    renderPlants();
    renderTimelinePanel();
    renderPlantContextBar();
    await loadPlantPhotos();
    renderPhotoGallery();
    renderGrowthAnalytics();
    renderCareInsights();
  });
}

function rememberDisplay(el) {
  if (!el) return;
  if (!el.dataset) return;
  if (!el.dataset.originalDisplay) {
    const computed = window.getComputedStyle(el).display;
    el.dataset.originalDisplay = computed === "none" ? "" : computed;
  }
}

function setVisible(el, visible) {
  if (!el) return;
  rememberDisplay(el);
  el.style.display = visible ? (el.dataset.originalDisplay || "") : "none";
}

function currentDueTaskCount() {
  const today = new Date().toISOString().slice(0, 10);
  return allTasks.filter((task) => task.status === "open" && task.due_at && task.due_at.slice(0, 10) <= today).length;
}

function currentOpenReminderCount() {
  return allTasks.filter((task) => task.status === "open" && task.notification_enabled).length;
}

function getMvpReadinessItems() {
  const hasAccount = currentPlanSource === "account";
  const hasPlants = plants.some((plant) => plantStatusValue(plant) !== "deleted");
  const hasActivePlant = plants.some((plant) => plantStatusValue(plant) === "active");
  const hasTasks = allTasks.length > 0;
  const hasLogs = allLogs.length > 0;
  const hasReminders = currentOpenReminderCount() > 0;
  const hasBillingContext = currentPlanSource === "account" || checkoutHistory.length > 0 || subscriptionHistory.length > 0;
  const settingsReady = Boolean($("backendUrl")?.value) && reminderPermissionState !== "default";
  return [
    { key: 'auth', label: t('mvp.auth.label'), done: hasAccount, detail: hasAccount ? t('mvp.auth.done') : t('mvp.auth.open') },
    { key: 'plants', label: t('mvp.plants.label'), done: hasPlants && hasActivePlant, detail: hasPlants ? t('mvp.plants.done', plants.filter((plant) => plantStatusValue(plant) !== 'deleted').length) : t('mvp.plants.open') },
    { key: 'worklog', label: t('mvp.worklog.label'), done: hasTasks && hasLogs, detail: t('mvp.worklog.detail', allTasks.length, allLogs.length) },
    { key: 'calendar', label: t('mvp.calendar.label'), done: hasReminders || hasTasks, detail: hasReminders ? t('mvp.calendar.done', currentOpenReminderCount()) : t('mvp.calendar.open') },
    { key: 'billing', label: t('mvp.billing.label'), done: hasBillingContext, detail: hasBillingContext ? t('mvp.billing.done') : t('mvp.billing.open') },
    { key: 'settings', label: t('mvp.settings.label'), done: settingsReady, detail: settingsReady ? t('mvp.settings.done') : t('mvp.settings.open') },
  ];
}

function renderMvpReadinessPanel() {
  if (!mvpReadinessPanel) return;
  const items = getMvpReadinessItems();
  const doneCount = items.filter((item) => item.done).length;
  const progressPercent = Math.round((doneCount / items.length) * 100);
  const missing = items.filter((item) => !item.done);
  mvpReadinessPanel.innerHTML = `
    <h2 style="margin-top:0; color:${TC.accent}; font-size:18px;">${t('mvp.title')}</h2>
    <div style="display:grid; grid-template-columns:1.1fr 1fr; gap:12px; align-items:start;">
      <div style="padding:14px; border-radius:14px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.10);">
        <div style="display:flex; align-items:center; justify-content:space-between; gap:12px;">
          <div>
            <div style="font-size:12px; color:${TC.textMuted}; text-transform:uppercase; letter-spacing:0.08em;">${t('mvp.completion')}</div>
            <div style="font-size:26px; font-weight:700; color:${TC.text}; margin-top:6px;">${doneCount}/${items.length}</div>
          </div>
          <div style="font-size:28px; font-weight:700; color:${TC.accent};">${progressPercent}%</div>
        </div>
        <div style="margin-top:12px; height:8px; border-radius:999px; background:${TC.inputBg}; overflow:hidden; border:1px solid rgba(${TC.accentRgb},0.08);">
          <div style="width:${progressPercent}%; height:100%; background:linear-gradient(90deg, rgba(${TC.accentRgb},0.45), rgba(${TC.accentRgb},0.95));"></div>
        </div>
        <div style="display:grid; gap:8px; margin-top:12px;">
          ${items.map((item) => `<div style="padding:10px 12px; border-radius:12px; background:${item.done ? `rgba(${TC.accentRgb},0.08)` : 'rgba(255,255,255,0.04)'}; border:1px solid ${item.done ? `rgba(${TC.accentRgb},0.14)` : 'rgba(255,255,255,0.08)'};">
            <div style="display:flex; justify-content:space-between; gap:8px; align-items:center;">
              <strong style="color:${item.done ? TC.text : TC.textMuted}; font-size:13px;">${item.label}</strong>
              <span style="font-size:11px; color:${item.done ? TC.accent : '#ffcf7b'};">${item.done ? t('mvp.itemReady') : t('mvp.itemOpen')}</span>
            </div>
            <div style="font-size:12px; color:${TC.textMuted}; margin-top:4px; line-height:1.5;">${item.detail}</div>
          </div>`).join('')}
        </div>
      </div>
      <div style="display:grid; gap:10px;">
        <div style="padding:14px; border-radius:14px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.10); color:${TC.textMuted}; line-height:1.6;">
          ${missing.length ? t('mvp.remaining', `<strong style="color:${TC.text};">${missing.map((item) => item.label).join(', ')}</strong>`) : t('mvp.allDone')}
        </div>
        <div style="display:flex; flex-wrap:wrap; gap:8px;">
          ${missing.some((item) => item.key === 'auth' || item.key === 'settings') ? `<button type="button" data-mvp-action="goto-settings">${t('mvp.gotoSettings')}</button>` : ''}
          ${missing.some((item) => item.key === 'plants') ? `<button type="button" data-mvp-action="goto-plants">${t('mvp.gotoPlants')}</button>` : ''}
          ${missing.some((item) => item.key === 'calendar') ? `<button type="button" data-mvp-action="goto-calendar">${t('mvp.gotoCalendar')}</button>` : ''}
          ${missing.some((item) => item.key === 'billing') ? `<button type="button" data-mvp-action="goto-billing">${t('mvp.gotoBilling')}</button>` : ''}
          <button type="button" data-mvp-action="sync-now">${t('mvp.syncNow')}</button>
        </div>
      </div>
    </div>
  `;
  mvpReadinessPanel.querySelectorAll('[data-mvp-action]').forEach((button) => {
    button.addEventListener('click', () => {
      const action = button.dataset.mvpAction || '';
      if (action === 'goto-settings') {
        switchPageView('settings');
      } else if (action === 'goto-plants') {
        switchPageView('plants');
      } else if (action === 'goto-calendar') {
        switchPageView('calendar');
      } else if (action === 'goto-billing') {
        switchPageView('billing');
      } else if (action === 'sync-now') {
        $('syncNow')?.click();
      }
    });
  });
}

function renderOnboardingPanel() {
  if (!onboardingPanel) return;
  const hasPlants = plants.length > 0;
  const hasSelection = Boolean(selectedPlant());
  const hasTasks = allTasks.length > 0;
  const hasLogs = allLogs.length > 0;
  const canCreate = canActivateMorePlants();
  const checklist = [
    { done: currentPlanSource === "account", label: t('onboard.check1') },
    { done: hasPlants, label: t('onboard.check2') },
    { done: hasSelection || hasTasks, label: t('onboard.check3') },
    { done: hasSelection || hasLogs, label: t('onboard.check4') },
  ];
  const doneCount = checklist.filter((item) => item.done).length;
  const progress = `${doneCount}/${checklist.length}`;
  const primaryAction = !hasPlants
    ? { label: canCreate ? t('onboard.primaryCreate') : t('onboard.primaryLimit'), action: canCreate ? "goto-create-plant" : "goto-billing" }
    : !hasSelection
      ? { label: t('onboard.primaryOverview'), action: "goto-plants" }
      : !hasTasks
        ? { label: t('onboard.primaryTask'), action: "focus-task" }
        : !hasLogs
          ? { label: t('onboard.primaryLog'), action: "focus-log" }
          : { label: t('onboard.primaryCalendar'), action: "goto-calendar" };
  onboardingPanel.innerHTML = `
    <h2 style="margin-top:0; color:${TC.accent}; font-size:18px;">${t('onboard.title')}</h2>
    <div style="display:grid; grid-template-columns:1.2fr 1fr; gap:12px; align-items:start;">
      <div style="padding:14px; border-radius:14px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.10);">
        <div style="font-size:12px; color:${TC.textMuted}; text-transform:uppercase; letter-spacing:0.08em;">${t('onboard.progress')}</div>
        <div style="display:flex; align-items:center; justify-content:space-between; gap:12px; margin-top:8px;">
          <div style="font-size:24px; font-weight:700; color:${TC.text};">${progress}</div>
          <div style="font-size:12px; color:${TC.textMuted};">${t('onboard.firstUse')}</div>
        </div>
        <div style="margin-top:12px; height:8px; border-radius:999px; background:${TC.inputBg}; overflow:hidden; border:1px solid rgba(${TC.accentRgb},0.08);">
          <div style="width:${(doneCount / checklist.length) * 100}%; height:100%; background:linear-gradient(90deg, rgba(${TC.accentRgb},0.45), rgba(${TC.accentRgb},0.9));"></div>
        </div>
        <div style="display:grid; gap:8px; margin-top:12px;">
          ${checklist.map((item) => `<div style="display:flex; gap:8px; align-items:flex-start; color:${item.done ? TC.accent : TC.textMuted};"><span style="display:inline-flex; align-items:center; justify-content:center; width:18px; height:18px; border-radius:999px; background:${item.done ? `rgba(${TC.accentRgb},0.14)` : 'rgba(255,255,255,0.06)'}; border:1px solid ${item.done ? `rgba(${TC.accentRgb},0.26)` : 'rgba(255,255,255,0.10)'}; color:${item.done ? TC.accent : TC.textMuted}; font-size:11px;">${item.done ? '✓' : '•'}</span><span style="font-size:13px; line-height:1.4;">${item.label}</span></div>`).join('')}
        </div>
      </div>
      <div style="display:grid; gap:10px;">
        <div style="padding:14px; border-radius:14px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.10); color:${TC.textMuted}; line-height:1.6;">
          ${!hasPlants
            ? t('onboard.descNoPlants')
            : !hasSelection
              ? t('onboard.descNoSelection')
              : t('onboard.descActive')}
        </div>
        <div style="display:flex; flex-wrap:wrap; gap:8px;">
          <button type="button" data-onboarding-action="${primaryAction.action}">${primaryAction.label}</button>
          <button type="button" data-onboarding-action="goto-settings">${t('onboard.accountSettings')}</button>
          <button type="button" data-onboarding-action="sync-now">${t('mvp.syncNow')}</button>
        </div>
      </div>
    </div>
  `;
  onboardingPanel.querySelectorAll('[data-onboarding-action]').forEach((button) => {
    button.addEventListener('click', async () => {
      const action = button.dataset.onboardingAction || '';
      if (action === 'goto-create-plant') {
        switchPageView('plants');
        window.setTimeout(() => $("plantName")?.focus(), 30);
      } else if (action === 'goto-plants') {
        switchPageView('plants');
      } else if (action === 'focus-task') {
        switchPageView('plants');
        window.setTimeout(() => $("taskTitle")?.focus(), 30);
      } else if (action === 'focus-log') {
        switchPageView('plants');
        window.setTimeout(() => $("logText")?.focus(), 30);
      } else if (action === 'goto-calendar') {
        switchPageView('calendar');
      } else if (action === 'goto-billing') {
        switchPageView('billing');
      } else if (action === 'goto-settings') {
        switchPageView('settings');
      } else if (action === 'sync-now') {
        $("syncNow")?.click();
      }
    });
  });
}

function renderSettingsPanel() {
  if (!settingsPanel) return;
  const reminderLabel = reminderPermissionState === "unsupported"
    ? t('settings.reminderUnsupported')
    : reminderPermissionState === "granted"
      ? t('settings.reminderGranted')
      : reminderPermissionState === "denied"
        ? t('settings.reminderDenied')
        : t('settings.reminderDefault');
  const syncMode = currentPlanSource === "account" ? "Account + Cloud-Sync" : "Guest + Local Cache";
  const visiblePlants = summaryPlantCount("visible_plants", plants.filter((plant) => plantStatusValue(plant) !== "deleted").length);
  const dueTasks = currentDueTaskCount();
  const isLoggedIn = currentPlanSource === "account";
  const userEmail = $("email")?.value?.trim() || "";

  // Build mood/theme section
  const moodSection = `
    <div class="settings-section" style="margin-top:0;">
      <h3>${t('settings.mood')}</h3>
      <div style="display:grid; grid-template-columns:repeat(auto-fill, minmax(130px, 1fr)); gap:8px;">
        ${THEMES.map(th => `
          <button class="btn ${currentTheme === th.id ? 'btn--primary' : ''}"
                  data-set-theme="${th.id}"
                  style="padding:12px 8px; text-align:center; display:flex; flex-direction:column; align-items:center; gap:6px;">
            <div style="display:flex; gap:3px;">
              ${th.preview.map(c => `<div style="width:14px; height:14px; border-radius:50%; background:${c}; border:1px solid rgba(255,255,255,0.1);"></div>`).join('')}
            </div>
            <span style="font-size:12px;">${escapeHtml(th.label)}</span>
          </button>
        `).join('')}
      </div>
      <div style="margin-top:10px; display:flex; align-items:center; gap:8px;">
        <label class="form-check" style="margin:0;">
          <input type="checkbox" id="soundToggle" ${notifSoundEnabled ? 'checked' : ''} />
          ${t('settings.sounds')}
        </label>
        <button class="btn btn--ghost btn--sm" id="testSoundBtn" style="font-size:11px;">${t('settings.test')}</button>
      </div>
      <div style="margin-top:10px;display:flex;align-items:center;gap:8px;">
        <label class="form-label" style="margin:0;">${t('settings.language')}</label>
        <select id="langSelect" class="form-select" style="width:auto;padding:4px 8px;font-size:12px;">
          ${LANGUAGES.map(l => `<option value="${l}" ${l === currentLang ? 'selected' : ''}>${LANGUAGE_LABELS[l]}</option>`).join('')}
        </select>
      </div>
    </div>
  `;

  // Build profile section
  const profileSection = `
    <div class="settings-section">
      <h3>${t('settings.profile')}</h3>
      <div class="settings-info">${isLoggedIn ? t('settings.loggedInAs', `<strong style="color:${TC.text};">${escapeHtml(userEmail || t('settings.loggedInUnknown'))}</strong>`) : t('settings.guestMode')}</div>
      <div class="settings-row">
        <div class="settings-field">
          <label for="settingsLocale">${t('settings.language')}</label>
          <select id="settingsLocale" class="form-select">
            ${LANGUAGES.map(l => `<option value="${l}" ${l === currentLang ? 'selected' : ''}>${LANGUAGE_LABELS[l]}</option>`).join('')}
          </select>
        </div>
        <div class="settings-field">
          <label for="settingsTimezone">${t('settings.timezone')}</label>
          <select id="settingsTimezone" class="form-select">
            <option value="Europe/Berlin" selected>Europe/Berlin</option>
            <option value="Europe/Vienna">Europe/Vienna</option>
            <option value="Europe/Zurich">Europe/Zurich</option>
            <option value="Europe/London">Europe/London</option>
            <option value="America/New_York">America/New_York</option>
            <option value="America/Los_Angeles">America/Los_Angeles</option>
            <option value="UTC">UTC</option>
          </select>
        </div>
      </div>
      <div class="settings-actions">
        <button type="button" class="btn btn--primary btn--sm" data-settings-action="save-profile">${t('settings.saveProfile')}</button>
      </div>
      <div id="profileFeedback"></div>
    </div>`;

  // Password change section (only when logged in)
  const passwordSection = isLoggedIn ? `
    <div class="settings-section">
      <h3>${t('settings.changePassword')}</h3>
      <div class="settings-field">
        <label for="settingsCurrentPw">${t('settings.currentPassword')}</label>
        <input id="settingsCurrentPw" type="password" class="form-input" placeholder="${t('settings.currentPassword')}" autocomplete="current-password" />
      </div>
      <div class="settings-field">
        <label for="settingsNewPw">${t('settings.newPassword')}</label>
        <input id="settingsNewPw" type="password" class="form-input" placeholder="${t('settings.newPasswordPlaceholder')}" autocomplete="new-password" />
      </div>
      <div class="settings-field">
        <label for="settingsConfirmPw">${t('settings.confirmPassword')}</label>
        <input id="settingsConfirmPw" type="password" class="form-input" placeholder="${t('settings.confirmPasswordPlaceholder')}" autocomplete="new-password" />
      </div>
      <div class="settings-actions">
        <button type="button" class="btn btn--primary btn--sm" data-settings-action="change-password">${t('settings.changePasswordBtn')}</button>
      </div>
      <div id="passwordFeedback"></div>
    </div>` : '';

  // Account deletion section (only when logged in)
  const deleteSection = isLoggedIn ? `
    <div class="settings-section" style="border-color: rgba(255,112,112,0.2);">
      <h3 style="color: ${TC.danger};">${t('settings.dangerZone')}</h3>
      <div class="settings-info">${t('settings.deleteAccountIrreversible')}</div>
      <div id="deleteAccountArea">
        <button type="button" class="btn btn--danger btn--sm" data-settings-action="show-delete-confirm">${t('settings.deleteAccount')}</button>
      </div>
      <div id="deleteFeedback"></div>
    </div>` : '';

  settingsPanel.innerHTML = `
    ${moodSection}
    <h2 style="margin-top:14px; color:${TC.accent}; font-size:18px;">${t('settings.mvp')}</h2>
    <div style="display:grid; grid-template-columns:repeat(auto-fit, minmax(180px, 1fr)); gap:10px; margin-bottom:12px;">
      <div style="padding:12px; border-radius:14px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.10);">
        <div style="font-size:12px; color:${TC.textMuted};">${t('settings.activePlan')}</div>
        <div style="font-size:18px; color:${TC.text}; font-weight:700; margin-top:4px;">${planLabel(currentPlan)}</div>
      </div>
      <div style="padding:12px; border-radius:14px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.10);">
        <div style="font-size:12px; color:${TC.textMuted};">${t('settings.syncMode')}</div>
        <div style="font-size:15px; color:${TC.text}; font-weight:600; margin-top:4px;">${syncMode}</div>
      </div>
      <div style="padding:12px; border-radius:14px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.10);">
        <div style="font-size:12px; color:${TC.textMuted};">${t('settings.desktopReminder')}</div>
        <div style="font-size:15px; color:${TC.text}; font-weight:600; margin-top:4px;">${reminderLabel}</div>
      </div>
      <div style="padding:12px; border-radius:14px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.10);">
        <div style="font-size:12px; color:${TC.textMuted};">${t('settings.dueToday')}</div>
        <div style="font-size:18px; color:${TC.text}; font-weight:700; margin-top:4px;">${dueTasks}</div>
      </div>
    </div>
    <div style="display:grid; gap:10px;">
      <div style="display:flex; flex-wrap:wrap; gap:8px;">
        <button type="button" data-settings-action="sync-now">${t('settings.syncNow')}</button>
        <button type="button" data-settings-action="goto-billing">${t('settings.openBilling')}</button>
        <button type="button" data-settings-action="goto-calendar">${t('settings.openCalendar')}</button>
        <button type="button" data-settings-action="focus-backend">${t('settings.checkBackend')}</button>
      </div>
      <div style="padding:12px; border-radius:14px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.10); color:${TC.textMuted}; font-size:13px; line-height:1.7;">
        Backend: ${$("backendUrl")?.value || "-"}<br />
        ${t('settings.visiblePlants')}: ${visiblePlants}<br />
        ${t('settings.freeSlots')}: ${Number.isFinite(currentPlantLimit) ? remainingPlantSlots() : "\u221e"}<br />
        ${t('settings.activeReminders')}: ${currentOpenReminderCount()}
      </div>
    </div>
    <div class="settings-section">
      <h3 style="color:${TC.accent};">${t('settings.importExport')}</h3>
      <div style="display:flex;gap:8px;flex-wrap:wrap;margin-bottom:10px;">
        <button class="btn btn--primary btn--sm" data-settings-action="export-all">${t('settings.exportAll')}</button>
        <button class="btn btn--sm" data-settings-action="export-plants">${t('settings.exportPlants')}</button>
        <button class="btn btn--sm" data-settings-action="export-tasks">${t('settings.exportTasks')}</button>
        <button class="btn btn--sm" data-settings-action="export-logs">${t('settings.exportLogs')}</button>
      </div>
      <div style="margin-top:12px;">
        <div style="font-size:13px;color:${TC.textMuted};margin-bottom:6px;">${t('settings.csvImportHint')}</div>
        <div style="display:flex;gap:8px;align-items:center;">
          <button class="btn btn--sm" data-settings-action="import-csv">${t('settings.importCsv')}</button>
          <span id="importStatus" style="font-size:12px;color:${TC.textDim};"></span>
        </div>
        <div style="margin-top:8px;font-size:11px;color:${TC.textDim};">
          ${escapeHtml(t('settings.csvFormats'))}
        </div>
      </div>
    </div>
    <div class="settings-section">
      <h3>\u{1F3D6}\uFE0F ${t('settings.vacation')}</h3>
      <label class="form-check" style="margin-bottom:8px;">
        <input type="checkbox" id="vacationToggle" ${vacationMode?.active ? 'checked' : ''} />
        ${t('settings.vacationEnable')}
      </label>
      <div style="margin-top:8px;display:${vacationMode?.active ? 'block' : 'none'};" id="vacationOptions">
        <div class="form-group">
          <label class="form-label">${t('settings.vacationReturn')}</label>
          <input type="date" id="vacationReturn" class="form-input" value="${vacationMode?.returnDate || ''}" />
        </div>
        <div class="form-hint">${t('settings.vacationHint')}</div>
      </div>
    </div>
    <div class="settings-section">
      <h3>${t('settings.team')}</h3>
      <div style="margin-bottom:10px;">
        <div style="font-size:13px;color:${TC.textMuted};margin-bottom:8px;">${t('settings.activeProfile', `<strong style="color:${TC.text};">${activeProfile ? escapeHtml(activeProfile.name) : t('settings.noneProfile')}</strong>`)}</div>
      </div>
      <div style="display:grid;gap:8px;margin-bottom:12px;">
        ${profiles.map(p => `
          <div style="display:flex;align-items:center;gap:10px;padding:10px;border-radius:10px;background:${p.profile_id === activeProfile?.profile_id ? `rgba(${TC.accentRgb},0.08)` : TC.surface};border:1px solid ${p.profile_id === activeProfile?.profile_id ? `rgba(${TC.accentRgb},0.2)` : `rgba(${TC.accentRgb},0.06)`};">
            <span style="font-size:20px;">${p.avatar_emoji || '\u{1F9D1}\u200D\u{1F33E}'}</span>
            <div style="flex:1;">
              <div style="font-weight:600;color:${TC.text};">${escapeHtml(p.name)}</div>
              <div style="font-size:11px;color:${TC.textDim};">${escapeHtml(roleLabel(p.role))}</div>
            </div>
            <div style="display:flex;gap:4px;">
              ${p.profile_id !== activeProfile?.profile_id ? `<button class="btn btn--sm btn--primary" data-switch-profile="${p.profile_id}">${t('settings.switchProfile')}</button>` : `<span class="cal-badge" style="background:rgba(${TC.accentRgb},0.15);color:${TC.accent};">${t('settings.activeBadge')}</span>`}
              ${!p.is_owner ? `<button class="btn btn--sm btn--ghost" data-delete-profile="${p.profile_id}" title="${t('settings.deleteProfileTitle')}">\u00d7</button>` : ''}
            </div>
          </div>
        `).join('')}
      </div>
      <div style="display:grid;gap:8px;padding:12px;border-radius:12px;background:${TC.surface};border:1px solid rgba(${TC.accentRgb},0.08);">
        <div style="font-size:13px;color:${TC.textMuted};font-weight:600;">${t('settings.newProfile')}</div>
        <div class="form-row" style="display:grid;grid-template-columns:1fr 1fr;gap:8px;">
          <div class="form-group">
            <label class="form-label">${t('settings.profileName')}</label>
            <input id="newProfileName" class="form-input" placeholder="${t('settings.profileNamePlaceholder')}" />
          </div>
          <div class="form-group">
            <label class="form-label">${t('settings.profileRole')}</label>
            <select id="newProfileRole" class="form-select">
              <option value="gardener">${t('role.gardener')}</option>
              <option value="admin">${t('role.admin')}</option>
              <option value="viewer">${t('role.viewer')}</option>
            </select>
          </div>
        </div>
        <div class="form-row" style="display:grid;grid-template-columns:1fr 1fr;gap:8px;">
          <div class="form-group">
            <label class="form-label">${t('settings.profileAvatar')}</label>
            <select id="newProfileEmoji" class="form-select">
              <option value="\u{1F9D1}\u200D\u{1F33E}">${t('avatar.gardenerM')}</option>
              <option value="\u{1F469}\u200D\u{1F33E}">${t('avatar.gardenerF')}</option>
              <option value="\u{1F468}\u200D\u{1F52C}">${t('avatar.scientist')}</option>
              <option value="\u{1F331}">${t('avatar.plant')}</option>
              <option value="\u{1F33F}">${t('avatar.herb')}</option>
              <option value="\u{1F33B}">${t('avatar.flower')}</option>
              <option value="\u{1F477}">${t('avatar.worker')}</option>
              <option value="\u{1F468}\u200D\u{1F4BC}">${t('avatar.manager')}</option>
            </select>
          </div>
          <div class="form-group">
            <label class="form-label">${t('settings.profileColor')}</label>
            <input id="newProfileColor" type="color" value="#6bffbc" class="form-input" />
          </div>
        </div>
        <button class="btn btn--primary btn--sm" data-settings-action="create-profile">${t('settings.createProfile')}</button>
      </div>
    </div>
    ${profileSection}
    ${passwordSection}
    ${deleteSection}
    <div class="settings-section">
      <h3>${t('settings.privacyLegal')}</h3>
      <div id="settingsConsentToggles" style="display:grid;gap:8px;margin-bottom:12px;"></div>
      <div style="display:flex;gap:8px;flex-wrap:wrap;margin-bottom:12px;">
        <button type="button" class="btn btn--ghost btn--sm" data-settings-action="open-privacy">${t('settings.openPrivacy')}</button>
        <button type="button" class="btn btn--ghost btn--sm" data-settings-action="open-imprint">${t('settings.openImprint')}</button>
        <button type="button" class="btn btn--primary btn--sm" data-settings-action="export-json">${t('settings.exportJson')}</button>
      </div>
      <div style="display:flex;gap:8px;align-items:center;flex-wrap:wrap;">
        <label style="font-size:13px;color:${TC.textMuted};">${t('retention.title')}</label>
        <select id="settingsRetentionSelect" class="form-select" style="width:auto;padding:4px 8px;font-size:12px;">
          ${RETENTION_OPTIONS.map(o => `<option value="${o.days}" ${getStoredRetentionDays() === o.days ? 'selected' : ''}>${t(o.key)}</option>`).join('')}
        </select>
      </div>
    </div>
  `;

  // Quick-action buttons
  settingsPanel.querySelectorAll('[data-settings-action]').forEach((button) => {
    button.addEventListener('click', async () => {
      const action = button.dataset.settingsAction || '';
      if (action === 'sync-now') {
        $('syncNow')?.click();
      } else if (action === 'goto-billing') {
        switchPageView('billing');
      } else if (action === 'goto-calendar') {
        switchPageView('calendar');
      } else if (action === 'focus-backend') {
        $('backendUrl')?.focus();
      } else if (action === 'save-profile') {
        await handleSaveProfile();
      } else if (action === 'change-password') {
        await handleChangePassword();
      } else if (action === 'show-delete-confirm') {
        showDeleteConfirmation();
      } else if (action === 'export-all') {
        await handleCsvExport('all');
      } else if (action === 'export-plants') {
        await handleCsvExport('plants');
      } else if (action === 'export-tasks') {
        await handleCsvExport('tasks');
      } else if (action === 'export-logs') {
        await handleCsvExport('logs');
      } else if (action === 'import-csv') {
        await handleCsvImport();
      } else if (action === 'open-privacy') {
        await openPrivacyDoc();
      } else if (action === 'open-imprint') {
        await openImprintDoc();
      } else if (action === 'export-json') {
        await handleExportJson();
      } else if (action === 'create-profile') {
        const name = document.getElementById("newProfileName")?.value?.trim();
        if (!name) return;
        await invoke("create_profile", {
          name,
          role: document.getElementById("newProfileRole")?.value || 'gardener',
          color: document.getElementById("newProfileColor")?.value || '#6bffbc',
          avatarEmoji: document.getElementById("newProfileEmoji")?.value || '\u{1F9D1}\u200D\u{1F33E}',
        });
        await loadProfiles();
        renderSettingsPanel();
        renderProfileIndicator();
        playNotificationSound('success');
      }
    });
  });

  // Profile switch buttons
  settingsPanel.querySelectorAll('[data-switch-profile]').forEach(btn => {
    btn.addEventListener('click', async () => {
      await invoke("switch_profile", { profileId: btn.dataset.switchProfile });
      await loadProfiles();
      renderSettingsPanel();
      renderAppNav();
      renderProfileIndicator();
      playNotificationSound('success');
    });
  });
  // Profile delete buttons
  settingsPanel.querySelectorAll('[data-delete-profile]').forEach(btn => {
    btn.addEventListener('click', async () => {
      if (!confirm(t('settings.deleteProfileConfirm'))) return;
      await invoke("delete_profile_local", { profileId: btn.dataset.deleteProfile });
      await loadProfiles();
      renderSettingsPanel();
      renderProfileIndicator();
    });
  });

  // Theme switcher buttons
  settingsPanel.querySelectorAll('[data-set-theme]').forEach((btn) => {
    btn.addEventListener('click', () => {
      applyTheme(btn.dataset.setTheme);
    });
  });

  // Sound toggle
  document.getElementById('soundToggle')?.addEventListener('change', (e) => {
    notifSoundEnabled = e.target.checked;
    localStorage.setItem('plants_calendar_sound', notifSoundEnabled ? 'on' : 'off');
  });
  document.getElementById('testSoundBtn')?.addEventListener('click', () => {
    playNotificationSound('reminder');
  });

  // Language selector (both in mood section and profile section)
  const handleLangChange = async (e) => {
    await setLanguage(e.target.value);
    // Re-render every translated surface so the switch is visible immediately.
    try { renderAppNav(); } catch (_) {}
    try { renderFab(); } catch (_) {}
    try { renderOnboardingOverlayText(); } catch (_) {}
    try { renderSettingsPanel(); } catch (_) {}
    try { renderCalendarToolbar && renderCalendarToolbar(); } catch (_) {}
    try { renderPlantContextBar && renderPlantContextBar(); } catch (_) {}
    try { renderPlants && renderPlants(); } catch (_) {}
    try { renderTimelinePanel && renderTimelinePanel(); } catch (_) {}
    try { renderLocationsView && renderLocationsView(); } catch (_) {}
    try { renderTipsView && renderTipsView(); } catch (_) {}
    try { renderDeviceManagementPanel && renderDeviceManagementPanel(); } catch (_) {}
    try { renderSensorWidget && renderSensorWidget(); } catch (_) {}
    try { renderClimateSettings && renderClimateSettings(); } catch (_) {}
    try { if (currentView === 'weather' && typeof renderWeatherDashboard === 'function') renderWeatherDashboard(); } catch (_) {}
    try { if (focusedDayKey && typeof showDayDetail === 'function') showDayDetail(focusedDayKey); } catch (_) {}
  };
  document.getElementById('langSelect')?.addEventListener('change', handleLangChange);
  document.getElementById('settingsLocale')?.addEventListener('change', handleLangChange);

  // Vacation mode handlers
  document.getElementById('vacationToggle')?.addEventListener('change', (e) => {
    if (e.target.checked) {
      vacationMode = { active: true, returnDate: document.getElementById('vacationReturn')?.value || '', multiplier: 1.5 };
    } else {
      vacationMode = null;
    }
    saveVacationMode();
    const optionsEl = document.getElementById('vacationOptions');
    if (optionsEl) optionsEl.style.display = vacationMode?.active ? 'block' : 'none';
    renderCalendarToolbar();
    renderTimelinePanel();
    renderPlants();
  });
  document.getElementById('vacationReturn')?.addEventListener('change', (e) => {
    if (vacationMode && vacationMode.active) {
      vacationMode.returnDate = e.target.value || '';
      saveVacationMode();
      renderTimelinePanel();
    }
  });

  renderClimateSettings();
  updateAuthStatusDisplay();

  // Phase C2: consent toggles + retention slider
  renderConsentToggles('settingsConsentToggles');
  document.getElementById('settingsRetentionSelect')?.addEventListener('change', async (e) => {
    const days = parseInt(e.target.value, 10);
    if (Number.isFinite(days) && days > 0) {
      await setRetentionDays(days);
    }
  });
}

async function handleSaveProfile() {
  const feedbackEl = document.getElementById('profileFeedback');
  const locale = document.getElementById('settingsLocale')?.value || 'de-DE';
  const timezone = document.getElementById('settingsTimezone')?.value || 'Europe/Berlin';
  if (feedbackEl) feedbackEl.innerHTML = '';
  try {
    showLoading(t('profile.saving'));
    await invoke("update_profile_smart", { payload: { locale, timezone } });
    if (feedbackEl) feedbackEl.innerHTML = `<div class="settings-success">${t('profile.saved')}</div>`;
  } catch (err) {
    if (feedbackEl) feedbackEl.innerHTML = `<div class="settings-error">${t('profile.error', err?.message || err)}</div>`;
  } finally {
    hideLoading();
  }
}

async function handleChangePassword() {
  const feedbackEl = document.getElementById('passwordFeedback');
  const currentPw = document.getElementById('settingsCurrentPw')?.value || '';
  const newPw = document.getElementById('settingsNewPw')?.value || '';
  const confirmPw = document.getElementById('settingsConfirmPw')?.value || '';
  if (feedbackEl) feedbackEl.innerHTML = '';

  if (!currentPw) {
    if (feedbackEl) feedbackEl.innerHTML = `<div class="settings-error">${t('password.missingCurrent')}</div>`;
    return;
  }
  if (newPw.length < 8) {
    if (feedbackEl) feedbackEl.innerHTML = `<div class="settings-error">${t('password.tooShort')}</div>`;
    return;
  }
  if (newPw !== confirmPw) {
    if (feedbackEl) feedbackEl.innerHTML = `<div class="settings-error">${t('password.mismatch')}</div>`;
    return;
  }
  try {
    showLoading(t('password.changing'));
    await invoke("change_password_smart", { payload: { current_password: currentPw, new_password: newPw } });
    if (feedbackEl) feedbackEl.innerHTML = `<div class="settings-success">${t('password.changed')}</div>`;
    const cpEl = document.getElementById('settingsCurrentPw');
    const npEl = document.getElementById('settingsNewPw');
    const cnpEl = document.getElementById('settingsConfirmPw');
    if (cpEl) cpEl.value = '';
    if (npEl) npEl.value = '';
    if (cnpEl) cnpEl.value = '';
  } catch (err) {
    if (feedbackEl) feedbackEl.innerHTML = `<div class="settings-error">${t('profile.error', err?.message || err)}</div>`;
  } finally {
    hideLoading();
  }
}

function showDeleteConfirmation() {
  const area = document.getElementById('deleteAccountArea');
  if (!area) return;
  area.innerHTML = `
    <div style="margin-top:8px;">
      <div class="settings-info" style="color:${TC.danger};">${t('delete.prompt')}</div>
      <input id="deleteConfirmInput" class="delete-confirm-input" placeholder="${t('delete.placeholder')}" autocomplete="off" />
      <div class="settings-actions" style="margin-top:10px;">
        <button type="button" class="btn btn--danger btn--sm" id="deleteAccountConfirmBtn" disabled>${t('delete.confirmBtn')}</button>
        <button type="button" class="btn btn--ghost btn--sm" id="deleteAccountCancelBtn">${t('delete.cancel')}</button>
      </div>
    </div>`;
  const input = document.getElementById('deleteConfirmInput');
  const confirmBtn = document.getElementById('deleteAccountConfirmBtn');
  const cancelBtn = document.getElementById('deleteAccountCancelBtn');
  input?.addEventListener('input', () => {
    if (confirmBtn) confirmBtn.disabled = (input.value.trim() !== t('delete.word'));
  });
  cancelBtn?.addEventListener('click', () => {
    area.innerHTML = `<button type="button" class="btn btn--danger btn--sm" data-settings-action="show-delete-confirm">${t('delete.btnOpen')}</button>`;
    area.querySelector('[data-settings-action="show-delete-confirm"]')?.addEventListener('click', () => showDeleteConfirmation());
  });
  confirmBtn?.addEventListener('click', async () => {
    await handleDeleteAccount();
  });
}

async function handleDeleteAccount() {
  const feedbackEl = document.getElementById('deleteFeedback');
  if (feedbackEl) feedbackEl.innerHTML = '';
  try {
    showLoading(t('delete.progress'));
    await invoke("delete_account_smart");
    // Clear session and redirect to calendar
    selectedPlantId = null;
    plants = [];
    tasks = [];
    logs = [];
    allTasks = [];
    allLogs = [];
    setCurrentPlanState("free", "guest");
    switchPageView('calendar');
    await refreshAll();
    if (feedbackEl) feedbackEl.innerHTML = `<div class="settings-success">${t('delete.done')}</div>`;
  } catch (err) {
    if (feedbackEl) feedbackEl.innerHTML = `<div class="settings-error">${t('profile.error', err?.message || err)}</div>`;
  } finally {
    hideLoading();
  }
}

// ── CSV Export / Import handlers ──

async function handleCsvExport(type) {
  try {
    showLoading(t('csv.exporting'));
    const data = await invoke("export_all_csv");
    let csvContent = '';
    let filename = '';
    const dateStr = new Date().toISOString().slice(0, 10);

    if (type === 'all') {
      csvContent = '### PFLANZEN ###\n' + data.plants + '\n### TASKS ###\n' + data.tasks + '\n### LOGS ###\n' + data.logs;
      filename = 'plants_calendar_export_' + dateStr + '.csv';
    } else if (type === 'plants') {
      csvContent = data.plants;
      filename = 'pflanzen_' + dateStr + '.csv';
    } else if (type === 'tasks') {
      csvContent = data.tasks;
      filename = 'tasks_' + dateStr + '.csv';
    } else if (type === 'logs') {
      csvContent = data.logs;
      filename = 'logs_' + dateStr + '.csv';
    }

    const savePath = await tauriSaveDialog({
      defaultPath: filename,
      filters: [{ name: 'CSV', extensions: ['csv'] }],
    });
    if (!savePath) { hideLoading(); return; }

    await invoke("write_export_file", { path: savePath, content: csvContent });
    playNotificationSound('success');
    hideLoading();
    alert(t('csv.exportSaved', savePath));
  } catch (err) {
    hideLoading();
    alert(t('csv.exportFailed', err?.message || err));
  }
}

async function handleCsvImport() {
  try {
    const selected = await tauriOpenDialog({
      multiple: false,
      filters: [{ name: 'CSV', extensions: ['csv', 'txt'] }],
      title: t('csv.pickTitle'),
    });
    if (!selected) return;
    const filePath = typeof selected === 'string' ? selected : selected.path;
    if (!filePath) return;

    showLoading(t('csv.importing'));
    const content = await invoke("read_import_file", { path: filePath });
    const imported = await invoke("import_plants_csv", { csvContent: content });
    playNotificationSound('success');
    hideLoading();
    alert(t('csv.imported', imported));
    await refreshAll();
  } catch (err) {
    hideLoading();
    alert(t('csv.importFailed', err?.message || err));
  }
}

function normalizePlan(plan) {
  const raw = String(plan || "free").toLowerCase();
  if (["basic", "pro", "csc", "free"].includes(raw)) return raw;
  return "free";
}

function planLimitFor(plan) {
  const normalized = normalizePlan(plan);
  if (normalized === "basic") return 3;
  if (normalized === "pro") return 100;
  if (normalized === "csc") return Number.POSITIVE_INFINITY;
  return 1;
}

function plantStatusValue(plant) {
  const raw = plant?.status ? String(plant.status).toLowerCase() : '';
  if (["active", "ended", "archived", "deleted"].includes(raw)) return raw;
  if (plant?.archived === true) return "archived";
  if (plant?.is_active === true) return "active";
  return "ended";
}

function plantStatusLabel(plant) {
  const value = plantStatusValue(plant);
  if (value === "archived") return t('plant.archived');
  if (value === "ended") return t('plant.ended');
  if (value === "deleted") return t('plant.deleted');
  return t('plant.active');
}

function plantStatusTone(plant) {
  const value = plantStatusValue(plant);
  if (value === "archived") {
    return {
      bg: 'rgba(255,215,110,0.12)',
      border: 'rgba(255,215,110,0.24)',
      color: '#ffd76e',
    };
  }
  if (value === "ended") {
    return {
      bg: 'rgba(255,255,255,0.08)',
      border: 'rgba(255,255,255,0.12)',
      color: TC.accent,
    };
  }
  if (value === "deleted") {
    return {
      bg: 'rgba(255,120,120,0.12)',
      border: 'rgba(255,120,120,0.24)',
      color: '#ffb3a7',
    };
  }
  return {
    bg: `rgba(${TC.accentRgb},0.12)`,
    border: `rgba(${TC.accentRgb},0.24)`,
    color: TC.accent,
  };
}

function isPlantActive(plant) {
  return plantStatusValue(plant) === "active";
}

function isPlantArchived(plant) {
  return plantStatusValue(plant) === "archived";
}

function isPlantEnded(plant) {
  return plantStatusValue(plant) === "ended";
}

function isPlantInactive(plant) {
  const value = plantStatusValue(plant);
  return value === "ended" || value === "archived";
}

function summaryPlantCount(key, fallback = 0) {
  if (!subscriptionStatusSummary) return fallback;
  const value = subscriptionStatusSummary[key];
  return Number.isFinite(value) ? value : fallback;
}

function planLabel(plan) {
  const normalized = normalizePlan(plan);
  if (normalized === "basic") return "Basic";
  if (normalized === "pro") return "Pro";
  if (normalized === "csc") return "CSC";
  return "Free";
}

function activePlantCount() {
  if (subscriptionStatusSummary && Number.isFinite(subscriptionStatusSummary.active_plants)) {
    return subscriptionStatusSummary.active_plants;
  }
  return plants.filter((plant) => isPlantActive(plant)).length;
}

function formatPlantLimit(limit) {
  return Number.isFinite(limit) ? String(limit) : "∞";
}

function setCurrentPlanState(plan, source = "guest") {
  currentPlan = normalizePlan(plan);
  currentPlantLimit = planLimitFor(currentPlan);
  currentPlanSource = source;
}

function canActivateMorePlants() {
  if (subscriptionStatusSummary && typeof subscriptionStatusSummary.can_create === 'boolean') {
    return subscriptionStatusSummary.can_create;
  }
  return !Number.isFinite(currentPlantLimit) || activePlantCount() < currentPlantLimit;
}

function isOverPlantLimit() {
  if (subscriptionStatusSummary && Number.isFinite(subscriptionStatusSummary.over_limit_by)) {
    return subscriptionStatusSummary.over_limit_by > 0;
  }
  return Number.isFinite(currentPlantLimit) && activePlantCount() > currentPlantLimit;
}

function planLimitMessage(mode = "create") {
  const actionText = mode === "reactivate" ? t('plant.limit.action.reactivate') : t('plant.limit.action.create');
  return t('plant.limit.message', planLabel(currentPlan), formatPlantLimit(currentPlantLimit), actionText);
}

function remainingPlantSlots() {
  if (subscriptionStatusSummary && subscriptionStatusSummary.remaining_slots != null) {
    return subscriptionStatusSummary.remaining_slots;
  }
  if (!Number.isFinite(currentPlantLimit)) return Number.POSITIVE_INFINITY;
  return Math.max(0, currentPlantLimit - activePlantCount());
}

function overflowPlantCount() {
  if (subscriptionStatusSummary && Number.isFinite(subscriptionStatusSummary.over_limit_by)) {
    return subscriptionStatusSummary.over_limit_by;
  }
  if (!Number.isFinite(currentPlantLimit)) return 0;
  return Math.max(0, activePlantCount() - currentPlantLimit);
}

function grandfatheredActivePlantIds() {
  if (!isOverPlantLimit()) return new Set();
  const overLimit = overflowPlantCount();
  const activePlants = [...plants]
    .filter((plant) => isPlantActive(plant))
    .sort((a, b) => String(b.updated_at || '').localeCompare(String(a.updated_at || '')));
  return new Set(activePlants.slice(-overLimit).map((plant) => plant.plant_id));
}

// ── Local Profile management ─────────────────────────────────────

function roleLabel(role) {
  const labels = { owner: t('role.owner'), admin: t('role.administrator'), gardener: t('role.gardener'), viewer: t('role.viewer') };
  return labels[role] || role;
}

async function loadProfiles() {
  try {
    profiles = await invoke("list_profiles");
    activeProfile = await invoke("get_active_profile");
    // Auto-create owner profile on first run
    if (!profiles.length) {
      const owner = await invoke("create_profile", { name: "Ich", role: "owner", color: "#6bffbc", avatarEmoji: "\u{1F9D1}\u200D\u{1F33E}" });
      profiles = [owner];
      activeProfile = owner;
      await invoke("switch_profile", { profileId: owner.profile_id });
    }
  } catch (_) { profiles = []; activeProfile = null; }
}

function renderProfileIndicator() {
  const el = document.getElementById('profileIndicator');
  if (!el) return;
  if (activeProfile) {
    el.innerHTML = `<span style="font-size:14px;">${activeProfile.avatar_emoji}</span> <span style="font-size:11px;color:${TC.textMuted};">${escapeHtml(activeProfile.name)}</span>`;
    el.style.cursor = 'pointer';
    el.title = t('profile.switchTitle');
    el.onclick = () => openProfileSwitchDropdown(el);
  } else {
    el.innerHTML = '';
  }
}

function openProfileSwitchDropdown(anchor) {
  // Remove any existing dropdown
  document.querySelectorAll('.profile-switch-dropdown').forEach(el => el.remove());
  if (!profiles.length) return;
  const rect = anchor.getBoundingClientRect();
  const dd = document.createElement('div');
  dd.className = 'profile-switch-dropdown';
  dd.style.cssText = `position:fixed;top:${rect.bottom + 4}px;left:${rect.left}px;z-index:9999;min-width:180px;background:${TC.cardSolid};border:1px solid ${TC.border};border-radius:12px;box-shadow:0 8px 32px rgba(0,0,0,0.3);padding:6px;`;
  dd.innerHTML = profiles.map(p => `
    <div class="profile-dd-item" data-pid="${p.profile_id}" style="display:flex;align-items:center;gap:8px;padding:8px 10px;border-radius:8px;cursor:pointer;${p.profile_id === activeProfile?.profile_id ? `background:rgba(${TC.accentRgb},0.10);` : ''}">
      <span style="font-size:18px;">${p.avatar_emoji || '\u{1F9D1}\u200D\u{1F33E}'}</span>
      <div style="flex:1;">
        <div style="font-weight:600;color:${TC.text};font-size:13px;">${escapeHtml(p.name)}</div>
        <div style="font-size:10px;color:${TC.textDim};">${escapeHtml(roleLabel(p.role))}</div>
      </div>
      ${p.profile_id === activeProfile?.profile_id ? `<span style="font-size:10px;color:${TC.accent};">aktiv</span>` : ''}
    </div>
  `).join('');
  document.body.appendChild(dd);
  dd.querySelectorAll('.profile-dd-item').forEach(item => {
    item.addEventListener('mouseenter', () => { item.style.background = `rgba(${TC.accentRgb},0.06)`; });
    item.addEventListener('mouseleave', () => { item.style.background = item.dataset.pid === activeProfile?.profile_id ? `rgba(${TC.accentRgb},0.10)` : ''; });
    item.addEventListener('click', async () => {
      dd.remove();
      await invoke("switch_profile", { profileId: item.dataset.pid });
      await loadProfiles();
      renderProfileIndicator();
      playNotificationSound('success');
    });
  });
  // Close on outside click
  const closeHandler = (e) => {
    if (!dd.contains(e.target) && e.target !== anchor) { dd.remove(); document.removeEventListener('click', closeHandler); }
  };
  setTimeout(() => document.addEventListener('click', closeHandler), 0);
}

// ── BLE / Device management ──────────────────────────────────────

async function loadLocations() {
  try { locations = await invoke("list_locations"); } catch (_) { locations = []; }
}

// Must be a function, not a const object: a const would resolve t() once at
// module load and keep the startup language forever after a language switch.
function getLocationTypeLabels() {
  return {
    growzelt: t('location.type.tent'), gewaechshaus: t('location.type.greenhouse'), outdoor: t('location.type.outdoor'),
    indoor: t('location.type.indoor'), balkon: t('location.type.balcony'), custom: t('location.type.custom'),
  };
}

function renderLocationsView() {
  const container = document.getElementById('locationsList');
  if (!container) return;

  const typeOptions = Object.entries(getLocationTypeLabels()).map(([val, label]) =>
    `<option value="${val}">${escapeHtml(label)}</option>`
  ).join('');

  let html = `<div class="view-card" style="margin-bottom:14px;">
    <h2 style="margin-top:0; color:${TC.accent}; font-size:18px;">${t('location.title')}</h2>
    <div style="display:grid; gap:10px; padding:12px; border-radius:12px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.12);">
      <div style="font-size:13px; color:${TC.textMuted}; margin-bottom:4px;">${t('location.createNew')}</div>
      <input id="locNameInput" class="form-input" placeholder="${t('location.namePlaceholder')}" />
      <select id="locTypeInput" class="form-select">${typeOptions}</select>
      <textarea id="locDescInput" class="form-input" placeholder="${t('location.descPlaceholder')}" rows="2" style="resize:vertical;"></textarea>
      <button id="locCreateBtn" class="btn btn--primary btn--sm">${t('location.createBtn')}</button>
    </div>
  </div>`;

  if (locations.length === 0) {
    html += `<div class="view-card" style="color:${TC.textMuted}; text-align:center; padding:24px;">${t('location.empty')}</div>`;
  } else {
    html += locations.map(loc => {
      const assignedPlants = plants.filter(p => p.location_id === loc.location_id);
      const typeBadge = getLocationTypeLabels()[loc.location_type] || escapeHtml(loc.location_type);
      return `<div class="view-card" style="margin-bottom:10px;">
        <div style="display:flex; justify-content:space-between; align-items:center;">
          <div>
            <strong style="color:${TC.text}; font-size:15px;">${escapeHtml(loc.name)}</strong>
            <span style="margin-left:8px; padding:2px 8px; border-radius:8px; background:rgba(${TC.accentRgb},0.12); color:${TC.accent}; font-size:11px;">${escapeHtml(typeBadge)}</span>
            <span style="margin-left:8px; color:${TC.textMuted}; font-size:12px;">${assignedPlants.length} ${t('plant.countSuffix')}</span>
          </div>
          <div style="display:flex; gap:6px;">
            <button class="btn btn--ghost btn--sm" data-loc-action="edit" data-loc-id="${loc.location_id}">${t('action.edit')}</button>
            <button class="btn btn--ghost btn--sm" data-loc-action="delete" data-loc-id="${loc.location_id}" style="color:#ff6b6b;">${t('action.delete')}</button>
          </div>
        </div>
        ${loc.description ? `<div style="margin-top:6px; color:${TC.textMuted}; font-size:12px;">${escapeHtml(loc.description)}</div>` : ''}
        ${assignedPlants.length ? `<div style="margin-top:8px; display:flex; flex-wrap:wrap; gap:6px;">
          ${assignedPlants.map(p => `<span style="padding:2px 8px; border-radius:8px; background:${sanitizeColorTag(p.color_tag)}33; color:${sanitizeColorTag(p.color_tag)}; font-size:11px; border:1px solid ${sanitizeColorTag(p.color_tag)}44;">${escapeHtml(p.name)}</span>`).join('')}
        </div>` : ''}
      </div>`;
    }).join('');
  }

  container.innerHTML = html;

  document.getElementById('locCreateBtn')?.addEventListener('click', async () => {
    const name = document.getElementById('locNameInput')?.value?.trim();
    if (!name) return;
    const location_type = document.getElementById('locTypeInput')?.value || 'custom';
    const description = document.getElementById('locDescInput')?.value?.trim() || null;
    try {
      await invoke("create_location", { name, locationType: location_type, description });
      await loadLocations();
      renderLocationsView();
    } catch (e) {
      console.error("create location failed:", e);
      logOut(t('location.createFailed', String(e)));
    }
  });

  container.querySelectorAll('[data-loc-action]').forEach(btn => {
    btn.addEventListener('click', async () => {
      const action = btn.dataset.locAction;
      const locId = btn.dataset.locId;
      if (action === 'delete') {
        try {
          await invoke("delete_location", { locationId: locId });
          await loadLocations();
          renderLocationsView();
          await refreshAll();
        } catch (e) { console.error("delete location failed:", e); }
      } else if (action === 'edit') {
        const loc = locations.find(l => l.location_id === locId);
        if (!loc) return;
        const newName = prompt(t('location.namePrompt'), loc.name);
        if (newName === null) return;
        try {
          await invoke("update_location", { locationId: locId, name: newName, locationType: loc.location_type, description: loc.description });
          await loadLocations();
          renderLocationsView();
        } catch (e) { console.error("update location failed:", e); }
      }
    });
  });
}

async function loadDevices() {
  try {
    devices = await invoke("list_devices");
  } catch (e) {
    console.warn("loadDevices failed:", e);
    devices = [];
  }
}

function getLinkedDevice(plantId) {
  return devices.find(d => d.linked_plant_id === plantId) || null;
}

function renderDeviceManagementPanel() {
  const panel = document.getElementById("deviceManagementPanel");
  if (!panel) return;

  // Browsers cannot reach devices on the local network (Tuya) and iOS has no
  // Web Bluetooth at all, so device management lives in the desktop app.
  if (!IS_TAURI) {
    panel.innerHTML = `<div class="view-card">
      <h3>${t('nav.devices')}</h3>
      <div class="settings-info">${t('devices.desktopOnlyHint')}</div>
    </div>`;
    return;
  }

  const deviceRows = devices.map(d => {
    const linkedPlant = plants.find(p => p.plant_id === d.linked_plant_id);
    const plantOptions = plants.map(p =>
      `<option value="${p.plant_id}" ${p.plant_id === d.linked_plant_id ? 'selected' : ''}>${escapeHtml(p.name)}</option>`
    ).join('');
    return `
      <div style="padding:10px; border-radius:12px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.10); margin-bottom:8px;">
        <div style="display:flex; justify-content:space-between; align-items:center;">
          <div>
            <strong style="color:${TC.text};">${escapeHtml(d.name)}</strong>
            <span style="color:${TC.textDim}; font-size:12px; margin-left:8px;">${escapeHtml(d.ble_address)}</span>
          </div>
          <button class="btn btn--danger btn--sm" data-delete-device="${d.device_id}">${t('devices.remove')}</button>
        </div>
        <div style="margin-top:6px; display:flex; align-items:center; gap:8px;">
          <label style="color:${TC.textMuted}; font-size:12px;">${t('devices.plantLabel')}</label>
          <select class="form-select" data-link-device="${d.device_id}" style="font-size:12px; padding:4px 8px;">
            <option value="">${t('devices.noneOption')}</option>
            ${plantOptions}
          </select>
          <span style="color:${TC.textDim}; font-size:11px;">${t('devices.lastSeen')} ${new Date(d.last_seen).toLocaleString(localeTag())}</span>
        </div>
      </div>`;
  }).join('');

  panel.innerHTML = `
    <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:12px;">
      <h3 style="color:${TC.accent}; font-size:16px; margin:0;">${t('devices.title')}</h3>
      <span style="padding:4px 10px; border-radius:999px; background:rgba(255,200,100,0.12); color:${TC.warning}; font-size:11px; font-weight:600;">${t('devices.experimental')}</span>
    </div>
    <div style="padding:12px; border-radius:12px; background:rgba(255,200,100,0.06); border:1px solid rgba(255,200,100,0.15); margin-bottom:14px; font-size:13px; color:${TC.textMuted};">
      ${t('devices.experimentalInfo')}
    </div>

    <div class="settings-section" style="margin-top:0; margin-bottom:14px;">
      <h3 style="font-size:14px;">${t('devices.tuyaTitle')}</h3>
      <p class="settings-info">${t('devices.tuyaInfo')}</p>
      <div class="form-row" style="margin-bottom:8px;">
        <div class="form-group">
          <label class="form-label">${t('devices.deviceId')}</label>
          <input id="tuyaDeviceId" class="form-input" placeholder="${t('devices.deviceIdPlaceholder')}" />
        </div>
        <div class="form-group">
          <label class="form-label">${t('devices.ip')}</label>
          <input id="tuyaIp" class="form-input" placeholder="${t('devices.ipPlaceholder')}" />
        </div>
      </div>
      <div class="form-group">
        <label class="form-label">${t('devices.localKey')}</label>
        <input id="tuyaLocalKey" class="form-input" placeholder="${t('devices.localKeyPlaceholder')}" />
      </div>
      <div class="form-group">
        <label class="form-label">${t('devices.name')}</label>
        <input id="tuyaDeviceName" class="form-input" placeholder="${t('devices.namePlaceholder')}" />
      </div>
      <div style="display:flex; gap:8px; margin-top:8px;">
        <button class="btn btn--primary btn--sm" id="tuyaAddBtn">${t('devices.tuyaAdd')}</button>
        <span id="tuyaAddStatus" style="color:${TC.textMuted}; font-size:12px; line-height:32px;"></span>
      </div>
    </div>

    <div class="settings-section" style="margin-bottom:14px;">
      <h3 style="font-size:14px;">${t('devices.bleTitle')}</h3>
      <div style="margin-bottom:8px;">
        <button class="btn btn--sm" id="bleScanBtn">${t('devices.bleStart')}</button>
        <span id="bleScanStatus" style="color:${TC.textMuted}; font-size:12px; margin-left:8px;"></span>
      </div>
      <div id="bleScanResults"></div>
    </div>

    <h3 style="font-size:14px; color:${TC.accent}; margin-bottom:8px;">${t('devices.connected')}</h3>
    ${devices.length ? deviceRows : `<div style="color:${TC.textDim}; font-size:13px;">${t('devices.noConnected')}</div>`}
  `;

  // Tuya device add handler
  document.getElementById("tuyaAddBtn")?.addEventListener("click", async () => {
    const statusEl = document.getElementById("tuyaAddStatus");
    const deviceId = $("tuyaDeviceId")?.value?.trim();
    const ip = $("tuyaIp")?.value?.trim();
    const localKey = $("tuyaLocalKey")?.value?.trim();
    const name = $("tuyaDeviceName")?.value?.trim() || "GGS Controller";

    if (!deviceId || !ip || !localKey) {
      if (statusEl) statusEl.textContent = t('devices.fillAll');
      return;
    }
    if (localKey.length < 8) {
      if (statusEl) statusEl.textContent = t('devices.keyTooShort');
      return;
    }
    if (!hasConsent('devices')) {
      notifyConsentMissing('devices', 'consent.missingDevicesToast');
      if (statusEl) statusEl.textContent = t('consent.missingDevicesToast');
      return;
    }

    const now = new Date().toISOString();
    const device = {
      device_id: crypto.randomUUID(),
      // The label the user typed. The Tuya device id travels in ble_address as a
      // "#<id>" suffix — it used to occupy this field, so the name was ignored.
      name: name,
      device_type: "tuya",
      ble_address: `tuya:${ip}:${localKey}#${deviceId}`,
      linked_plant_id: null,
      last_seen: now,
      created_at: now,
      updated_at: now,
    };

    try {
      await invoke("save_device", { device });
      if (statusEl) statusEl.textContent = t('devices.added');
      // Clear form
      if ($("tuyaDeviceId")) $("tuyaDeviceId").value = '';
      if ($("tuyaIp")) $("tuyaIp").value = '';
      if ($("tuyaLocalKey")) $("tuyaLocalKey").value = '';
      if ($("tuyaDeviceName")) $("tuyaDeviceName").value = '';
      await loadDevices();
      renderDeviceManagementPanel();
      renderSensorWidget();
    } catch (err) {
      if (statusEl) statusEl.textContent = t('devices.error') + (err?.message || err);
    }
  });

  // BLE scan handler
  document.getElementById("bleScanBtn")?.addEventListener("click", async () => {
    const statusEl = document.getElementById("bleScanStatus");
    const resultsEl = document.getElementById("bleScanResults");
    if (!hasConsent('devices')) {
      notifyConsentMissing('devices', 'consent.missingDevicesToast');
      if (statusEl) statusEl.textContent = t('consent.missingDevicesToast');
      return;
    }
    if (statusEl) statusEl.textContent = t('devices.scanning');
    try {
      const found = await invoke("ble_scan");
      if (statusEl) statusEl.textContent = t('devices.found', found.length);
      if (resultsEl) {
        resultsEl.innerHTML = found.map(f => `
          <div style="padding:8px; border-radius:10px; background:#0d1f19; border:1px solid rgba(${TC.accentRgb},0.08); margin-bottom:6px; display:flex; justify-content:space-between; align-items:center;">
            <div>
              <span style="color:${TC.text};">${escapeHtml(f.name)}</span>
              <span style="color:${TC.textDim}; font-size:11px; margin-left:6px;">${escapeHtml(f.address)}</span>
              ${f.rssi != null ? `<span style="color:${TC.textDim}; font-size:11px; margin-left:4px;">(${f.rssi} dBm)</span>` : ''}
            </div>
            <button class="btn btn--sm" data-connect-ble="${escapeHtml(f.address)}" data-connect-name="${escapeHtml(f.name)}">${t('devices.connect')}</button>
          </div>
        `).join('');
        resultsEl.querySelectorAll("[data-connect-ble]").forEach(btn => {
          btn.addEventListener("click", async () => {
            const address = btn.dataset.connectBle;
            const name = btn.dataset.connectName || "GGS Device";
            const now = new Date().toISOString();
            const device = {
              device_id: crypto.randomUUID(),
              name: name,
              device_type: "ggs",
              ble_address: address,
              linked_plant_id: null,
              last_seen: now,
              created_at: now,
              updated_at: now,
            };
            try {
              await invoke("save_device", { device });
              await loadDevices();
              renderDeviceManagementPanel();
              renderSensorWidget();
            } catch (err) {
              alert(t('devices.connectFailed') + (err?.message || err));
            }
          });
        });
      }
    } catch (err) {
      if (statusEl) statusEl.textContent = t('devices.scanFailed') + (err?.message || err);
    }
  });

  panel.querySelectorAll("[data-delete-device]").forEach(btn => {
    btn.addEventListener("click", async () => {
      try {
        await invoke("remove_device", { deviceId: btn.dataset.deleteDevice });
        await loadDevices();
        renderDeviceManagementPanel();
        renderSensorWidget();
      } catch (err) {
        alert(t('devices.removeFailed') + (err?.message || err));
      }
    });
  });

  panel.querySelectorAll("[data-link-device]").forEach(sel => {
    sel.addEventListener("change", async () => {
      const deviceId = sel.dataset.linkDevice;
      const plantId = sel.value || null;
      try {
        await invoke("link_device", { deviceId, plantId });
        await loadDevices();
        renderSensorWidget();
      } catch (err) {
        alert(t('devices.linkFailed') + (err?.message || err));
      }
    });
  });
}

function renderLcStatusCard(result) {
  const s = result?.status || {};
  const methods = (result?.methods || []).join(", ") || "--";
  const frameCount = result?.frame_count ?? 0;
  const rawBytes = result?.raw_bytes ?? 0;

  const modeLabel = {
    0: t('lc.mode.auto'),
    1: t('lc.mode.manual'),
    2: t('lc.mode.slave'),
    3: t('lc.mode.timer'),
  }[s.mode] || (s.mode != null ? t('lc.mode.unknown', s.mode) : "--");

  const brightness = s.brightness != null ? `${s.brightness}%` : "--";
  const brightnessBarWidth = s.brightness != null ? Math.max(0, Math.min(100, s.brightness)) : 0;
  const pwm = s.pwm != null ? s.pwm : "--";
  const pwmPct = s.pwm != null ? Math.round((s.pwm / 1023) * 100) : null;
  const screenOn = s.is_screen_on === true ? t('devices.on') : s.is_screen_on === false ? t('devices.off') : "--";

  const upTimeStr = s.up_time != null
    ? formatUpTime(s.up_time)
    : "--";
  const memStr = s.mem_free != null ? `${s.mem_free} KB` : "--";

  const wifiBadge = s.wifi_connected === true
    ? `<span style="color:#6bffbc;">${t('devices.status.connected')}</span>`
    : s.wifi_connected === false
      ? `<span style="color:${TC.textDim};">${t('devices.status.disconnected')}</span>`
      : `<span style="color:${TC.textDim};">--</span>`;
  const wifiSsid = s.wifi_ssid ? ` (${escapeHtml(s.wifi_ssid)})` : "";
  const mqttBadge = s.mqtt_connected === true
    ? `<span style="color:#6bffbc;">${t('devices.status.connected')}</span>`
    : `<span style="color:${TC.textDim};">${t('devices.status.disconnected')}</span>`;
  const btBadge = s.bt_connected === true
    ? `<span style="color:#6bffbc;">${t('devices.status.connected')}</span>`
    : `<span style="color:${TC.textDim};">${t('devices.status.disconnected')}</span>`;

  return `
    <div class="view-card" style="padding:14px; border:1px solid rgba(${TC.accentRgb},0.18); background:${TC.surface};">
      <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:12px;">
        <h4 style="margin:0; color:${TC.accent}; font-size:14px;">${t('lc.statusTitle')}</h4>
        <span style="font-size:10px; color:${TC.textDim};">${frameCount} Frames, ${rawBytes} Bytes</span>
      </div>

      <div style="display:grid; grid-template-columns:repeat(auto-fit, minmax(140px, 1fr)); gap:10px; margin-bottom:12px;">
        <div style="padding:10px; border-radius:10px; background:${TC.bg}; border:1px solid rgba(${TC.accentRgb},0.08);">
          <div style="font-size:10px; color:${TC.textMuted}; text-transform:uppercase; letter-spacing:0.5px;">${t('devices.brightness')}</div>
          <div style="font-size:22px; font-weight:700; color:${TC.accent}; margin-top:4px;">${brightness}</div>
          <div style="margin-top:6px; height:4px; background:rgba(${TC.accentRgb},0.12); border-radius:2px; overflow:hidden;">
            <div style="width:${brightnessBarWidth}%; height:100%; background:${TC.accent};"></div>
          </div>
        </div>
        <div style="padding:10px; border-radius:10px; background:${TC.bg}; border:1px solid rgba(${TC.accentRgb},0.08);">
          <div style="font-size:10px; color:${TC.textMuted}; text-transform:uppercase; letter-spacing:0.5px;">PWM</div>
          <div style="font-size:22px; font-weight:700; color:${TC.text}; margin-top:4px;">${pwm}</div>
          <div style="font-size:10px; color:${TC.textDim}; margin-top:2px;">${pwmPct != null ? `${pwmPct}% von 1023` : ""}</div>
        </div>
        <div style="padding:10px; border-radius:10px; background:${TC.bg}; border:1px solid rgba(${TC.accentRgb},0.08);">
          <div style="font-size:10px; color:${TC.textMuted}; text-transform:uppercase; letter-spacing:0.5px;">${t('devices.mode')}</div>
          <div style="font-size:16px; font-weight:600; color:${TC.text}; margin-top:4px;">${escapeHtml(modeLabel)}</div>
          <div style="font-size:10px; color:${TC.textDim}; margin-top:2px;">${t('devices.display', screenOn)}</div>
        </div>
        <div style="padding:10px; border-radius:10px; background:${TC.bg}; border:1px solid rgba(${TC.accentRgb},0.08);">
          <div style="font-size:10px; color:${TC.textMuted}; text-transform:uppercase; letter-spacing:0.5px;">${t('devices.model')}</div>
          <div style="font-size:16px; font-weight:600; color:${TC.text}; margin-top:4px;">${s.light_model != null ? `#${s.light_model}` : "--"}</div>
          <div style="font-size:10px; color:${TC.textDim}; margin-top:2px;">ADC: ${s.adc_led_dim ?? "--"}</div>
        </div>
      </div>

      <div style="display:grid; grid-template-columns:repeat(auto-fit, minmax(180px, 1fr)); gap:8px; font-size:12px; color:${TC.text};">
        <div><span style="color:${TC.textMuted};">${t('lc.firmware')}:</span> ${escapeHtml(s.fw_version || "--")}</div>
        <div><span style="color:${TC.textMuted};">${t('lc.hardware')}:</span> ${escapeHtml(s.hw_version || "--")}</div>
        <div><span style="color:${TC.textMuted};">${t('lc.build')}:</span> ${escapeHtml(s.build_time || "--")}</div>
        <div><span style="color:${TC.textMuted};">${t('lc.uptime')}:</span> ${upTimeStr}</div>
        <div><span style="color:${TC.textMuted};">${t('lc.ramFree')}:</span> ${memStr}</div>
        <div><span style="color:${TC.textMuted};">${t('devices.bootCount')}:</span> ${s.up_count ?? "--"}</div>
        <div><span style="color:${TC.textMuted};">${t('devices.deviceTime')}:</span> ${escapeHtml(s.localtime || "--")}</div>
        <div><span style="color:${TC.textMuted};">${t('lc.pid')}:</span> ${escapeHtml(s.pid || "--")}</div>
      </div>

      <div style="margin-top:12px; padding-top:10px; border-top:1px solid rgba(${TC.accentRgb},0.08); display:grid; grid-template-columns:repeat(auto-fit, minmax(140px, 1fr)); gap:8px; font-size:12px;">
        <div><span style="color:${TC.textMuted};">${t('lc.wifi')}:</span> ${wifiBadge}${wifiSsid}</div>
        <div><span style="color:${TC.textMuted};">${t('lc.mqtt')}:</span> ${mqttBadge}</div>
        <div><span style="color:${TC.textMuted};">${t('lc.bluetooth')}:</span> ${btBadge}</div>
      </div>

      <div style="margin-top:10px; font-size:10px; color:${TC.textDim};">${t('devices.methods', escapeHtml(methods))}</div>
    </div>
  `;
}

function formatUpTime(seconds) {
  const n = Number(seconds);
  if (!Number.isFinite(n) || n < 0) return "--";
  const days = Math.floor(n / 86400);
  const hours = Math.floor((n % 86400) / 3600);
  const mins = Math.floor((n % 3600) / 60);
  if (days > 0) return `${days}d ${hours}h ${mins}m`;
  if (hours > 0) return `${hours}h ${mins}m`;
  return `${mins}m`;
}

function renderSensorWidget() {
  const widget = document.getElementById("sensorWidget");
  if (!widget) return;
  // Live sensor reads need Bluetooth/LAN access, which only the desktop app has.
  if (!IS_TAURI) {
    widget.style.display = "none";
    return;
  }

  if (!selectedPlantId) {
    widget.style.display = "none";
    return;
  }

  const device = getLinkedDevice(selectedPlantId);
  if (!device) {
    widget.style.display = "none";
    return;
  }

  const reading = latestReadings[device.device_id];
  widget.style.display = "";

  const tempColor = reading?.temp != null
    ? (reading.temp > 30 ? TC.danger : reading.temp < 18 ? '#70b0ff' : '#6bffbc')
    : TC.textDim;
  const humiColor = reading?.humidity != null
    ? (reading.humidity > 70 ? '#70b0ff' : reading.humidity < 30 ? TC.danger : '#6bffbc')
    : TC.textDim;
  const vpdColor = reading?.vpd != null
    ? (reading.vpd > 1.6 ? TC.danger : reading.vpd < 0.4 ? '#70b0ff' : '#6bffbc')
    : TC.textDim;

  const temp = reading?.temp != null ? `${reading.temp.toFixed(1)}` : '--';
  const humi = reading?.humidity != null ? `${reading.humidity.toFixed(0)}` : '--';
  const vpd = reading?.vpd != null ? `${reading.vpd.toFixed(2)}` : '--';
  const fanOn = reading ? (reading.fan_level > 0 ? t('devices.on') : t('devices.off')) : '--';
  const fanLevel = reading?.fan_level ?? '--';
  const lightLevel = reading?.light_level ?? '--';
  const lastUpdate = reading?.created_at ? new Date(reading.created_at).toLocaleString(localeTag()) : t('devices.noData');

  widget.innerHTML = `
    <div class="view-card" style="margin-bottom:12px;">
      <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:10px;">
        <h3 style="margin:0; color:${TC.accent}; font-size:15px;">${escapeHtml(device.name)} - ${t('devices.liveSensors')}</h3>
        <div style="display:flex; gap:6px;">
          <button class="btn btn--sm btn--primary" id="sensorRefreshBtn">${t('devices.fetch')}</button>
          <button class="btn btn--sm" id="sensorDebugBtn">${t('devices.debugBle')}</button>
          <button class="btn btn--sm" id="lcStatusBtn">${t('devices.lcStatus')}</button>
          <button class="btn btn--sm" id="sensorAutoLogBtn">${t('devices.autoLog')}</button>
          <button class="btn btn--ghost btn--sm" id="sensorUnlinkBtn">${t('devices.unlink')}</button>
        </div>
      </div>
      <div style="display:grid; grid-template-columns:repeat(auto-fit, minmax(100px, 1fr)); gap:10px; margin-bottom:10px;">
        <div style="text-align:center; padding:10px; border-radius:12px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.08);">
          <div style="font-size:11px; color:${TC.textMuted};">${t('devices.temperature')}</div>
          <div style="font-size:24px; font-weight:700; color:${tempColor};">${temp}<span style="font-size:14px;">&deg;C</span></div>
        </div>
        <div style="text-align:center; padding:10px; border-radius:12px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.08);">
          <div style="font-size:11px; color:${TC.textMuted};">${t('devices.humidity')}</div>
          <div style="font-size:24px; font-weight:700; color:${humiColor};">${humi}<span style="font-size:14px;">%</span></div>
        </div>
        <div style="text-align:center; padding:10px; border-radius:12px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.08);">
          <div style="font-size:11px; color:${TC.textMuted};">VPD</div>
          <div style="font-size:24px; font-weight:700; color:${vpdColor};">${vpd}<span style="font-size:14px;"> kPa</span></div>
        </div>
        <div style="text-align:center; padding:10px; border-radius:12px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.08);">
          <div style="font-size:11px; color:${TC.textMuted};">${t('devices.fan')}</div>
          <div style="font-size:18px; font-weight:600; color:${TC.text};">${fanOn} (${fanLevel})</div>
        </div>
        <div style="text-align:center; padding:10px; border-radius:12px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.08);">
          <div style="font-size:11px; color:${TC.textMuted};">${t('devices.light')}</div>
          <div style="font-size:18px; font-weight:600; color:${TC.text};">${lightLevel}%</div>
        </div>
      </div>
      <div style="color:${TC.textDim}; font-size:11px;">${t('devices.lastUpdate')} ${lastUpdate}</div>
      <div id="lcStatusContainer" style="margin-top:12px;"></div>
      <div id="sensorChartContainer" style="margin-top:12px;">
        <div style="display:flex; gap:6px; margin-bottom:8px;">
          <button class="btn btn--ghost btn--sm sensor-range-btn" data-range="60">1h</button>
          <button class="btn btn--ghost btn--sm sensor-range-btn" data-range="360">6h</button>
          <button class="btn btn--ghost btn--sm sensor-range-btn active" data-range="1440">24h</button>
          <button class="btn btn--ghost btn--sm sensor-range-btn" data-range="10080">7d</button>
        </div>
        <canvas id="sensorChart" height="180"></canvas>
      </div>
    </div>
  `;

  document.getElementById("lcStatusBtn")?.addEventListener("click", async () => {
    const btn = document.getElementById("lcStatusBtn");
    const container = document.getElementById("lcStatusContainer");
    if (btn) { btn.disabled = true; btn.textContent = t('devices.reading'); }
    try {
      const result = await invoke("lc_read_status", { deviceId: device.device_id });
      if (container) container.innerHTML = renderLcStatusCard(result);
      logOut(result);
    } catch (err) {
      if (container) container.innerHTML = `<div style="color:${TC.danger}; padding:10px;">${t('devices.lcStatusFailed')}${escapeHtml(String(err?.message || err))}</div>`;
    } finally {
      if (btn) { btn.disabled = false; btn.textContent = t('devices.lcStatus'); }
    }
  });

  document.getElementById("sensorDebugBtn")?.addEventListener("click", async () => {
    const btn = document.getElementById("sensorDebugBtn");
    if (btn) { btn.disabled = true; btn.textContent = t('devices.reading'); }
    try {
      const result = await invoke("ble_debug_read", { deviceId: device.device_id });
      const errPart = result.log_error ? `\n\nFEHLER: ${result.log_error}` : '';
      const msg = `BLE Debug:\n\nRaw Bytes: ${result.raw_bytes}\n\nLog-Datei:\n${result.log_path || '(nicht geschrieben)'}${errPart}\n\nHex:\n${result.hex_preview}\n\nASCII:\n${result.ascii_preview}`;
      alert(msg);
      logOut(result);
    } catch (err) {
      alert(t('devices.bleDebugFailed') + (err?.message || err));
    } finally {
      if (btn) { btn.disabled = false; btn.textContent = t('devices.debugBle'); }
    }
  });

  document.getElementById("sensorRefreshBtn")?.addEventListener("click", async () => {
    const btn = document.getElementById("sensorRefreshBtn");
    if (btn) { btn.disabled = true; btn.textContent = t('devices.reading'); }
    try {
      const isTuya = device.device_type === "tuya" || device.ble_address?.startsWith("tuya:");
      if (isTuya) {
        const result = await invoke("tuya_read_status", { deviceId: device.device_id });
        logOut(result);
        await loadLatestReadings();
        renderSensorWidget();
      } else {
        // Auto-detect protocol: try LC first (plain JSON), fall back to GGS (Tuya-encrypted)
        let handledAsLc = false;
        try {
          const lc = await invoke("lc_read_status", { deviceId: device.device_id });
          if (lc && lc.frame_count > 0) {
            const container = document.getElementById("lcStatusContainer");
            if (container) container.innerHTML = renderLcStatusCard(lc);
            logOut(lc);
            handledAsLc = true;
          }
        } catch (_) { /* ignore, fall through to GGS */ }

        if (!handledAsLc) {
          await invoke("ble_read_telemetry", { deviceId: device.device_id });
          await loadLatestReadings();
          renderSensorWidget();
        }
      }
    } catch (err) {
      alert(t('devices.sensorFailed') + (err?.message || err));
    } finally {
      if (btn) { btn.disabled = false; btn.textContent = t('devices.fetch'); }
    }
  });

  document.getElementById("sensorUnlinkBtn")?.addEventListener("click", async () => {
    try {
      await invoke("link_device", { deviceId: device.device_id, plantId: null });
      await loadDevices();
      renderSensorWidget();
      renderDeviceManagementPanel();
    } catch (err) {
      alert(t('devices.unlinkFailed') + (err?.message || err));
    }
  });

  document.getElementById("sensorAutoLogBtn")?.addEventListener("click", async () => {
    try {
      const log = await invoke("create_auto_log_from_reading", { deviceId: device.device_id });
      if (log) {
        await loadLogs();
        alert(t('devices.autoLogCreated'));
      }
    } catch (err) {
      alert(t('devices.autoLogFailed') + (err?.message || err));
    }
  });

  widget.querySelectorAll(".sensor-range-btn").forEach(btn => {
    btn.addEventListener("click", () => {
      widget.querySelectorAll(".sensor-range-btn").forEach(b => b.classList.remove("active"));
      btn.classList.add("active");
      const minutes = parseInt(btn.dataset.range, 10);
      renderSensorChart(device.device_id, minutes);
    });
  });

  // Load chart for default 24h
  renderSensorChart(device.device_id, 1440);
}

// Chart.js is bundled and loaded on first use. It used to come from a CDN,
// which broke charts offline and sent every visitor's IP address to a third
// party on each page load (the reasoning of LG Muenchen, 3 O 17493/20, on
// externally loaded web fonts applies equally here).
let chartModule = null;
async function loadChart() {
  if (!chartModule) chartModule = import('chart.js/auto').then((m) => m.default).catch((err) => {
    chartModule = null;
    console.warn('chart.js could not be loaded', err);
    return null;
  });
  return chartModule;
}

async function renderSensorChart(deviceId, rangeMinutes) {
  const canvas = document.getElementById("sensorChart");
  if (!canvas) return;

  const limit = Math.min(500, Math.max(50, Math.floor(rangeMinutes / 2)));
  let readings = [];
  try {
    readings = await invoke("list_device_readings", { deviceId, limit });
  } catch (e) {
    console.warn("Failed to load readings for chart:", e);
    return;
  }

  // Filter by time range
  const cutoff = new Date(Date.now() - rangeMinutes * 60 * 1000);
  readings = readings.filter(r => new Date(r.created_at) >= cutoff);
  readings.reverse(); // oldest first

  const Chart = await loadChart();
  if (!Chart) {
    canvas.parentElement.innerHTML = `<div style="color:${TC.textDim}; font-size:12px; padding:8px;">${t('chart.notLoaded')}</div>`;
    return;
  }

  if (sensorChartInstance) {
    sensorChartInstance.destroy();
    sensorChartInstance = null;
  }

  const labels = readings.map(r => new Date(r.created_at).toLocaleTimeString(localeTag(), { hour: "2-digit", minute: "2-digit" }));
  const tempData = readings.map(r => r.temp);
  const humiData = readings.map(r => r.humidity);
  const vpdData = readings.map(r => r.vpd);

  sensorChartInstance = new Chart(canvas, {
    type: "line",
    data: {
      labels,
      datasets: [
        { label: t('chart.temp'), data: tempData, borderColor: "#ff9966", backgroundColor: "rgba(255,153,102,0.1)", yAxisID: "y", tension: 0.3, pointRadius: 1 },
        { label: t('chart.humidity'), data: humiData, borderColor: "#6bbbff", backgroundColor: "rgba(107,187,255,0.1)", yAxisID: "y", tension: 0.3, pointRadius: 1 },
        { label: t('chart.vpd'), data: vpdData, borderColor: "#6bffbc", backgroundColor: "rgba(107,255,188,0.1)", yAxisID: "y2", tension: 0.3, pointRadius: 1 },
      ],
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      interaction: { mode: "index", intersect: false },
      plugins: { legend: { labels: { color: "#acd1c3", font: { size: 11 } } } },
      scales: {
        x: { ticks: { color: TC.textDim, maxTicksLimit: 10, font: { size: 10 } }, grid: { color: `rgba(${TC.accentRgb},0.05)` } },
        y: { position: "left", ticks: { color: TC.textDim, font: { size: 10 } }, grid: { color: `rgba(${TC.accentRgb},0.05)` }, title: { display: true, text: t('chart.axis.tempHumi'), color: TC.textDim, font: { size: 10 } } },
        y2: { position: "right", ticks: { color: TC.textDim, font: { size: 10 } }, grid: { drawOnChartArea: false }, title: { display: true, text: t('chart.axis.vpd'), color: TC.textDim, font: { size: 10 } } },
      },
    },
  });
}

async function loadLatestReadings() {
  for (const device of devices) {
    try {
      const readings = await invoke("list_device_readings", { deviceId: device.device_id, limit: 1 });
      if (readings.length > 0) {
        latestReadings[device.device_id] = readings[0];
      }
    } catch (e) {
      console.warn("Failed to load latest reading for device", device.device_id, e);
    }
  }
}

function renderPlanStatusLine() {
  const active = activePlantCount();
  const limitText = formatPlantLimit(currentPlantLimit);
  const withinLimit = canActivateMorePlants();
  const overCount = overflowPlantCount();
  const tone = withinLimit ? TC.textMuted : '#ffb3a7';
  const badgeBg = withinLimit ? `rgba(${TC.accentRgb},0.08)` : 'rgba(255,120,120,0.12)';
  const badgeBorder = withinLimit ? `rgba(${TC.accentRgb},0.18)` : 'rgba(255,120,120,0.24)';
  return `
    <div style="margin-top:10px; padding:10px 12px; border-radius:12px; border:1px solid ${badgeBorder}; background:${badgeBg}; font-size:13px; color:${tone};">
      ${t('plan.statusLine', `<strong style="color:${TC.text};">${planLabel(currentPlan)}</strong>`, active, limitText, isOverPlantLimit() ? t('plan.overLimit', overCount) : t('plan.freeSlots', Number.isFinite(remainingPlantSlots()) ? remainingPlantSlots() : '∞'), currentPlanSource === 'account' ? t('plan.source.account') : t('plan.source.guest'))}
    </div>
  `;
}

function setCheckoutState(next) {
  checkoutState = { ...checkoutState, ...next, updatedAt: new Date().toISOString() };
}

function resetCheckoutState(message) {
  checkoutState = { status: "idle", plan: null, message: message ?? t('billing.noUpgrade'), checkoutUrl: null, updatedAt: null };
}

function renderCheckoutStateCard() {
  const tone = checkoutState.status === "ready" ? "#9fd7ff" : checkoutState.status === "error" ? "#ffb3a7" : TC.textMuted;
  const border = checkoutState.status === "ready" ? "rgba(159,215,255,0.25)" : checkoutState.status === "error" ? "rgba(255,120,120,0.24)" : "rgba(255,255,255,0.08)";
  const bg = checkoutState.status === "ready" ? "rgba(159,215,255,0.08)" : checkoutState.status === "error" ? "rgba(255,120,120,0.10)" : "rgba(255,255,255,0.03)";
  const updated = checkoutState.updatedAt ? ` · ${formatDate(checkoutState.updatedAt)}` : "";
  const action = checkoutState.checkoutUrl
    ? `<div style="margin-top:8px; display:flex; gap:8px; flex-wrap:wrap;"><a href="${checkoutState.checkoutUrl}" target="_blank" rel="noreferrer" style="color:#9fd7ff; text-decoration:none; font-weight:600;">${t('billing.openCheckout')}</a><button type="button" data-checkout-webhook-plan="${checkoutState.plan || ''}" style="padding:6px 10px;">${t('billing.simulateWebhook')}</button></div>`
    : checkoutState.plan
      ? `<div style="margin-top:8px; font-size:12px; color:${TC.textMuted};">${t('billing.checkoutPrepared')}</div><div style="margin-top:8px;"><button type="button" data-checkout-webhook-plan="${checkoutState.plan || ''}" style="padding:6px 10px;">${t('billing.simulateWebhook')}</button></div>`
      : '';
  return `
    <div style="margin:12px 0; padding:12px; border-radius:12px; border:1px solid ${border}; background:${bg}; color:${tone}; font-size:12px; line-height:1.5;">
      <div style="display:flex; justify-content:space-between; gap:8px; align-items:center;">
        <strong style="color:${TC.text};">${t('billing.checkoutStatusLabel')}</strong>
        <span style="padding:4px 8px; border-radius:999px; background:rgba(255,255,255,0.06); color:${tone};">${checkoutState.status === 'ready' ? t('billing.ready') : checkoutState.status === 'error' ? t('billing.errorWord') : t('billing.idle')}</span>
      </div>
      <div style="margin-top:6px;">${checkoutState.message}${updated}</div>
      ${checkoutState.plan ? `<div style="margin-top:6px; color:#9fb8ad;">${t('billing.targetPlan')}: ${planLabel(checkoutState.plan)}</div>` : ''}
      ${action}
    </div>
  `;
}


function loadCheckoutHistory() {
  try {
    const raw = localStorage.getItem(CHECKOUT_HISTORY_STORAGE_KEY);
    const parsed = raw ? JSON.parse(raw) : [];
    checkoutHistory = Array.isArray(parsed) ? parsed : [];
  } catch {
    checkoutHistory = [];
  }
}

function persistCheckoutHistory() {
  try {
    localStorage.setItem(CHECKOUT_HISTORY_STORAGE_KEY, JSON.stringify(checkoutHistory.slice(0, 12)));
  } catch {}
}

function addCheckoutHistoryEntry(entry) {
  checkoutHistory = [
    {
      id: `${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
      createdAt: new Date().toISOString(),
      ...entry,
    },
    ...checkoutHistory,
  ].slice(0, 12);
  persistCheckoutHistory();
}

function mapServerHistoryEntry(entry) {
  return {
    id: entry?.event_id || entry?.eventId || `${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
    createdAt: entry?.created_at || entry?.createdAt || new Date().toISOString(),
    plan: normalizePlan(entry?.plan || "free"),
    previousPlan: entry?.previous_plan || entry?.previousPlan || null,
    checkoutId: entry?.checkout_id || entry?.checkoutId || null,
    provider: entry?.provider || null,
    eventKind: entry?.event_kind || entry?.eventKind || "status",
    checkoutStatus: entry?.checkout_status || entry?.checkoutStatus || null,
    status: String(entry?.checkout_status || entry?.checkoutStatus || entry?.event_kind || entry?.eventKind || "info").toLowerCase(),
    message: entry?.message || t('billing.noDetailText'),
    mode: String(entry?.event_kind || entry?.eventKind || "status").includes("webhook") ? "webhook" : "checkout",
    source: "server",
  };
}

async function loadSubscriptionHistory() {
  if (currentPlanSource !== "account") {
    subscriptionHistory = [];
    subscriptionHistorySource = "local";
    return;
  }
  try {
    const result = await invoke("subscription_history_smart");
    const items = Array.isArray(result?.history) ? result.history : [];
    subscriptionHistory = items.map(mapServerHistoryEntry);
    subscriptionHistorySource = "server";
  } catch (_) {
    subscriptionHistory = [];
    subscriptionHistorySource = "local";
  }
}


function activeSubscriptionHistory() {
  return subscriptionHistorySource === 'server' && subscriptionHistory.length ? subscriptionHistory : checkoutHistory;
}

function latestSubscriptionHistoryEntry() {
  const history = activeSubscriptionHistory();
  return history.length ? history[0] : null;
}

function normalizedSubscriptionLifecycleStatus() {
  const latest = latestSubscriptionHistoryEntry();
  const raw = String(latest?.checkoutStatus || latest?.status || latest?.eventKind || checkoutState.status || '').toLowerCase();
  if (checkoutState.status === 'error' || raw.includes('error') || raw.includes('fail')) return 'error';
  if (raw.includes('pending') || raw.includes('prepared') || raw.includes('await')) return 'pending';
  if (raw.includes('paid') || raw.includes('active') || raw.includes('success') || raw.includes('activated') || checkoutState.status === 'ready') return 'active';
  return currentPlanSource === 'account' ? 'active' : 'guest';
}

function renderUnifiedSubscriptionStatusCard() {
  const latest = latestSubscriptionHistoryEntry();
  const lifecycle = normalizedSubscriptionLifecycleStatus();
  const tone = lifecycle === 'error'
    ? '#ffb3a7'
    : lifecycle === 'pending'
      ? '#9fd7ff'
      : lifecycle === 'guest'
        ? TC.textMuted
        : '#9ff1c5';
  const border = lifecycle === 'error'
    ? 'rgba(255,120,120,0.24)'
    : lifecycle === 'pending'
      ? 'rgba(159,215,255,0.22)'
      : lifecycle === 'guest'
        ? 'rgba(255,255,255,0.08)'
        : `rgba(${TC.accentRgb},0.18)`;
  const bg = lifecycle === 'error'
    ? 'rgba(255,120,120,0.10)'
    : lifecycle === 'pending'
      ? 'rgba(159,215,255,0.08)'
      : lifecycle === 'guest'
        ? 'rgba(255,255,255,0.03)'
        : `rgba(${TC.accentRgb},0.07)`;
  const statusLabel = lifecycle === 'error'
    ? t('billing.statusProblem')
    : lifecycle === 'pending'
      ? t('billing.statusPending')
      : lifecycle === 'guest'
        ? t('billing.statusGuest')
        : t('billing.statusActive');
  const latestLine = latest
    ? `${latest.message || t('plan.latestStatus')} · ${formatDate(latest.createdAt)}`
    : currentPlanSource === 'account'
      ? t('plan.noServerActivity')
      : t('plan.guestInfo');
  const historySourceLabel = subscriptionHistorySource === 'server' && subscriptionHistory.length ? t('billing.historyBackend') : t('billing.historyLocal');
  const active = activePlantCount();
  const limitText = formatPlantLimit(currentPlantLimit);
  const capacityLine = isOverPlantLimit()
    ? t('plan.capacityOver', overflowPlantCount())
    : t('plan.capacityInside', active, limitText, Number.isFinite(remainingPlantSlots()) ? t('plan.slotsFree', remainingPlantSlots()) : t('plan.noLimit'));
  const endedCount = summaryPlantCount('ended_plants', plants.filter((plant) => isPlantEnded(plant)).length);
  const archivedCount = summaryPlantCount('archived_plants', plants.filter((plant) => isPlantArchived(plant)).length);
  const visibleCount = summaryPlantCount('visible_plants', plants.filter((plant) => plantStatusValue(plant) !== "deleted").length);
  const detailLine = subscriptionStatusSummary ? `${t('plan.ended')} ${endedCount} · ${t('plan.archived')} ${archivedCount} · ${t('plan.visible')} ${visibleCount} · ${t('plan.grandfathered')} ${subscriptionStatusSummary.grandfathered_active_plants ?? 0}` : '';
  return `
    <div style="margin:12px 0; padding:12px; border-radius:12px; border:1px solid ${border}; background:${bg}; color:${tone}; font-size:12px; line-height:1.55;">
      <div style="display:flex; justify-content:space-between; gap:8px; align-items:center; flex-wrap:wrap;">
        <strong style="color:${TC.text};">${t('billing.aboStatus')}</strong>
        <span style="padding:4px 8px; border-radius:999px; background:rgba(255,255,255,0.06); color:${tone};">${statusLabel}</span>
      </div>
      <div style="margin-top:8px; color:${TC.text};"><strong>${planLabel(currentPlan)}</strong> · ${getPlanConfig().find((entry) => entry.key === currentPlan)?.price || t('plan.price.free')} · ${getPlanConfig().find((entry) => entry.key === currentPlan)?.limitText || ''}</div>
      <div style="margin-top:6px; color:#9fb8ad;">${capacityLine}</div>
      ${detailLine ? `<div style="margin-top:6px; color:${TC.textMuted};">${detailLine}</div>` : ''}
      <div style="margin-top:6px; color:${TC.textMuted};">${t('billing.source')}: ${currentPlanSource === 'account' ? t('billing.sourceAccount') : t('billing.sourceGuest')} · ${t('billing.history')}: ${historySourceLabel}</div>
      <div style="margin-top:8px; padding-top:8px; border-top:1px solid rgba(255,255,255,0.08); color:${TC.textMuted};">${latestLine}</div>
      ${checkoutState.plan && checkoutState.plan !== currentPlan ? `<div style="margin-top:6px; color:#9fd7ff;">${t('plan.checkoutTarget', planLabel(checkoutState.plan))}</div>` : ''}
    </div>
  `;
}

function renderCheckoutHistoryCard() {
  const activeHistory = subscriptionHistorySource === 'server' && subscriptionHistory.length ? subscriptionHistory : checkoutHistory;
  const sourceLabel = subscriptionHistorySource === 'server' && subscriptionHistory.length ? t('billing.historyBackend') : t('billing.historyLocal');
  const items = activeHistory.slice(0, 8).map((entry) => {
    const statusTone = String(entry.status || '').includes('success') || String(entry.status || '').includes('active')
      ? '#9ff1c5'
      : String(entry.status || '').includes('error') || String(entry.status || '').includes('fail')
        ? '#ffb3a7'
        : '#9fd7ff';
    const modeLabel = entry.mode === 'webhook'
      ? t('billing.webhook')
      : entry.mode === 'checkout'
        ? t('billing.checkout')
        : t('billing.statusWord');
    const planText = entry.plan ? planLabel(entry.plan) : '—';
    const eventMeta = entry.eventKind ? ` · ${String(entry.eventKind).replaceAll('_', ' ')}` : '';
    return `
      <div style="padding:10px 12px; border-radius:12px; border:1px solid rgba(255,255,255,0.08); background:rgba(255,255,255,0.03); font-size:12px; line-height:1.45;">
        <div style="display:flex; justify-content:space-between; gap:8px; align-items:center;">
          <strong style="color:${TC.text};">${modeLabel} · ${planText}${eventMeta}</strong>
          <span style="padding:4px 8px; border-radius:999px; background:rgba(255,255,255,0.06); color:${statusTone};">${entry.status || 'info'}</span>
        </div>
        <div style="margin-top:6px; color:#9fb8ad;">${entry.message || t('billing.noDetail')}</div>
        <div style="margin-top:6px; color:${TC.textMuted};">${formatDate(entry.createdAt)}${entry.checkoutId ? ` · ${entry.checkoutId}` : ''}${entry.provider ? ` · ${entry.provider}` : ''}</div>
      </div>
    `;
  }).join('');

  return `
    <div style="margin-top:12px; padding:12px; border-radius:12px; border:1px solid rgba(255,255,255,0.08); background:rgba(255,255,255,0.02);">
      <div style="display:flex; justify-content:space-between; gap:8px; align-items:center; margin-bottom:8px;">
        <strong style="color:${TC.text};">${t('billing.checkoutHistory')}</strong>
        <span style="font-size:12px; color:${TC.textMuted};">${activeHistory.length} ${t('billing.entries')} · ${sourceLabel}</span>
      </div>
      ${items || `<div style="font-size:12px; color:${TC.textMuted};">${t('billing.noActivity')}</div>`}
    </div>
  `;
}

function getPlanConfig() {
  return [
    { key: "free",  title: "Free",  price: t('plan.price.free'),  limitText: t('plan.limit.1'),         accent: TC.textMuted, detail: t('plan.free.detail') },
    { key: "basic", title: "Basic", price: t('plan.price.basic'), limitText: t('plan.limit.3'),         accent: TC.accent,    detail: t('plan.basic.detail') },
    { key: "pro",   title: "Pro",   price: t('plan.price.pro'),   limitText: t('plan.limit.100'),       accent: "#9fd7ff",         detail: t('plan.pro.detail') },
    { key: "csc",   title: "CSC",   price: t('plan.price.csc'),   limitText: t('plan.limit.unlimited'), accent: "#ffd76e",         detail: t('plan.csc.detail') },
  ];
}
const PLAN_CONFIG = getPlanConfig();

function planPreviewFor(targetPlan) {
  const normalized = normalizePlan(targetPlan);
  const currentActive = activePlantCount();
  const nextLimit = planLimitFor(normalized);
  const overAfterChange = Number.isFinite(nextLimit) && currentActive > nextLimit;
  const remainingSlots = Number.isFinite(nextLimit) ? Math.max(0, nextLimit - currentActive) : null;
  const title = planLabel(normalized);
  const price = getPlanConfig().find((entry) => entry.key === normalized)?.price || "";
  const tone = overAfterChange ? '#ffd1c7' : '#a9f7cd';
  const bg = overAfterChange ? 'rgba(255,120,120,0.10)' : `rgba(${TC.accentRgb},0.08)`;
  const border = overAfterChange ? 'rgba(255,120,120,0.24)' : `rgba(${TC.accentRgb},0.18)`;
  const limitStr = formatPlantLimit(nextLimit);
  const slotsStr = remainingSlots === null ? t('plan.noLimit').replace(/\.$/, '') : remainingSlots;
  const plantWord = remainingSlots === 1 ? t('plant.singular') : t('plant.plural');
  const summary = overAfterChange
    ? t('plan.switch.warnOver', currentActive, title, limitStr, limitStr)
    : t('plan.switch.warnOk', title, slotsStr, plantWord);
  const cleanupHint = overAfterChange
    ? t('plan.switch.cleanupHint')
    : t('plan.switch.preserveHint');
  return {
    title,
    price,
    summary,
    cleanupHint,
    tone,
    bg,
    border,
    overAfterChange,
  };
}

function renderSubscriptionPanel() {
  if (!subscriptionPanel) return;
  const sessionHint = currentPlanSource === "account"
    ? t('billing.accountHint')
    : t('billing.guestHint');
  const cards = getPlanConfig().map((plan) => {
    const active = normalizePlan(plan.key) === currentPlan;
    const buttonDisabled = currentPlanSource !== "account" || active;
    const buttonLabel = active ? t('billing.activeBadge') : currentPlanSource !== "account" ? t('billing.loginNeeded') : t('billing.prepareCheckout', plan.title);
    const border = active ? `1px solid ${plan.accent}` : '1px solid rgba(255,255,255,0.08)';
    const glow = active ? `0 0 0 1px ${plan.accent} inset, 0 0 18px rgba(${TC.accentRgb},0.08)` : 'none';
    const preview = !active ? planPreviewFor(plan.key) : null;
    return `
      <div style="padding:14px; border-radius:16px; background:${TC.inputBg}; border:${border}; box-shadow:${glow};">
        <div style="display:flex; justify-content:space-between; align-items:center; gap:8px;">
          <strong style="font-size:16px; color:${active ? plan.accent : TC.text};">${plan.title}</strong>
          <span style="font-size:12px; padding:4px 8px; border-radius:999px; background:${active ? `rgba(${TC.accentRgb},0.12)` : 'rgba(255,255,255,0.06)'}; color:${active ? plan.accent : '#9fb8ad'};">${active ? t('billing.activeBadge') : t('billing.planBadge')}</span>
        </div>
        <div style="margin-top:8px; font-size:14px; color:${TC.text};">${plan.price}</div>
        <div style="margin-top:6px; font-size:13px; color:#9fb8ad;">${plan.limitText}</div>
        <div style="margin-top:8px; font-size:12px; color:${TC.textMuted}; line-height:1.5;">${plan.detail}</div>
        ${preview ? `<div style="margin-top:10px; padding:10px 12px; border-radius:12px; border:1px solid ${preview.border}; background:${preview.bg}; color:${preview.tone}; font-size:12px; line-height:1.5;">
          <div style="font-weight:700; color:${TC.text}; margin-bottom:4px;">${t('billing.previewTitle', preview.title)}</div>
          <div>${preview.summary}</div>
          <div style="margin-top:6px; color:#9fe0c1;">${preview.cleanupHint}</div>
        </div>` : ''}
        <button ${buttonDisabled ? 'disabled' : ''} data-plan-action="${plan.key}" style="margin-top:12px; width:100%; opacity:${buttonDisabled ? '0.6' : '1'};">${buttonLabel}</button>
        ${!active && currentPlanSource === "account" ? `<div style="margin-top:6px; font-size:11px; color:${TC.textMuted};">${t('billing.mvpNote')}</div>` : ''}
      </div>
    `;
  }).join('');
  subscriptionPanel.innerHTML = `
    <h2 style="margin-top:0; color:${TC.accent}; font-size:18px;">${t('billing.title')}</h2>
    <div style="font-size:13px; color:${TC.textMuted}; line-height:1.5; margin-bottom:12px;">${sessionHint}</div>
    <div style="margin-bottom:12px; padding:10px 12px; border-radius:12px; border:1px solid rgba(255,255,255,0.08); background:rgba(255,255,255,0.03); color:#9fb8ad; font-size:12px; line-height:1.5;">
      ${t('billing.planSwitchRule')}
    </div>
    <div style="margin-bottom:12px; padding:10px 12px; border-radius:12px; border:1px solid rgba(${TC.accentRgb},0.10); background:${TC.surface}; color:${TC.textMuted}; font-size:12px; line-height:1.6;">
      ${t('billing.mvpStatus')}
    </div>
    ${renderUnifiedSubscriptionStatusCard()}
    ${renderCheckoutStateCard()}
    ${renderCheckoutHistoryCard()}
    <div style="display:grid; grid-template-columns:repeat(auto-fit, minmax(220px, 1fr)); gap:12px; margin-top:12px;">${cards}</div>
  `;
}

const PLANT_FILTER_STORAGE_KEYS = {
  search: 'bs_plants_search_query',
  phase: 'bs_plants_phase_filter',
  state: 'bs_plants_state_filter',
  sort: 'bs_plants_sort',
};

function loadPlantFilterPrefs() {
  try {
    plantSearchQuery = localStorage.getItem(PLANT_FILTER_STORAGE_KEYS.search) || "";
    plantPhaseFilter = localStorage.getItem(PLANT_FILTER_STORAGE_KEYS.phase) || "all";
    plantStateFilter = localStorage.getItem(PLANT_FILTER_STORAGE_KEYS.state) || "all";
    plantSort = localStorage.getItem(PLANT_FILTER_STORAGE_KEYS.sort) || "updated_desc";
  } catch (_) {}
}

function persistPlantFilterPrefs() {
  try {
    localStorage.setItem(PLANT_FILTER_STORAGE_KEYS.search, plantSearchQuery || "");
    localStorage.setItem(PLANT_FILTER_STORAGE_KEYS.phase, plantPhaseFilter || "all");
    localStorage.setItem(PLANT_FILTER_STORAGE_KEYS.state, plantStateFilter || "all");
    localStorage.setItem(PLANT_FILTER_STORAGE_KEYS.sort, plantSort || "updated_desc");
  } catch (_) {}
}

function applyPlantFilterPrefsToControls() {
  if ($("plantSearch")) $("plantSearch").value = plantSearchQuery;
  if ($("plantPhaseFilter")) $("plantPhaseFilter").value = plantPhaseFilter;
  if ($("plantStateFilter")) $("plantStateFilter").value = plantStateFilter;
  if ($("plantSort")) $("plantSort").value = plantSort;
}

const plantFormError = null; // Now in modal
const taskFormError = null; // Now in modal
const logFormError = null; // Now in modal
const plantPlanHint = null; // Now in modal

function showFormError(node, message) {
  if (!node) return;
  node.textContent = message;
  node.style.display = message ? "block" : "none";
}

function clearFormError(node) {
  showFormError(node, "");
}

function resetPlantForm() {
  if ($("plantName")) $("plantName").value = "";
  if ($("plantColor")) $("plantColor").value = "#0B3D2E";
  if ($("plantPhase")) $("plantPhase").value = "veg";
  clearFormError(plantFormError);
}

function timelineDefaultDueValue() {
  const baseKey = timelineFocusKey || new Date().toISOString().slice(0, 10);
  return `${baseKey}T09:00`;
}

function applyTimelineDueToTaskForm() {
  const input = $("taskDueAt");
  if (!input) return;
  input.value = timelineDefaultDueValue();
}

function timelineDefaultLogValue() {
  const baseKey = timelineFocusKey || new Date().toISOString().slice(0, 10);
  return `${baseKey}T12:00`;
}

function applyTimelineDateToLogForm() {
  const input = $("logCreatedAt");
  if (!input) return;
  input.value = timelineDefaultLogValue();
  const hint = $("logDateHint");
  if (hint) hint.textContent = `Log-Zeitpunkt: ${dateLabelFromKey(timelineFocusKey || new Date().toISOString().slice(0, 10))}`;
}

function resetTaskForm() {
  if ($("taskTitle")) $("taskTitle").value = "";
  if ($("taskCategory")) $("taskCategory").value = "water";
  if ($("taskRepeat")) $("taskRepeat").value = "";
  const toggle = $("taskNotificationEnabled");
  if (toggle) toggle.checked = true;
  applyTimelineDueToTaskForm();
  clearFormError(taskFormError);
}

const TASK_TEMPLATES = {
  water: {
    titleKey: "taskTemplate.water",
    category: "water",
    repeatHours: 48,
    notificationEnabled: true,
  },
  feed: {
    titleKey: "taskTemplate.feed",
    category: "feed",
    repeatHours: 168,
    notificationEnabled: true,
  },
  check: {
    title: "Check-in",
    category: "check",
    repeatHours: 24,
    notificationEnabled: true,
  },
  train: {
    titleKey: "taskTemplate.trainLst",
    category: "train",
    repeatHours: 72,
    notificationEnabled: true,
  },
};

function applyTaskTemplate(templateKey) {
  if (templateKey === "reset") {
    resetTaskForm();
    return;
  }
  const template = TASK_TEMPLATES[templateKey];
  if (!template) return;
  if ($("taskTitle")) $("taskTitle").value = template.titleKey ? t(template.titleKey) : template.title;
  if ($("taskCategory")) $("taskCategory").value = template.category;
  if ($("taskRepeat")) $("taskRepeat").value = template.repeatHours ?? "";
  const toggle = $("taskNotificationEnabled");
  if (toggle) toggle.checked = template.notificationEnabled !== false;
  applyTimelineDueToTaskForm();
  clearFormError(taskFormError);
  $("taskTitle")?.focus();
  $("taskTitle")?.select();
}

const TASK_SETS = {
  starter: [
    { title: "Check-in", category: "check", repeat_interval_hours: 24, notification_enabled: true, offsetHours: 0 },
    { titleKey: "taskTemplate.water", category: "water", repeat_interval_hours: 48, notification_enabled: true, offsetHours: 12 },
    { titleKey: "taskTemplate.feed", category: "feed", repeat_interval_hours: 168, notification_enabled: true, offsetHours: 72 },
  ],
  watering: [
    { titleKey: "taskTemplate.water", category: "water", repeat_interval_hours: 48, notification_enabled: true, offsetHours: 0 },
    { titleKey: "taskTemplate.drainCheck", category: "check", repeat_interval_hours: 48, notification_enabled: true, offsetHours: 6 },
    { titleKey: "taskTemplate.waterNext", category: "note", repeat_interval_hours: 48, notification_enabled: false, offsetHours: 42 },
  ],
  flower: [
    { titleKey: "phase.primaryTask.flower", category: "check", repeat_interval_hours: 24, notification_enabled: true, offsetHours: 0 },
    { titleKey: "taskTemplate.waterFlower", category: "water", repeat_interval_hours: 48, notification_enabled: true, offsetHours: 10 },
    { titleKey: "taskTemplate.trainSupport", category: "train", repeat_interval_hours: 72, notification_enabled: true, offsetHours: 36 },
  ],
};

const TASK_SET_LABELS = {
  get starter() { return t("taskset.starter"); },
  get watering() { return t("taskset.watering"); },
  get flower() { return t("taskset.flower"); },
};

const PHASE_TASK_SET_SUGGESTIONS = {
  seed: ["starter"],
  veg: ["starter", "watering"],
  flower: ["flower", "watering"],
  harvest: ["flower"],
  dry: ["watering"],
  cure: ["watering"],
  custom: ["starter", "watering"],
};

function phaseSuggestedSetKeys(phase) {
  return PHASE_TASK_SET_SUGGESTIONS[phase || "veg"] || PHASE_TASK_SET_SUGGESTIONS.veg;
}


const PHASE_HINT_PHASES = ['seed', 'veg', 'flower', 'harvest', 'dry', 'cure', 'custom'];

function phaseHintsFor(phase) {
  const p = PHASE_HINT_PHASES.includes(phase) ? phase : 'veg';
  return [
    { tone: 'focus',    title: t(`phaseHint.${p}.focus.title`),    text: t(`phaseHint.${p}.focus.text`) },
    { tone: 'calendar', title: t(`phaseHint.${p}.calendar.title`), text: t(`phaseHint.${p}.calendar.text`) },
  ];
}

function phaseHintsForPlant(plant) {
  if (!plant) return [];
  return phaseHintsFor(plant.phase || 'veg');
}

function phaseHintToneStyles(tone) {
  if (tone === 'calendar') {
    return { border: `rgba(${TC.accentRgb},0.18)`, bg: '#0a1512', accent: TC.textMuted };
  }
  return { border: `rgba(${TC.accentRgb},0.18)`, bg: '#0d1c17', accent: TC.accent };
}

function renderPhaseHintCards(plant, compact = false) {
  const hints = phaseHintsForPlant(plant);
  if (!hints.length) return '';
  return hints.map((hint) => {
    const tone = phaseHintToneStyles(hint.tone);
    return `
      <div style="padding:${compact ? '10px' : '12px'}; border-radius:12px; background:${tone.bg}; border:1px solid ${tone.border};">
        <div style="font-size:12px; color:${tone.accent};">${hint.title}</div>
        <div style="margin-top:6px; font-size:${compact ? '12px' : '13px'}; color:#dffcef; line-height:1.45;">${hint.text}</div>
      </div>`;
  }).join('');
}

function phasePrimaryTaskTitle(phase) {
  const map = {
    seed: 'phase.primaryTask.seed',
    veg: 'phase.primaryTask.veg',
    flower: 'phase.primaryTask.flower',
    harvest: 'phase.primaryTask.harvest',
    dry: 'phase.primaryTask.dry',
    cure: 'phase.primaryTask.cure',
    custom: 'phase.primaryTask.custom',
  };
  return t(map[phase] || 'phase.primaryTask.custom');
}

function phasePreferredTimelineMode(phase) {
  const map = {
    seed: 'day',
    veg: 'week',
    flower: 'week',
    harvest: 'day',
    dry: 'day',
    cure: 'week',
    custom: 'week',
  };
  return map[phase] || 'week';
}

function renderPhaseHintActions(plant, compact = false) {
  if (!plant) return '';
  const suggested = phaseSuggestedSetKeys(plant.phase);
  const primarySet = suggested[0] || 'starter_week';
  return `
    <div style="display:flex; flex-wrap:wrap; gap:8px; align-items:center; margin-top:${compact ? '8px' : '10px'};">
      <button type="button" data-phase-action="set" data-phase-set="${primarySet}" style="border:1px solid rgba(${TC.accentRgb},0.18); background:#0d1c17; color:#dffcef;">${t('phaseHint.applySet')}</button>
      <button type="button" data-phase-action="task" style="border:1px solid rgba(${TC.accentRgb},0.14); background:${TC.surface}; color:#dffcef;">${t('phaseHint.prepareTask')}</button>
      <button type="button" data-phase-action="focus" style="border:1px solid rgba(${TC.accentRgb},0.18); background:#0a1512; color:${TC.textMuted};">${t('phaseHint.setFocus')}</button>
      <div style="font-size:12px; color:${TC.textMuted};">${TASK_SET_LABELS[primarySet] || t('phaseHint.standardSet')} · ${phaseLabel(plant.phase)}</div>
    </div>`;
}

async function runPhaseHintAction(action, plantId, setKey = null) {
  if (!plantId) return;
  const plant = plants.find((entry) => entry.plant_id === plantId);
  if (!plant) return;
  if (selectedPlantId !== plantId) {
    await focusPlantFromHome(plantId);
  }
  if (action === 'set') {
    await createTaskSet(setKey || phaseSuggestedSetKeys(plant.phase)[0] || 'starter_week');
    return;
  }
  if (action === 'task') {
    await prepareTaskFromHome(plantId, { prefillTitle: phasePrimaryTaskTitle(plant.phase) });
    return;
  }
  if (action === 'focus') {
    timelineMode = phasePreferredTimelineMode(plant.phase);
    renderTimelinePanel();
    applyTimelineDueToTaskForm();
    applyTimelineDateToLogForm();
    return;
  }
}

function renderPhaseTaskSets() {
  if (!phaseTaskSetBar) return;
  const plant = selectedPlant();
  if (!plant) {
    phaseTaskSetBar.innerHTML = `<div style="font-size:12px; color:${TC.textMuted};">${t('phase.pickPlant')}</div>`;
    return;
  }
  const keys = phaseSuggestedSetKeys(plant.phase);
  phaseTaskSetBar.innerHTML = keys.map((key) => `
    <button type="button" data-phase-task-set="${key}" style="border:1px solid rgba(${TC.accentRgb},0.18); background:#0a1512; color:#dffcef;">${TASK_SET_LABELS[key] || key}</button>`).join("")
    + `<div style="font-size:12px; color:${TC.textMuted}; align-self:center;">${t('phase.recommendedFor', phaseLabel(plant.phase))}</div>`;
}


function addHoursToIso(baseIso, hours) {
  const base = new Date(baseIso);
  if (Number.isNaN(base.getTime())) return baseIso;
  base.setHours(base.getHours() + hours);
  return base.toISOString();
}

async function createTaskSet(setKey) {
  clearFormError(taskFormError);
  if (!selectedPlantId) {
    showFormError(taskFormError, t('plant.chooseFirst'));
    return;
  }
  const set = TASK_SETS[setKey];
  if (!set) return;
  const baseDue = datetimeLocalToIso($("taskDueAt")?.value) || datetimeLocalToIso(timelineDefaultDueValue()) || new Date().toISOString();
  try {
    for (const item of set) {
      await invoke("create_task_smart", {
        payload: {
          plant_id: selectedPlantId,
          title: item.titleKey ? t(item.titleKey) : item.title,
          category: item.category,
          due_at: addHoursToIso(baseDue, item.offsetHours ?? 0),
          repeat_interval_hours: item.repeat_interval_hours ?? null,
          notification_enabled: item.notification_enabled !== false,
        },
      });
    }
    logOut({ ok: true, task_set: setKey, created: set.length });
    await refreshAll({ autoReminderReason: "task_set_create" });
  } catch (err) {
    showFormError(taskFormError, `Standardset konnte nicht angelegt werden: ${err}`);
  }
}

function resetLogForm() {
  if ($("logText")) $("logText").value = "";
  if ($("logPh")) $("logPh").value = "";
  if ($("logEc")) $("logEc").value = "";
  if ($("logTemp")) $("logTemp").value = "";
  if ($("logRh")) $("logRh").value = "";
  applyTimelineDateToLogForm();
  clearFormError(logFormError);
}

function parseOptionalNumber(inputId) {
  const raw = $(inputId).value.trim();
  if (!raw) return null;
  const n = Number(raw);
  return Number.isFinite(n) ? n : NaN;
}


function logOut(data) {
  output.textContent = typeof data === "string" ? data : JSON.stringify(data, null, 2);
}

function phaseLabel(phase) {
  const p = phase || 'veg';
  const key = 'phase.' + p;
  // Check if translation exists in current or fallback locale
  if (localeData[key] || fallbackLocale[key]) return t(key);
  return p.replace(/_/g, " ");
}

function formatDate(value) {
  if (!value) return "-";
  try { return new Date(value).toLocaleString(localeTag()); } catch { return value; }
}

function toDatetimeLocalValue(value) {
  if (!value) return "";
  const d = new Date(value);
  if (Number.isNaN(d.getTime())) return "";
  const pad = (n) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

function datetimeLocalToIso(value) {
  if (!value) return null;
  const d = new Date(value);
  if (Number.isNaN(d.getTime())) return null;
  return d.toISOString();
}

function selectedPlant() {
  return plants.find((p) => p.plant_id === selectedPlantId) || null;
}

function plantNameById(plantId) {
  return escapeHtml(plants.find((plant) => plant.plant_id === plantId)?.name || t('plant.unknown'));
}

async function focusPlantFromHome(plantId) {
  if (!plantId) return;
  selectedPlantId = plantId;
  renderPlants();
  await loadTasks();
  await loadLogs();
  renderDetailPanel();
  renderTimelinePanel();
  renderReminderPanel();
  await loadPlantPhotos();
  renderPhotoGallery();
  renderGrowthAnalytics();
  renderCareInsights();
}

async function prepareLogFromHome(plantId, options = {}) {
  if (!plantId) return;
  await focusPlantFromHome(plantId);
  const targetKey = options.dayKey || normalizeDateKey(options.createdAt) || new Date().toISOString().slice(0, 10);
  setTimelineFocusDay(targetKey);
  timelineMode = 'day';
  renderTimelinePanel();
  openModal('log', {
    plantId,
    createdAt: targetKey,
    text: options.prefillText || '',
  });
}

async function prepareTaskFromHome(plantId, options = {}) {
  if (!plantId) return;
  await focusPlantFromHome(plantId);
  const targetKey = options.dayKey || normalizeDateKey(options.dueAt) || new Date().toISOString().slice(0, 10);
  setTimelineFocusDay(targetKey);
  timelineMode = 'day';
  renderTimelinePanel();
  openModal('task', {
    plantId,
    dueDate: targetKey,
    title: options.prefillTitle || '',
  });
}

function passesHomeLogRangeFilter(log) {
  if (!log || !log.created_at) return homeLogRangeFilter === 'all';
  const logTime = new Date(log.created_at).getTime();
  if (Number.isNaN(logTime)) return homeLogRangeFilter === 'all';
  const now = Date.now();
  const dayMs = 24 * 60 * 60 * 1000;
  if (homeLogRangeFilter === 'today') {
    return normalizeDateKey(log.created_at) === new Date().toISOString().slice(0, 10);
  }
  if (homeLogRangeFilter === '7d') return logTime >= now - (7 * dayMs);
  if (homeLogRangeFilter === '30d') return logTime >= now - (30 * dayMs);
  return true;
}

function filteredHomeActivityLogs() {
  return [...allLogs]
    .filter((log) => homeLogPlantFilter === 'all' || log.plant_id === homeLogPlantFilter)
    .filter((log) => homeLogTypeFilter === 'all' || (log.log_type || 'note') === homeLogTypeFilter)
    .filter((log) => passesHomeLogRangeFilter(log))
    .sort((a, b) => new Date(b.created_at || 0).getTime() - new Date(a.created_at || 0).getTime());
}

function homeMonthGridData() {
  const focusKey = homeMonthFocusKey || new Date().toISOString().slice(0, 10);
  return monthGridFromFocusKey(focusKey).map(({ key, inMonth }) => {
    const dayTasks = allTasks.filter((task) => normalizeDateKey(task.due_at || task.updated_at || task.created_at) === key);
    const openTasks = dayTasks.filter((task) => task.status === "open");
    const doneTasks = dayTasks.filter((task) => task.status === "done");
    const dayLogs = allLogs.filter((log) => normalizeDateKey(log.created_at) === key);
    const topPlants = [...new Set([...openTasks.slice(0,2).map((t)=>t.plant_id), ...dayLogs.slice(0,2).map((l)=>l.plant_id)].filter(Boolean))];
    return {
      key,
      inMonth,
      dayNumber: dateFromKey(key).getDate(),
      openCount: openTasks.length,
      doneCount: doneTasks.length,
      logCount: dayLogs.length,
      plantIds: topPlants,
      topTasks: openTasks.sort((a,b)=> new Date(a.due_at||0).getTime()-new Date(b.due_at||0).getTime()).slice(0,2),
    };
  });
}

function navigateHomeMonth(delta) {
  homeMonthFocusKey = addMonthsToKey(homeMonthFocusKey || new Date().toISOString().slice(0, 10), delta);
}


function renderHomeSubscriptionCard() {
  const active = activePlantCount();
  const ended = summaryPlantCount('ended_plants', plants.filter((plant) => isPlantEnded(plant)).length);
  const archived = summaryPlantCount('archived_plants', plants.filter((plant) => isPlantArchived(plant)).length);
  const visible = summaryPlantCount('visible_plants', plants.filter((plant) => plantStatusValue(plant) !== "deleted").length);
  const limit = currentPlantLimit;
  const over = overflowPlantCount();
  const remaining = remainingPlantSlots();
  const limitText = formatPlantLimit(limit);
  const sourceLabel = currentPlanSource === 'account' ? 'Account' : 'Guest';
  const latestHistory = (subscriptionHistorySource === 'server' && subscriptionHistory.length ? subscriptionHistory : checkoutHistory)[0] || null;
  const latestEvent = latestHistory
    ? `${latestHistory.mode || latestHistory.eventKind || 'Abo'} · ${latestHistory.plan ? planLabel(latestHistory.plan) : '—'} · ${latestHistory.status || '—'}`
    : 'Noch kein Abo-Ereignis';
  const lifecycle = normalizedSubscriptionLifecycleStatus();
  const statusLabel = lifecycle === 'error'
    ? 'Problem'
    : lifecycle === 'pending'
      ? 'Checkout offen'
      : lifecycle === 'guest'
        ? 'Guest / Free'
        : 'Aktiv';
  const toneBorder = isOverPlantLimit()
    ? 'rgba(255,122,122,0.28)'
    : remaining === 0
      ? 'rgba(255,214,102,0.22)'
      : `rgba(${TC.accentRgb},0.18)`;
  const toneGlow = isOverPlantLimit()
    ? '0 0 0 1px rgba(255,122,122,0.08) inset'
    : remaining === 0
      ? '0 0 0 1px rgba(255,214,102,0.06) inset'
      : `0 0 0 1px rgba(${TC.accentRgb},0.05) inset`;
  const headline = isOverPlantLimit()
    ? t('home.slots.overlimit', over)
    : Number.isFinite(remaining)
      ? t('home.slots.remaining', remaining)
      : t('home.slots.unlimited');
  const subline = isOverPlantLimit()
    ? t('home.slots.sublineOver')
    : Number.isFinite(remaining)
      ? t('home.slots.sublineOk')
      : t('home.slots.sublineUnlimited');
  return `
    <div style="display:grid; gap:10px; padding:12px; border-radius:12px; background:${TC.surface}; border:1px solid ${toneBorder}; box-shadow:${toneGlow};">
      <div style="display:flex; justify-content:space-between; gap:12px; flex-wrap:wrap; align-items:flex-start;">
        <div>
          <div style="font-size:13px; color:${TC.textMuted};">${t('home.subscription.header')}</div>
          <div style="margin-top:4px; font-size:18px; font-weight:700; color:${TC.text};">${planLabel(currentPlan)} · Aktiv ${active} / ${limitText}</div>
          <div style="margin-top:4px; font-size:12px; color:${isOverPlantLimit() ? '#ff9b9b' : TC.textMuted};">${headline}</div>
        </div>
        <div style="display:flex; gap:8px; flex-wrap:wrap;">
          <span style="padding:6px 10px; border-radius:999px; background:rgba(${TC.accentRgb},0.10); color:${TC.accent}; font-size:11px;">${t('home.subscription.source', sourceLabel)}</span>
          <span style="padding:6px 10px; border-radius:999px; background:rgba(255,255,255,0.06); color:#dcefe8; font-size:11px;">${getPlanConfig().find((entry) => entry.key === currentPlan)?.price || t('home.subscription.defaultPrice')}</span>
        </div>
      </div>
      <div style="font-size:12px; color:#9fb8ad; line-height:1.5;">${subline}</div>
      <div style="display:grid; grid-template-columns:repeat(auto-fit, minmax(160px, 1fr)); gap:8px;">
        <div style="padding:10px; border-radius:10px; background:${TC.cardSolid}; border:1px solid rgba(${TC.accentRgb},0.10);">
          <div style="font-size:11px; color:${TC.textMuted};">${t('home.subscription.activePlan')}</div>
          <div style="margin-top:4px; font-size:15px; color:${TC.text}; font-weight:700;">${planLabel(currentPlan)}</div>
        </div>
        <div style="padding:10px; border-radius:10px; background:${TC.cardSolid}; border:1px solid rgba(${TC.accentRgb},0.10);">
          <div style="font-size:11px; color:${TC.textMuted};">${t('home.subscription.slots')}</div>
          <div style="margin-top:4px; font-size:15px; color:${TC.text}; font-weight:700;">${active} / ${limitText}</div>
        </div>
        <div style="padding:10px; border-radius:10px; background:${TC.cardSolid}; border:1px solid rgba(${TC.accentRgb},0.10);">
          <div style="font-size:11px; color:${TC.textMuted};">${t('home.subscription.checkoutStatus')}</div>
          <div style="margin-top:4px; font-size:15px; color:${TC.text}; font-weight:700;">${statusLabel}</div>
        </div>
        <div style="padding:10px; border-radius:10px; background:${TC.cardSolid}; border:1px solid rgba(${TC.accentRgb},0.10);">
          <div style="font-size:11px; color:${TC.textMuted};">${t('home.subscription.statusMix')}</div>
          <div style="margin-top:4px; font-size:13px; color:#dcefe8; font-weight:600;">${t('home.subscription.statusMixValue', ended, archived, visible)}</div>
        </div>
        <div style="padding:10px; border-radius:10px; background:${TC.cardSolid}; border:1px solid rgba(${TC.accentRgb},0.10);">
          <div style="font-size:11px; color:${TC.textMuted};">${t('home.subscription.lastEvent')}</div>
          <div style="margin-top:4px; font-size:13px; color:#dcefe8; font-weight:600;">${latestEvent}</div>
        </div>
      </div>
    </div>`;
}


function computePlantDashboardSummary(plant) {
  const plantTasks = tasks.filter((t) => t.plant_id === plant.plant_id);
  const openTasks = plantTasks.filter((t) => t.status === "open");
  const nextOpenTask = [...openTasks]
    .sort((a, b) => {
      const aTs = a.due_at ? new Date(a.due_at).getTime() : Number.MAX_SAFE_INTEGER;
      const bTs = b.due_at ? new Date(b.due_at).getTime() : Number.MAX_SAFE_INTEGER;
      return aTs - bTs;
    })[0] || null;

  const reminderCandidates = reminderCandidatesForPlant(plant.plant_id);
  const nextReminder = reminderCandidates[0] || null;

  const plantLogs = logs
    .filter((l) => l.plant_id === plant.plant_id)
    .sort((a, b) => new Date(b.created_at || 0).getTime() - new Date(a.created_at || 0).getTime());
  const lastLog = plantLogs[0] || null;

  const focusWeekStart = startOfWeekFromKey(timelineFocusKey);
  const focusWeekEnd = new Date(focusWeekStart);
  focusWeekEnd.setDate(focusWeekStart.getDate() + 6);
  const weekTasks = plantTasks.filter((task) => {
    if (!task.due_at) return false;
    const due = new Date(task.due_at);
    return !Number.isNaN(due.getTime()) && due >= focusWeekStart && due <= focusWeekEnd;
  });
  const weekDone = weekTasks.filter((task) => task.status === "done").length;
  const weekTotal = weekTasks.length;
  const weekProgressLabel = weekTotal ? `${weekDone}/${weekTotal} erledigt` : 'Noch keine Wochen-Tasks';
  const weekProgressPercent = weekTotal ? Math.max(6, Math.round((weekDone / weekTotal) * 100)) : 0;

  return {
    nextOpenTask,
    nextReminder,
    lastLog,
    phaseText: `${phaseLabel(plant.phase)} · Woche ${plant.phase_week ?? '-'}`,
    weekProgressLabel,
    weekProgressPercent,
  };
}


function normalizeDateKey(value) {
  if (!value) return null;
  const d = new Date(value);
  if (Number.isNaN(d.getTime())) return null;
  return d.toISOString().slice(0, 10);
}

function dateLabelFromKey(key) {
  if (!key) return t('timeline.noDate');
  const d = new Date(`${key}T00:00:00`);
  return d.toLocaleDateString(localeTag(), { weekday: "short", day: "2-digit", month: "2-digit", year: "numeric" });
}

function dateFromKey(key) {
  return new Date(`${key}T00:00:00`);
}

function startOfWeekFromKey(key) {
  const d = dateFromKey(key);
  const day = d.getDay();
  const diff = day === 0 ? -6 : 1 - day;
  d.setDate(d.getDate() + diff);
  return d;
}

function startOfMonthFromKey(key) {
  const d = dateFromKey(key);
  d.setDate(1);
  return d;
}

function endOfMonthFromKey(key) {
  const d = startOfMonthFromKey(key);
  d.setMonth(d.getMonth() + 1);
  d.setDate(0);
  return d;
}

function addDaysToKey(key, days) {
  const d = dateFromKey(key);
  d.setDate(d.getDate() + days);
  return d.toISOString().slice(0, 10);
}

function addMonthsToKey(key, months) {
  const d = dateFromKey(key);
  d.setMonth(d.getMonth() + months);
  return d.toISOString().slice(0, 10);
}

function weekRangeLabelFromKey(key) {
  const start = startOfWeekFromKey(key);
  const end = new Date(start);
  end.setDate(start.getDate() + 6);
  const startLabel = start.toLocaleDateString(localeTag(), { day: "2-digit", month: "2-digit" });
  const endLabel = end.toLocaleDateString(localeTag(), { day: "2-digit", month: "2-digit", year: "numeric" });
  return `${startLabel} – ${endLabel}`;
}

function monthRangeLabelFromKey(key) {
  return startOfMonthFromKey(key).toLocaleDateString(localeTag(), { month: "long", year: "numeric" });
}

function weekKeysFromFocusKey(key) {
  const start = startOfWeekFromKey(key);
  return Array.from({ length: 7 }, (_, index) => {
    const current = new Date(start);
    current.setDate(start.getDate() + index);
    return current.toISOString().slice(0, 10);
  });
}

function monthGridFromFocusKey(key) {
  const start = startOfMonthFromKey(key);
  const gridStart = startOfWeekFromKey(start.toISOString().slice(0, 10));
  return Array.from({ length: 42 }, (_, index) => {
    const current = new Date(gridStart);
    current.setDate(gridStart.getDate() + index);
    const currentKey = current.toISOString().slice(0, 10);
    const inMonth = current.getMonth() === start.getMonth();
    return { key: currentKey, inMonth };
  });
}


/**
 * Expand recurring tasks into virtual occurrences for calendar display.
 * Computed once per render cycle via refreshExpandedTasks().
 */
let _allTasksExpanded = [];
let _selectedTasksExpanded = [];

// Debounce cache: task_id -> timestamp of last Done click.
// Prevents rapid double-clicks (UI freezes, user clicks multiple times) from creating duplicate logs/updates.
const _recentTaskDone = new Map();
const TASK_DONE_DEBOUNCE_MS = 5000;

function guardTaskDone(taskId) {
  const now = Date.now();
  const last = _recentTaskDone.get(taskId);
  if (last && now - last < TASK_DONE_DEBOUNCE_MS) return false;
  _recentTaskDone.set(taskId, now);
  // Cleanup old entries
  for (const [key, ts] of _recentTaskDone) {
    if (now - ts > TASK_DONE_DEBOUNCE_MS * 10) _recentTaskDone.delete(key);
  }
  return true;
}

// Check whether a "Gießen erledigt (Task: X)" log was recently created for the same plant+task.
// Prevents duplicate auto-watering logs when Done is clicked multiple times.
function hasRecentWateringLog(plantId, taskTitle) {
  if (!plantId || !Array.isArray(allLogs)) return false;
  const cutoff = Date.now() - 60 * 1000; // 1 minute
  const expectedText = t('log.waterDoneAuto', taskTitle);
  return allLogs.some(log => {
    if (log.plant_id !== plantId) return false;
    if (log.text !== expectedText) return false;
    const ts = new Date(log.created_at).getTime();
    return Number.isFinite(ts) && ts >= cutoff;
  });
}

function expandRecurringTasks(taskList) {
  const expanded = [];
  // Only project virtual occurrences into today or the future. Past occurrences
  // of a recurring task were either done (advanced away) or skipped; projecting
  // them would re-raise them as "overdue" forever.
  const todayStart = new Date();
  todayStart.setHours(0, 0, 0, 0);
  const todayMs = todayStart.getTime();
  for (const task of taskList) {
    expanded.push(task);
    if (task.repeat_interval_hours && task.repeat_interval_hours > 0 && task.due_at) {
      const intervalMs = task.repeat_interval_hours * 3600 * 1000;
      const start = new Date(task.due_at).getTime();
      if (isNaN(start)) continue;
      const until = task.repeat_until ? new Date(task.repeat_until).getTime() : (start + 28 * 24 * 3600 * 1000);
      if (isNaN(until)) continue;
      const maxOccurrences = 500;
      let count = 0;
      let next = start + intervalMs;
      while (next <= until && count < maxOccurrences) {
        if (next >= todayMs) {
          expanded.push({
            ...task,
            due_at: new Date(next).toISOString(),
            _virtual: true,
            _occurrence: count + 1,
          });
        }
        next += intervalMs;
        count++;
      }
    }
  }
  return expanded;
}

function refreshExpandedTasks() {
  _allTasksExpanded = expandRecurringTasks(allTasks);
  _selectedTasksExpanded = selectedPlantId
    ? expandRecurringTasks(tasks.filter(t => t.plant_id === selectedPlantId))
    : _allTasksExpanded;
}

function dayStats(dayKey) {
  const srcTasks = selectedPlantId ? _selectedTasksExpanded : _allTasksExpanded;
  const srcLogs = selectedPlantId ? logs : allLogs;
  const dayTasks = srcTasks.filter((task) => normalizeDateKey(task.due_at || task.updated_at || task.created_at) === dayKey);
  const dayLogs = srcLogs.filter((log) => normalizeDateKey(log.created_at) === dayKey);
  const open = dayTasks.filter((task) => task.status === "open").length;
  const done = dayTasks.filter((task) => task.status === "done").length;
  const skipped = dayTasks.filter((task) => task.status === "skipped").length;
  const overdue = dayTasks.filter((task) => task.status === "open" && task.due_at && new Date(task.due_at).getTime() < Date.now()).length;
  const harvests = dayLogs.filter((log) => log.metrics?.harvest_kg || log.metrics?.harvest_count).length;
  const waterings = dayLogs.filter((log) => log.metrics?.water_liters).length;
  return {
    totalTasks: dayTasks.length,
    open,
    done,
    skipped,
    overdue,
    logs: dayLogs.length,
    harvests,
    waterings,
  };
}

function daySignalTone(stats) {
  if (stats.overdue > 0) {
    return {
      border: '#ff7a7a',
      accent: '#ff9b9b',
      glow: '0 0 0 1px rgba(255,122,122,0.14), 0 0 18px rgba(255,122,122,0.08)',
      label: t('day.signal.overdue'),
    };
  }
  if (stats.open > 0) {
    return {
      border: TC.accent,
      accent: TC.accent,
      glow: `0 0 0 1px rgba(${TC.accentRgb},0.12), 0 0 18px rgba(${TC.accentRgb},0.06)`,
      label: t('day.signal.open'),
    };
  }
  if (stats.done > 0 || stats.logs > 0) {
    return {
      border: TC.textMuted,
      accent: TC.textMuted,
      glow: `0 0 0 1px rgba(${TC.accentRgb},0.10)`,
      label: t('day.signal.active'),
    };
  }
  return {
    border: `rgba(${TC.accentRgb},0.10)`,
    accent: TC.textMuted,
    glow: 'none',
    label: t('day.signal.empty'),
  };
}


function reminderPermissionLabel() {
  if (reminderPermissionState === "granted") return t('reminder.perm.granted');
  if (reminderPermissionState === "denied") return t('reminder.perm.denied');
  if (reminderPermissionState === "unsupported") return t('reminder.perm.unsupported');
  return t('reminder.perm.prompt');
}

async function requestReminderPermission() {
  if (typeof Notification === "undefined") {
    output.textContent = t('reminder.runtimeUnsupported');
    reminderPermissionState = "unsupported";
    renderReminderPanel();
    return;
  }
  reminderPermissionState = await Notification.requestPermission();
  renderReminderPanel();
}

let autoReminderCheckInFlight = false;

async function checkDueNotifications(options = {}) {
  const { silent = false, withinMinutes = 30, reason = "manual" } = options;
  try {
    const result = await invoke("list_due_notification_candidates", { withinMinutes });
    const reminders = result?.reminders ?? [];
    if (!reminders.length) {
      if (!silent) output.textContent = t('reminder.none');
      renderReminderPanel();
      return { sent: 0, reason };
    }
    if (typeof Notification === "undefined") {
      if (!silent) output.textContent = t('reminder.noApi', reminders.length);
      renderReminderPanel();
      return { sent: 0, reason, unsupported: true };
    }
    if (Notification.permission !== "granted") {
      reminderPermissionState = Notification.permission;
      if (!silent) output.textContent = t('reminder.notAllowed', reminders.length);
      renderReminderPanel();
      return { sent: 0, reason, permission: Notification.permission };
    }
    for (const reminder of reminders) {
      const dueLabel = formatDate(reminder.due_at);
      const body = reminder.overdue
        ? t('reminder.body.overdue', reminder.title, dueLabel)
        : t('reminder.body.due', reminder.title, dueLabel);
      new Notification(t('reminder.notificationTitle'), { body });
      await invoke("mark_notification_sent", { payload: { task_id: reminder.task_id, due_at: reminder.due_at } });
    }
    if (!silent) output.textContent = t('reminder.marked', reminders.length);
    renderReminderPanel();
    await refreshStatus();
    return { sent: reminders.length, reason };
  } catch (error) {
    if (!silent) output.textContent = String(error);
    return { sent: 0, reason, error: String(error) };
  }
}

async function autoCheckDueNotifications(reason = "auto") {
  if (autoReminderCheckInFlight) return { skipped: true, reason };
  if (typeof Notification === "undefined") {
    reminderPermissionState = "unsupported";
    renderReminderPanel();
    return { skipped: true, reason, unsupported: true };
  }
  reminderPermissionState = Notification.permission;
  renderReminderPanel();
  if (Notification.permission !== "granted") return { skipped: true, reason, permission: Notification.permission };
  autoReminderCheckInFlight = true;
  try {
    return await checkDueNotifications({ silent: true, withinMinutes: 30, reason });
  } finally {
    autoReminderCheckInFlight = false;
  }
}

function notificationStateBadge(enabled) {
  return enabled
    ? `<span style="padding:4px 8px; border-radius:999px; background:rgba(${TC.accentRgb},0.14); color:${TC.accent}; font-size:11px;">${t('reminder.pill.on')}</span>`
    : `<span style="padding:4px 8px; border-radius:999px; background:rgba(${TC.accentRgb},0.10); color:${TC.textMuted}; font-size:11px;">${t('reminder.pill.off')}</span>`;
}

function reminderCandidatesForPlant(plantId) {
  const now = Date.now();
  return tasks
    .filter((task) => task.plant_id === plantId && task.notification_enabled && task.status === 'open')
    .map((task) => {
      const dueTs = task.due_at ? new Date(task.due_at).getTime() : null;
      const deltaHours = dueTs == null ? null : Math.round((dueTs - now) / 3600000);
      let bucket = 'later';
      if (dueTs != null && dueTs < now) bucket = 'overdue';
      else if (deltaHours != null && deltaHours <= 6) bucket = 'now';
      else if (deltaHours != null && deltaHours <= 24) bucket = 'today';
      else if (deltaHours != null && deltaHours <= 48) bucket = 'soon';
      return { task, dueTs, deltaHours, bucket };
    })
    .sort((a, b) => (a.dueTs ?? Number.MAX_SAFE_INTEGER) - (b.dueTs ?? Number.MAX_SAFE_INTEGER));
}

function renderReminderPanel() {
  const plant = selectedPlant();
  if (!plant) {
    reminderPanel.innerHTML = `
      <div style="display:grid; gap:10px; color:${TC.textMuted};">
        <h2 style="margin:0; color:${TC.accent}; font-size:18px;">${t('detail.reminderTitle')}</h2>
        <div>${t('detail.reminderSelectFirst')}</div>
      </div>`;
    return;
  }

  const candidates = reminderCandidatesForPlant(plant.plant_id);
  const overdue = candidates.filter((item) => item.bucket === 'overdue').length;
  const dueSoon = candidates.filter((item) => item.bucket === 'now' || item.bucket === 'today').length;
  const later = candidates.filter((item) => item.bucket === 'soon' || item.bucket === 'later').length;

  reminderPanel.innerHTML = `
    <div style="display:grid; gap:14px;">
      <div style="display:flex; justify-content:space-between; gap:16px; align-items:flex-start; flex-wrap:wrap;">
        <div>
          <h2 style="margin:0; color:${TC.accent}; font-size:18px;">${t('detail.reminderTitle')}</h2>
          <div style="margin-top:4px; color:${TC.textMuted}; font-size:13px;">${t('reminder.panelHint')}</div>
        </div>
        <div style="display:flex; gap:8px; flex-wrap:wrap; align-items:center; justify-content:flex-end;">
          <span style="padding:6px 10px; border-radius:999px; background:rgba(${TC.accentRgb},0.10); color:${TC.textMuted}; font-size:12px;">${reminderPermissionLabel()}</span>
          <div>${notificationStateBadge(candidates.length > 0)}</div>
        </div>
      </div>
      <div style="display:flex; gap:8px; flex-wrap:wrap;">
        <button id="allowReminderNotifications" type="button">${t('detail.reminderAllow')}</button>
        <button id="checkReminderNotifications" type="button">${t('detail.reminderCheck')}</button>
      </div>

      <div style="display:grid; grid-template-columns:repeat(auto-fit, minmax(140px, 1fr)); gap:10px;">
        <div style="padding:12px; border-radius:12px; background:${TC.surface}; border:1px solid rgba(255,122,122,0.16);">
          <div style="font-size:12px; color:${TC.textMuted};">${t('detail.overdue')}</div>
          <div style="margin-top:6px; font-weight:700; color:#ff9b9b;">${overdue}</div>
        </div>
        <div style="padding:12px; border-radius:12px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.16);">
          <div style="font-size:12px; color:${TC.textMuted};">${t('detail.until24h')}</div>
          <div style="margin-top:6px; font-weight:700; color:${TC.accent};">${dueSoon}</div>
        </div>
        <div style="padding:12px; border-radius:12px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.16);">
          <div style="font-size:12px; color:${TC.textMuted};">${t('detail.later')}</div>
          <div style="margin-top:6px; font-weight:700; color:${TC.textMuted};">${later}</div>
        </div>
      </div>

      <div style="display:grid; gap:10px;">
        ${candidates.length ? candidates.slice(0, 6).map(({ task, bucket, deltaHours }) => `
          <div style="padding:12px; border-radius:12px; background:${TC.surface}; border:1px solid ${bucket === 'overdue' ? 'rgba(255,122,122,0.18)' : bucket === 'now' || bucket === 'today' ? `rgba(${TC.accentRgb},0.18)` : `rgba(${TC.accentRgb},0.12)`};">
            <div style="display:flex; justify-content:space-between; gap:10px; align-items:flex-start;">
              <div>
                <div style="font-weight:700;">${escapeHtml(task.title)}</div>
                <div style="margin-top:4px; font-size:12px; color:${TC.textMuted};">${t('reminder.row.due', escapeHtml(task.category), formatDate(task.due_at))}</div>
              </div>
              <div style="display:flex; gap:6px; flex-wrap:wrap; justify-content:flex-end;">
                ${notificationStateBadge(task.notification_enabled)}
                <span style="padding:4px 8px; border-radius:999px; background:${bucket === 'overdue' ? 'rgba(255,122,122,0.14)' : bucket === 'now' || bucket === 'today' ? `rgba(${TC.accentRgb},0.14)` : `rgba(${TC.accentRgb},0.12)`}; color:${bucket === 'overdue' ? '#ff9b9b' : bucket === 'now' || bucket === 'today' ? TC.accent : TC.textMuted}; font-size:11px;">${t('reminder.bucket.' + bucket)}${deltaHours != null ? ` · ${deltaHours}h` : ''}</span>
              </div>
            </div>
          </div>`).join('') : `<div style="padding:12px; border-radius:12px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.10); color:${TC.textMuted};">${t('detail.noOpenReminders')}</div>`}
      </div>
    </div>`;

  $("allowReminderNotifications")?.addEventListener("click", requestReminderPermission);
  $("checkReminderNotifications")?.addEventListener("click", checkDueNotifications);
}

function daySignalMarkup(stats, compact = false) {
  const pills = [];
  if (stats.overdue > 0) pills.push(`<span style="padding:3px 7px; border-radius:999px; background:rgba(255,122,122,0.14); color:#ff9b9b; font-size:${compact ? '10px' : '11px'};">${stats.overdue} overdue</span>`);
  if (stats.open > 0) pills.push(`<span style="padding:3px 7px; border-radius:999px; background:rgba(${TC.accentRgb},0.14); color:${TC.accent}; font-size:${compact ? '10px' : '11px'};">${stats.open} offen</span>`);
  if (stats.done > 0) pills.push(`<span style="padding:3px 7px; border-radius:999px; background:rgba(${TC.accentRgb},0.14); color:${TC.textMuted}; font-size:${compact ? '10px' : '11px'};">${stats.done} done</span>`);
  if (stats.skipped > 0) pills.push(`<span style="padding:3px 7px; border-radius:999px; background:rgba(255,214,102,0.12); color:#ffd666; font-size:${compact ? '10px' : '11px'};">${stats.skipped} skipped</span>`);
  if (stats.logs > 0) pills.push(`<span style="padding:3px 7px; border-radius:999px; background:rgba(111,155,255,0.12); color:#9fc2ff; font-size:${compact ? '10px' : '11px'};">${stats.logs} logs</span>`);
  return pills.join('');
}

function itemMatchesTimelineWindow(item, mode, focusKey) {
  if (!item.dateKey) return false;
  if (mode === "day") return item.dateKey === focusKey;
  if (mode === "week") {
    const start = startOfWeekFromKey(focusKey);
    const end = new Date(start);
    end.setDate(start.getDate() + 6);
    const current = dateFromKey(item.dateKey);
    return current >= start && current <= end;
  }
  const monthStart = startOfMonthFromKey(focusKey);
  const monthEnd = endOfMonthFromKey(focusKey);
  const current = dateFromKey(item.dateKey);
  return current >= monthStart && current <= monthEnd;
}

function setTimelineFocusDay(dayKey) {
  if (!dayKey) return;
  timelineFocusKey = dayKey;
  renderTimelinePanel();
  applyTimelineDueToTaskForm();
}


function renderCalendarToolbar() {
  const toolbar = document.getElementById('calendarToolbar');
  if (!toolbar) return;
  const focusDate = dateFromKey(timelineFocusKey);
  let periodLabel = '';
  if (timelineMode === 'year') periodLabel = `${focusDate.getFullYear()}`;
  else if (timelineMode === 'month') periodLabel = `${getMonthName(focusDate.getMonth() + 1)} ${focusDate.getFullYear()}`;
  else if (timelineMode === 'week') periodLabel = `${t('cal.kw')} ${getISOWeek(focusDate)} \u00b7 ${focusDate.getFullYear()}`;
  else periodLabel = formatDate(focusDate);

  toolbar.innerHTML = `
    <div class="cal-toolbar-group">
      <button class="cal-toolbar-btn ${timelineMode==='day'?'cal-toolbar-btn--active':''}" id="timelineModeDay">${t('cal.day')}</button>
      <button class="cal-toolbar-btn ${timelineMode==='week'?'cal-toolbar-btn--active':''}" id="timelineModeWeek">${t('cal.week')}</button>
      <button class="cal-toolbar-btn ${timelineMode==='month'?'cal-toolbar-btn--active':''}" id="timelineModeMonth">${t('cal.month')}</button>
      <button class="cal-toolbar-btn ${timelineMode==='year'?'cal-toolbar-btn--active':''}" id="timelineModeYear">${t('cal.year')}</button>
    </div>
    <button class="cal-toolbar-btn" id="timelinePrev">&larr;</button>
    <span class="cal-period-label">${periodLabel}</span>
    <button class="cal-toolbar-btn" id="timelineNext">&rarr;</button>
    <button class="cal-toolbar-btn" id="timelineToday">${t('cal.today')}</button>
    ${weatherData?.current ? `<div style="display:flex;align-items:center;gap:6px;font-size:12px;color:${TC.textMuted};padding:0 8px;" title="${t('timeline.weatherTitle')}">
      <span>${weatherIcon(weatherData.current.weather_code)}</span>
      <span>${Math.round(weatherData.current.temperature_2m)}&deg;</span>
      <span style="color:${TC.textDim};">${weatherData.current.relative_humidity_2m}%</span>
    </div>` : ''}
    ${(() => { const pnw = plants.filter(p => isPlantActive(p) && calculateWateringNeed(p).urgency !== 'ok').length; return pnw > 0 ? '<span style="font-size:11px;color:' + TC.warning + ';padding:0 6px;" title="' + t('timeline.waterNeedTitle', pnw) + '">\u{1F4A7} ' + pnw + '</span>' : ''; })()}
    ${vacationMode && vacationMode.active ? '<span style="font-size:11px;color:' + TC.warning + ';padding:0 4px;" title="' + t('vacation.activeTitle') + '">\u{1F3D6}\uFE0F</span>' : ''}
    <div class="cal-toolbar-group" style="margin-left:auto;">
      <select id="calendarModeSelect" class="form-select" style="font-size:12px;padding:4px 8px;background:${TC.inputBg};color:${TC.text};border:1px solid rgba(${TC.accentRgb},0.15);border-radius:8px;font-family:inherit;cursor:pointer;">
        <option value="standard" ${calendarMode === 'standard' ? 'selected' : ''}>${t('calmode.standard')}</option>
        <option value="moon" ${calendarMode === 'moon' ? 'selected' : ''}>${t('calmode.moon')}</option>
        <option value="chinese" ${calendarMode === 'chinese' ? 'selected' : ''}>${t('calmode.chinese')}</option>
        <option value="persian" ${calendarMode === 'persian' ? 'selected' : ''}>${t('calmode.persian')}</option>
        <option value="julian" ${calendarMode === 'julian' ? 'selected' : ''}>${t('calmode.julian')}</option>
        <option value="islamic" ${calendarMode === 'islamic' ? 'selected' : ''}>${t('calmode.islamic')}</option>
        <option value="jewish" ${calendarMode === 'jewish' ? 'selected' : ''}>${t('calmode.jewish')}</option>
        <option value="bauern" ${calendarMode === 'bauern' ? 'selected' : ''}>${t('calmode.farmer')}</option>
      </select>
    </div>
  `;
  document.getElementById('calendarModeSelect')?.addEventListener('change', (e) => {
    calendarMode = e.target.value;
    localStorage.setItem('plants_calendar_mode', calendarMode);
    renderTimelinePanel();
  });
  document.getElementById('timelineModeDay')?.addEventListener('click', () => { timelineMode = 'day'; renderCalendarToolbar(); renderTimelinePanel(); });
  document.getElementById('timelineModeWeek')?.addEventListener('click', () => { timelineMode = 'week'; renderCalendarToolbar(); renderTimelinePanel(); });
  document.getElementById('timelineModeMonth')?.addEventListener('click', () => { timelineMode = 'month'; renderCalendarToolbar(); renderTimelinePanel(); });
  document.getElementById('timelineModeYear')?.addEventListener('click', () => { timelineMode = 'year'; renderCalendarToolbar(); renderTimelinePanel(); });
  document.getElementById('timelinePrev')?.addEventListener('click', () => { shiftTimelineFocus(-1); renderCalendarToolbar(); renderTimelinePanel(); });
  document.getElementById('timelineNext')?.addEventListener('click', () => { shiftTimelineFocus(1); renderCalendarToolbar(); renderTimelinePanel(); });
  document.getElementById('timelineToday')?.addEventListener('click', () => { timelineFocusKey = new Date().toISOString().slice(0, 10); renderCalendarToolbar(); renderTimelinePanel(); });
}

function getISOWeek(d) {
  const date = new Date(Date.UTC(d.getFullYear(), d.getMonth(), d.getDate()));
  date.setUTCDate(date.getUTCDate() + 4 - (date.getUTCDay() || 7));
  const yearStart = new Date(Date.UTC(date.getUTCFullYear(), 0, 1));
  return Math.ceil((((date - yearStart) / 86400000) + 1) / 7);
}

function shiftTimelineFocus(dir) {
  if (timelineMode === 'year') {
    timelineFocusKey = addMonthsToKey(timelineFocusKey, dir * 12);
  } else if (timelineMode === 'month') {
    timelineFocusKey = addMonthsToKey(timelineFocusKey, dir);
  } else if (timelineMode === 'week') {
    timelineFocusKey = addDaysToKey(timelineFocusKey, dir * 7);
  } else {
    timelineFocusKey = addDaysToKey(timelineFocusKey, dir);
  }
}

function dayPlantDots(dayKey) {
  const taskPlantIds = _allTasksExpanded
    .filter(t => normalizeDateKey(t.due_at) === dayKey)
    .map(t => t.plant_id);
  const logPlantIds = allLogs
    .filter(l => normalizeDateKey(l.created_at) === dayKey)
    .map(l => l.plant_id);
  const uniqueIds = [...new Set([...taskPlantIds, ...logPlantIds])];
  return uniqueIds.map(pid => {
    const plant = plants.find(p => p.plant_id === pid);
    return `<span class="cal-dot" style="background:${sanitizeColorTag(plant?.color_tag || TC.accent)};" title="${escapeHtml(plant?.name || '')}"></span>`;
  }).join('');
}

let focusedDayKey = null;

function showDayDetail(dayKey) {
  focusedDayKey = dayKey;
  const panel = document.getElementById('dayDetailPanel');
  if (!panel) return;

  const sourceTasks = selectedPlantId ? _selectedTasksExpanded : _allTasksExpanded;
  const sourceLogs = selectedPlantId ? allLogs.filter(l => l.plant_id === selectedPlantId) : allLogs;

  const dayTasks = sourceTasks.filter(t => normalizeDateKey(t.due_at) === dayKey);
  const dayLogs = sourceLogs.filter(l => normalizeDateKey(l.created_at) === dayKey);

  const dateLabel = formatDate(dateFromKey(dayKey));

  const dayMoonInfo = (calendarMode === 'moon' || calendarMode === 'chinese') ? getMoonPhase(dateFromKey(dayKey)) : null;
  const dayChineseInfo = calendarMode === 'chinese' ? getChineseDate(dateFromKey(dayKey)) : null;
  const dayPersianInfo = calendarMode === 'persian' ? getPersianDate(dateFromKey(dayKey)) : null;
  const dayJulianInfo = calendarMode === 'julian' ? getJulianDate(dateFromKey(dayKey)) : null;
  const dayIslamicInfo = calendarMode === 'islamic' ? getIslamicDate(dateFromKey(dayKey)) : null;
  const dayJewishInfo = calendarMode === 'jewish' ? getJewishDate(dateFromKey(dayKey)) : null;
  const dayBauernInfo = calendarMode === 'bauern' ? getBauernkalenderInfo(dateFromKey(dayKey)) : null;

  const dayDetailExtraHtml = (() => {
    if (dayPersianInfo) {
      return `<div style="padding:8px 12px;border-radius:10px;background:rgba(255,200,100,0.06);border:1px solid rgba(255,200,100,0.12);margin-bottom:8px;font-size:12px;">
        <div style="font-weight:600;color:${TC.text};">&#9728;&#65039; Persisch: ${dayPersianInfo.day} ${escapeHtml(dayPersianInfo.monthName)} ${dayPersianInfo.year}</div>
      </div>`;
    }
    if (dayJulianInfo) {
      return `<div style="padding:8px 12px;border-radius:10px;background:rgba(180,200,255,0.06);border:1px solid rgba(180,200,255,0.12);margin-bottom:8px;font-size:12px;">
        <div style="font-weight:600;color:${TC.text};">&#10013;&#65039; Julianisch: ${dayJulianInfo.day}. ${escapeHtml(dayJulianInfo.monthName)} ${dayJulianInfo.year}</div>
        <div style="color:${TC.textDim};margin-top:2px;">${t('cal.julian.diff', dayJulianInfo.correction)}</div>
      </div>`;
    }
    if (dayIslamicInfo) {
      return `<div style="padding:8px 12px;border-radius:10px;background:rgba(100,200,150,0.06);border:1px solid rgba(100,200,150,0.12);margin-bottom:8px;font-size:12px;">
        <div style="font-weight:600;color:${TC.text};">&#9770;&#65039; Islamisch: ${dayIslamicInfo.day} ${escapeHtml(dayIslamicInfo.monthName)} ${dayIslamicInfo.year} AH</div>
      </div>`;
    }
    if (dayJewishInfo) {
      return `<div style="padding:8px 12px;border-radius:10px;background:rgba(150,180,255,0.06);border:1px solid rgba(150,180,255,0.12);margin-bottom:8px;font-size:12px;">
        <div style="font-weight:600;color:${TC.text};">&#10017;&#65039; J\u00fcdisch: ${dayJewishInfo.day} ${escapeHtml(dayJewishInfo.monthName)} ${dayJewishInfo.year}</div>
      </div>`;
    }
    if (dayBauernInfo) {
      return `<div style="padding:8px 12px;border-radius:10px;background:rgba(200,180,100,0.06);border:1px solid rgba(200,180,100,0.12);margin-bottom:8px;font-size:12px;">
        <div style="font-weight:600;color:${TC.text};">&#127806; ${escapeHtml(dayBauernInfo.phenoSeason)}</div>
        ${dayBauernInfo.lostag ? `<div style="color:${TC.warning};margin-top:2px;font-weight:600;">${escapeHtml(dayBauernInfo.name)}</div>` : ''}
        <div style="color:${TC.textMuted};margin-top:4px;font-style:italic;">${escapeHtml(dayBauernInfo.spruch)}</div>
      </div>`;
    }
    return '';
  })();

  panel.innerHTML = `
    <div class="day-detail-content">
    <div class="day-detail-header">
      <span class="day-detail-title">${dateLabel}</span>
      <div class="day-detail-actions">
        <button class="btn btn--sm btn--primary" data-day-add-task="${dayKey}">+ Task</button>
        <button class="btn btn--sm" data-day-add-log="${dayKey}">+ Log</button>
        <button class="btn btn--sm" style="background:rgba(100,180,255,0.15);color:${TC.info};" data-day-add-water="${dayKey}">&#128167; Gie\u00dfen</button>
        <button class="btn btn--sm" style="background:rgba(255,200,100,0.15);color:${TC.warning};" data-day-add-harvest="${dayKey}">&#127806; Ernten</button>
        <button class="btn btn--sm btn--ghost" id="closeDayDetail">&times;</button>
      </div>
    </div>
    ${dayMoonInfo ? `
      <div style="display:flex;gap:12px;align-items:center;padding:8px 12px;border-radius:10px;background:rgba(200,180,255,0.06);border:1px solid rgba(200,180,255,0.12);margin-bottom:8px;font-size:12px;">
        <span style="font-size:20px;" title="${escapeHtml(dayMoonInfo.gardenTip)}">${dayMoonInfo.emoji}</span>
        <div style="flex:1;">
          <div style="font-weight:600;color:${TC.text};">${escapeHtml(dayMoonInfo.name)} &middot; ${escapeHtml(dayMoonInfo.zodiac)}</div>
          <div style="color:${TC.textMuted};margin-top:2px;">${escapeHtml(dayMoonInfo.gardenTip)}</div>
          <div style="color:#c8b4ff;margin-top:2px;">${escapeHtml(dayMoonInfo.element)} &middot; Beleuchtung: ${Math.round(dayMoonInfo.illumination * 100)}%</div>
        </div>
        ${dayChineseInfo ? `<div style="text-align:right;color:${TC.textDim};font-size:11px;">
          <div>${t('cal.lunarLabel', dayChineseInfo.month, dayChineseInfo.day)}</div>
          <div>${escapeHtml(dayChineseInfo.animal)} (${escapeHtml(dayChineseInfo.element)})</div>
        </div>` : ''}
      </div>
    ` : ''}
    ${dayDetailExtraHtml}
    ${dayTasks.length ? `
      <div class="day-detail-section">
        <div class="day-detail-section-title">Tasks (${dayTasks.length})</div>
        ${dayTasks.map(tsk => {
          const plant = plants.find(p => p.plant_id === tsk.plant_id);
          const isVirtual = tsk._virtual;
          const isFuture = new Date(tsk.due_at) > new Date(Date.now() + 24 * 3600000); // more than 1 day in future
          const repeatLabel = tsk.repeat_interval_hours ? ` · alle ${tsk.repeat_interval_hours}h` : '';
          const showDoneBtn = !isFuture; // Show Done for all non-future tasks (real or virtual)
          return `<div class="day-detail-item" ${isVirtual ? `style="opacity:0.85; border-style:dashed;"` : ''}>
            <span class="cal-dot" style="background:${sanitizeColorTag(plant?.color_tag || TC.accent)};"></span>
            <span style="flex:1;">${escapeHtml(tsk.title)}${repeatLabel}${isVirtual ? ` <span style="font-size:10px;color:${TC.textDim};">(geplant)</span>` : ''}</span>
            <span class="status-badge status-badge--${tsk.status === 'done' ? 'active' : tsk.status === 'open' ? 'ended' : 'archived'}">${escapeHtml(tsk.status)}</span>
            ${showDoneBtn ? `<button class="btn btn--sm" data-day-task-done="${tsk.task_id}">${t('task.doneBtn')}</button>` : ''}
            ${tsk.repeat_interval_hours && tsk.repeat_interval_hours > 0 ? `<button class="btn btn--sm" data-day-task-stop-repeat="${tsk.task_id}" title="${t('task.stopRepeatTitle')}">${t('task.stopRepeat')}</button>` : ''}
            <button class="btn btn--sm" data-day-task-delete="${tsk.task_id}" title="${t('task.deleteTitle')}" style="color:#d66;">🗑</button>
          </div>`;
        }).join('')}
      </div>
    ` : ''}
    ${dayLogs.length ? `
      <div class="day-detail-section">
        <div class="day-detail-section-title">Logs (${dayLogs.length})</div>
        ${dayLogs.map(l => {
          const plant = plants.find(p => p.plant_id === l.plant_id);
          return `<div class="day-detail-item">
            <span class="cal-dot" style="background:${sanitizeColorTag(plant?.color_tag || TC.accent)};"></span>
            <span style="flex:1;">${escapeHtml(l.text || l.log_type || 'Log')}</span>
            <span style="color:${TC.textDim};font-size:11px;">${escapeHtml(plant?.name || '')}</span>
            <button class="btn btn--sm" data-day-log-delete="${l.log_id}" title="${t('log.deleteTitle')}" style="color:#d66;">🗑</button>
          </div>`;
        }).join('')}
      </div>
    ` : ''}
    ${!dayTasks.length && !dayLogs.length ? `<div class="empty-state">${t('day.empty')}</div>` : ''}
    </div>
  `;
  panel.classList.add('day-detail--open');

  // Close on X button
  panel.querySelector('#closeDayDetail')?.addEventListener('click', closeDayDetail);
  // Close on click outside the content.
  // The panel element is persistent, so bind this exactly once — otherwise
  // every day-click and every refreshAll() stacks another listener on it.
  if (!panel.dataset.outsideClickBound) {
    panel.dataset.outsideClickBound = '1';
    panel.addEventListener('click', (e) => {
      if (e.target === panel) closeDayDetail();
    });
  }
  panel.querySelectorAll('[data-day-add-task]').forEach(btn => {
    btn.addEventListener('click', (e) => { e.stopPropagation(); openModal('task', { dueDate: dayKey }); });
  });
  panel.querySelectorAll('[data-day-add-log]').forEach(btn => {
    btn.addEventListener('click', (e) => { e.stopPropagation(); openModal('log', { createdAt: dayKey }); });
  });
  panel.querySelectorAll('[data-day-add-water]').forEach(btn => {
    btn.addEventListener('click', (e) => { e.stopPropagation(); openModal('log', { createdAt: dayKey, logType: 'watering' }); });
  });
  panel.querySelectorAll('[data-day-add-harvest]').forEach(btn => {
    btn.addEventListener('click', (e) => { e.stopPropagation(); openModal('log', { createdAt: dayKey, logType: 'harvest' }); });
  });
  panel.querySelectorAll('[data-day-task-delete]').forEach(btn => {
    btn.addEventListener('click', async (e) => {
      e.stopPropagation();
      if (btn.disabled) return;
      const taskId = btn.dataset.dayTaskDelete;
      const task = allTasks.find(t => t.task_id === taskId) || _allTasksExpanded.find(t => t.task_id === taskId);
      const label = task?.title || 'Task';
      if (!confirm(task?.repeat_interval_hours ? t('task.confirmDeleteRepeating', label) : t('task.confirmDelete', label))) return;
      btn.disabled = true;
      btn.textContent = '...';
      try {
        await invoke("delete_task_smart", { taskId });
        await refreshAll({ autoReminderReason: "task_action" });
        closeDayDetail();
      } catch (err) {
        console.error("Task löschen fehlgeschlagen:", err);
        alert(t('action.deleteError', (err?.message || err)));
        btn.textContent = '🗑';
        btn.disabled = false;
      }
    });
  });
  panel.querySelectorAll('[data-day-log-delete]').forEach(btn => {
    btn.addEventListener('click', async (e) => {
      e.stopPropagation();
      if (btn.disabled) return;
      const logId = btn.dataset.dayLogDelete;
      if (!confirm(t('log.confirmDelete'))) return;
      btn.disabled = true;
      btn.textContent = '...';
      try {
        await invoke("delete_log_smart", { logId });
        await refreshAll({ autoReminderReason: "log_action" });
        closeDayDetail();
      } catch (err) {
        console.error("Log löschen fehlgeschlagen:", err);
        alert(t('action.deleteError', (err?.message || err)));
        btn.textContent = '🗑';
        btn.disabled = false;
      }
    });
  });
  panel.querySelectorAll('[data-day-task-stop-repeat]').forEach(btn => {
    btn.addEventListener('click', async (e) => {
      e.stopPropagation();
      if (btn.disabled) return;
      const taskId = btn.dataset.dayTaskStopRepeat;
      const task = allTasks.find(t => t.task_id === taskId) || _allTasksExpanded.find(t => t.task_id === taskId);
      const label = task?.title || 'Task';
      if (!confirm(t('task.confirmDeleteRepeating', label))) return;
      btn.disabled = true;
      btn.textContent = '...';
      try {
        try { await invoke("update_task_smart", { payload: { task_id: taskId, repeat_interval_hours: 0, status: "done" } }); } catch (_) {}
        await invoke("delete_task_smart", { taskId });
        await refreshAll({ autoReminderReason: "task_action" });
        closeDayDetail();
      } catch (err) {
        console.error("Wiederholung stoppen fehlgeschlagen:", err);
        alert(t('action.deleteError', (err?.message || err)));
        btn.textContent = t('task.stopRepeat');
        btn.disabled = false;
      }
    });
  });
  panel.querySelectorAll('[data-day-task-done]').forEach(btn => {
    btn.addEventListener('click', async (e) => {
      e.stopPropagation();
      if (btn.disabled) return;
      btn.disabled = true;
      btn.textContent = '...';
      const taskId = btn.dataset.dayTaskDone;
      const task = allTasks.find(t => t.task_id === taskId) || _allTasksExpanded.find(t => t.task_id === taskId);
      try {
        if (!guardTaskDone(taskId)) {
          // Recently processed, skip duplicate click
          btn.textContent = 'Done';
          btn.disabled = false;
          return;
        }
        playNotificationSound('success');
        await invoke("update_task_status_smart", { payload: { task_id: taskId, status: "done" } });
        if (task && task.category === "water" && !hasRecentWateringLog(task.plant_id, task.title)) {
          try {
            await invoke("create_log_smart", { payload: {
              plant_id: task.plant_id,
              log_type: "watering",
              text: t('log.waterDoneAuto', task.title),
              water_liters: null, water_method: "giessen",
              ph: null, ec: null, temp_c: null, rh: null,
              harvest_kg: null, harvest_count: null, harvest_quality: null,
              created_at: new Date().toISOString(),
            }});
          } catch (_) {}
        }
        await refreshAll({ autoReminderReason: "task_action" });
      } catch (err) {
        logOut("Task-Aktion fehlgeschlagen: " + err);
        btn.textContent = 'Done';
        btn.disabled = false;
      }
    });
  });
}

function closeDayDetail() {
  focusedDayKey = null;
  const panel = document.getElementById('dayDetailPanel');
  if (panel) panel.classList.remove('day-detail--open');
}

// Generic popup showing filtered tasks/logs for a plant stat card
function openPlantStatPopup(plant, kind) {
  if (!plant) return;
  const pid = plant.plant_id;
  let title = '';
  let items = [];
  let itemType = 'task'; // 'task' | 'log'
  if (kind === 'openTasks') {
    title = t('popup.title.openTasks', plant.name);
    items = allTasks.filter(t => t.plant_id === pid && t.status === 'open');
  } else if (kind === 'allTasks') {
    title = t('popup.title.allTasks', plant.name);
    items = allTasks.filter(t => t.plant_id === pid);
  } else if (kind === 'reminderEnabled') {
    title = t('popup.title.reminderEnabled', plant.name);
    items = allTasks.filter(t => t.plant_id === pid && t.notification_enabled);
  } else if (kind === 'reminderOpen') {
    title = t('popup.title.reminderOpen', plant.name);
    items = allTasks.filter(t => t.plant_id === pid && t.notification_enabled && t.status === 'open');
  } else if (kind === 'allLogs') {
    title = t('popup.title.allLogs', plant.name);
    items = allLogs.filter(l => l.plant_id === pid);
    itemType = 'log';
  } else {
    return;
  }

  // Sort: tasks by due_at asc, logs by created_at desc
  if (itemType === 'task') {
    items.sort((a, b) => new Date(a.due_at || 0) - new Date(b.due_at || 0));
  } else {
    items.sort((a, b) => new Date(b.created_at || 0) - new Date(a.created_at || 0));
  }

  // Build popup HTML
  let overlay = document.getElementById('plantStatPopup');
  if (!overlay) {
    overlay = document.createElement('div');
    overlay.id = 'plantStatPopup';
    overlay.style.cssText = 'position:fixed; inset:0; background:rgba(0,0,0,0.6); display:flex; align-items:center; justify-content:center; z-index:9999; padding:20px;';
    document.body.appendChild(overlay);
  }

  const rowsHtml = items.length ? items.map(it => {
    if (itemType === 'task') {
      const statusColor = it.status === 'open' ? TC.warning : it.status === 'done' ? TC.accent : TC.textMuted;
      const repeatLbl = it.repeat_interval_hours ? ` · alle ${it.repeat_interval_hours}h` : '';
      return `<div style="padding:10px; border-radius:8px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.1); display:flex; gap:8px; align-items:center;">
        <div style="flex:1;">
          <div style="font-weight:600; color:${TC.text};">${escapeHtml(it.title)}</div>
          <div style="font-size:12px; color:${TC.textMuted};">${escapeHtml(it.category || '')} · ${t('task.dueOn', formatDate(it.due_at))}${repeatLbl}</div>
        </div>
        <span style="padding:3px 8px; border-radius:10px; font-size:11px; background:rgba(${TC.accentRgb},0.1); color:${statusColor};">${escapeHtml(it.status)}</span>
        ${it.status === 'open' ? `<button class="btn btn--sm" data-popup-done="${it.task_id}">${t('task.doneBtn')}</button>` : ''}
        <button class="btn btn--sm" data-popup-del-task="${it.task_id}" title="${t('action.deleteTitle')}" style="color:#d66;">🗑</button>
      </div>`;
    } else {
      const metricsLbl = it.metrics?.water_liters ? `💧 ${it.metrics.water_liters}L ` : '';
      const harvestLbl = it.metrics?.harvest_kg ? `🍅 ${it.metrics.harvest_kg}kg ` : (it.metrics?.harvest_count ? `🍅 ${t('log.unitsCount', it.metrics.harvest_count)} ` : '');
      return `<div style="padding:10px; border-radius:8px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.1); display:flex; gap:8px; align-items:center;">
        <div style="flex:1;">
          <div style="font-weight:600; color:${TC.text};">${escapeHtml(it.log_type || 'Log')} ${metricsLbl}${harvestLbl}</div>
          <div style="font-size:12px; color:${TC.textMuted};">${formatDate(it.created_at)}${it.text ? ' · ' + escapeHtml(it.text.slice(0, 80)) : ''}</div>
        </div>
        <button class="btn btn--sm" data-popup-del-log="${it.log_id}" title="${t('action.deleteTitle')}" style="color:#d66;">🗑</button>
      </div>`;
    }
  }).join('') : `<div style="color:${TC.textMuted}; padding:20px; text-align:center;">${t('popup.empty')}</div>`;

  overlay.innerHTML = `
    <div style="background:${TC.cardSolid}; border:1px solid rgba(${TC.accentRgb},0.2); border-radius:14px; padding:20px; max-width:700px; width:100%; max-height:80vh; display:flex; flex-direction:column; gap:12px;">
      <div style="display:flex; justify-content:space-between; align-items:center; gap:12px;">
        <h3 style="margin:0; color:${TC.accent};">${escapeHtml(title)}</h3>
        <button id="plantStatPopupClose">✕</button>
      </div>
      <div style="font-size:13px; color:${TC.textMuted};">${t('popup.countEntries', items.length)}</div>
      <div style="overflow-y:auto; display:flex; flex-direction:column; gap:6px; padding-right:6px;">
        ${rowsHtml}
      </div>
    </div>
  `;

  const close = () => overlay.remove();
  overlay.addEventListener('click', (e) => { if (e.target === overlay) close(); });
  overlay.querySelector('#plantStatPopupClose')?.addEventListener('click', close);

  // Wire actions
  overlay.querySelectorAll('[data-popup-done]').forEach(b => {
    b.addEventListener('click', async () => {
      const tid = b.dataset.popupDone;
      if (!guardTaskDone(tid)) return;
      b.disabled = true;
      try {
        await invoke("update_task_status_smart", { payload: { task_id: tid, status: "done" } });
        await refreshAll({ autoReminderReason: "task_action" });
        close();
        openPlantStatPopup(plant, kind); // refresh popup
      } catch (err) { alert(t('action.genericError', (err?.message || err))); b.disabled = false; }
    });
  });
  overlay.querySelectorAll('[data-popup-del-task]').forEach(b => {
    b.addEventListener('click', async () => {
      const tid = b.dataset.popupDelTask;
      if (!confirm(t('task.confirmDelete', ''))) return;
      b.disabled = true;
      try {
        await invoke("delete_task_smart", { taskId: tid });
        await refreshAll({ autoReminderReason: "task_action" });
        close();
        openPlantStatPopup(plant, kind);
      } catch (err) { alert(t('action.genericError', (err?.message || err))); b.disabled = false; }
    });
  });
  overlay.querySelectorAll('[data-popup-del-log]').forEach(b => {
    b.addEventListener('click', async () => {
      const lid = b.dataset.popupDelLog;
      if (!confirm(t('log.confirmDelete'))) return;
      b.disabled = true;
      try {
        await invoke("delete_log_smart", { logId: lid });
        await refreshAll({ autoReminderReason: "log_action" });
        close();
        openPlantStatPopup(plant, kind);
      } catch (err) { alert(t('action.genericError', (err?.message || err))); b.disabled = false; }
    });
  });
}

// FAB
let fabOpen = false;

function toggleFab() {
  fabOpen = !fabOpen;
  document.getElementById('fabMain')?.classList.toggle('fab-main--open', fabOpen);
  document.getElementById('fabMenu')?.classList.toggle('fab-menu--open', fabOpen);
}

function closeFab() {
  fabOpen = false;
  document.getElementById('fabMain')?.classList.remove('fab-main--open');
  document.getElementById('fabMenu')?.classList.remove('fab-menu--open');
}

// Modal system
let activeModal = null;

function openModal(type, prefill = {}) {
  activeModal = type;
  closeFab();
  const titleEl = document.getElementById('modalTitle');
  const body = document.getElementById('modalBody');
  const footer = document.getElementById('modalFooter');

  if (type === 'plant') {
    titleEl.textContent = t('plant.new');
    body.innerHTML = renderPlantFormHTML(prefill);
    footer.innerHTML = `<button class="btn btn--ghost" id="modalCancel">${t('action.cancel')}</button><button class="btn btn--primary" id="modalSubmitPlant">${t('plant.create')}</button>`;
    document.getElementById('modalSubmitPlant')?.addEventListener('click', handleCreatePlant);
    document.getElementById('plantCatalogSelect')?.addEventListener('change', (e) => {
      const entry = catalogById(e.target.value);
      const preview = document.getElementById('catalogInfoPreview');
      if (!entry) {
        if (preview) preview.style.display = 'none';
        return;
      }
      const nameEl = $("plantName");
      const colorEl = $("plantColor");
      const phaseEl = $("plantPhase");
      const lifecycleEl = $("plantLifecycle");
      if (nameEl && !nameEl.value.trim()) nameEl.value = entry.name_de;
      if (colorEl) colorEl.value = entry.color_suggestion;
      if (phaseEl) phaseEl.value = entry.default_phase;
      if (lifecycleEl) lifecycleEl.value = entry.lifecycle;
      if (preview) {
        const monthNames = ['','Jan','Feb','Mrz','Apr','Mai','Jun','Jul','Aug','Sep','Okt','Nov','Dez'];
        const sunLabel = {full:'Volle Sonne',partial:'Halbschatten',shade:'Schatten'}[entry.sun]||entry.sun;
        const waterLabel = {low:'Wenig',medium:'Mittel',high:'Viel'}[entry.water]||entry.water;
        const sowIn = entry.sow_indoor_months.length ? entry.sow_indoor_months.map(m=>monthNames[m]).join(', ') : '-';
        const sowOut = entry.sow_outdoor_months.length ? entry.sow_outdoor_months.map(m=>monthNames[m]).join(', ') : '-';
        const harv = entry.harvest_months.length ? entry.harvest_months.map(m=>monthNames[m]).join(', ') : '-';
        const goodN = entry.good_neighbors.length ? entry.good_neighbors.map(id => { const c = catalogById(id); return c ? c.name_de : id; }).join(', ') : '-';
        const badN = entry.bad_neighbors.length ? entry.bad_neighbors.map(id => { const c = catalogById(id); return c ? c.name_de : id; }).join(', ') : '-';
        preview.style.display = '';
        preview.innerHTML = `
          <div style="margin-top:8px; padding:10px; border-radius:10px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.10); font-size:12px; color:${TC.textMuted};">
            <div style="color:${TC.textMuted}; margin-bottom:4px; font-weight:600;">${t('catalog.info', escapeHtml(entry.name_de))}</div>
            <div style="display:grid; grid-template-columns:1fr 1fr; gap:3px 12px;">
              <div>${t('catalog.sowIndoor')} ${sowIn}</div><div>${t('catalog.sowOutdoor')} ${sowOut}</div>
              <div>${t('catalog.harvest')} ${harv}</div><div>${t('catalog.spacing')} ${entry.spacing_cm} cm</div>
              <div>${t('catalog.sun')} ${sunLabel}</div><div>${t('catalog.water')} ${waterLabel}</div>
            </div>
            ${entry.tips_de ? `<div style="margin-top:4px; font-style:italic; color:${TC.textMuted};">${escapeHtml(entry.tips_de)}</div>` : ''}
            <div style="margin-top:6px;">
              <span style="color:${TC.accent};">${t('companion.good')}:</span> ${goodN}<br/>
              <span style="color:#ff6b6b;">${t('companion.bad')}:</span> ${badN}
            </div>
            ${(entry.toxic_cats || entry.toxic_dogs || entry.allergen || entry.toxic_humans) ? `<div style="margin-top:6px;display:flex;gap:4px;flex-wrap:wrap;">${renderSafetyBadgesFromEntry(entry)}</div>${entry.safety_note ? `<div style="font-size:11px;color:${TC.danger};margin-top:3px;">${escapeHtml(entry.safety_note)}</div>` : ''}` : ''}
            ${entry.propagation_methods && entry.propagation_methods.length ? `<div style="margin-top:6px;"><span style="font-size:11px;color:${TC.textMuted};font-weight:600;">${t('catalog.propagation')}</span> ${renderPropagationBadges(entry)}</div>` : ''}
          </div>`;
      }
    });
  } else if (type === 'task') {
    titleEl.textContent = t('task.new');
    body.innerHTML = renderTaskFormHTML(prefill);
    footer.innerHTML = `<button class="btn btn--ghost" id="modalCancel">${t('action.cancel')}</button><button class="btn btn--primary" id="modalSubmitTask">${t('task.create')}</button>`;
    document.getElementById('modalSubmitTask')?.addEventListener('click', handleCreateTask);
  } else if (type === 'log') {
    titleEl.textContent = t('log.new');
    body.innerHTML = renderLogFormHTML(prefill);
    footer.innerHTML = `<button class="btn btn--ghost" id="modalCancel">${t('action.cancel')}</button><button class="btn btn--primary" id="modalSubmitLog">${t('log.create')}</button>`;
    document.getElementById('modalSubmitLog')?.addEventListener('click', handleCreateLog);
    document.getElementById('logPhotoSelectBtn')?.addEventListener('click', async () => {
      try {
        const selected = await tauriOpenDialog({
          multiple: false,
          filters: [{ name: t('log.photoDialogFilter'), extensions: ['jpg', 'jpeg', 'png', 'gif', 'webp', 'bmp'] }],
          title: t('log.photoDialogTitle'),
        });
        if (!selected) return;
        const filePath = typeof selected === 'string' ? selected : selected.path;
        if (!filePath) return;
        const pathEl = document.getElementById('logPhotoFilePath');
        const labelEl = document.getElementById('logPhotoPath');
        if (pathEl) pathEl.value = filePath;
        if (labelEl) labelEl.textContent = filePath.split(/[/\\]/).pop();
      } catch (_) {}
    });
  } else if (type === 'auth') {
    titleEl.textContent = t('settings.login');
    body.innerHTML = `
      <div style="padding:12px; border-radius:12px; background:rgba(${TC.accentRgb},0.06); border:1px solid rgba(${TC.accentRgb},0.12); margin-bottom:14px; font-size:13px; color:${TC.textMuted};">
        ${IS_TAURI ? t('auth.modalHint') : t('auth.webModalHint')}
      </div>
      <div class="form-group"${IS_TAURI ? '' : ' style="display:none;"'}>
        <label class="form-label">${t('auth.field.backendUrl')}</label>
        <input id="authModalBackendUrl" class="form-input" placeholder="http://localhost:8080" value="${escapeHtml($('backendUrl')?.value || 'http://localhost:8080')}" />
      </div>
      <div class="form-group">
        <label class="form-label">${t('auth.field.email')}</label>
        <input id="authModalEmail" class="form-input" type="email" placeholder="${t('auth.emailPlaceholder')}" />
      </div>
      <div class="form-group">
        <label class="form-label">${t('auth.field.password')}</label>
        <input id="authModalPassword" class="form-input" type="password" placeholder="${t('auth.passwordPlaceholder')}" />
      </div>
      <div id="authModalError" class="form-error" style="display:none;"></div>
      <div id="authModalSuccess" class="form-hint" style="display:none; color:${TC.accent};"></div>
    `;
    footer.innerHTML = `
      <button class="btn btn--ghost" id="modalCancel">${t('action.cancel')}</button>
      <button class="btn" id="authModalRegister">${t('auth.register')}</button>
      <button class="btn btn--primary" id="authModalLogin">${t('auth.login')}</button>
    `;
    const showAuthError = (msg) => {
      const el = document.getElementById('authModalError');
      const suc = document.getElementById('authModalSuccess');
      if (suc) { suc.style.display = 'none'; }
      if (el) { el.textContent = msg; el.style.display = 'block'; }
    };
    const showAuthSuccess = (msg) => {
      const el = document.getElementById('authModalSuccess');
      const err = document.getElementById('authModalError');
      if (err) { err.style.display = 'none'; }
      if (el) { el.textContent = msg; el.style.display = 'block'; }
    };
    const getAuthFields = () => {
      const email = document.getElementById('authModalEmail')?.value?.trim();
      const password = document.getElementById('authModalPassword')?.value;
      const backendUrl = document.getElementById('authModalBackendUrl')?.value?.trim();
      // Sync to hidden fields used by existing handlers
      if ($('email')) $('email').value = email || '';
      if ($('password')) $('password').value = password || '';
      if ($('backendUrl')) $('backendUrl').value = backendUrl || '';
      return { email, password, backendUrl };
    };
    document.getElementById('authModalLogin')?.addEventListener('click', async () => {
      const { email, password, backendUrl } = getAuthFields();
      if (!email || !password) { showAuthError(t('auth.emailPasswordRequired')); return; }
      try {
        const result = await invoke("auth_login", { payload: { email, password, backend_url: backendUrl || null } });
        logOut(result);
        showAuthSuccess(t('auth.loginSuccess'));
        setTimeout(() => { closeModal(); refreshAll({ autoReminderReason: "auth_change" }); }, 800);
      } catch (err) {
        showAuthError(friendlyAuthError(err));
      }
    });
    document.getElementById('authModalRegister')?.addEventListener('click', async () => {
      const { email, password, backendUrl } = getAuthFields();
      if (!email || !password) { showAuthError(t('auth.emailPasswordRequired')); return; }
      if (password.length < 8) { showAuthError(t('auth.passwordMinLength')); return; }
      try {
        const result = await invoke("auth_register", { payload: { email, password, backend_url: backendUrl || null } });
        logOut(result);
        showAuthSuccess(t('auth.registerSuccess'));
        setTimeout(() => { closeModal(); refreshAll({ autoReminderReason: "auth_change" }); }, 800);
      } catch (err) {
        showAuthError(friendlyAuthError(err));
      }
    });
  }

  const _mo1 = document.getElementById('modalOverlay'); if (_mo1) _mo1.style.display = '';
  document.getElementById('modalCancel')?.addEventListener('click', closeModal);
  body.querySelector('input, textarea, select')?.focus();
}

function closeModal() {
  activeModal = null;
  const _mo2 = document.getElementById('modalOverlay'); if (_mo2) _mo2.style.display = 'none';
}

function renderPlantFormHTML(prefill = {}) {
  const categories = {
    gemuese: t('plant.category.vegetables'),
    kraeuter: t('plant.category.herbs'),
    obst: t('plant.category.fruit'),
    beeren: t('plant.category.berries'),
    blumen: t('plant.category.flowers'),
  };
  const catalogOptions = Object.entries(categories).map(([cat, label]) => {
    const entries = plantCatalog.filter(e => e.category === cat);
    if (!entries.length) return '';
    return `<optgroup label="${label}">${entries.map(e => `<option value="${e.id}">${e.emoji ? e.emoji + ' ' : ''}${e.name_de}</option>`).join('')}</optgroup>`;
  }).join('');

  return `
    <div class="form-group">
      <label class="form-label">${t('plant.catalog')} (optional)</label>
      <select id="plantCatalogSelect" class="form-select">
        <option value="">${t('plant.ownPlant')}</option>
        ${catalogOptions}
      </select>
    </div>
    <div class="form-group">
      <label class="form-label">${t('plant.name')}</label>
      <input id="plantName" class="form-input" placeholder="${t('plant.form.namePlaceholder')}" value="${escapeHtml(prefill.name || '')}" />
    </div>
    <div class="form-row">
      <div class="form-group">
        <label class="form-label">${t('plant.color')}</label>
        <input id="plantColor" type="color" value="${sanitizeColorTag(prefill.color || '#0B3D2E')}" class="form-input" />
      </div>
      <div class="form-group">
        <label class="form-label">${t('plant.phase')}</label>
        <select id="plantPhase" class="form-select">
          <option value="seed">${t('phase.seed')}</option>
          <option value="veg" selected>${t('phase.veg')}</option>
          <option value="flower">${t('phase.flower')}</option>
          <option value="harvest">${t('phase.harvest')}</option>
          <option value="dry">${t('phase.dry')}</option>
          <option value="cure">${t('phase.cure')}</option>
          <option value="dormant">${t('phase.dormant')}</option>
          <option value="custom">${t('phase.custom')}</option>
        </select>
      </div>
    </div>
    <div class="form-group">
      <label class="form-label">${t('plant.location')}</label>
      <select id="plantLocation" class="form-select">
        <option value="">${t('plant.noLocation')}</option>
        ${locations.map(l => `<option value="${l.location_id}">${escapeHtml(l.name)}</option>`).join('')}
      </select>
    </div>
    <div class="form-group">
      <label class="form-label">${t('plant.lifecycle')}</label>
      <select id="plantLifecycle" class="form-select">
        <option value="annual">${t('plant.annual')}</option>
        <option value="perennial">${t('plant.perennial')}</option>
      </select>
    </div>
    <div class="form-row">
      <div class="form-group">
        <label class="form-label">${t('plant.potSize')}</label>
        <input id="plantPotSize" type="number" step="0.5" class="form-input" placeholder="z.B. 11" />
      </div>
      <div class="form-group">
        <label class="form-label">${t('plant.soil')}</label>
        <select id="plantSoilType" class="form-select">
          <option value="">${t('soil.default')}</option>
          <option value="erde">${t('soil.erde')}</option>
          <option value="kokos">${t('soil.kokos')}</option>
          <option value="hydro">${t('soil.hydro')}</option>
          <option value="steinwolle">${t('soil.steinwolle')}</option>
          <option value="perlite">${t('soil.perlite')}</option>
        </select>
      </div>
    </div>
    <div id="catalogInfoPreview" style="display:none;"></div>
    <div id="plantFormError" class="form-error" style="display:none;"></div>
    <div id="plantPlanHint" class="form-hint"></div>
  `;
}

function renderTaskFormHTML(prefill = {}) {
  const dueDefault = prefill.dueDate
    ? `${prefill.dueDate}T09:00`
    : (timelineFocusKey ? `${timelineFocusKey}T09:00` : '');
  const plantOptions = plants.filter(p => plantStatusValue(p) === 'active').map(p =>
    `<option value="${p.plant_id}" ${p.plant_id === (prefill.plantId || selectedPlantId) ? 'selected' : ''}>${escapeHtml(p.name)}</option>`
  ).join('');

  return `
    <div class="form-group">
      <label class="form-label">${t('nav.plants')}</label>
      <select id="taskPlantSelect" class="form-select">
        <option value="">${t('plant.choose')}</option>
        ${plantOptions}
      </select>
    </div>
    <div class="form-group">
      <label class="form-label">${t('task.title')}</label>
      <input id="taskTitle" class="form-input" placeholder="${t('task.titlePlaceholder')}" value="${escapeHtml(prefill.title || '')}" />
    </div>
    <div class="form-row">
      <div class="form-group">
        <label class="form-label">${t('task.category')}</label>
        <select id="taskCategory" class="form-select">
          <option value="water">${t('cat.water')}</option>
          <option value="feed">${t('cat.feed')}</option>
          <option value="check">${t('cat.check')}</option>
          <option value="train">${t('cat.train')}</option>
          <option value="note">${t('cat.note')}</option>
        </select>
      </div>
      <div class="form-group">
        <label class="form-label">${t('task.repeat')}</label>
        <input id="taskRepeat" type="number" class="form-input" min="0" placeholder="${t('task.repeatHoursPlaceholder')}" value="${escapeHtml(String(prefill.repeat ?? ''))}" />
      </div>
    </div>
    <div class="form-group">
      <label class="form-label">${t('task.due')}</label>
      <input id="taskDueAt" type="datetime-local" class="form-input" value="${dueDefault}" />
    </div>
    <div class="form-row">
      <div class="form-group">
        <label class="form-label">${t('task.repeatUntil')} (optional)</label>
        <input id="taskRepeatUntil" type="date" class="form-input" value="" />
      </div>
      <div class="form-group">
        <label class="form-check" style="margin-top:20px;">
          <input id="taskNotificationEnabled" type="checkbox" checked />
          ${t('task.reminder')}
        </label>
      </div>
    </div>
    <div id="taskFormError" class="form-error" style="display:none;"></div>
  `;
}

function renderLogFormHTML(prefill = {}) {
  const dateDefault = prefill.createdAt
    ? `${prefill.createdAt}T12:00`
    : (timelineFocusKey ? `${timelineFocusKey}T12:00` : '');
  const plantOptions = plants.filter(p => plantStatusValue(p) !== 'deleted').map(p =>
    `<option value="${p.plant_id}" ${p.plant_id === (prefill.plantId || selectedPlantId) ? 'selected' : ''}>${escapeHtml(p.name)}</option>`
  ).join('');
  const logTypeDefault = prefill.logType || 'note';

  return `
    <div class="form-group">
      <label class="form-label">${t('nav.plants')}</label>
      <select id="logPlantSelect" class="form-select">
        <option value="">${t('plant.choose')}</option>
        ${plantOptions}
      </select>
    </div>
    <div class="form-group">
      <label class="form-label">${t('log.type')}</label>
      <select id="logType" class="form-select">
        <option value="note" ${logTypeDefault === 'note' ? 'selected' : ''}>${t('log.note')}</option>
        <option value="measurement" ${logTypeDefault === 'measurement' ? 'selected' : ''}>${t('log.measurement')}</option>
        <option value="harvest" ${logTypeDefault === 'harvest' ? 'selected' : ''}>${t('log.harvest')}</option>
        <option value="watering" ${logTypeDefault === 'watering' ? 'selected' : ''}>${t('log.watering')}</option>
      </select>
    </div>
    <div class="form-group">
      <label class="form-label">${t('log.text')}</label>
      <textarea id="logText" class="form-textarea" placeholder="${t('log.textPlaceholder')}">${escapeHtml(prefill.text || '')}</textarea>
    </div>
    <div class="form-row">
      <div class="form-group">
        <label class="form-label">pH</label>
        <input id="logPh" type="number" step="0.1" class="form-input" placeholder="z.B. 6.2" />
      </div>
      <div class="form-group">
        <label class="form-label">EC</label>
        <input id="logEc" type="number" step="0.1" class="form-input" placeholder="z.B. 1.2" />
      </div>
    </div>
    <div class="form-row">
      <div class="form-group">
        <label class="form-label">Temp (\u00b0C)</label>
        <input id="logTemp" type="number" step="0.1" class="form-input" placeholder="z.B. 24" />
      </div>
      <div class="form-group">
        <label class="form-label">RH (%)</label>
        <input id="logRh" type="number" step="0.1" class="form-input" placeholder="z.B. 55" />
      </div>
    </div>
    <div class="form-row">
      <div class="form-group">
        <label class="form-label">${t('harvest.kg')}</label>
        <input id="logHarvestKg" type="number" step="0.1" class="form-input" placeholder="z.B. 2.5" />
      </div>
      <div class="form-group">
        <label class="form-label">${t('harvest.count')}</label>
        <input id="logHarvestCount" type="number" class="form-input" placeholder="z.B. 12" />
      </div>
    </div>
    <div class="form-group">
      <label class="form-label">${t('harvest.quality')}</label>
      <select id="logHarvestQuality" class="form-select">
        <option value="">--</option>
        <option value="excellent">${t('harvest.excellent')}</option>
        <option value="good">${t('harvest.good')}</option>
        <option value="average">${t('harvest.average')}</option>
        <option value="poor">${t('harvest.poor')}</option>
      </select>
    </div>
    <div class="form-row">
      <div class="form-group">
        <label class="form-label">${t('water.liters')}</label>
        <input id="logWaterLiters" type="number" step="0.1" class="form-input" placeholder="z.B. 2.0" />
      </div>
      <div class="form-group">
        <label class="form-label">${t('water.method')}</label>
        <select id="logWaterMethod" class="form-select">
          <option value="">--</option>
          <option value="giessen">${t('water.pour')}</option>
          <option value="tropf">${t('water.drip')}</option>
          <option value="spruehen">${t('water.spray')}</option>
          <option value="regen">${t('water.rain')}</option>
        </select>
      </div>
    </div>
    <div class="form-group">
      <label class="form-label">${t('log.date')}</label>
      <input id="logCreatedAt" type="datetime-local" class="form-input" value="${dateDefault}" />
    </div>
    <div class="form-group">
      <label class="form-label">${t('log.photo')} (optional)</label>
      <div style="display:flex;gap:8px;align-items:center;">
        <button type="button" class="btn btn--sm" id="logPhotoSelectBtn">${t('log.pickPhoto')}</button>
        <span id="logPhotoPath" style="font-size:12px;color:${TC.textDim};overflow:hidden;text-overflow:ellipsis;white-space:nowrap;max-width:200px;"></span>
      </div>
      <input type="hidden" id="logPhotoFilePath" />
    </div>
    <div id="logFormError" class="form-error" style="display:none;"></div>
  `;
}

async function handleCreatePlant() {
  const errEl = $("plantFormError");
  if (errEl) { errEl.textContent = ''; errEl.style.display = 'none'; }
  const name = $("plantName")?.value?.trim();
  if (!name) {
    if (errEl) { errEl.textContent = t('plant.form.nameRequired'); errEl.style.display = 'block'; }
    return;
  }
  if (!canActivateMorePlants()) {
    if (errEl) { errEl.textContent = planLimitMessage("create"); errEl.style.display = 'block'; }
    return;
  }
  try {
    const catalogSelect = $("plantCatalogSelect")?.value || null;
    const potSizeRaw = $("plantPotSize")?.value?.trim();
    const potSize = potSizeRaw ? Number(potSizeRaw) : null;
    const soilType = $("plantSoilType")?.value || null;
    const payload = {
      name,
      color_tag: $("plantColor")?.value || '#0B3D2E',
      phase: $("plantPhase")?.value || 'veg',
      phase_week: 1,
      location_id: $("plantLocation")?.value || null,
      lifecycle: $("plantLifecycle")?.value || 'annual',
      catalog_id: catalogSelect || null,
      pot_size_liters: potSize,
      soil_type: soilType,
    };
    const plant = await invoke("create_plant_smart", { payload });
    selectedPlantId = plant.plant_id;
    logOut(plant);
    playNotificationSound('success');
    closeModal();
    await refreshAll({ autoReminderReason: "plant_create" });
  } catch (err) {
    if (errEl) { errEl.textContent = t('plant.form.createFailed', err); errEl.style.display = 'block'; }
  }
}

async function handleCreateTask() {
  const errEl = $("taskFormError");
  if (errEl) { errEl.textContent = ''; errEl.style.display = 'none'; }
  const plantId = $("taskPlantSelect")?.value;
  if (!plantId) {
    if (errEl) { errEl.textContent = t('plant.form.selectRequired'); errEl.style.display = 'block'; }
    return;
  }
  const title = $("taskTitle")?.value?.trim();
  if (!title) {
    if (errEl) { errEl.textContent = t('task.form.titleRequired'); errEl.style.display = 'block'; }
    return;
  }
  const repeatRaw = $("taskRepeat")?.value?.trim();
  const repeat = repeatRaw ? Number(repeatRaw) : null;
  if (repeat !== null && (Number.isNaN(repeat) || repeat < 0)) {
    if (errEl) { errEl.textContent = "Repeat muss leer oder eine Zahl ab 0 sein."; errEl.style.display = 'block'; }
    return;
  }
  const due_at = datetimeLocalToIso($("taskDueAt")?.value);
  const notification_enabled = $("taskNotificationEnabled")?.checked ?? true;
  const repeatUntilRaw = $("taskRepeatUntil")?.value;
  const repeat_until = repeatUntilRaw || null;
  if (!due_at) {
    if (errEl) { errEl.textContent = t('task.validDueDate'); errEl.style.display = 'block'; }
    return;
  }
  try {
    const task = await invoke("create_task_smart", {
      payload: {
        plant_id: plantId,
        title,
        category: $("taskCategory")?.value || 'water',
        due_at,
        repeat_interval_hours: repeat,
        repeat_until,
        notification_enabled,
      },
    });
    logOut(task);
    playNotificationSound('success');
    closeModal();
    await refreshAll({ autoReminderReason: "task_create" });
  } catch (err) {
    if (errEl) { errEl.textContent = `Task konnte nicht angelegt werden: ${err}`; errEl.style.display = 'block'; }
  }
}

async function handleCreateLog() {
  const errEl = $("logFormError");
  if (errEl) { errEl.textContent = ''; errEl.style.display = 'none'; }
  const plantId = $("logPlantSelect")?.value;
  if (!plantId) {
    if (errEl) { errEl.textContent = t('plant.form.selectRequired'); errEl.style.display = 'block'; }
    return;
  }
  const textValue = $("logText")?.value?.trim() || '';
  const ph = parseOptionalNumber("logPh");
  const ec = parseOptionalNumber("logEc");
  const temp_c = parseOptionalNumber("logTemp");
  const rh = parseOptionalNumber("logRh");
  const harvest_kg = parseOptionalNumber("logHarvestKg");
  const harvest_count = $("logHarvestCount")?.value ? parseInt($("logHarvestCount").value, 10) : null;
  const harvest_quality = $("logHarvestQuality")?.value || null;
  const water_liters = parseOptionalNumber("logWaterLiters");
  const water_method = $("logWaterMethod")?.value || null;
  const log_type = $("logType")?.value || "note";
  if ([ph, ec, temp_c, rh, harvest_kg, water_liters].some(Number.isNaN)) {
    if (errEl) { errEl.textContent = "Messwerte m\u00fcssen leer oder g\u00fcltige Zahlen sein."; errEl.style.display = 'block'; }
    return;
  }
  const hasAnyValue = textValue || ph !== null || ec !== null || temp_c !== null || rh !== null
    || harvest_kg !== null || harvest_count !== null || harvest_quality
    || water_liters !== null || water_method;
  if (!hasAnyValue) {
    if (errEl) { errEl.textContent = t('log.form.valueRequired'); errEl.style.display = 'block'; }
    return;
  }
  try {
    const payload = {
      plant_id: plantId,
      log_type,
      created_at: datetimeLocalToIso($("logCreatedAt")?.value),
      text: textValue,
      ph,
      ec,
      temp_c,
      rh,
      harvest_kg,
      harvest_count,
      harvest_quality,
      water_liters,
      water_method,
    };
    const log = await invoke("create_log_smart", { payload });
    logOut(log);
    playNotificationSound('success');
    // Attach photo if selected via Tauri dialog
    const logPhotoPath = $("logPhotoFilePath")?.value;
    if (logPhotoPath) {
      try {
        await invoke("add_photo", {
          plantId: plantId,
          logId: log.log_id,
          sourcePath: logPhotoPath,
          caption: null,
        });
      } catch (photoErr) {
        console.warn("Photo attachment failed:", photoErr);
      }
    }
    closeModal();
    await refreshAll({ autoReminderReason: "log_create" });
  } catch (err) {
    if (errEl) { errEl.textContent = `Log konnte nicht angelegt werden: ${err}`; errEl.style.display = 'block'; }
  }
}

// Browser build only: whether the HTTP session is live. The desktop always has
// its local database, so this stays irrelevant there.
let webSignedIn = false;

/** Re-read the browser session and sync the account controls to it. */
async function refreshWebSessionState() {
  if (IS_TAURI) return;
  try {
    const status = await invoke("get_sync_status");
    webSignedIn = !!status.session_active;
  } catch (_) {
    webSignedIn = false;
  }
  const signIn = document.getElementById('openAuthModal');
  const signOut = document.getElementById('webLogout');
  if (signIn) signIn.style.display = webSignedIn ? 'none' : '';
  if (signOut) signOut.style.display = webSignedIn ? '' : 'none';
  updateOnboardingVisibility();
}

function updateOnboardingVisibility() {
  const overlay = document.getElementById('onboardingOverlay');
  if (!overlay) return;
  // In the browser nothing can be saved before signing in, so the "create your
  // first plant" prompt would only lead to an error. The sign-in dialog takes
  // its place.
  if (!IS_TAURI && !webSignedIn) {
    overlay.style.display = 'none';
    return;
  }
  const hasPlants = plants.some(p => plantStatusValue(p) !== 'deleted');
  overlay.style.display = hasPlants ? 'none' : '';
}


function renderTimelinePanel() {
  // Vacation mode banner
  const vacBanner = document.getElementById('vacationBanner');
  if (vacBanner) {
    if (vacationMode && vacationMode.active) {
      const returnStr = vacationMode.returnDate ? ' bis ' + formatDate(new Date(vacationMode.returnDate)) : '';
      vacBanner.innerHTML = '<div style="background:rgba(255,200,100,0.1);border:1px solid rgba(255,200,100,0.2);border-radius:10px;padding:8px 14px;margin-bottom:10px;display:flex;justify-content:space-between;align-items:center;">'
        + '<span>\u{1F3D6}\uFE0F ' + t('vacation.banner') + escapeHtml(returnStr) + '</span>'
        + '<button class="btn btn--sm" id="vacBannerEnd">' + t('vacation.end') + '</button>'
        + '</div>';
      document.getElementById('vacBannerEnd')?.addEventListener('click', disableVacationMode);
    } else {
      vacBanner.innerHTML = '';
    }
  }
  const hasVisiblePlants = plants.some(p => plantStatusValue(p) !== 'deleted');
  if (!hasVisiblePlants && timelinePanel) {
    timelinePanel.innerHTML = `
      <div class="empty-state-card">
        <div class="empty-state-title">${t('cal.empty.title')}</div>
        <div class="empty-state-hint">${t('cal.empty.hint')}</div>
        <div class="empty-state-actions">
          <button type="button" id="calEmptyCreatePlant" class="btn btn--primary btn--sm">${t('cal.empty.create')}</button>
          <button type="button" id="calEmptyGotoPlants" class="btn btn--sm">${t('cal.empty.manage')}</button>
        </div>
      </div>`;
    document.getElementById('calEmptyCreatePlant')?.addEventListener('click', () => { switchPageView('plants'); window.setTimeout(() => $("plantName")?.focus(), 30); });
    document.getElementById('calEmptyGotoPlants')?.addEventListener('click', () => switchPageView('plants'));
    return;
  }
  const plant = selectedPlant();
  const sourceTasks = plant ? _selectedTasksExpanded : _allTasksExpanded;
  const sourceLogs = plant ? logs.filter(l => l.plant_id === plant.plant_id) : allLogs;

  const taskItems = sourceTasks
    .map((task) => ({
      type: "task",
      id: task.task_id,
      title: task.title,
      subtitle: `${task.category} · ${task.notification_enabled ? "Reminder an" : "Reminder aus"}`,
      badge: task.status === "done" ? t('task.done') : task.status === "skipped" ? "Skipped" : t('task.open'),
      sortAt: task.due_at || task.updated_at || task.created_at,
      dateKey: normalizeDateKey(task.due_at || task.updated_at || task.created_at),
    }));

  const logItems = sourceLogs
    .map((log) => {
      let badge = log.metrics ? `pH ${log.metrics.ph ?? '-'} \u00b7 EC ${log.metrics.ec ?? '-'}` : t('misc.entry');
      if (log.metrics?.harvest_kg || log.metrics?.harvest_count) {
        badge = `\u{1F33E} ${log.metrics.harvest_kg ? log.metrics.harvest_kg + ' kg' : ''}${log.metrics.harvest_count ? ' ' + log.metrics.harvest_count + ' Stk' : ''}`;
      }
      if (log.metrics?.water_liters) {
        const method = log.metrics.water_method ? ' (' + log.metrics.water_method + ')' : '';
        badge = `\u{1F4A7} ${log.metrics.water_liters} L${method}`;
      }
      return {
        type: "log",
        id: log.log_id,
        title: log.log_type,
        subtitle: log.text || t('misc.noText'),
        badge,
        sortAt: log.created_at,
        dateKey: normalizeDateKey(log.created_at),
      };
    });

  const merged = [...taskItems, ...logItems].sort((a, b) => {
    const av = a.sortAt ? new Date(a.sortAt).getTime() : 0;
    const bv = b.sortAt ? new Date(b.sortAt).getTime() : 0;
    return bv - av;
  });

  // Don't override timelineFocusKey — the calendar is a real grid,
  // the user should be able to navigate freely to any month.

  const filtered = merged.filter((item) => itemMatchesTimelineWindow(item, timelineMode, timelineFocusKey));
  const grouped = new Map();
  for (const item of filtered) {
    const key = item.dateKey || "ohne-datum";
    if (!grouped.has(key)) grouped.set(key, []);
    grouped.get(key).push(item);
  }

  const sortedKeys = [...grouped.keys()].sort((a, b) => {
    if (a === "ohne-datum") return 1;
    if (b === "ohne-datum") return -1;
    return new Date(b).getTime() - new Date(a).getTime();
  });

  const nextOpenTasks = taskItems
    .filter((ti) => ti.badge === t('task.open') && itemMatchesTimelineWindow(ti, timelineMode, timelineFocusKey))
    .sort((a, b) => {
      const av = a.sortAt ? new Date(a.sortAt).getTime() : Number.MAX_SAFE_INTEGER;
      const bv = b.sortAt ? new Date(b.sortAt).getTime() : Number.MAX_SAFE_INTEGER;
      return av - bv;
    })
    .slice(0, 3);

  const periodLabel = timelineMode === "day"
    ? dateLabelFromKey(timelineFocusKey)
    : timelineMode === "week"
      ? weekRangeLabelFromKey(timelineFocusKey)
      : monthRangeLabelFromKey(timelineFocusKey);
  const weekKeys = weekKeysFromFocusKey(timelineFocusKey);
  const monthGrid = monthGridFromFocusKey(timelineFocusKey);

  const todayStr = new Date().toISOString().slice(0, 10);

  // Moon legend for moon mode
  const moonLegendHtml = calendarMode === 'moon' ? `
    <div style="display:flex; gap:12px; flex-wrap:wrap; padding:8px 0; font-size:11px; color:${TC.textMuted}; border-bottom:1px solid rgba(${TC.accentRgb},0.08); margin-bottom:8px;">
      <span>\u{1F311} ${t('moon.legend.newmoon')}</span><span>\u{1F312}\u{1F313}\u{1F314} ${t('moon.legend.waxing')}</span>
      <span>\u{1F315} ${t('moon.legend.full')}</span><span>\u{1F316}\u{1F317}\u{1F318} ${t('moon.legend.waning')}</span>
      <span title="Feuer=Frucht, Erde=Wurzel, Luft=Bl\u00fcte, Wasser=Blatt">${t('moon.legend.elements')}</span>
    </div>` : '';

  // Chinese calendar header
  const chineseHeaderHtml = calendarMode === 'chinese' ? (() => {
    const cInfo = getChineseDate(dateFromKey(timelineFocusKey));
    const animalEmojis = { dragon: '\u{1F409}', snake: '\u{1F40D}', horse: '\u{1F40E}', goat: '\u{1F410}', monkey: '\u{1F412}' };
    const animalEmoji = animalEmojis[cInfo.animal] || '\u{1F43E}';
    return `<div style="padding:8px 0; font-size:13px; color:${TC.textMuted}; border-bottom:1px solid rgba(${TC.accentRgb},0.08); margin-bottom:8px;">
      <span style="font-size:16px;">${animalEmoji}</span> ${t('cal.chinese.headline', escapeHtml(cInfo.animalLabel), escapeHtml(cInfo.elementLabel), cInfo.year)}
    </div>`;
  })() : '';

  const persianHeaderHtml = calendarMode === 'persian' ? (() => {
    const pi = getPersianDate(dateFromKey(timelineFocusKey));
    return `<div style="padding:8px 0; font-size:13px; color:${TC.textMuted}; border-bottom:1px solid rgba(${TC.accentRgb},0.08); margin-bottom:8px;">
      <span style="font-size:16px;">&#9728;&#65039;</span> ${t('cal.persian.headline', escapeHtml(pi.monthName), pi.year)}
    </div>`;
  })() : '';

  const julianHeaderHtml = calendarMode === 'julian' ? (() => {
    const ji = getJulianDate(dateFromKey(timelineFocusKey));
    return `<div style="padding:8px 0; font-size:13px; color:${TC.textMuted}; border-bottom:1px solid rgba(${TC.accentRgb},0.08); margin-bottom:8px;">
      <span style="font-size:16px;">&#10013;&#65039;</span> ${t('cal.julian.headline', ji.day, escapeHtml(ji.monthName), ji.year, ji.correction)}
    </div>`;
  })() : '';

  const islamicHeaderHtml = calendarMode === 'islamic' ? (() => {
    const ii = getIslamicDate(dateFromKey(timelineFocusKey));
    return `<div style="padding:8px 0; font-size:13px; color:${TC.textMuted}; border-bottom:1px solid rgba(${TC.accentRgb},0.08); margin-bottom:8px;">
      <span style="font-size:16px;">&#9770;&#65039;</span> ${t('cal.islamic.headline', escapeHtml(ii.monthName), ii.year)}
    </div>`;
  })() : '';

  const jewishHeaderHtml = calendarMode === 'jewish' ? (() => {
    const jhi = getJewishDate(dateFromKey(timelineFocusKey));
    return `<div style="padding:8px 0; font-size:13px; color:${TC.textMuted}; border-bottom:1px solid rgba(${TC.accentRgb},0.08); margin-bottom:8px;">
      <span style="font-size:16px;">&#10017;&#65039;</span> J\u00fcdischer Kalender \u2014 ${escapeHtml(jhi.monthName)} ${jhi.year}
    </div>`;
  })() : '';

  const bauernHeaderHtml = calendarMode === 'bauern' ? (() => {
    const bi = getBauernkalenderInfo(dateFromKey(timelineFocusKey));
    return `<div style="padding:8px 0; font-size:13px; color:${TC.textMuted}; border-bottom:1px solid rgba(${TC.accentRgb},0.08); margin-bottom:8px;">
      <span style="font-size:16px;">&#127806;</span> Bauernkalender \u2014 ${escapeHtml(bi.phenoSeason)}
      <div style="margin-top:4px;font-style:italic;font-size:12px;">${escapeHtml(bi.spruch)}</div>
    </div>`;
  })() : '';

  const extraHeaderHtml = persianHeaderHtml + julianHeaderHtml + islamicHeaderHtml + jewishHeaderHtml + bauernHeaderHtml;

  if (timelineMode === 'year') {
    const year = dateFromKey(timelineFocusKey).getFullYear();
    const monthNames = Array.from({ length: 12 }, (_, i) => getMonthName(i + 1));
    const weekdayMini = getWeekdayHeaders();

    // Precompute activity per day for the whole year (faster than recomputing in loop)
    const activityByDay = new Map();
    const addDay = (k, plantId) => {
      if (!activityByDay.has(k)) activityByDay.set(k, new Set());
      if (plantId) activityByDay.get(k).add(plantId);
    };
    for (const task of _allTasksExpanded) {
      const k = normalizeDateKey(task.due_at);
      if (k && k.slice(0, 4) === String(year)) addDay(k, task.plant_id);
    }
    for (const log of allLogs) {
      const k = normalizeDateKey(log.created_at);
      if (k && k.slice(0, 4) === String(year)) addDay(k, log.plant_id);
    }

    const monthsHtml = Array.from({ length: 12 }, (_, monthIdx) => {
      // Build grid for this month (same logic as monthGrid for main month)
      const firstDay = new Date(year, monthIdx, 1);
      const lastDay = new Date(year, monthIdx + 1, 0);
      const firstWeekday = (firstDay.getDay() + 6) % 7; // Mo=0
      const daysInMonth = lastDay.getDate();
      const cells = [];
      for (let i = 0; i < firstWeekday; i++) cells.push(null);
      for (let d = 1; d <= daysInMonth; d++) {
        const key = `${year}-${String(monthIdx + 1).padStart(2, '0')}-${String(d).padStart(2, '0')}`;
        cells.push({ key, day: d });
      }
      while (cells.length % 7 !== 0) cells.push(null);
      let totalActivity = 0;
      for (const c of cells) if (c && activityByDay.has(c.key)) totalActivity += activityByDay.get(c.key).size;

      const daysHtml = cells.map(c => {
        if (!c) return `<div class="cal-year-day cal-year-day--empty"></div>`;
        const isToday = c.key === todayStr;
        const plantIds = activityByDay.get(c.key);
        const dotColor = plantIds && plantIds.size > 0
          ? sanitizeColorTag(plants.find(p => p.plant_id === [...plantIds][0])?.color_tag || TC.accent)
          : null;
        const intensity = plantIds ? Math.min(plantIds.size, 4) : 0;
        return `<div class="cal-year-day ${isToday ? 'cal-year-day--today' : ''}" data-year-day="${c.key}" title="${plantIds ? t('cal.year.dayTitle', c.key, plantIds.size) : c.key}" style="${dotColor ? `background:rgba(${hexToRgb(dotColor)},${0.15 + intensity * 0.15});` : ''}">
          <span class="cal-year-day-num">${c.day}</span>
        </div>`;
      }).join('');

      return `
        <div class="cal-year-month" data-year-month="${year}-${String(monthIdx + 1).padStart(2, '0')}">
          <div class="cal-year-month-header">
            <span class="cal-year-month-name">${escapeHtml(monthNames[monthIdx])}</span>
            ${totalActivity > 0 ? `<span class="cal-year-month-count">${totalActivity}</span>` : ''}
          </div>
          <div class="cal-year-month-weekdays">
            ${weekdayMini.map(d => `<div class="cal-year-weekday">${d.slice(0,1)}</div>`).join('')}
          </div>
          <div class="cal-year-month-grid">
            ${daysHtml}
          </div>
        </div>`;
    }).join('');

    timelinePanel.innerHTML = `
      ${chineseHeaderHtml}${extraHeaderHtml}
      <div class="cal-year-grid">
        ${monthsHtml}
      </div>`;

    // Click day → switch to month mode on that date
    timelinePanel.querySelectorAll('[data-year-day]').forEach(cell => {
      cell.addEventListener('click', (e) => {
        e.stopPropagation();
        const key = cell.dataset.yearDay;
        timelineFocusKey = key;
        timelineMode = 'month';
        renderCalendarToolbar();
        renderTimelinePanel();
      });
    });
    // Click month header → switch to month mode on that month
    timelinePanel.querySelectorAll('[data-year-month]').forEach(el => {
      el.querySelector('.cal-year-month-header')?.addEventListener('click', (e) => {
        e.stopPropagation();
        const ym = el.dataset.yearMonth;
        timelineFocusKey = `${ym}-01`;
        timelineMode = 'month';
        renderCalendarToolbar();
        renderTimelinePanel();
      });
    });
  } else if (timelineMode === 'month') {
    timelinePanel.innerHTML = `
      ${moonLegendHtml}${chineseHeaderHtml}${extraHeaderHtml}
      <div class="cal-grid">
        ${getWeekdayHeaders().map(d => `<div class="cal-day-header">${d}</div>`).join('')}
        ${monthGrid.map(({ key: dayKey, inMonth }) => {
          const isFocused = dayKey === focusedDayKey;
          const isToday = dayKey === todayStr;
          const stats = dayStats(dayKey);
          const dots = dayPlantDots(dayKey);
          const moonInfo = (calendarMode === 'moon' || calendarMode === 'chinese') ? getMoonPhase(dateFromKey(dayKey)) : null;
          const chineseInfo = calendarMode === 'chinese' ? getChineseDate(dateFromKey(dayKey)) : null;
          const persianInfo = calendarMode === 'persian' ? getPersianDate(dateFromKey(dayKey)) : null;
          const julianInfo = calendarMode === 'julian' ? getJulianDate(dateFromKey(dayKey)) : null;
          const islamicInfo = calendarMode === 'islamic' ? getIslamicDate(dateFromKey(dayKey)) : null;
          const jewishInfo = calendarMode === 'jewish' ? getJewishDate(dateFromKey(dayKey)) : null;
          const bauernInfo = calendarMode === 'bauern' ? getBauernkalenderInfo(dateFromKey(dayKey)) : null;
          return `
            <div class="cal-day-cell ${isToday ? 'cal-day-cell--today' : ''} ${isFocused ? 'cal-day-cell--focused' : ''} ${!inMonth ? 'cal-day-cell--outside' : ''}" data-timeline-day="${dayKey}">
              <div style="display:flex;justify-content:space-between;align-items:center;">
                <span class="cal-day-num">${dateFromKey(dayKey).getDate()}</span>
                ${moonInfo ? `<span style="font-size:14px;" title="${escapeHtml(moonInfo.gardenTip)}">${moonInfo.emoji}</span>` : ''}
                ${chineseInfo ? `<span style="font-size:9px;color:${TC.textDim};">${chineseInfo.month}/${chineseInfo.day}</span>` : ''}
                <div class="cal-day-dots">${dots}</div>
              </div>
              ${persianInfo ? `<div style="font-size:9px;color:${TC.textDim};">${persianInfo.day} ${escapeHtml(persianInfo.monthName)}</div>` : ''}
              ${julianInfo ? `<div style="font-size:9px;color:${TC.textDim};">Jul: ${julianInfo.day}.${julianInfo.month}.</div>` : ''}
              ${islamicInfo ? `<div style="font-size:9px;color:${TC.textDim};">${islamicInfo.day} ${escapeHtml(islamicInfo.monthName)}</div>` : ''}
              ${jewishInfo ? `<div style="font-size:9px;color:${TC.textDim};">${jewishInfo.day} ${escapeHtml(jewishInfo.monthName)}</div>` : ''}
              ${bauernInfo?.lostag ? `<div style="font-size:9px;color:${TC.warning};font-weight:600;" title="${escapeHtml(bauernInfo.spruch)}">${escapeHtml(bauernInfo.name)}</div>` : ''}
              <div class="cal-day-badges">
                ${stats.open > 0 ? `<span class="cal-badge">${stats.open}<span class="cal-badge-label"> ${t('misc.open')}</span></span>` : ''}
                ${stats.overdue > 0 ? `<span class="cal-badge cal-badge--overdue">${stats.overdue}<span class="cal-badge-label"> ${t('misc.overdue')}</span></span>` : ''}
                ${stats.done > 0 ? `<span class="cal-badge">${stats.done}<span class="cal-badge-label"> ${t('misc.done')}</span></span>` : ''}
                ${stats.logs > 0 ? `<span class="cal-badge cal-badge--log">${stats.logs}<span class="cal-badge-label"> ${t('misc.logs')}</span></span>` : ''}
                ${stats.harvests > 0 ? `<span class="cal-badge" style="background:rgba(255,200,100,0.15);color:${TC.warning};">&#127806; ${stats.harvests}</span>` : ''}
                ${stats.waterings > 0 ? `<span class="cal-badge" style="background:rgba(100,180,255,0.15);color:${TC.info};">&#128167; ${stats.waterings}</span>` : ''}
                ${calendarMode === 'moon' && moonInfo ? `<span class="cal-badge" style="background:rgba(200,180,255,0.12);color:#c8b4ff;" title="${escapeHtml(moonInfo.gardenTip)}">${escapeHtml(moonInfo.element)}</span>` : ''}
              </div>
            </div>`;
        }).join('')}
      </div>`;
  } else if (timelineMode === 'week') {
    timelinePanel.innerHTML = `
      ${moonLegendHtml}${chineseHeaderHtml}${extraHeaderHtml}
      <div class="cal-grid">
        ${weekKeys.map((dayKey) => {
          const isFocused = dayKey === focusedDayKey;
          const isToday = dayKey === todayStr;
          const stats = dayStats(dayKey);
          const dots = dayPlantDots(dayKey);
          const moonInfo = (calendarMode === 'moon' || calendarMode === 'chinese') ? getMoonPhase(dateFromKey(dayKey)) : null;
          const chineseInfo = calendarMode === 'chinese' ? getChineseDate(dateFromKey(dayKey)) : null;
          const persianInfo = calendarMode === 'persian' ? getPersianDate(dateFromKey(dayKey)) : null;
          const julianInfo = calendarMode === 'julian' ? getJulianDate(dateFromKey(dayKey)) : null;
          const islamicInfo = calendarMode === 'islamic' ? getIslamicDate(dateFromKey(dayKey)) : null;
          const jewishInfo = calendarMode === 'jewish' ? getJewishDate(dateFromKey(dayKey)) : null;
          const bauernInfo = calendarMode === 'bauern' ? getBauernkalenderInfo(dateFromKey(dayKey)) : null;
          return `
            <div class="cal-day-cell ${isToday ? 'cal-day-cell--today' : ''} ${isFocused ? 'cal-day-cell--focused' : ''}" data-timeline-day="${dayKey}" style="min-height:140px;">
              <div style="display:flex;justify-content:space-between;align-items:center;">
                <span style="font-size:11px;color:${TC.textDim};">${dateFromKey(dayKey).toLocaleDateString(localeTag(), { weekday: 'short' })}</span>
                ${moonInfo ? `<span style="font-size:14px;" title="${escapeHtml(moonInfo.gardenTip)}">${moonInfo.emoji}</span>` : ''}
                ${chineseInfo ? `<span style="font-size:9px;color:${TC.textDim};">${chineseInfo.month}/${chineseInfo.day}</span>` : ''}
                <span class="cal-day-num">${dateFromKey(dayKey).toLocaleDateString(localeTag(), { day: '2-digit', month: '2-digit' })}</span>
              </div>
              ${persianInfo ? `<div style="font-size:9px;color:${TC.textDim};">${persianInfo.day} ${escapeHtml(persianInfo.monthName)}</div>` : ''}
              ${julianInfo ? `<div style="font-size:9px;color:${TC.textDim};">Jul: ${julianInfo.day}.${julianInfo.month}.</div>` : ''}
              ${islamicInfo ? `<div style="font-size:9px;color:${TC.textDim};">${islamicInfo.day} ${escapeHtml(islamicInfo.monthName)}</div>` : ''}
              ${jewishInfo ? `<div style="font-size:9px;color:${TC.textDim};">${jewishInfo.day} ${escapeHtml(jewishInfo.monthName)}</div>` : ''}
              ${bauernInfo?.lostag ? `<div style="font-size:9px;color:${TC.warning};font-weight:600;" title="${escapeHtml(bauernInfo.spruch)}">${escapeHtml(bauernInfo.name)}</div>` : ''}
              <div class="cal-day-dots">${dots}</div>
              <div class="cal-day-badges">
                ${stats.open > 0 ? `<span class="cal-badge">${stats.open}<span class="cal-badge-label"> ${t('misc.open')}</span></span>` : ''}
                ${stats.overdue > 0 ? `<span class="cal-badge cal-badge--overdue">${stats.overdue}</span>` : ''}
                ${stats.logs > 0 ? `<span class="cal-badge cal-badge--log">${stats.logs}<span class="cal-badge-label"> ${t('misc.logs')}</span></span>` : ''}
                ${stats.harvests > 0 ? `<span class="cal-badge" style="background:rgba(255,200,100,0.15);color:${TC.warning};">&#127806; ${stats.harvests}</span>` : ''}
                ${stats.waterings > 0 ? `<span class="cal-badge" style="background:rgba(100,180,255,0.15);color:${TC.info};">&#128167; ${stats.waterings}</span>` : ''}
                ${calendarMode === 'moon' && moonInfo ? `<span class="cal-badge" style="background:rgba(200,180,255,0.12);color:#c8b4ff;" title="${escapeHtml(moonInfo.gardenTip)}">${escapeHtml(moonInfo.element)}</span>` : ''}
              </div>
            </div>`;
        }).join('')}
      </div>`;
  } else {
    // Day mode: show grouped items
    const dayModeMoonInfo = (calendarMode === 'moon' || calendarMode === 'chinese') ? getMoonPhase(dateFromKey(timelineFocusKey)) : null;
    const dayModeChineseInfo = calendarMode === 'chinese' ? getChineseDate(dateFromKey(timelineFocusKey)) : null;
    const dayModeMoonHtml = dayModeMoonInfo ? `
      <div style="display:flex;gap:12px;align-items:center;padding:10px 14px;border-radius:12px;background:rgba(200,180,255,0.06);border:1px solid rgba(200,180,255,0.12);margin-bottom:12px;font-size:12px;">
        <span style="font-size:24px;" title="${escapeHtml(dayModeMoonInfo.gardenTip)}">${dayModeMoonInfo.emoji}</span>
        <div style="flex:1;">
          <div style="font-weight:600;color:${TC.text};">${escapeHtml(dayModeMoonInfo.name)} &middot; ${escapeHtml(dayModeMoonInfo.zodiac)}</div>
          <div style="color:${TC.textMuted};margin-top:2px;">${escapeHtml(dayModeMoonInfo.gardenTip)}</div>
          <div style="color:#c8b4ff;margin-top:2px;">${escapeHtml(dayModeMoonInfo.element)} &middot; Beleuchtung: ${Math.round(dayModeMoonInfo.illumination * 100)}%</div>
        </div>
        ${dayModeChineseInfo ? `<div style="text-align:right;color:${TC.textDim};font-size:11px;">
          <div>${t('cal.lunarLabel', dayModeChineseInfo.month, dayModeChineseInfo.day)}</div>
          <div>${escapeHtml(dayModeChineseInfo.animal)} (${escapeHtml(dayModeChineseInfo.element)})</div>
        </div>` : ''}
      </div>` : '';
    const dayModePersianInfo = calendarMode === 'persian' ? getPersianDate(dateFromKey(timelineFocusKey)) : null;
    const dayModeJulianInfo = calendarMode === 'julian' ? getJulianDate(dateFromKey(timelineFocusKey)) : null;
    const dayModeIslamicInfo = calendarMode === 'islamic' ? getIslamicDate(dateFromKey(timelineFocusKey)) : null;
    const dayModeJewishInfo = calendarMode === 'jewish' ? getJewishDate(dateFromKey(timelineFocusKey)) : null;
    const dayModeBauernInfo = calendarMode === 'bauern' ? getBauernkalenderInfo(dateFromKey(timelineFocusKey)) : null;
    const dayModeExtraHtml = (() => {
      if (dayModePersianInfo) {
        return `<div style="display:flex;gap:12px;align-items:center;padding:10px 14px;border-radius:12px;background:rgba(255,200,100,0.06);border:1px solid rgba(255,200,100,0.12);margin-bottom:12px;font-size:12px;">
          <span style="font-size:24px;">&#9728;&#65039;</span>
          <div style="flex:1;">
            <div style="font-weight:600;color:${TC.text};">${t('calmode.persian')}</div>
            <div style="color:${TC.textMuted};margin-top:2px;">${dayModePersianInfo.day} ${escapeHtml(dayModePersianInfo.monthName)} ${dayModePersianInfo.year}</div>
          </div>
        </div>`;
      }
      if (dayModeJulianInfo) {
        return `<div style="display:flex;gap:12px;align-items:center;padding:10px 14px;border-radius:12px;background:rgba(180,200,255,0.06);border:1px solid rgba(180,200,255,0.12);margin-bottom:12px;font-size:12px;">
          <span style="font-size:24px;">&#10013;&#65039;</span>
          <div style="flex:1;">
            <div style="font-weight:600;color:${TC.text};">${t('calmode.julian')}</div>
            <div style="color:${TC.textMuted};margin-top:2px;">${t('cal.julian.dayModeLabel', dayModeJulianInfo.day, escapeHtml(dayModeJulianInfo.monthName), dayModeJulianInfo.year, dayModeJulianInfo.correction)}</div>
          </div>
        </div>`;
      }
      if (dayModeIslamicInfo) {
        return `<div style="display:flex;gap:12px;align-items:center;padding:10px 14px;border-radius:12px;background:rgba(100,200,150,0.06);border:1px solid rgba(100,200,150,0.12);margin-bottom:12px;font-size:12px;">
          <span style="font-size:24px;">&#9770;&#65039;</span>
          <div style="flex:1;">
            <div style="font-weight:600;color:${TC.text};">${t('calmode.islamic')}</div>
            <div style="color:${TC.textMuted};margin-top:2px;">${dayModeIslamicInfo.day} ${escapeHtml(dayModeIslamicInfo.monthName)} ${dayModeIslamicInfo.year} AH</div>
          </div>
        </div>`;
      }
      if (dayModeJewishInfo) {
        return `<div style="display:flex;gap:12px;align-items:center;padding:10px 14px;border-radius:12px;background:rgba(150,180,255,0.06);border:1px solid rgba(150,180,255,0.12);margin-bottom:12px;font-size:12px;">
          <span style="font-size:24px;">&#10017;&#65039;</span>
          <div style="flex:1;">
            <div style="font-weight:600;color:${TC.text};">J\u00fcdischer Kalender</div>
            <div style="color:${TC.textMuted};margin-top:2px;">${dayModeJewishInfo.day} ${escapeHtml(dayModeJewishInfo.monthName)} ${dayModeJewishInfo.year}</div>
          </div>
        </div>`;
      }
      if (dayModeBauernInfo) {
        return `<div style="display:flex;gap:12px;align-items:center;padding:10px 14px;border-radius:12px;background:rgba(200,180,100,0.06);border:1px solid rgba(200,180,100,0.12);margin-bottom:12px;font-size:12px;">
          <span style="font-size:24px;">&#127806;</span>
          <div style="flex:1;">
            <div style="font-weight:600;color:${TC.text};">Bauernkalender \u2014 ${escapeHtml(dayModeBauernInfo.phenoSeason)}</div>
            ${dayModeBauernInfo.lostag ? `<div style="color:${TC.warning};margin-top:2px;font-weight:600;">${escapeHtml(dayModeBauernInfo.name)}</div>` : ''}
            <div style="color:${TC.textMuted};margin-top:2px;font-style:italic;">${escapeHtml(dayModeBauernInfo.spruch)}</div>
          </div>
        </div>`;
      }
      return '';
    })();
    timelinePanel.innerHTML = `
      ${dayModeMoonHtml}${dayModeExtraHtml}
      <div style="display:grid; gap:12px;">
        ${sortedKeys.length ? sortedKeys.map((key) => `
          <div style="padding:12px; border-radius:12px; background:rgba(11,23,20,0.5); border:1px solid rgba(107,255,188,0.12);">
            <div style="display:flex; justify-content:space-between; gap:10px; align-items:center; margin-bottom:8px;">
              <span style="font-weight:700; color:var(--text);">${key === 'ohne-datum' ? t('timeline.noDate') : dateLabelFromKey(key)}</span>
              <span style="font-size:12px; color:${TC.textDim};">${t('popup.countEntries', grouped.get(key).length)}</span>
            </div>
            <div style="display:grid; gap:6px;">
              ${grouped.get(key).map((item) => {
                const itemPlant = plants.find(p => p.plant_id === (item.plantId || ''));
                return `
                <div class="day-detail-item">
                  <span class="cal-badge" style="background:${item.type === 'task' ? 'rgba(107,255,188,0.12)' : 'rgba(100,180,255,0.12)'}; color:${item.type === 'task' ? 'var(--accent)' : TC.info};">${item.type === 'task' ? 'Task' : 'Log'}</span>
                  <span style="flex:1;font-weight:600;">${escapeHtml(item.title)}</span>
                  <span style="font-size:12px;color:${TC.textDim};">${escapeHtml(item.subtitle)}</span>
                  <span style="font-size:12px;color:${TC.textMuted};">${escapeHtml(item.badge)}</span>
                </div>`;
              }).join('')}
            </div>
          </div>`).join('') : `<div class="empty-state">${t('timeline.emptyRange')}</div>`}
      </div>`;
  }

  // Day cell click → open day detail
  timelinePanel.querySelectorAll('[data-timeline-day]').forEach((node) => {
    node.addEventListener('click', () => {
      const dayKey = node.getAttribute('data-timeline-day');
      if (!dayKey || dayKey === 'ohne-datum') return;
      setTimelineFocusDay(dayKey);
      showDayDetail(dayKey);
      renderTimelinePanel();
    });
  });
}

function getHarvestSummary(plantId) {
  const harvestLogs = allLogs.filter(l => l.plant_id === plantId && (l.metrics?.harvest_kg || l.metrics?.harvest_count));
  const totalKg = harvestLogs.reduce((sum, l) => sum + (l.metrics?.harvest_kg || 0), 0);
  const totalCount = harvestLogs.reduce((sum, l) => sum + (l.metrics?.harvest_count || 0), 0);
  return { totalKg, totalCount, entries: harvestLogs.length };
}

function getWateringSummary(plantId, days) {
  days = days || 7;
  const cutoff = new Date(Date.now() - days * 24 * 60 * 60 * 1000);
  const waterLogs = allLogs.filter(l => l.plant_id === plantId && l.metrics?.water_liters && new Date(l.created_at) >= cutoff);
  const totalLiters = waterLogs.reduce((sum, l) => sum + (l.metrics?.water_liters || 0), 0);
  const sorted = waterLogs.slice().sort((a, b) => new Date(b.created_at) - new Date(a.created_at));
  const lastWatered = sorted.length ? sorted[0] : null;
  return { totalLiters, lastWatered, entries: waterLogs.length };
}

function renderDetailPanel() {
  const plant = selectedPlant();
  if (!plant) {
    detailPanel.innerHTML = `
      <div style="display:grid; gap:10px; color:${TC.textMuted};">
        <h2 style="margin:0; color:${TC.accent}; font-size:18px;">${t('plant.detailsTitle')}</h2>
        <div>${t('plant.detailsEmpty')}</div>
      </div>`;
    return;
  }

  const plantTasks = tasks.filter((t) => t.plant_id === plant.plant_id);
  const taskCount = plantTasks.length;
  const openTasks = plantTasks.filter((t) => t.status === 'open').length;
  const reminderEnabled = plantTasks.filter((t) => t.notification_enabled).length;
  const reminderOpen = plantTasks.filter((t) => t.status === 'open' && t.notification_enabled).length;
  const logCount = logs.filter((l) => l.plant_id === plant.plant_id).length;
  const harvestSum = getHarvestSummary(plant.plant_id);
  const waterSum = getWateringSummary(plant.plant_id, 7);
  const summary = computePlantDashboardSummary(plant);

  detailPanel.innerHTML = `
    <div style="display:grid; gap:14px;">
      <div style="display:flex; justify-content:space-between; gap:16px; align-items:flex-start; flex-wrap:wrap;">
        <div style="display:flex; gap:12px; align-items:flex-start;">
          <div style="width:18px; height:18px; border-radius:999px; margin-top:4px; background:${sanitizeColorTag(plant.color_tag)}; border:1px solid rgba(255,255,255,0.18);"></div>
          <div>
            <h2 style="margin:0; color:${TC.accent}; font-size:20px;">${escapeHtml(plant.name)}</h2>
            <div style="margin-top:4px; color:${TC.textMuted}; font-size:13px;">${phaseLabel(plant.phase)} · ${t('plant.week')} ${plant.phase_week ?? '-'} · ${plantStatusLabel(plant)}</div>
          </div>
        </div>
        <div style="display:flex; gap:8px; flex-wrap:wrap;">
          <button id="detailAdvanceWeek">${t('plant.advanceWeek')}</button>
          <button id="detailEditToggle">${editingPlantId === plant.plant_id ? t('detail.editClose') : t('detail.editOpen')}</button>
        </div>
      </div>
      ${renderCatalogInfo(plant)}
      ${plant.catalog_id ? renderPlantingCalendar(plant) : ''}
      ${renderCompanionInfo(plant)}

      <div style="display:grid; grid-template-columns:repeat(auto-fit, minmax(140px, 1fr)); gap:10px;">
        <div style="padding:12px; border-radius:12px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.12);">
          <div style="font-size:12px; color:${TC.textMuted};">${t('detail.status')}</div>
          <div style="margin-top:6px; font-weight:700;">${isPlantActive(plant) ? t('plant.active') : t('detail.readOnly')}</div>
        </div>
        <div data-stat-click="openTasks" title="${t('detail.openTasks')}" style="cursor:pointer; padding:12px; border-radius:12px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.12); transition:all 0.12s;">
          <div style="font-size:12px; color:${TC.textMuted};">${t('detail.openTasks')}</div>
          <div style="margin-top:6px; font-weight:700;">${openTasks}</div>
        </div>
        <div data-stat-click="allTasks" title="${t('detail.totalTasks')}" style="cursor:pointer; padding:12px; border-radius:12px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.12); transition:all 0.12s;">
          <div style="font-size:12px; color:${TC.textMuted};">${t('detail.totalTasks')}</div>
          <div style="margin-top:6px; font-weight:700;">${taskCount}</div>
        </div>
        <div data-stat-click="reminderEnabled" title="${t('detail.reminderActive')}" style="cursor:pointer; padding:12px; border-radius:12px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.12); transition:all 0.12s;">
          <div style="font-size:12px; color:${TC.textMuted};">${t('detail.reminderActive')}</div>
          <div style="margin-top:6px; font-weight:700;">${reminderEnabled}</div>
        </div>
        <div data-stat-click="reminderOpen" title="${t('detail.openReminders')}" style="cursor:pointer; padding:12px; border-radius:12px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.12); transition:all 0.12s;">
          <div style="font-size:12px; color:${TC.textMuted};">${t('detail.openReminders')}</div>
          <div style="margin-top:6px; font-weight:700;">${reminderOpen}</div>
        </div>
        <div data-stat-click="allLogs" title="${t('detail.totalLogs')}" style="cursor:pointer; padding:12px; border-radius:12px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.12); transition:all 0.12s;">
          <div style="font-size:12px; color:${TC.textMuted};">${t('detail.totalLogs')}</div>
          <div style="margin-top:6px; font-weight:700;">${logCount}</div>
        </div>
        ${harvestSum.entries > 0 ? `<div style="padding:12px; border-radius:12px; background:${TC.surface}; border:1px solid rgba(255,200,100,0.2);">
          <div style="font-size:12px; color:${TC.warning};">&#127806; Ernte gesamt</div>
          <div style="margin-top:6px; font-weight:700; color:${TC.warning};">${harvestSum.totalKg ? harvestSum.totalKg.toFixed(1) + ' kg' : ''}${harvestSum.totalKg && harvestSum.totalCount ? ' / ' : ''}${harvestSum.totalCount ? harvestSum.totalCount + ' Stk' : ''}</div>
          <div style="margin-top:2px; font-size:11px; color:${TC.textMuted};">${harvestSum.entries} Eintr\u00e4ge</div>
        </div>` : ''}
        ${waterSum.entries > 0 ? `<div style="padding:12px; border-radius:12px; background:${TC.surface}; border:1px solid rgba(100,180,255,0.2);">
          <div style="font-size:12px; color:${TC.info};">&#128167; Wasser (7 Tage)</div>
          <div style="margin-top:6px; font-weight:700; color:${TC.info};">${waterSum.totalLiters.toFixed(1)} L</div>
          <div style="margin-top:2px; font-size:11px; color:${TC.textMuted};">${waterSum.entries} Eintr\u00e4ge${waterSum.lastWatered ? ' · Letztes: ' + formatDate(waterSum.lastWatered.created_at) : ''}</div>
        </div>` : ''}
      </div>

      <div style="display:grid; gap:10px;">
        <div style="display:flex; justify-content:space-between; gap:12px; align-items:center; flex-wrap:wrap;">
          <div>
            <div style="font-size:12px; color:${TC.textMuted};">${t('plant.dashboardLabel')}</div>
            <div style="margin-top:4px; color:${TC.textMuted}; font-size:13px;">${t('plant.dashboardHint')}</div>
          </div>
          <div style="font-size:12px; color:${TC.textMuted};">Fokuswoche: ${weekRangeLabelFromKey(timelineFocusKey)}</div>
        </div>
        <div style="display:grid; grid-template-columns:repeat(auto-fit, minmax(180px, 1fr)); gap:10px;">
          <div style="padding:12px; border-radius:12px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.12);">
            <div style="font-size:12px; color:${TC.textMuted};">${t('detail.nextOpenTask')}</div>
            <div style="margin-top:6px; font-weight:700; color:${TC.text};">${summary.nextOpenTask ? escapeHtml(summary.nextOpenTask.title) : t('detail.nothingOpen')}</div>
            <div style="margin-top:4px; font-size:12px; color:${TC.textMuted};">${summary.nextOpenTask ? `${formatDate(summary.nextOpenTask.due_at)}` : t('detail.allDone')}</div>
          </div>
          <div style="padding:12px; border-radius:12px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.12);">
            <div style="font-size:12px; color:${TC.textMuted};">${t('detail.nextReminder')}</div>
            <div style="margin-top:6px; font-weight:700; color:${summary.nextReminder ? TC.accent : TC.text};">${summary.nextReminder ? escapeHtml(summary.nextReminder.task.title) : t('detail.noReminder')}</div>
            <div style="margin-top:4px; font-size:12px; color:${TC.textMuted};">${summary.nextReminder ? `${summary.nextReminder.bucket} · ${formatDate(summary.nextReminder.task.due_at)}` : t('detail.activateReminder')}</div>
          </div>
          <div style="padding:12px; border-radius:12px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.12);">
            <div style="font-size:12px; color:${TC.textMuted};">${t('detail.lastLog')}</div>
            <div style="margin-top:6px; font-weight:700; color:${TC.text};">${summary.lastLog ? formatDate(summary.lastLog.created_at) : t('detail.noLog')}</div>
            <div style="margin-top:4px; font-size:12px; color:${TC.textMuted};">${summary.lastLog ? escapeHtml(summary.lastLog.text || summary.lastLog.log_type || t('log.entryExists')) : t('detail.documentNext')}</div>
          </div>
          <div style="padding:12px; border-radius:12px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.12);">
            <div style="font-size:12px; color:${TC.textMuted};">${t('detail.currentPhase')}</div>
            <div style="margin-top:6px; font-weight:700; color:${TC.text};">${summary.phaseText}</div>
            <div style="margin-top:4px; font-size:12px; color:${TC.textMuted};">${t('detail.status')} ${plantStatusLabel(plant)}</div>
          </div>
          <div style="padding:12px; border-radius:12px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.12);">
            <div style="font-size:12px; color:${TC.textMuted};">${t('detail.weekProgress')}</div>
            <div style="margin-top:6px; font-weight:700; color:${TC.text};">${summary.weekProgressLabel}</div>
            <div style="margin-top:8px; height:8px; border-radius:999px; background:${TC.cardSolid}; border:1px solid rgba(${TC.accentRgb},0.08); overflow:hidden;">
              <div style="width:${summary.weekProgressPercent}%; height:100%; background:linear-gradient(90deg, #0B3D2E 0%, ${TC.accent} 100%);"></div>
            </div>
          </div>
        </div>
      </div>

      <div style="display:grid; gap:10px;">
        <div style="display:flex; justify-content:space-between; gap:12px; align-items:center; flex-wrap:wrap;">
          <div>
            <div style="font-size:12px; color:${TC.textMuted};">${t('detail.phaseHints')}</div>
            <div style="margin-top:4px; color:${TC.textMuted}; font-size:13px;">${t('detail.phaseHintsIntro', phaseLabel(plant.phase))}</div>
          </div>
          <div style="font-size:12px; color:${TC.textMuted};">${TASK_SET_LABELS[phaseSuggestedSetKeys(plant.phase)[0]] || t('detail.defaultSet')}</div>
        </div>
        <div style="display:grid; grid-template-columns:repeat(auto-fit, minmax(220px, 1fr)); gap:10px;">
          ${renderPhaseHintCards(plant)}
          ${renderPhaseHintActions(plant)}
        </div>
      </div>

      ${isPerennial(plant) ? `
      <div style="display:grid; gap:10px;">
        <div style="display:flex; justify-content:space-between; gap:12px; align-items:center; flex-wrap:wrap;">
          <div>
            <div style="font-size:12px; color:${TC.textMuted};">${t('detail.perennialSeason')}</div>
            <div style="margin-top:4px; color:${TC.textMuted}; font-size:13px;">${t('detail.seasonInfo', plantAge(plant) ?? 0, plant.plant_year || '-')}</div>
          </div>
          <button id="detailStartNewSeason" style="padding:6px 14px; border-radius:8px; border:1px solid rgba(180,120,255,0.3); background:rgba(180,120,255,0.1); color:#c4a0ff;">${t('plant.newSeasonStart')}</button>
        </div>
        <div id="seasonHistoryPanel" style="display:grid; gap:8px;">
          <div style="color:${TC.textDim}; font-size:12px;">${t('detail.seasonLoading')}</div>
        </div>
      </div>
      ` : ''}

      <div style="padding:12px; border-radius:12px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.12); color:${TC.textMuted}; font-size:13px;">
        ${t('plant.detailsIntro')}
      </div>
    </div>`;

  $("detailAdvanceWeek")?.addEventListener("click", async () => {
    const updated = await invoke("update_plant_smart", {
      payload: {
        plant_id: plant.plant_id,
        name: plant.name,
        color_tag: plant.color_tag,
        phase: plant.phase,
        phase_week: (plant.phase_week ?? 0) + 1,
        status: plantStatusValue(plant),
        ...plantExtraFields(plant),
      },
    });
    logOut(updated);
    await refreshAll();
  });

  $("detailEditToggle")?.addEventListener("click", () => {
    editingPlantId = editingPlantId === plant.plant_id ? null : plant.plant_id;
    renderPlants();
    renderDetailPanel();
  });

  // Clickable plant stats → open popup with filtered list
  detailPanel.querySelectorAll('[data-stat-click]').forEach(el => {
    el.addEventListener('mouseenter', () => { el.style.background = `rgba(${TC.accentRgb},0.12)`; });
    el.addEventListener('mouseleave', () => { el.style.background = TC.surface; });
    el.addEventListener('click', () => {
      const kind = el.dataset.statClick;
      openPlantStatPopup(plant, kind);
    });
  });

  detailPanel.querySelectorAll('[data-phase-action]').forEach((node) => {
    node.addEventListener('click', async () => {
      const action = node.getAttribute('data-phase-action');
      const setKey = node.getAttribute('data-phase-set');
      await runPhaseHintAction(action, plant.plant_id, setKey);
    });
  });

  // Perennial season panel
  if (isPerennial(plant)) {
    $("detailStartNewSeason")?.addEventListener("click", async () => {
      try {
        const season = await invoke("start_new_season", { plantId: plant.plant_id });
        logOut(season);
        await refreshAll();
      } catch (err) {
        logOut(t('season.startFailed', err));
      }
    });
    // Load seasons asynchronously
    (async () => {
      try {
        const seasons = await invoke("list_plant_seasons", { plantId: plant.plant_id });
        const panel = $("seasonHistoryPanel");
        if (!panel) return;
        if (!seasons.length) {
          panel.innerHTML = `<div style="color:${TC.textDim}; font-size:12px;">${t('season.empty')}</div>`;
          return;
        }
        panel.innerHTML = seasons.map(s => `
          <div style="padding:10px; border-radius:10px; background:${TC.surface}; border:1px solid rgba(180,120,255,0.12); display:grid; gap:6px;">
            <div style="display:flex; justify-content:space-between; align-items:center;">
              <div style="font-weight:700; color:#c4a0ff;">${t('season.label', escapeHtml(String(s.year)))}</div>
              <div style="font-size:11px; color:${TC.textMuted};">${s.started_at ? formatDate(s.started_at) : '-'} ${s.ended_at ? ' \u2192 ' + formatDate(s.ended_at) : ' (laufend)'}</div>
            </div>
            <div style="display:flex; gap:12px; flex-wrap:wrap; font-size:12px; color:${TC.textMuted};">
              ${s.harvest_kg != null ? '<span>' + t('season.harvestKg', escapeHtml(String(s.harvest_kg))) + '</span>' : ''}
              ${s.harvest_count != null ? '<span>' + t('season.harvestCount', escapeHtml(String(s.harvest_count))) + '</span>' : ''}
              <span>${t('season.pruning', s.pruning_done ? t('season.done') : t('season.pending'))}</span>
            </div>
            ${s.notes ? `<div style="font-size:12px; color:${TC.textMuted};">` + escapeHtml(s.notes) + '</div>' : ''}
            <div style="display:flex; gap:6px; justify-content:flex-end;">
              <button data-season-edit="${s.season_id}" style="font-size:11px; padding:4px 10px; border-radius:6px; border:1px solid rgba(180,120,255,0.2); background:rgba(180,120,255,0.08); color:#c4a0ff;">${t('action.edit')}</button>
            </div>
          </div>
        `).join('');
        // Season edit handlers
        panel.querySelectorAll('[data-season-edit]').forEach(btn => {
          btn.addEventListener('click', async () => {
            const seasonId = btn.getAttribute('data-season-edit');
            const season = seasons.find(s => s.season_id === seasonId);
            if (!season) return;
            const notesInput = prompt(t('season.prompt.notes'), season.notes || '');
            const harvestKgInput = prompt(t('season.prompt.harvestKg'), season.harvest_kg != null ? String(season.harvest_kg) : '');
            const harvestCountInput = prompt(t('season.prompt.harvestCount'), season.harvest_count != null ? String(season.harvest_count) : '');
            const pruningInput = confirm(t('season.prompt.pruningDone'));
            try {
              await invoke("update_plant_season", {
                payload: {
                  season_id: seasonId,
                  notes: notesInput || null,
                  harvest_kg: harvestKgInput ? parseFloat(harvestKgInput) : null,
                  harvest_count: harvestCountInput ? parseInt(harvestCountInput) : null,
                  pruning_done: pruningInput,
                }
              });
              renderDetailPanel();
            } catch (err) {
              logOut("Saison-Update fehlgeschlagen: " + err);
            }
          });
        });
      } catch (err) {
        const panel = $("seasonHistoryPanel");
        if (panel) panel.innerHTML = `<div style="color:#ffb3a7; font-size:12px;">${t('season.loadFailed')}</div>`;
      }
    })();
  }

  renderReminderPanel();
}

async function refreshStatus() {
  try {
    const status = await invoke("get_sync_status");
    let plan = "free";
    let source = status.session_active ? "account" : "guest";
    try {
      const summary = await invoke("subscription_status_smart");
      subscriptionStatusSummary = summary || null;
      plan = summary?.plan || plan;
    } catch (_) {
      subscriptionStatusSummary = null;
      if (status.session_active) {
        try {
          const me = await invoke("auth_fetch_me");
          plan = me?.plan || "free";
        } catch (_) {
          plan = "free";
          source = "guest";
        }
      }
    }
    setCurrentPlanState(plan, source);
    if (subscriptionStatusSummary?.plant_limit == null) {
      currentPlantLimit = Number.POSITIVE_INFINITY;
    } else if (Number.isFinite(subscriptionStatusSummary?.plant_limit)) {
      currentPlantLimit = subscriptionStatusSummary.plant_limit;
    }
    await loadSubscriptionHistory();
    if (statusBox) statusBox.innerHTML = `
      <div>${t('settings.status.sessionActive')}: ${status.session_active ? t('settings.status.yes') : t('settings.status.no')}</div>
      <div>${t('settings.status.userId')}: ${status.user_id ?? "-"}</div>
      <div>${t('settings.status.authMode')}: ${status.auth_mode ?? "-"}</div>
      <div>${t('settings.status.backendUrl')}: ${status.backend_url ?? "-"}</div>
      <div>${t('settings.status.dirtyRecords')}: ${status.dirty_count ?? 0}</div>
      <div>${t('settings.status.lastSync')}: ${status.last_sync_at ?? "-"}</div>
      ${renderPlanStatusLine()}
    `;
    renderSubscriptionPanel();
    renderSettingsPanel();
    applyPageView();
  } catch (err) {
    setCurrentPlanState("free", "guest");
    subscriptionStatusSummary = null;
    subscriptionHistory = [];
    subscriptionHistorySource = "local";
    if (statusBox) statusBox.textContent = t('settings.statusError', err);
    renderSubscriptionPanel();
    renderSettingsPanel();
    applyPageView();
  }
}


function filteredPlants() {
  const query = plantSearchQuery.trim().toLowerCase();
  const visible = plants.filter((plant) => {
    const matchesQuery = !query || [plant.name, plant.phase, plant.plant_id].filter(Boolean).some((v) => String(v).toLowerCase().includes(query));
    const matchesPhase = plantPhaseFilter === "all" || plant.phase === plantPhaseFilter;
    const matchesState = plantStateFilter === "all"
      || (plantStateFilter === "active" && isPlantActive(plant))
      || (plantStateFilter === "ended" && isPlantEnded(plant))
      || (plantStateFilter === "inactive" && isPlantInactive(plant))
      || (plantStateFilter === "archived" && isPlantArchived(plant));
    return matchesQuery && matchesPhase && matchesState;
  });
  return visible.sort((a, b) => {
    if (plantSort === "name_asc") return String(a.name || "").localeCompare(String(b.name || ""), "de");
    if (plantSort === "phase") return String(a.phase || "").localeCompare(String(b.phase || ""), "de");
    if (plantSort === "week_desc") return Number(b.phase_week || 0) - Number(a.phase_week || 0);
    return String(b.updated_at || b.created_at || "").localeCompare(String(a.updated_at || a.created_at || ""));
  });
}

function renderPlantFilterStatus(visibleCount, totalCount) {
  if (!plantsFilterStatus) return;
  const parts = [];
  if (plantSearchQuery.trim()) parts.push(`Suche: ${plantSearchQuery.trim()}`);
  if (plantPhaseFilter !== "all") parts.push(`Phase: ${phaseLabel(plantPhaseFilter)}`);
  if (plantStateFilter !== "all") parts.push(`Status: ${plantStateFilter === "active" ? "Aktiv" : plantStateFilter === "ended" ? "Beendet" : plantStateFilter === "inactive" ? "Inaktiv (Beendet + Archiviert)" : "Archiviert"}`);
  const filterText = parts.length ? parts.join(" · ") : t('plant.filter.none');
  plantsFilterStatus.textContent = t('plant.filter.status', visibleCount, totalCount, filterText, activePlantCount(), formatPlantLimit(currentPlantLimit), planLabel(currentPlan));
}

function renderPlantPlanHint() {
  if (!plantPlanHint) return;
  const active = activePlantCount();
  const limitText = formatPlantLimit(currentPlantLimit);
  const blocked = !canActivateMorePlants();
  const overLimit = isOverPlantLimit();
  plantPlanHint.innerHTML = blocked
    ? `<span style="color:#ffb3a7;">${overLimit ? t('plant.planHintBlockedOver', planLabel(currentPlan), active, limitText, overflowPlantCount(), overflowPlantCount() === 1 ? t('plant.plantOne') : t('plant.plantMany')) : t('plant.planHintBlocked', planLabel(currentPlan), active, limitText)}</span>`
    : `<span>${t('plant.planHintOk', planLabel(currentPlan), active, limitText, Number.isFinite(currentPlantLimit) ? remainingPlantSlots() : t('plant.moreAdditional'))}</span>`;
}

function renderPlants() {
  const current = selectedPlant();
  selectedPlantLabel.textContent = current ? t('plant.selectedLabel', current.name) : t('plant.noneSelected');
  renderPlantPlanHint();
  renderDetailPanel();
  if (!plants.length) {
    renderPlantFilterStatus(0, 0);
    plantsList.innerHTML = `<div class="empty-state-card">
      <div class="empty-state-title">${t('plant.emptyTitle')}</div>
      <div class="empty-state-hint">${t('plant.emptyHint')}</div>
      <div class="empty-state-actions">
        <button type="button" id="emptyPlantsCreate" class="btn btn--primary btn--sm">${t('plant.emptyCreate')}</button>
        <button type="button" id="emptyPlantsBilling" class="btn btn--sm">${t('plant.emptyBilling')}</button>
      </div>
      <div style="margin-top:12px; font-size:12px; color:${TC.textDim};">${t('plant.emptyTip')}</div>
    </div>`;
    $("emptyPlantsCreate")?.addEventListener("click", () => { switchPageView("plants"); $("plantName")?.focus(); });
    $("emptyPlantsBilling")?.addEventListener("click", () => switchPageView("billing"));
    renderDetailPanel();
    renderTimelinePanel();
    renderPhaseTaskSets();
    return;
  }
  const visiblePlants = filteredPlants();
  renderPlantFilterStatus(visiblePlants.length, plants.length);
  if (!visiblePlants.length) {
    plantsList.innerHTML = `<div class="empty-state-card">
      <div class="empty-state-title">${t('plant.noHits')}</div>
      <div class="empty-state-hint">${t('plant.noHitsHint')}</div>
      <div class="empty-state-actions"><button type="button" id="resetPlantFilters" class="btn btn--sm">${t('plants.resetFilters')}</button></div>
    </div>`;
    $("resetPlantFilters")?.addEventListener("click", () => { plantSearchQuery = ""; plantPhaseFilter = "all"; plantStateFilter = "all"; plantSort = "updated_desc"; applyPlantFilterPrefsToControls(); persistPlantFilterPrefs(); renderPlants(); });
    renderTimelinePanel();
    renderPhaseTaskSets();
    return;
  }
  const grandfatheredIds = grandfatheredActivePlantIds();
  plantsList.innerHTML = visiblePlants.map((plant) => {
    const isEditing = editingPlantId === plant.plant_id;
    const isGrandfathered = grandfatheredIds.has(plant.plant_id);
    const primaryStatusTone = plantStatusTone(plant);
    const statusBadges = [
      `<span style="font-size:11px; padding:4px 8px; border-radius:999px; background:${primaryStatusTone.bg}; border:1px solid ${primaryStatusTone.border}; color:${primaryStatusTone.color};">${plantStatusLabel(plant).toLowerCase()}</span>`,
      isGrandfathered
        ? `<span style="font-size:11px; padding:4px 8px; border-radius:999px; background:rgba(255,120,120,0.12); border:1px solid rgba(255,120,120,0.24); color:#ffb3a7;">${t('account.overLimitContinues')}</span>`
        : '',
      isPlantActive(plant) && !isGrandfathered && Number.isFinite(currentPlantLimit)
        ? `<span style="font-size:11px; padding:4px 8px; border-radius:999px; background:rgba(159,215,255,0.12); border:1px solid rgba(159,215,255,0.24); color:#9fd7ff;">${t('account.withinPlanLimit')}</span>`
        : '',
    ].filter(Boolean).join(' ');
    return `
    <div class="entity-row" data-plant-id="${plant.plant_id}" style="padding:12px; border-radius:14px; border:1px solid ${isGrandfathered ? 'rgba(255,120,120,0.32)' : selectedPlantId === plant.plant_id ? TC.accent : `rgba(${TC.accentRgb},0.15)`}; background:${selectedPlantId === plant.plant_id ? '#10231d' : isGrandfathered ? '#1a1212' : TC.cardSolid}; box-shadow:${isGrandfathered ? '0 0 0 1px rgba(255,120,120,0.08), 0 0 24px rgba(255,120,120,0.06)' : 'none'};">
      <div style="display:flex; justify-content:space-between; gap:12px; align-items:flex-start;">
        <div style="display:flex; gap:10px; align-items:flex-start; flex:1;">
          <div style="width:14px; height:14px; border-radius:999px; margin-top:4px; background:${sanitizeColorTag(plant.color_tag)}; border:1px solid rgba(255,255,255,0.15);"></div>
          <div style="flex:1;">
            <div style="font-weight:700; display:flex; gap:8px; flex-wrap:wrap; align-items:center;">${(() => { const ce = catalogById(plant.catalog_id); return ce && ce.emoji ? ce.emoji + ' ' : ''; })()}${escapeHtml(plant.name)}${isPerennial(plant) ? `<span style="font-size:10px; padding:2px 6px; border-radius:6px; background:rgba(180,120,255,0.15); border:1px solid rgba(180,120,255,0.3); color:#c4a0ff; font-weight:500;">${t('plant.perennial')}</span>` : ''}</div>
            <div style="font-size:12px; color:${TC.textMuted}; margin-top:4px;">${phaseLabel(plant.phase)} · ${t('plant.week')} ${plant.phase_week ?? '-'} · ${plantStatusLabel(plant)}${isPerennial(plant) ? ` · ${t('plant.yearsOld', plantAge(plant) ?? 0)}` : ''}${isPerennial(plant) && plant.plant_year ? ` · ${t('plant.season', plant.plant_year)}` : ''}${isPlantEnded(plant) ? ' · ' + t('plant.notCountingLimit') : isPlantArchived(plant) ? ' · ' + t('plant.reactivatable') : ''}${(() => { const loc = locations.find(l => l.location_id === plant.location_id); return loc ? ` · <span style="color:${TC.accent};">${escapeHtml(loc.name)}</span>` : ''; })()}</div>
            <div style="display:flex; gap:6px; flex-wrap:wrap; margin-top:8px;">${statusBadges}</div>
            ${renderCompanionInfo(plant)}
            ${plant.catalog_id ? renderPlantingCalendar(plant) : ''}
            ${plant.catalog_id ? `<div style="display:flex;gap:4px;flex-wrap:wrap;margin-top:6px;">${renderSafetyBadges(plant)} ${renderHarvestBadge(plant)}</div>` : ''}
            ${(() => { const watering = isPlantActive(plant) ? calculateWateringNeed(plant) : null; if (!watering) return ''; const wColor = watering.urgency === 'overdue' ? TC.danger : watering.urgency === 'soon' ? TC.warning : TC.textDim; return '<div style="margin-top:6px;font-size:11px;color:' + wColor + ';">' + (watering.urgency === 'overdue' ? '\u{1F534}' : watering.urgency === 'soon' ? '\u{1F7E1}' : '\u{1F7E2}') + ' ' + escapeHtml(watering.reason) + '</div>'; })()}
            ${isGrandfathered ? `<div style="margin-top:8px; font-size:12px; color:#ffb3a7;">${t('plant.grandfatheredHint')}</div>` : ''}
          </div>
        </div>
        <div style="display:flex; gap:6px; flex-wrap:wrap; justify-content:flex-end;">
          <button data-action="select" data-plant-id="${plant.plant_id}">${t('plant.openBtn')}</button>
          <button data-action="advance" data-plant-id="${plant.plant_id}">${t('plant.advanceWeek')}</button>
          <button data-action="prepare-task" data-plant-id="${plant.plant_id}">${t('task.title')}</button>
          <button data-action="prepare-log" data-plant-id="${plant.plant_id}">Log</button>
          <button data-action="toggle-archive" data-plant-id="${plant.plant_id}" ${isPlantArchived(plant) && !canActivateMorePlants() ? 'data-blocked-reactivate="1"' : ''}>${isPlantArchived(plant) ? t('plant.reactivate') : t('plant.archiveBtn')}</button>
          ${isPerennial(plant) ? `<button data-action="start-season" data-plant-id="${plant.plant_id}">${t('plant.newSeason')}</button>` : ''}
          <button data-action="end-run" data-plant-id="${plant.plant_id}" ${!isPlantActive(plant) ? "disabled" : ""}>${t('plant.endRun')}</button>
          <button data-action="edit" data-plant-id="${plant.plant_id}">${isEditing ? t('action.close') : t('action.edit')}</button>
          <button data-action="delete" data-plant-id="${plant.plant_id}">${t('action.delete')}</button>
        </div>
      </div>
      ${isEditing ? `
        <div style="margin-top:12px; padding:12px; border-radius:12px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.12); display:grid; gap:10px;">
          <input data-edit-field="name" data-plant-id="${plant.plant_id}" value="${escapeHtml(plant.name)}" style="width:100%; padding:10px; border-radius:10px; border:1px solid #1f4036; background:${TC.inputBg}; color:${TC.text};" />
          <div style="display:grid; grid-template-columns:1fr 1fr 1fr; gap:10px;">
            <input data-edit-field="color" data-plant-id="${plant.plant_id}" type="color" value="${sanitizeColorTag(plant.color_tag)}" style="width:100%; height:44px; border-radius:10px; border:1px solid #1f4036; background:${TC.inputBg};" />
            <select data-edit-field="phase" data-plant-id="${plant.plant_id}" style="width:100%; padding:10px; border-radius:10px; border:1px solid #1f4036; background:${TC.inputBg}; color:${TC.text};">
              <option value="seed" ${plant.phase === 'seed' ? 'selected' : ''}>${t('phase.seed')}</option>
              <option value="veg" ${plant.phase === 'veg' ? 'selected' : ''}>${t('phase.veg')}</option>
              <option value="flower" ${plant.phase === 'flower' ? 'selected' : ''}>${t('phase.flower')}</option>
              <option value="harvest" ${plant.phase === 'harvest' ? 'selected' : ''}>${t('phase.harvest')}</option>
              <option value="dry" ${plant.phase === 'dry' ? 'selected' : ''}>${t('phase.dry')}</option>
              <option value="cure" ${plant.phase === 'cure' ? 'selected' : ''}>${t('phase.cure')}</option>
              <option value="dormant" ${plant.phase === 'dormant' ? 'selected' : ''}>${t('phase.dormant')}</option>
              <option value="custom" ${plant.phase === 'custom' ? 'selected' : ''}>${t('phase.custom')}</option>
            </select>
            <input data-edit-field="phase_week" data-plant-id="${plant.plant_id}" type="number" min="0" value="${plant.phase_week ?? 0}" style="width:100%; padding:10px; border-radius:10px; border:1px solid #1f4036; background:${TC.inputBg}; color:${TC.text};" />
          </div>
          <select data-edit-field="location" data-plant-id="${plant.plant_id}" style="width:100%; padding:10px; border-radius:10px; border:1px solid #1f4036; background:${TC.inputBg}; color:${TC.text};">
            <option value="">${t('plant.noLocation')}</option>
            ${locations.map(l => `<option value="${l.location_id}" ${plant.location_id === l.location_id ? 'selected' : ''}>${escapeHtml(l.name)}</option>`).join('')}
          </select>
          <select data-edit-field="lifecycle" data-plant-id="${plant.plant_id}" style="width:100%; padding:10px; border-radius:10px; border:1px solid #1f4036; background:${TC.inputBg}; color:${TC.text};">
            <option value="annual" ${plant.lifecycle !== 'perennial' ? 'selected' : ''}>${t('plant.annual')}</option>
            <option value="perennial" ${plant.lifecycle === 'perennial' ? 'selected' : ''}>${t('plant.perennial')}</option>
          </select>
          <div style="display:grid; grid-template-columns:1fr 1fr; gap:10px;">
            <input data-edit-field="pot_size" data-plant-id="${plant.plant_id}" type="number" step="0.5" placeholder="${t('plant.potSize')}" value="${plant.pot_size_liters ?? ''}" style="width:100%; padding:10px; border-radius:10px; border:1px solid #1f4036; background:${TC.inputBg}; color:${TC.text};" />
            <select data-edit-field="soil_type" data-plant-id="${plant.plant_id}" style="width:100%; padding:10px; border-radius:10px; border:1px solid #1f4036; background:${TC.inputBg}; color:${TC.text};">
              <option value="">${t('soil.default')}</option>
              <option value="erde" ${plant.soil_type === 'erde' ? 'selected' : ''}>${t('soil.erde')}</option>
              <option value="kokos" ${plant.soil_type === 'kokos' ? 'selected' : ''}>${t('soil.kokos')}</option>
              <option value="hydro" ${plant.soil_type === 'hydro' ? 'selected' : ''}>${t('soil.hydro')}</option>
              <option value="steinwolle" ${plant.soil_type === 'steinwolle' ? 'selected' : ''}>${t('soil.steinwolle')}</option>
              <option value="perlite" ${plant.soil_type === 'perlite' ? 'selected' : ''}>${t('soil.perlite')}</option>
            </select>
          </div>
          <div style="display:flex; gap:8px; justify-content:flex-end;">
            <button data-action="save-edit" data-plant-id="${plant.plant_id}">${t('action.save')}</button>
            <button data-action="cancel-edit" data-plant-id="${plant.plant_id}">${t('action.cancel')}</button>
          </div>
        </div>` : ''}
    </div>`;
  }).join("");

  plantsList.querySelectorAll("button").forEach((btn) => {
    btn.addEventListener("click", async () => {
      const plantId = btn.dataset.plantId;
      const action = btn.dataset.action;
      if (action === "select") {
        selectedPlantId = plantId;
        renderPlants();
        await loadTasks();
        await loadLogs();
        renderSensorWidget();
        await loadPlantPhotos();
        renderPhotoGallery();
        renderGrowthAnalytics();
        renderCareInsights();
        return;
      }
      if (action === "edit") {
        editingPlantId = editingPlantId === plantId ? null : plantId;
        renderPlants();
        return;
      }
      if (action === "cancel-edit") {
        editingPlantId = null;
        renderPlants();
        return;
      }
      if (action === "start-season") {
        try {
          const season = await invoke("start_new_season", { plantId });
          logOut(season);
          await refreshAll();
        } catch (err) {
          logOut(t('season.startFailed', err));
        }
        return;
      }
      if (action === "prepare-task") {
        await prepareTaskFromHome(plantId, {});
        return;
      }
      if (action === "prepare-log") {
        await prepareLogFromHome(plantId, {});
        return;
      }
      try {
        if (action === "advance") {
          const plant = plants.find((p) => p.plant_id === plantId);
          if (!plant) return;
          const updated = await invoke("update_plant_smart", {
            payload: {
              plant_id: plant.plant_id,
              name: plant.name,
              color_tag: plant.color_tag,
              phase: plant.phase,
              phase_week: (plant.phase_week ?? 0) + 1,
              status: plantStatusValue(plant),
              ...plantExtraFields(plant),
            },
          });
          logOut(updated);
          await refreshAll();
          return;
        }
        if (action === "end-run") {
          const plant = plants.find((p) => p.plant_id === plantId);
          if (!plant) return;
          const updated = await invoke("update_plant_smart", {
            payload: {
              plant_id: plant.plant_id,
              name: plant.name,
              color_tag: plant.color_tag,
              phase: plant.phase,
              phase_week: plant.phase_week,
              status: "ended",
              ...plantExtraFields(plant),
            },
          });
          logOut(updated);
          await refreshAll();
          return;
        }
        if (action === "toggle-archive") {
          const plant = plants.find((p) => p.plant_id === plantId);
          if (!plant) return;
          const archived = !isPlantArchived(plant);
          if (!archived && !canActivateMorePlants()) {
            logOut(planLimitMessage("reactivate"));
            return;
          }
          const nextStatus = archived ? 'archived' : 'active';
          const updated = await invoke("update_plant_smart", {
            payload: {
              plant_id: plant.plant_id,
              name: plant.name,
              color_tag: plant.color_tag,
              phase: plant.phase,
              phase_week: plant.phase_week ?? 0,
              status: nextStatus,
              ...plantExtraFields(plant),
            },
          });
          if (selectedPlantId === plantId && archived) {
            selectedPlantId = null;
          }
          logOut(updated);
          await refreshAll();
          return;
        }
        if (action === "save-edit") {
          const plant = plants.find((p) => p.plant_id === plantId);
          if (!plant) return;
          const nameEl = plantsList.querySelector(`[data-edit-field="name"][data-plant-id="${plantId}"]`);
          const colorEl = plantsList.querySelector(`[data-edit-field="color"][data-plant-id="${plantId}"]`);
          const phaseEl = plantsList.querySelector(`[data-edit-field="phase"][data-plant-id="${plantId}"]`);
          const phaseWeekEl = plantsList.querySelector(`[data-edit-field="phase_week"][data-plant-id="${plantId}"]`);
          const locationEl = plantsList.querySelector(`[data-edit-field="location"][data-plant-id="${plantId}"]`);
          const lifecycleEl = plantsList.querySelector(`[data-edit-field="lifecycle"][data-plant-id="${plantId}"]`);
          const potSizeEl = plantsList.querySelector(`[data-edit-field="pot_size"][data-plant-id="${plantId}"]`);
          const soilTypeEl = plantsList.querySelector(`[data-edit-field="soil_type"][data-plant-id="${plantId}"]`);
          const name = nameEl?.value?.trim() || plant.name;
          const color = colorEl?.value || plant.color_tag;
          const phase = phaseEl?.value || plant.phase;
          const phaseWeekRaw = phaseWeekEl?.value;
          const phaseWeek = phaseWeekRaw === "" || phaseWeekRaw == null ? null : Number(phaseWeekRaw);
          const locationId = locationEl?.value || null;
          const lifecycle = lifecycleEl?.value || plant.lifecycle || 'annual';
          const potSizeRaw = potSizeEl?.value?.trim();
          const potSize = potSizeRaw ? Number(potSizeRaw) : null;
          const soilType = soilTypeEl?.value || null;
          const updated = await invoke("update_plant_smart", {
            payload: {
              plant_id: plant.plant_id,
              name,
              color_tag: color,
              phase,
              phase_week: phaseWeek,
              status: plantStatusValue(plant),
              location_id: locationId,
              lifecycle,
              pot_size_liters: potSize,
              soil_type: soilType,
            },
          });
          editingPlantId = null;
          logOut(updated);
          await refreshAll();
          return;
        }
        if (action === "delete") {
          await invoke("delete_plant_smart", { plantId });
          if (selectedPlantId === plantId) selectedPlantId = null;
          if (editingPlantId === plantId) editingPlantId = null;
          await refreshAll();
        }
      } catch (err) {
        logOut(t('plant.action.failed', (err?.message || err)));
      }
    });
  });
}

function renderTasks() {
  const visible = selectedPlantId ? tasks.filter((task) => task.plant_id === selectedPlantId) : [];
  if (!selectedPlantId) {
    tasksList.innerHTML = `<div style="color:${TC.textMuted};">${t('plant.chooseFirst')}</div>`;
    renderDetailPanel();
    renderReminderPanel();
    renderTimelinePanel();
    renderPhaseTaskSets();
    return;
  }
  if (!visible.length) {
    tasksList.innerHTML = `<div style="color:${TC.textMuted};">${t('task.emptyForPlant')}</div>`;
    renderDetailPanel();
    renderReminderPanel();
    renderTimelinePanel();
    renderPhaseTaskSets();
    return;
  }
  tasksList.innerHTML = visible.map((task) => {
    const isEditing = editingTaskId === task.task_id;
    return `
    <div class="entity-row" style="padding:12px; border-radius:14px; border:1px solid rgba(${TC.accentRgb},0.15); background:${TC.cardSolid};">
      <div style="display:flex; justify-content:space-between; gap:8px;">
        <div style="flex:1;">
          <div style="font-weight:700;">${escapeHtml(task.title)}</div>
          <div style="font-size:12px; color:${TC.textMuted};">${escapeHtml(task.category)} · ${escapeHtml(task.status)} · ${t('task.dueOn', formatDate(task.due_at))}${task.repeat_interval_hours ? ` · alle ${task.repeat_interval_hours}h` : ''}${task.repeat_until ? ` bis ${new Date(task.repeat_until).toLocaleDateString(localeTag())}` : ''}</div>
        </div>
        <div style="display:flex; gap:6px; flex-wrap:wrap; justify-content:flex-end;">
          <button data-action="done" data-task-id="${task.task_id}">${t('task.doneBtn')}</button>
          <button data-action="skip" data-task-id="${task.task_id}">${t('task.skipBtn')}</button>
          <button data-action="edit" data-task-id="${task.task_id}">${isEditing ? t('action.close') : t('action.edit')}</button>
          ${task.repeat_interval_hours && task.repeat_interval_hours > 0 ? `<button data-action="stop-repeat" data-task-id="${task.task_id}" title="${t('task.stopRepeatTitle')}">${t('task.stopRepeat')}</button>` : ''}
          <button data-action="delete" data-task-id="${task.task_id}">${t('action.delete')}</button>
        </div>
      </div>
      ${isEditing ? `
      <div style="margin-top:12px; padding:12px; border-radius:12px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.12); display:grid; gap:10px;">
        <input data-task-edit="title" data-task-id="${task.task_id}" value="${escapeHtml(task.title)}" style="width:100%; padding:10px; border-radius:10px; border:1px solid #1f4036; background:${TC.inputBg}; color:${TC.text};" />
        <div style="display:grid; grid-template-columns:1fr 1fr 1fr; gap:10px;">
          <select data-task-edit="category" data-task-id="${task.task_id}" style="width:100%; padding:10px; border-radius:10px; border:1px solid #1f4036; background:${TC.inputBg}; color:${TC.text};">
            <option value="water" ${task.category === 'water' ? 'selected' : ''}>${t('cat.water')}</option>
            <option value="feed" ${task.category === 'feed' ? 'selected' : ''}>${t('cat.feed')}</option>
            <option value="check" ${task.category === 'check' ? 'selected' : ''}>${t('cat.check')}</option>
            <option value="train" ${task.category === 'train' ? 'selected' : ''}>${t('cat.train')}</option>
            <option value="note" ${task.category === 'note' ? 'selected' : ''}>${t('cat.note')}</option>
          </select>
          <input data-task-edit="repeat" data-task-id="${task.task_id}" type="number" min="0" value="${task.repeat_interval_hours ?? ''}" placeholder="${t('task.repeatPlaceholder')}" style="width:100%; padding:10px; border-radius:10px; border:1px solid #1f4036; background:${TC.inputBg}; color:${TC.text};" />
          <select data-task-edit="status" data-task-id="${task.task_id}" style="width:100%; padding:10px; border-radius:10px; border:1px solid #1f4036; background:${TC.inputBg}; color:${TC.text};">
            <option value="open" ${task.status === 'open' ? 'selected' : ''}>${t('task.open')}</option>
            <option value="done" ${task.status === 'done' ? 'selected' : ''}>${t('task.done')}</option>
            <option value="skipped" ${task.status === 'skipped' ? 'selected' : ''}>${t('task.skipped')}</option>
          </select>
        </div>
        <input data-task-edit="due_at" data-task-id="${task.task_id}" type="datetime-local" value="${toDatetimeLocalValue(task.due_at)}" style="width:100%; padding:10px; border-radius:10px; border:1px solid #1f4036; background:${TC.inputBg}; color:${TC.text};" />
        <label style="display:flex; align-items:center; gap:8px; color:${TC.textMuted}; font-size:13px;">
          <input data-task-edit="notification_enabled" data-task-id="${task.task_id}" type="checkbox" ${task.notification_enabled ? 'checked' : ''} />
          Reminder aktiv
        </label>
        <div style="display:flex; gap:8px; justify-content:flex-end;">
          <button data-action="save-edit" data-task-id="${task.task_id}">${t('action.save')}</button>
          <button data-action="cancel-edit" data-task-id="${task.task_id}">${t('action.cancel')}</button>
        </div>
      </div>` : ''}
    </div>`;
  }).join("");
  renderDetailPanel();
  renderReminderPanel();
  renderTimelinePanel();

  tasksList.querySelectorAll("button").forEach((btn) => {
    btn.addEventListener("click", async () => {
      const taskId = btn.dataset.taskId;
      const action = btn.dataset.action;
      const task = visible.find((item) => item.task_id === taskId);
      if (!task) return;
      if (action === "edit") {
        editingTaskId = editingTaskId === taskId ? null : taskId;
        renderTasks();
        renderDetailPanel();
        return;
      }
      if (action === "cancel-edit") {
        editingTaskId = null;
        renderTasks();
        return;
      }
      try {
        if (action === "delete") {
          if (!confirm(t('task.confirmDelete', task.title))) return;
          await invoke("delete_task_smart", { taskId });
        } else if (action === "stop-repeat") {
          if (!confirm(t('task.confirmDeleteRepeating', task.title))) return;
          // First stop recurrence, then delete the root task so no virtual instances remain.
          try { await invoke("update_task_smart", { payload: { task_id: taskId, repeat_interval_hours: 0, status: "done" } }); } catch (_) {}
          await invoke("delete_task_smart", { taskId });
        } else if (action === "save-edit") {
          const title = tasksList.querySelector(`[data-task-edit="title"][data-task-id="${taskId}"]`)?.value?.trim() || task.title;
          const category = tasksList.querySelector(`[data-task-edit="category"][data-task-id="${taskId}"]`)?.value || task.category;
          const status = tasksList.querySelector(`[data-task-edit="status"][data-task-id="${taskId}"]`)?.value || task.status;
          const repeatRaw = tasksList.querySelector(`[data-task-edit="repeat"][data-task-id="${taskId}"]`)?.value;
          const repeat = repeatRaw === "" || repeatRaw == null ? null : Number(repeatRaw);
          const dueAtRaw = tasksList.querySelector(`[data-task-edit="due_at"][data-task-id="${taskId}"]`)?.value;
          const due_at = datetimeLocalToIso(dueAtRaw) || task.due_at;
          const notification_enabled = !!tasksList.querySelector(`[data-task-edit="notification_enabled"][data-task-id="${taskId}"]`)?.checked;
          await invoke("update_task_smart", { payload: { task_id: taskId, title, category, due_at, repeat_interval_hours: repeat, status, notification_enabled } });
          editingTaskId = null;
        } else {
          if (action === "done" && !guardTaskDone(taskId)) {
            // Recently processed, skip duplicate click
            return;
          }
          const result = await invoke("update_task_status_smart", { payload: { task_id: taskId, status: action === "done" ? "done" : "skipped" } });
          playNotificationSound('success');
          // Auto-create watering log when completing a water task (idempotent)
          if (action === "done" && task.category === "water" && !hasRecentWateringLog(task.plant_id, task.title)) {
            try {
              await invoke("create_log_smart", { payload: {
                plant_id: task.plant_id,
                log_type: "watering",
                text: t('log.waterDoneAuto', task.title),
                water_liters: null, water_method: "giessen",
                ph: null, ec: null, temp_c: null, rh: null,
                harvest_kg: null, harvest_count: null, harvest_quality: null,
                created_at: new Date().toISOString(),
              }});
            } catch (_) {} // Log creation is best-effort
          }
        }
        await refreshAll({ autoReminderReason: "task_action" });
      } catch (err) {
        console.error("Task-Aktion fehlgeschlagen:", action, err);
        logOut("Task-Aktion fehlgeschlagen: " + (err?.message || err));
        if (action === "delete" || action === "stop-repeat") {
          alert(t('action.genericError', (err?.message || err)));
        }
      }
    });
  });
}

function renderLogs() {
  const visible = selectedPlantId ? logs.filter((log) => log.plant_id === selectedPlantId) : [];
  if (!selectedPlantId) {
    logsList.innerHTML = `<div style="color:${TC.textMuted};">${t('plant.chooseFirst')}</div>`;
    renderDetailPanel();
    renderReminderPanel();
    renderTimelinePanel();
    renderPhaseTaskSets();
    return;
  }
  if (!visible.length) {
    logsList.innerHTML = `<div style="color:${TC.textMuted};">${t('log.emptyForPlant')}</div>`;
    renderDetailPanel();
    renderReminderPanel();
    renderTimelinePanel();
    renderPhaseTaskSets();
    return;
  }
  logsList.innerHTML = visible.map((log) => {
    const isEditing = editingLogId === log.log_id;
    const harvestInfo = (log.metrics?.harvest_kg || log.metrics?.harvest_count)
      ? `<span class="cal-badge" style="background:rgba(255,200,100,0.15);color:${TC.warning};">&#127806; ${log.metrics.harvest_kg ? log.metrics.harvest_kg + ' kg' : ''} ${log.metrics.harvest_count ? log.metrics.harvest_count + ' Stk' : ''}</span>`
      : '';
    const qualityInfo = log.metrics?.harvest_quality
      ? `<span class="cal-badge" style="background:rgba(255,200,100,0.10);color:#d4a84b;">${escapeHtml(log.metrics.harvest_quality)}</span>`
      : '';
    const waterInfo = log.metrics?.water_liters
      ? `<span class="cal-badge" style="background:rgba(100,180,255,0.15);color:${TC.info};">&#128167; ${log.metrics.water_liters} L${log.metrics.water_method ? ' (' + escapeHtml(log.metrics.water_method) + ')' : ''}</span>`
      : '';
    return `
    <div class="entity-row" style="padding:12px; border-radius:14px; border:1px solid rgba(${TC.accentRgb},0.15); background:${TC.cardSolid};">
      <div style="display:flex; justify-content:space-between; gap:8px;">
        <div style="flex:1;">
          <div style="font-weight:700;">${escapeHtml(log.log_type)}</div>
          <div style="font-size:12px; color:${TC.textMuted};">${formatDate(log.created_at)}</div>
          <div style="margin-top:8px;">${escapeHtml(log.text ?? "-")}</div>
          ${log.metrics ? `<div style="margin-top:6px; font-size:12px; color:${TC.textMuted};">pH ${log.metrics.ph ?? '-'} · EC ${log.metrics.ec ?? '-'} · Temp ${log.metrics.temp_c ?? '-'} · RH ${log.metrics.rh ?? '-'}</div>` : ""}
          ${(harvestInfo || qualityInfo || waterInfo) ? `<div style="margin-top:6px; display:flex; gap:6px; flex-wrap:wrap;">${harvestInfo}${qualityInfo}${waterInfo}</div>` : ''}
        </div>
        <div style="display:flex; gap:6px; flex-wrap:wrap;"><button data-action="edit" data-log-id="${log.log_id}">${isEditing ? t('action.close') : t('action.edit')}</button><button data-action="delete" data-log-id="${log.log_id}">${t('action.delete')}</button></div>
      </div>
      ${isEditing ? `
      <div style="margin-top:12px; padding:12px; border-radius:12px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.12); display:grid; gap:10px;">
        <select data-log-edit="log_type" data-log-id="${log.log_id}" style="width:100%; padding:10px; border-radius:10px; border:1px solid #1f4036; background:${TC.inputBg}; color:${TC.text};">
          <option value="note" ${log.log_type === 'note' ? 'selected' : ''}>${t('log.note')}</option>
          <option value="measurement" ${log.log_type === 'measurement' ? 'selected' : ''}>${t('log.measurement')}</option>
          <option value="action" ${log.log_type === 'action' ? 'selected' : ''}>${t('log.action')}</option>
          <option value="photo" ${log.log_type === 'photo' ? 'selected' : ''}>${t('log.photo')}</option>
          <option value="harvest" ${log.log_type === 'harvest' ? 'selected' : ''}>${t('log.harvest')}</option>
          <option value="watering" ${log.log_type === 'watering' ? 'selected' : ''}>${t('log.watering')}</option>
        </select>
        <textarea data-log-edit="text" data-log-id="${log.log_id}" rows="3" style="width:100%; padding:10px; border-radius:10px; border:1px solid #1f4036; background:${TC.inputBg}; color:${TC.text};">${escapeHtml(log.text ?? '')}</textarea>
        <div style="display:grid; grid-template-columns:1fr 1fr; gap:10px;">
          <input data-log-edit="ph" data-log-id="${log.log_id}" type="number" step="0.1" value="${log.metrics?.ph ?? ''}" placeholder="pH" style="padding:10px; border-radius:10px; border:1px solid #1f4036; background:${TC.inputBg}; color:${TC.text};" />
          <input data-log-edit="ec" data-log-id="${log.log_id}" type="number" step="0.1" value="${log.metrics?.ec ?? ''}" placeholder="EC" style="padding:10px; border-radius:10px; border:1px solid #1f4036; background:${TC.inputBg}; color:${TC.text};" />
        </div>
        <div style="display:grid; grid-template-columns:1fr 1fr; gap:10px;">
          <input data-log-edit="temp_c" data-log-id="${log.log_id}" type="number" step="0.1" value="${log.metrics?.temp_c ?? ''}" placeholder="Temp \u00b0C" style="padding:10px; border-radius:10px; border:1px solid #1f4036; background:${TC.inputBg}; color:${TC.text};" />
          <input data-log-edit="rh" data-log-id="${log.log_id}" type="number" step="0.1" value="${log.metrics?.rh ?? ''}" placeholder="RH %" style="padding:10px; border-radius:10px; border:1px solid #1f4036; background:${TC.inputBg}; color:${TC.text};" />
        </div>
        <div style="display:grid; grid-template-columns:1fr 1fr; gap:10px;">
          <input data-log-edit="harvest_kg" data-log-id="${log.log_id}" type="number" step="0.1" value="${log.metrics?.harvest_kg ?? ''}" placeholder="${t('log.harvest.kgPh')}" style="padding:10px; border-radius:10px; border:1px solid #1f4036; background:${TC.inputBg}; color:${TC.text};" />
          <input data-log-edit="harvest_count" data-log-id="${log.log_id}" type="number" value="${log.metrics?.harvest_count ?? ''}" placeholder="${t('log.harvest.countPh')}" style="padding:10px; border-radius:10px; border:1px solid #1f4036; background:${TC.inputBg}; color:${TC.text};" />
        </div>
        <div style="display:grid; grid-template-columns:1fr 1fr; gap:10px;">
          <select data-log-edit="harvest_quality" data-log-id="${log.log_id}" style="padding:10px; border-radius:10px; border:1px solid #1f4036; background:${TC.inputBg}; color:${TC.text};">
            <option value="">${t('harvest.quality.placeholder')}</option>
            <option value="excellent" ${log.metrics?.harvest_quality === 'excellent' ? 'selected' : ''}>${t('harvest.excellent')}</option>
            <option value="good" ${log.metrics?.harvest_quality === 'good' ? 'selected' : ''}>${t('harvest.good')}</option>
            <option value="average" ${log.metrics?.harvest_quality === 'average' ? 'selected' : ''}>${t('harvest.average')}</option>
            <option value="poor" ${log.metrics?.harvest_quality === 'poor' ? 'selected' : ''}>${t('harvest.poor')}</option>
          </select>
          <input data-log-edit="water_liters" data-log-id="${log.log_id}" type="number" step="0.1" value="${log.metrics?.water_liters ?? ''}" placeholder="${t('water.liters')}" style="padding:10px; border-radius:10px; border:1px solid #1f4036; background:${TC.inputBg}; color:${TC.text};" />
        </div>
        <select data-log-edit="water_method" data-log-id="${log.log_id}" style="width:100%; padding:10px; border-radius:10px; border:1px solid #1f4036; background:${TC.inputBg}; color:${TC.text};">
          <option value="">${t('water.methodPlaceholder')}</option>
          <option value="giessen" ${log.metrics?.water_method === 'giessen' ? 'selected' : ''}>${t('water.pour')}</option>
          <option value="tropf" ${log.metrics?.water_method === 'tropf' ? 'selected' : ''}>${t('water.drip')}</option>
          <option value="spruehen" ${log.metrics?.water_method === 'spruehen' ? 'selected' : ''}>${t('water.spray')}</option>
          <option value="regen" ${log.metrics?.water_method === 'regen' ? 'selected' : ''}>${t('water.rain')}</option>
        </select>
        <div style="display:flex; gap:8px; justify-content:flex-end;">
          <button data-action="save-edit" data-log-id="${log.log_id}">${t('action.save')}</button>
          <button data-action="cancel-edit" data-log-id="${log.log_id}">${t('action.cancel')}</button>
        </div>
      </div>` : ''}
    </div>`;
  }).join("");
  renderDetailPanel();
  renderReminderPanel();
  renderTimelinePanel();

  logsList.querySelectorAll("button").forEach((btn) => {
    btn.addEventListener("click", async () => {
      const logId = btn.dataset.logId;
      const action = btn.dataset.action;
      const log = visible.find((item) => item.log_id === logId);
      if (!log) return;
      if (action === "edit") {
        editingLogId = editingLogId === logId ? null : logId;
        renderLogs();
        renderDetailPanel();
        return;
      }
      if (action === "cancel-edit") {
        editingLogId = null;
        renderLogs();
        return;
      }
      try {
        if (action === "save-edit") {
          const logType = logsList.querySelector(`[data-log-edit="log_type"][data-log-id="${logId}"]`)?.value || log.log_type;
          const textValue = logsList.querySelector(`[data-log-edit="text"][data-log-id="${logId}"]`)?.value ?? log.text ?? "";
          const numberFrom = (field) => {
            const raw = logsList.querySelector(`[data-log-edit="${field}"][data-log-id="${logId}"]`)?.value;
            return raw === "" || raw == null ? null : Number(raw);
          };
          const stringFrom = (field) => {
            const raw = logsList.querySelector(`[data-log-edit="${field}"][data-log-id="${logId}"]`)?.value;
            return raw || null;
          };
          const intFrom = (field) => {
            const raw = logsList.querySelector(`[data-log-edit="${field}"][data-log-id="${logId}"]`)?.value;
            return raw === "" || raw == null ? null : parseInt(raw, 10);
          };
          await invoke("update_log_smart", { payload: {
            log_id: logId,
            log_type: logType,
            text: textValue,
            ph: numberFrom('ph'),
            ec: numberFrom('ec'),
            temp_c: numberFrom('temp_c'),
            rh: numberFrom('rh'),
            harvest_kg: numberFrom('harvest_kg'),
            harvest_count: intFrom('harvest_count'),
            harvest_quality: stringFrom('harvest_quality'),
            water_liters: numberFrom('water_liters'),
            water_method: stringFrom('water_method'),
          }});
          editingLogId = null;
        } else if (action === "delete") {
          if (!confirm(t('log.confirmDelete'))) return;
          await invoke("delete_log_smart", { logId });
        }
        await refreshAll({ autoReminderReason: "log_action" });
      } catch (err) {
        console.error("Log-Aktion fehlgeschlagen:", action, err);
        logOut("Log-Aktion fehlgeschlagen: " + (err?.message || err));
        if (action === "delete") alert(t('action.genericError', (err?.message || err)));
      }
    });
  });
}

async function loadPlants() {
  plants = await invoke("list_current_plants");
  if (selectedPlantId && !plants.some((p) => p.plant_id === selectedPlantId)) selectedPlantId = null;
  if (!selectedPlantId && plants.length) selectedPlantId = plants[0].plant_id;
  renderPlants();
  renderTimelinePanel();
  renderPhaseTaskSets();
}

async function loadTasks() {
  tasks = selectedPlantId ? await invoke("list_current_tasks", { plantId: selectedPlantId }) : [];
  renderTasks();
}

async function loadLogs() {
  logs = selectedPlantId ? await invoke("list_current_logs", { plantId: selectedPlantId }) : [];
  renderLogs();
}

// Browser build only: whether this account may upload photos and, after a
// paid plan ended, when its photos will be deleted. The desktop keeps photos
// locally and has no such rule.
let webPhotoStatus = null;

async function loadPlantPhotos() {
  if (!IS_TAURI) {
    try { webPhotoStatus = await invoke("web_photo_status"); }
    catch (_) { webPhotoStatus = null; }
  }
  if (!selectedPlantId) { plantPhotos = []; return; }
  try { plantPhotos = await invoke("list_plant_photos", { plantId: selectedPlantId }); }
  catch (_) { plantPhotos = []; }
}

async function loadCatalog() {
  if (plantCatalog.length) return;
  try { plantCatalog = await invoke("get_plant_catalog"); }
  catch (_) { plantCatalog = []; }
}

function catalogById(id) {
  return plantCatalog.find(c => c.id === id);
}

// ── Feature: Safety Badges (Allergie- & Haustier-Check) ──
function renderSafetyBadges(plant) {
  const entry = catalogById(plant.catalog_id);
  if (!entry) return '';
  const badges = [];
  if (entry.toxic_cats) badges.push(`<span style="padding:2px 6px;border-radius:6px;background:rgba(255,100,100,0.12);color:${TC.danger};font-size:10px;" title="${escapeHtml(entry.safety_note)}">&#128049; Giftig</span>`);
  if (entry.toxic_dogs) badges.push(`<span style="padding:2px 6px;border-radius:6px;background:rgba(255,100,100,0.12);color:${TC.danger};font-size:10px;" title="${escapeHtml(entry.safety_note)}">&#128054; Giftig</span>`);
  if (entry.allergen) badges.push(`<span style="padding:2px 6px;border-radius:6px;background:rgba(255,200,100,0.12);color:${TC.warning};font-size:10px;">&#9888;&#65039; Allergen</span>`);
  if (entry.toxic_humans) badges.push(`<span style="padding:2px 6px;border-radius:6px;background:rgba(255,100,100,0.15);color:${TC.danger};font-size:10px;" title="${escapeHtml(entry.safety_note)}">&#9888;&#65039; Giftig (roh)</span>`);
  return badges.join(' ');
}

function renderSafetyBadgesFromEntry(entry) {
  if (!entry) return '';
  const badges = [];
  if (entry.toxic_cats) badges.push(`<span style="padding:2px 6px;border-radius:6px;background:rgba(255,100,100,0.12);color:${TC.danger};font-size:10px;" title="${escapeHtml(entry.safety_note)}">&#128049; Giftig</span>`);
  if (entry.toxic_dogs) badges.push(`<span style="padding:2px 6px;border-radius:6px;background:rgba(255,100,100,0.12);color:${TC.danger};font-size:10px;" title="${escapeHtml(entry.safety_note)}">&#128054; Giftig</span>`);
  if (entry.allergen) badges.push(`<span style="padding:2px 6px;border-radius:6px;background:rgba(255,200,100,0.12);color:${TC.warning};font-size:10px;">&#9888;&#65039; Allergen</span>`);
  if (entry.toxic_humans) badges.push(`<span style="padding:2px 6px;border-radius:6px;background:rgba(255,100,100,0.15);color:${TC.danger};font-size:10px;" title="${escapeHtml(entry.safety_note)}">&#9888;&#65039; Giftig (roh)</span>`);
  return badges.join(' ');
}

// ── Feature: Propagation Guide ──
function renderPropagationBadges(entry) {
  if (!entry || !entry.propagation_methods || !entry.propagation_methods.length) return '';
  const methodLabels = { steckling: t('propagation.cutting'), ableger: t('propagation.offshoot'), samen: t('propagation.seed'), teilung: t('propagation.division'), veredelung: t('propagation.grafting') };
  const badges = entry.propagation_methods.map(m => {
    const label = methodLabels[m] || m;
    return `<span style="padding:2px 6px;border-radius:6px;background:rgba(${TC.accentRgb},0.12);color:${TC.accent};font-size:10px;">${escapeHtml(label)}</span>`;
  }).join(' ');
  const note = entry.propagation_note ? `<div style="font-size:11px;color:${TC.textMuted};margin-top:3px;">${escapeHtml(entry.propagation_note)}</div>` : '';
  return `<div style="margin-top:4px;">${badges}${note}</div>`;
}

// ── Feature: Harvest/Bloom Prediction ──
function getHarvestPrediction(plant) {
  const entry = catalogById(plant.catalog_id);
  if (!entry || !entry.harvest_months || !entry.harvest_months.length) return null;

  const now = new Date();
  const currentMonth = now.getMonth() + 1;

  // Already in harvest/dry/cure phase
  if (plant.phase === 'harvest' || plant.phase === 'dry' || plant.phase === 'cure') {
    return { daysUntil: 0, status: 'ready', text: t('harvest.ready') };
  }

  // Find next harvest month
  let nextHarvestMonth = entry.harvest_months.find(m => m >= currentMonth);
  if (!nextHarvestMonth) nextHarvestMonth = entry.harvest_months[0];

  let targetDate = new Date(now.getFullYear(), nextHarvestMonth - 1, 15);
  if (targetDate < now) targetDate.setFullYear(targetDate.getFullYear() + 1);

  const daysUntil = Math.ceil((targetDate - now) / 86400000);

  if (plant.phase === 'flower' && daysUntil < 30) {
    return { daysUntil, status: 'soon', text: t('harvest.inDays', daysUntil) };
  }

  return { daysUntil, status: 'growing', text: t('harvest.estimated', t(`month.${nextHarvestMonth}`)) };
}

function renderHarvestBadge(plant) {
  if (!isPlantActive(plant)) return '';
  const pred = getHarvestPrediction(plant);
  if (!pred) return '';
  const colors = { ready: TC.success, soon: TC.warning, growing: TC.textDim };
  const bgs = { ready: 'rgba(100,255,150,0.12)', soon: 'rgba(255,200,100,0.12)', growing: `rgba(${TC.accentRgb},0.08)` };
  return `<span style="padding:2px 6px;border-radius:6px;background:${bgs[pred.status]};color:${colors[pred.status]};font-size:10px;">${escapeHtml(pred.text)}</span>`;
}

// ── Feature: Sustainability Tracker ──
function calculateSustainability() {
  const activePlantCount = plants.filter(p => isPlantActive(p)).length;
  const totalWaterLiters = allLogs.reduce((sum, l) => sum + (l.metrics?.water_liters || 0), 0);
  const totalHarvestKg = allLogs.reduce((sum, l) => sum + (l.metrics?.harvest_kg || 0), 0);

  const co2SavedKg = totalHarvestKg * 0.5;
  const co2AbsorbedKg = activePlantCount * 0.02 * (new Date().getMonth() + 1);

  const waterPerKgHarvest = totalHarvestKg > 0 ? Math.round(totalWaterLiters / totalHarvestKg) : null;

  const tips = [];
  if (waterPerKgHarvest && waterPerKgHarvest > 50) tips.push(t('sustain.tip.mulch'));
  if (activePlantCount < 3) tips.push(t('sustain.tip.morePlants'));
  tips.push(t('sustain.tip.peatFree'));
  tips.push(t('sustain.tip.compost'));
  if (totalHarvestKg > 0) tips.push(t('sustain.tip.selfGrown', totalHarvestKg.toFixed(1)));

  return {
    activePlants: activePlantCount,
    totalWaterLiters: Math.round(totalWaterLiters),
    totalHarvestKg: Math.round(totalHarvestKg * 10) / 10,
    co2SavedKg: Math.round((co2SavedKg + co2AbsorbedKg) * 10) / 10,
    waterPerKgHarvest,
    tips,
  };
}

function getCompanionStatusLocal(idA, idB) {
  const entryA = catalogById(idA);
  if (entryA) {
    if (entryA.good_neighbors.includes(idB)) return 'good';
    if (entryA.bad_neighbors.includes(idB)) return 'bad';
  }
  const entryB = catalogById(idB);
  if (entryB) {
    if (entryB.good_neighbors.includes(idA)) return 'good';
    if (entryB.bad_neighbors.includes(idA)) return 'bad';
  }
  return 'neutral';
}

function renderCompanionInfo(plant) {
  if (!plant.catalog_id) return '';
  const companions = plants
    .filter(p => p.plant_id !== plant.plant_id && p.catalog_id && plantStatusValue(p) === 'active')
    .map(p => {
      const ce = catalogById(p.catalog_id);
      return {
        name: p.name,
        emoji: ce ? ce.emoji || '' : '',
        status: getCompanionStatusLocal(plant.catalog_id, p.catalog_id),
        color_tag: p.color_tag,
      };
    })
    .filter(c => c.status !== 'neutral');
  if (!companions.length) return '';
  const good = companions.filter(c => c.status === 'good');
  const bad = companions.filter(c => c.status === 'bad');
  return `
    <div style="margin-top:8px;">
      <div style="font-size:12px; color:${TC.textMuted}; margin-bottom:4px;">${t('catalog.companion')}</div>
      ${good.map(c => `<span style="display:inline-block; font-size:11px; padding:3px 8px; border-radius:999px; background:rgba(${TC.accentRgb},0.15); border:1px solid rgba(${TC.accentRgb},0.3); color:${TC.accent}; margin:2px 4px 2px 0;">${c.emoji ? c.emoji + ' ' : ''}${escapeHtml(c.name)}</span>`).join('')}
      ${bad.map(c => `<span style="display:inline-block; font-size:11px; padding:3px 8px; border-radius:999px; background:rgba(255,99,99,0.15); border:1px solid rgba(255,99,99,0.3); color:#ff6b6b; margin:2px 4px 2px 0;">${c.emoji ? c.emoji + ' ' : ''}${escapeHtml(c.name)}</span>`).join('')}
    </div>
  `;
}

function renderCatalogInfo(plant) {
  if (!plant.catalog_id) return '';
  const entry = catalogById(plant.catalog_id);
  if (!entry) return '';
  const sunLabel = { full: 'Volle Sonne', partial: 'Halbschatten', shade: 'Schatten' }[entry.sun] || entry.sun;
  const waterLabel = { low: 'Wenig', medium: 'Mittel', high: 'Viel' }[entry.water] || entry.water;
  const monthNames = ['', 'Jan','Feb','Mrz','Apr','Mai','Jun','Jul','Aug','Sep','Okt','Nov','Dez'];
  const sowIndoor = entry.sow_indoor_months.length ? entry.sow_indoor_months.map(m => monthNames[m]).join(', ') : '-';
  const sowOutdoor = entry.sow_outdoor_months.length ? entry.sow_outdoor_months.map(m => monthNames[m]).join(', ') : '-';
  const harvest = entry.harvest_months.length ? entry.harvest_months.map(m => monthNames[m]).join(', ') : '-';
  return `
    <div style="margin-top:10px; padding:10px; border-radius:10px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.10);">
      <div style="font-size:12px; color:${TC.textMuted}; margin-bottom:6px;">${t('catalog.info', '').replace(/\s*:\s*$/,'')}</div>
      <div style="display:grid; grid-template-columns:1fr 1fr; gap:4px 12px; font-size:12px; color:${TC.textMuted};">
        <div>${t('catalog.sowIndoor')} ${sowIndoor}</div>
        <div>${t('catalog.sowOutdoor')} ${sowOutdoor}</div>
        <div>${t('catalog.harvest')} ${harvest}</div>
        <div>${t('catalog.spacing')} ${entry.spacing_cm} cm</div>
        <div>${t('catalog.sun')} ${sunLabel}</div>
        <div>${t('catalog.water')} ${waterLabel}</div>
      </div>
      ${entry.tips_de ? `<div style="margin-top:6px; font-size:12px; color:${TC.textMuted}; font-style:italic;">${escapeHtml(entry.tips_de)}</div>` : ''}
      ${(entry.toxic_cats || entry.toxic_dogs || entry.allergen || entry.toxic_humans) ? `<div style="margin-top:6px;display:flex;gap:4px;flex-wrap:wrap;">${renderSafetyBadgesFromEntry(entry)}</div>${entry.safety_note ? `<div style="font-size:11px;color:${TC.danger};margin-top:3px;">${escapeHtml(entry.safety_note)}</div>` : ''}` : ''}
      ${entry.propagation_methods && entry.propagation_methods.length ? `<div style="margin-top:6px;"><span style="font-size:11px;color:${TC.textMuted};font-weight:600;">${t('catalog.propagation')}</span> ${renderPropagationBadges(entry)}</div>` : ''}
    </div>
  `;
}

// ── Climate / Weather ──────────────────────────────────────────────

async function loadClimateConfig() {
  try { climateConfig = await invoke("get_climate_config"); } catch(_) { climateConfig = null; }
}

async function saveClimateConfig(config) {
  try {
    await invoke("save_climate_config", { config });
    climateConfig = config;
  } catch (err) {
    console.error("Failed to save climate config", err);
  }
}

async function loadWeather() {
  if (!climateConfig?.latitude || !climateConfig?.longitude) return;
  if (!hasConsent('weather')) {
    notifyConsentMissing('weather', 'consent.missingWeatherToast');
    weatherData = null;
    return;
  }
  try {
    weatherData = await invoke("fetch_and_log_weather", { lat: climateConfig.latitude, lon: climateConfig.longitude });
    renderWeatherWidget();
    renderCalendarToolbar();
  } catch(_) { weatherData = null; }
}

function weatherIcon(code) {
  if (code == null) return '\u{1F321}\u{FE0F}';
  if (code === 0) return '\u{2600}\u{FE0F}';
  if (code <= 3) return '\u{26C5}';
  if (code <= 49) return '\u{1F32B}\u{FE0F}';
  if (code <= 59) return '\u{1F327}\u{FE0F}';
  if (code <= 69) return '\u{1F328}\u{FE0F}';
  if (code <= 79) return '\u{1F328}\u{FE0F}';
  if (code <= 82) return '\u{1F327}\u{FE0F}';
  if (code <= 86) return '\u{2744}\u{FE0F}';
  if (code >= 95) return '\u{26C8}\u{FE0F}';
  return '\u{1F321}\u{FE0F}';
}

function weatherLabel(code) {
  if (code == null) return t('weather.cond.unknown');
  if (code === 0) return t('weather.cond.clear');
  if (code <= 3) return t('weather.cond.partial');
  if (code <= 49) return t('weather.cond.fog');
  if (code <= 59) return t('weather.cond.drizzle');
  if (code <= 69) return t('weather.cond.rain');
  if (code <= 79) return t('weather.cond.sleet');
  if (code <= 82) return t('weather.cond.shower');
  if (code <= 86) return t('weather.cond.snow');
  if (code >= 95) return t('weather.cond.thunder');
  return t('weather.cond.unknown');
}

function renderWeatherWidget() {
  // Legacy: now rendered inline in toolbar and dashboard
}

let weatherDashboardLocationId = null; // null = default/global
let weatherDashboardHours = 24;
let weatherChartInstances = [];

async function renderWeatherDashboard() {
  const container = document.getElementById('weatherDashboard');
  if (!container) return;

  // Build location options
  const locsWithCoords = locations.filter(l => l.latitude && l.longitude);
  let locationOptions = `<option value="" ${!weatherDashboardLocationId ? 'selected' : ''}>${t('weather.standard')}${climateConfig?.city_name ? ' (' + escapeHtml(climateConfig.city_name) + ')' : ''}</option>`;
  for (const loc of locsWithCoords) {
    locationOptions += `<option value="${loc.location_id}" ${weatherDashboardLocationId === loc.location_id ? 'selected' : ''}>${escapeHtml(loc.name)}</option>`;
  }

  // Determine current lat/lon
  let currentLat = climateConfig?.latitude;
  let currentLon = climateConfig?.longitude;
  let currentLabel = climateConfig?.city_name || t('weather.standard');
  if (weatherDashboardLocationId) {
    const loc = locations.find(l => l.location_id === weatherDashboardLocationId);
    if (loc && loc.latitude && loc.longitude) {
      currentLat = loc.latitude;
      currentLon = loc.longitude;
      currentLabel = loc.name;
    }
  }

  if (!currentLat || !currentLon) {
    container.innerHTML = `<div class="view-card" style="padding:24px; text-align:center;">
      <h2 style="color:${TC.accent}; margin-top:0;">${t('weather.title')}</h2>
      <div style="color:${TC.textMuted}; margin-bottom:12px;">${t('weather.needLocation')}</div>
      <button class="btn btn--primary btn--sm" id="weatherGoToSettings">${t('weather.openSettings')}</button>
    </div>`;
    document.getElementById('weatherGoToSettings')?.addEventListener('click', () => switchPageView('settings'));
    return;
  }

  // Fetch fresh weather data for selected location
  let wData = weatherData;
  try {
    wData = await invoke("fetch_and_log_weather", {
      locationId: weatherDashboardLocationId || undefined,
      lat: currentLat,
      lon: currentLon
    });
  } catch (e) {
    console.warn("Weather fetch failed:", e);
  }

  const current = wData?.current;
  const hourly = wData?.hourly;
  const daily = wData?.daily;

  // Current weather card
  const temp = current?.temperature_2m ?? '-';
  const humidity = current?.relative_humidity_2m ?? '-';
  const wind = current?.wind_speed_10m ?? '-';
  const precip = current?.precipitation ?? 0;
  const code = current?.weather_code;

  // Hourly forecast strip (next 24h)
  let hourlyStripHtml = '';
  if (hourly?.time) {
    const now = new Date();
    const startIdx = hourly.time.findIndex(t => new Date(t) >= now);
    const sliceStart = Math.max(0, startIdx);
    const sliceEnd = Math.min(hourly.time.length, sliceStart + 24);
    for (let i = sliceStart; i < sliceEnd; i++) {
      const h = new Date(hourly.time[i]).getHours();
      const hTemp = hourly.temperature_2m?.[i] != null ? Math.round(hourly.temperature_2m[i]) : '-';
      const hHumi = hourly.relative_humidity_2m?.[i] != null ? hourly.relative_humidity_2m[i] : '';
      const hPrecip = hourly.precipitation?.[i] ?? 0;
      hourlyStripHtml += `<div style="text-align:center; padding:4px 6px; border-radius:8px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.08); min-width:42px; flex-shrink:0;">
        <div style="font-size:10px; color:${TC.textDim};">${String(h).padStart(2, '0')}:00</div>
        <div style="font-size:13px; color:${TC.text}; font-weight:600;">${hTemp}&deg;</div>
        ${hHumi ? `<div style="font-size:9px; color:${TC.textDim};">${hHumi}%</div>` : ''}
        ${hPrecip > 0 ? `<div style="font-size:9px; color:${TC.info};">${hPrecip.toFixed(1)}</div>` : ''}
      </div>`;
    }
  }

  // 7-day forecast
  const dayNames = ['So','Mo','Di','Mi','Do','Fr','Sa'];
  let dailyHtml = '';
  if (daily?.time && daily?.temperature_2m_max && daily?.temperature_2m_min) {
    dailyHtml = daily.time.map((dateStr, i) => {
      const d = new Date(dateStr);
      const dayName = dayNames[d.getDay()];
      const tMax = daily.temperature_2m_max[i] != null ? Math.round(daily.temperature_2m_max[i]) : '-';
      const tMin = daily.temperature_2m_min[i] != null ? Math.round(daily.temperature_2m_min[i]) : '-';
      const rain = daily.precipitation_sum?.[i] ?? 0;
      return `<div style="text-align:center; padding:6px 8px; border-radius:8px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.08); min-width:52px; flex-shrink:0;">
        <div style="font-size:10px; color:${TC.textMuted};">${dayName}</div>
        <div style="font-size:10px; color:${TC.textDim};">${String(d.getDate()).padStart(2,'0')}.${String(d.getMonth()+1).padStart(2,'0')}</div>
        <div style="font-size:14px; color:${TC.text}; font-weight:600;">${tMax}&deg;</div>
        <div style="font-size:11px; color:${TC.textDim};">${tMin}&deg;</div>
        ${rain > 0 ? `<div style="font-size:10px; color:${TC.info};">${rain.toFixed(1)} mm</div>` : ''}
      </div>`;
    }).join('');
  }

  // Period selector for history charts
  const periodOptions = [
    { value: 24, label: '24h' },
    { value: 48, label: '48h' },
    { value: 168, label: t('weather.7d') },
  ];
  const periodBtns = periodOptions.map(p =>
    `<button class="cal-toolbar-btn ${weatherDashboardHours === p.value ? 'cal-toolbar-btn--active' : ''}" data-weather-hours="${p.value}">${p.label}</button>`
  ).join('');

  // Locations without coordinates
  const locsNoCoords = locations.filter(l => !l.latitude || !l.longitude);
  let locConfigHtml = '';
  if (locsNoCoords.length > 0) {
    locConfigHtml = `<div class="view-card" style="margin-top:14px;">
      <h3 style="color:${TC.accent}; margin-top:0; font-size:14px;">${t('weather.noCoords')}</h3>
      <div style="color:${TC.textMuted}; font-size:12px; margin-bottom:8px;">${t('weather.noCoordsHint')}</div>
      ${locsNoCoords.map(loc => `
        <div style="display:flex; align-items:center; gap:8px; margin-bottom:6px; padding:8px; border-radius:8px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.08);">
          <strong style="color:${TC.text}; font-size:13px; min-width:100px;">${escapeHtml(loc.name)}</strong>
          <input class="form-input" placeholder="${t('weather.searchCity')}" data-loc-coord-search="${loc.location_id}" style="flex:1; font-size:12px;" />
          <button class="btn btn--sm btn--primary" data-loc-coord-btn="${loc.location_id}">${t('weather.searchBtn')}</button>
          <div data-loc-coord-results="${loc.location_id}" style="display:none;"></div>
        </div>
      `).join('')}
    </div>`;
  }

  container.innerHTML = `
    <div class="view-card" style="margin-bottom:14px;">
      <div style="display:flex; align-items:center; justify-content:space-between; margin-bottom:12px;">
        <h2 style="margin:0; color:${TC.accent}; font-size:18px;">${t('weather.title')}</h2>
        <div style="display:flex; align-items:center; gap:8px;">
          <select id="weatherLocationSelect" class="form-select" style="font-size:12px; padding:4px 8px; background:${TC.inputBg}; color:${TC.text}; border:1px solid rgba(${TC.accentRgb},0.15); border-radius:8px;">${locationOptions}</select>
          <button class="btn btn--sm btn--ghost" id="weatherRefreshBtn">${t('weather.refresh')}</button>
        </div>
      </div>

      <!-- Current weather -->
      ${current ? `
      <div style="display:flex; align-items:center; gap:20px; padding:16px; border-radius:12px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.10); margin-bottom:14px;">
        <div style="font-size:48px;">${weatherIcon(code)}</div>
        <div>
          <div style="font-size:32px; color:${TC.text}; font-weight:700;">${temp}&deg;C</div>
          <div style="font-size:13px; color:${TC.textMuted};">${escapeHtml(weatherLabel(code))}</div>
          <div style="font-size:12px; color:${TC.textDim};">${escapeHtml(currentLabel)}</div>
        </div>
        <div style="display:flex; flex-direction:column; gap:4px; margin-left:auto;">
          <div style="font-size:12px; color:${TC.textMuted};">${t('weather.humidityShort')}: ${humidity}%</div>
          <div style="font-size:12px; color:${TC.textMuted};">${t('weather.windShort')}: ${wind} km/h</div>
          ${precip > 0 ? `<div style="font-size:12px; color:${TC.info};">${t('weather.precip')}: ${precip} mm</div>` : ''}
        </div>
      </div>` : `<div style="color:${TC.textMuted}; padding:12px;">${t('weather.noData')}</div>`}

      <!-- 24h hourly strip -->
      ${hourlyStripHtml ? `
      <div style="margin-bottom:14px;">
        <div style="font-size:13px; color:${TC.textMuted}; margin-bottom:6px;">${t('weather.hourly')}</div>
        <div style="display:flex; gap:4px; overflow-x:auto; padding-bottom:4px;">${hourlyStripHtml}</div>
      </div>` : ''}

      <!-- 7-day forecast -->
      ${dailyHtml ? `
      <div style="margin-bottom:14px;">
        <div style="font-size:13px; color:${TC.textMuted}; margin-bottom:6px;">${t('weather.forecast7')}</div>
        <div style="display:flex; gap:4px; overflow-x:auto; padding-bottom:4px;">${dailyHtml}</div>
      </div>` : ''}
    </div>

    <!-- History Charts -->
    <div class="view-card" style="margin-bottom:14px;">
      <div style="display:flex; align-items:center; justify-content:space-between; margin-bottom:12px;">
        <h3 style="margin:0; color:${TC.accent}; font-size:15px;">${t('weather.historyTitle')}</h3>
        <div class="cal-toolbar-group">${periodBtns}</div>
      </div>
      <div style="margin-bottom:14px;">
        <div style="font-size:12px; color:${TC.textMuted}; margin-bottom:4px;">${t('weather.temperature')}</div>
        <div style="height:180px; position:relative;"><canvas id="weatherChartTemp"></canvas></div>
      </div>
      <div style="margin-bottom:14px;">
        <div style="font-size:12px; color:${TC.textMuted}; margin-bottom:4px;">${t('weather.humidity')}</div>
        <div style="height:180px; position:relative;"><canvas id="weatherChartHumidity"></canvas></div>
      </div>
      <div>
        <div style="font-size:12px; color:${TC.textMuted}; margin-bottom:4px;">${t('weather.wind')}</div>
        <div style="height:180px; position:relative;"><canvas id="weatherChartWind"></canvas></div>
      </div>
    </div>

    <div style="font-size:11px; color:${TC.textDim}; text-align:center; padding:8px;">${t('weather.dataSource')}</div>

    ${locConfigHtml}
  `;

  // Event listeners
  document.getElementById('weatherLocationSelect')?.addEventListener('change', async (e) => {
    weatherDashboardLocationId = e.target.value || null;
    await renderWeatherDashboard();
  });

  document.getElementById('weatherRefreshBtn')?.addEventListener('click', async () => {
    await renderWeatherDashboard();
  });

  container.querySelectorAll('[data-weather-hours]').forEach(btn => {
    btn.addEventListener('click', async () => {
      weatherDashboardHours = parseInt(btn.dataset.weatherHours) || 24;
      // TODO: Wetter zeigt bei 7d (168h) momentan keine 7-Tage-Daten im Hourly-Strip;
      // die Hourly-Ansicht ist auf 24 Slots fest verdrahtet. renderWeatherCharts nutzt
      // zwar weatherDashboardHours korrekt, doch der Stundenverlauf/Forecast bleibt 24h.
      await renderWeatherDashboard();
    });
  });

  // Location coordinate search
  container.querySelectorAll('[data-loc-coord-btn]').forEach(btn => {
    btn.addEventListener('click', async () => {
      const locId = btn.dataset.locCoordBtn;
      const input = container.querySelector(`[data-loc-coord-search="${locId}"]`);
      const resultsDiv = container.querySelector(`[data-loc-coord-results="${locId}"]`);
      if (!input || !resultsDiv) return;
      const query = input.value?.trim();
      if (!query) return;
      if (!hasConsent('weather')) {
        notifyConsentMissing('weather', 'consent.missingWeatherToast');
        return;
      }
      try {
        const results = await invoke("search_city", { query });
        if (!results?.results?.length) {
          resultsDiv.style.display = '';
          resultsDiv.innerHTML = `<div style="color:${TC.textMuted}; font-size:11px; padding:4px;">${t('weather.noResults')}</div>`;
          return;
        }
        resultsDiv.style.display = '';
        resultsDiv.innerHTML = results.results.map(r =>
          `<button class="btn btn--ghost btn--sm" style="font-size:11px; margin:2px;" data-loc-set-coord="${locId}" data-lat="${r.latitude}" data-lon="${r.longitude}">${escapeHtml(r.name)}${r.admin1 ? ', ' + escapeHtml(r.admin1) : ''} (${r.latitude.toFixed(2)}, ${r.longitude.toFixed(2)})</button>`
        ).join('');
        resultsDiv.querySelectorAll('[data-loc-set-coord]').forEach(rb => {
          rb.addEventListener('click', async () => {
            const lat = parseFloat(rb.dataset.lat);
            const lon = parseFloat(rb.dataset.lon);
            const loc = locations.find(l => l.location_id === locId);
            if (!loc) return;
            try {
              await invoke("update_location", {
                locationId: locId,
                name: loc.name,
                locationType: loc.location_type,
                description: loc.description,
                latitude: lat,
                longitude: lon
              });
              await loadLocations();
              await renderWeatherDashboard();
            } catch (err) {
              console.error("Failed to set coordinates:", err);
              logOut(t('location.coordSaveFailed', String(err)));
            }
          });
        });
      } catch (err) {
        resultsDiv.style.display = '';
        resultsDiv.innerHTML = `<div style="color:${TC.danger}; font-size:11px;">${escapeHtml(String(err))}</div>`;
      }
    });
  });

  // Render history charts
  await renderWeatherCharts(weatherDashboardLocationId, weatherDashboardHours);
}

async function renderWeatherCharts(locationId, hours) {
  // Destroy existing chart instances
  for (const c of weatherChartInstances) {
    try { c.destroy(); } catch (_) {}
  }
  weatherChartInstances = [];

  const Chart = await loadChart();
  if (!Chart) {
    const tempCanvas = document.getElementById('weatherChartTemp');
    if (tempCanvas?.parentElement) tempCanvas.parentElement.innerHTML = `<div style="color:${TC.textDim}; font-size:12px; padding:8px;">${t('chart.notLoaded')}</div>`;
    return;
  }

  let records = [];
  try {
    records = await invoke("list_weather_history", { locationId: locationId || undefined, hours });
  } catch (e) {
    console.warn("Failed to load weather history:", e);
    return;
  }

  if (records.length === 0) {
    const tempCanvas = document.getElementById('weatherChartTemp');
    if (tempCanvas?.parentElement) tempCanvas.parentElement.innerHTML = `<div style="color:${TC.textDim}; font-size:12px; padding:8px;">${t('weather.noHistory')}</div>`;
    return;
  }

  const labels = records.map(r => {
    const d = new Date(r.recorded_at);
    return hours <= 48
      ? d.toLocaleTimeString(localeTag(), { hour: "2-digit", minute: "2-digit" })
      : d.toLocaleDateString(localeTag(), { day: "2-digit", month: "2-digit" }) + ' ' + d.toLocaleTimeString(localeTag(), { hour: "2-digit", minute: "2-digit" });
  });

  const chartOpts = {
    responsive: true,
    maintainAspectRatio: false,
    interaction: { mode: "index", intersect: false },
    plugins: { legend: { display: false } },
    scales: {
      x: { ticks: { color: TC.textDim, maxTicksLimit: 12, font: { size: 10 } }, grid: { color: `rgba(${TC.accentRgb},0.05)` } },
      y: { ticks: { color: TC.textDim, font: { size: 10 } }, grid: { color: `rgba(${TC.accentRgb},0.05)` } },
    },
  };

  // Temperature chart
  const tempCanvas = document.getElementById('weatherChartTemp');
  if (tempCanvas) {
    weatherChartInstances.push(new Chart(tempCanvas, {
      type: 'line',
      data: {
        labels,
        datasets: [{ label: t('chart.temp'), data: records.map(r => r.temp_c), borderColor: '#ff9966', backgroundColor: 'rgba(255,153,102,0.1)', tension: 0.3, pointRadius: 1, fill: true }],
      },
      options: chartOpts,
    }));
  }

  // Humidity chart
  const humiCanvas = document.getElementById('weatherChartHumidity');
  if (humiCanvas) {
    weatherChartInstances.push(new Chart(humiCanvas, {
      type: 'line',
      data: {
        labels,
        datasets: [{ label: t('chart.humidity'), data: records.map(r => r.humidity), borderColor: '#6bbbff', backgroundColor: 'rgba(107,187,255,0.1)', tension: 0.3, pointRadius: 1, fill: true }],
      },
      options: chartOpts,
    }));
  }

  // Wind chart
  const windCanvas = document.getElementById('weatherChartWind');
  if (windCanvas) {
    weatherChartInstances.push(new Chart(windCanvas, {
      type: 'line',
      data: {
        labels,
        datasets: [{ label: t('chart.wind'), data: records.map(r => r.wind_kmh), borderColor: '#6bffbc', backgroundColor: 'rgba(107,255,188,0.1)', tension: 0.3, pointRadius: 1, fill: true }],
      },
      options: chartOpts,
    }));
  }
}

function renderSeasonAlerts() {
  const container = document.getElementById('seasonAlerts');
  if (!container) return;
  if (!plantCatalog.length) { container.style.display = 'none'; return; }

  const currentMonth = new Date().getMonth() + 1;
  const sowIndoorNow = [];
  const sowOutdoorNow = [];
  const harvestNow = [];

  for (const entry of plantCatalog) {
    if (entry.sow_indoor_months && entry.sow_indoor_months.includes(currentMonth)) {
      sowIndoorNow.push((entry.emoji || '') + ' ' + entry.name_de + ' ' + t('catalog.inlineSuffix.sowIndoor'));
    }
    if (entry.sow_outdoor_months && entry.sow_outdoor_months.includes(currentMonth)) {
      sowOutdoorNow.push((entry.emoji || '') + ' ' + entry.name_de + ' ' + t('catalog.inlineSuffix.sowOutdoor'));
    }
    if (entry.harvest_months && entry.harvest_months.includes(currentMonth)) {
      harvestNow.push((entry.emoji || '') + ' ' + entry.name_de);
    }
  }

  if (!sowIndoorNow.length && !sowOutdoorNow.length && !harvestNow.length) {
    container.style.display = 'none';
    return;
  }

  const sowAll = [...sowIndoorNow, ...sowOutdoorNow];
  let html = '';
  if (sowAll.length) {
    html += `<div style="padding:8px 12px; border-radius:10px; background:rgba(107,255,188,0.08); border:1px solid rgba(107,255,188,0.15); margin-bottom:6px; font-size:12px; color:${TC.textMuted};">
      <span style="color:#6bffbc; font-weight:600;">${t('tips.sowNow')}</span> ${sowAll.map(n => escapeHtml(n)).join(', ')}
    </div>`;
  }
  if (harvestNow.length) {
    html += `<div style="padding:8px 12px; border-radius:10px; background:rgba(255,200,100,0.08); border:1px solid rgba(255,200,100,0.15); margin-bottom:6px; font-size:12px; color:${TC.textMuted};">
      <span style="color:${TC.warning}; font-weight:600;">${t('tips.harvestNow')}</span> ${harvestNow.map(n => escapeHtml(n)).join(', ')}
    </div>`;
  }

  container.style.display = '';
  container.innerHTML = html;
}

function renderTipsView() {
  const content = document.getElementById('tipsContent');
  if (!content) return;

  const currentMonth = new Date().getMonth() + 1;

  // Group catalog plants by activity this month
  const sowIndoor = plantCatalog.filter(e => e.sow_indoor_months.includes(currentMonth));
  const sowOutdoor = plantCatalog.filter(e => e.sow_outdoor_months.includes(currentMonth));
  const harvestNow = plantCatalog.filter(e => e.harvest_months.includes(currentMonth));

  // Build a 12-month overview
  const monthOverview = Array.from({length: 12}, (_, i) => {
    const m = i + 1;
    return {
      month: m,
      name: getMonthName(m),
      sowIndoor: plantCatalog.filter(e => e.sow_indoor_months.includes(m)),
      sowOutdoor: plantCatalog.filter(e => e.sow_outdoor_months.includes(m)),
      harvest: plantCatalog.filter(e => e.harvest_months.includes(m)),
    };
  });

  content.innerHTML = `
    <h2 style="color:${TC.accent};margin:0 0 16px;">${t('tips.title')}</h2>

    <!-- Light Meter -->
    <div class="view-card" style="margin-bottom:14px;">
      <h3 style="color:${TC.accent};margin:0 0 10px;">\u{1F4A1} ${t('tips.lightMeter')}</h3>
      <div style="font-size:13px;color:${TC.textMuted};margin-bottom:10px;">${t('tips.lightMeterDesc')}</div>
      <button class="btn btn--sm" id="lightMeterBtn">\u{1F4F8} ${t('tips.measureLight')}</button>
      <div id="lightMeterResult" style="margin-top:10px;"></div>
    </div>

    <!-- Current month highlights -->
    <div class="view-card" style="margin-bottom:14px;">
      <h3 style="color:${TC.accent};margin:0 0 10px;">${getMonthName(currentMonth)} — ${t('tips.thisMonth')}</h3>
      ${sowIndoor.length ? `
        <div style="margin-bottom:10px;">
          <div style="font-size:12px;color:${TC.info};font-weight:600;margin-bottom:4px;">\u{1F3E0} ${t('tips.sowIndoor')}</div>
          <div style="display:flex;gap:6px;flex-wrap:wrap;">
            ${sowIndoor.map(e => `<span class="cal-badge" style="background:rgba(100,180,255,0.12);color:${TC.info};">${e.emoji || ''} ${escapeHtml(e.name_de)}</span>`).join('')}
          </div>
        </div>
      ` : ''}
      ${sowOutdoor.length ? `
        <div style="margin-bottom:10px;">
          <div style="font-size:12px;color:${TC.accent};font-weight:600;margin-bottom:4px;">\u{1F331} ${t('tips.sowOutdoor')}</div>
          <div style="display:flex;gap:6px;flex-wrap:wrap;">
            ${sowOutdoor.map(e => `<span class="cal-badge" style="background:rgba(${TC.accentRgb},0.12);color:${TC.accent};">${e.emoji || ''} ${escapeHtml(e.name_de)}</span>`).join('')}
          </div>
        </div>
      ` : ''}
      ${harvestNow.length ? `
        <div style="margin-bottom:10px;">
          <div style="font-size:12px;color:${TC.warning};font-weight:600;margin-bottom:4px;">\u{1F33E} ${t('tips.harvestTime')}</div>
          <div style="display:flex;gap:6px;flex-wrap:wrap;">
            ${harvestNow.map(e => `<span class="cal-badge" style="background:rgba(255,200,100,0.12);color:${TC.warning};">${e.emoji || ''} ${escapeHtml(e.name_de)}</span>`).join('')}
          </div>
        </div>
      ` : ''}
    </div>

    <!-- Safety Warnings -->
    ${(() => {
      const toxicPlants = plants.filter(p => isPlantActive(p) && p.catalog_id).map(p => {
        const e = catalogById(p.catalog_id);
        if (!e) return null;
        if (!e.toxic_cats && !e.toxic_dogs && !e.toxic_humans && !e.allergen) return null;
        return { plant: p, entry: e };
      }).filter(Boolean);
      if (!toxicPlants.length) return '';
      return `<div class="view-card" style="margin-bottom:14px;">
        <h3 style="color:${TC.danger};margin:0 0 10px;">&#9888;&#65039; ${t('tips.safety')}</h3>
        <div style="font-size:12px;color:${TC.textMuted};margin-bottom:8px;">${t('tips.toxicWarning')}</div>
        <div style="display:grid;gap:6px;">
          ${toxicPlants.map(tp => `<div style="padding:8px;border-radius:8px;background:rgba(255,100,100,0.06);border:1px solid rgba(255,100,100,0.15);">
            <div style="font-weight:600;font-size:13px;color:${TC.text};">${tp.entry.emoji || ''} ${escapeHtml(tp.plant.name)}</div>
            <div style="display:flex;gap:4px;flex-wrap:wrap;margin-top:4px;">${renderSafetyBadgesFromEntry(tp.entry)}</div>
            ${tp.entry.safety_note ? `<div style="font-size:11px;color:${TC.textMuted};margin-top:3px;">${escapeHtml(tp.entry.safety_note)}</div>` : ''}
          </div>`).join('')}
        </div>
      </div>`;
    })()}

    <!-- Harvest Predictions -->
    ${(() => {
      const activePreds = plants.filter(p => isPlantActive(p) && p.catalog_id).map(p => {
        const pred = getHarvestPrediction(p);
        if (!pred) return null;
        const ce = catalogById(p.catalog_id);
        return { plant: p, pred, emoji: ce ? ce.emoji || '' : '' };
      }).filter(Boolean);
      if (!activePreds.length) return '';
      activePreds.sort((a, b) => a.pred.daysUntil - b.pred.daysUntil);
      return `<div class="view-card" style="margin-bottom:14px;">
        <h3 style="color:${TC.accent};margin:0 0 10px;">&#127806; ${t('tips.harvestForecast')}</h3>
        <div style="display:grid;gap:4px;">
          ${activePreds.map(ap => {
            const colors = { ready: TC.success, soon: TC.warning, growing: TC.textMuted };
            return `<div style="display:flex;justify-content:space-between;align-items:center;padding:6px 8px;border-radius:8px;background:${TC.surface};font-size:12px;">
              <span style="color:${TC.text};">${ap.emoji} ${escapeHtml(ap.plant.name)}</span>
              <span style="color:${colors[ap.pred.status]};font-weight:500;">${escapeHtml(ap.pred.text)}</span>
            </div>`;
          }).join('')}
        </div>
      </div>`;
    })()}

    <!-- Propagation Guide -->
    <div class="view-card" style="margin-bottom:14px;">
      <h3 style="color:${TC.accent};margin:0 0 10px;">&#127793; ${t('tips.propagation')}</h3>
      <div style="font-size:12px;color:${TC.textMuted};margin-bottom:8px;">${t('tips.howToPropagate')}</div>
      <div style="display:grid;gap:6px;">
        ${plantCatalog.filter(e => e.propagation_methods && e.propagation_methods.length).map(e => `<div style="padding:6px 8px;border-radius:8px;background:${TC.surface};font-size:12px;">
          <div style="font-weight:600;color:${TC.text};">${e.emoji || ''} ${escapeHtml(e.name_de)}</div>
          ${renderPropagationBadges(e)}
        </div>`).join('')}
      </div>
    </div>

    <!-- Sustainability Tracker -->
    ${(() => {
      const sus = calculateSustainability();
      return `<div class="view-card" style="margin-bottom:14px;">
        <h3 style="color:${TC.accent};margin:0 0 10px;">&#127759; ${t('tips.sustainability')}</h3>
        <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;margin-bottom:10px;">
          <div style="padding:10px;border-radius:10px;background:${TC.surface};text-align:center;">
            <div style="font-size:20px;font-weight:700;color:${TC.accent};">${sus.co2SavedKg}</div>
            <div style="font-size:11px;color:${TC.textMuted};">${t('tips.co2Saved')}</div>
          </div>
          <div style="padding:10px;border-radius:10px;background:${TC.surface};text-align:center;">
            <div style="font-size:20px;font-weight:700;color:${TC.info};">${sus.totalWaterLiters}</div>
            <div style="font-size:11px;color:${TC.textMuted};">${t('tips.litersWater')}</div>
          </div>
          <div style="padding:10px;border-radius:10px;background:${TC.surface};text-align:center;">
            <div style="font-size:20px;font-weight:700;color:${TC.warning};">${sus.totalHarvestKg}</div>
            <div style="font-size:11px;color:${TC.textMuted};">${t('tips.kgHarvest')}</div>
          </div>
          <div style="padding:10px;border-radius:10px;background:${TC.surface};text-align:center;">
            <div style="font-size:20px;font-weight:700;color:${TC.textMuted};">${sus.waterPerKgHarvest != null ? sus.waterPerKgHarvest : '-'}</div>
            <div style="font-size:11px;color:${TC.textMuted};">${t('tips.waterPerKg')}</div>
          </div>
        </div>
        <div style="display:grid;gap:4px;">
          ${sus.tips.map(t => `<div style="padding:6px 8px;border-radius:8px;background:rgba(${TC.accentRgb},0.06);font-size:12px;color:${TC.textMuted};">${escapeHtml(t)}</div>`).join('')}
        </div>
      </div>`;
    })()}

    <!-- 12-Month Overview -->
    <div class="view-card">
      <h3 style="color:${TC.accent};margin:0 0 10px;">${t('tips.yearOverviewTitle')}</h3>
      <div style="display:grid;gap:10px;">
        ${monthOverview.map(mo => `
          <div style="padding:10px;border-radius:12px;background:${mo.month === currentMonth ? 'rgba('+TC.accentRgb+',0.06)' : TC.surface};border:1px solid ${mo.month === currentMonth ? 'rgba('+TC.accentRgb+',0.2)' : 'rgba('+TC.accentRgb+',0.06)'};${mo.month === currentMonth ? 'box-shadow:inset 0 0 0 1px rgba('+TC.accentRgb+',0.08);' : ''}">
            <div style="font-weight:600;color:${mo.month === currentMonth ? TC.accent : TC.text};margin-bottom:6px;">${escapeHtml(mo.name)}</div>
            <div style="display:flex;gap:4px;flex-wrap:wrap;font-size:11px;">
              ${mo.sowIndoor.map(e => `<span title="${t('tips.sowIndoorTitle')}: ${escapeHtml(e.name_de)}" style="padding:2px 6px;border-radius:6px;background:rgba(100,180,255,0.1);color:${TC.info};">${e.emoji || '\u{1F331}'} ${escapeHtml(e.name_de)}</span>`).join('')}
              ${mo.sowOutdoor.map(e => `<span title="${t('tips.sowOutdoorTitle')}: ${escapeHtml(e.name_de)}" style="padding:2px 6px;border-radius:6px;background:rgba(${TC.accentRgb},0.1);color:${TC.accent};">${e.emoji || '\u{1F331}'} ${escapeHtml(e.name_de)}</span>`).join('')}
              ${mo.harvest.map(e => `<span title="${t('tips.harvestLabel')}: ${escapeHtml(e.name_de)}" style="padding:2px 6px;border-radius:6px;background:rgba(255,200,100,0.1);color:${TC.warning};">${e.emoji || '\u{1F33E}'} ${escapeHtml(e.name_de)}</span>`).join('')}
              ${!mo.sowIndoor.length && !mo.sowOutdoor.length && !mo.harvest.length ? `<span style="color:${TC.textDim};">${t('tips.noActivities')}</span>` : ''}
            </div>
          </div>
        `).join('')}
      </div>
    </div>
  `;

  // Wire up light meter button
  document.getElementById('lightMeterBtn')?.addEventListener('click', handleLightMeter);
}

function renderPlantingCalendar(plant) {
  const entry = catalogById(plant.catalog_id);
  if (!entry) return '';

  const months = ['J','F','M','A','M','J','J','A','S','O','N','D'];
  const currentMonth = new Date().getMonth() + 1;

  return `
    <div style="display:flex; gap:2px; margin-top:6px;">
      ${months.map((label, i) => {
        const m = i + 1;
        const isSowIndoor = entry.sow_indoor_months && entry.sow_indoor_months.includes(m);
        const isSowOutdoor = entry.sow_outdoor_months && entry.sow_outdoor_months.includes(m);
        const isHarvest = entry.harvest_months && entry.harvest_months.includes(m);
        const isCurrent = m === currentMonth;

        let bg = `rgba(${TC.accentRgb},0.05)`;
        let color = '#4a7a6a';
        if (isSowIndoor) { bg = 'rgba(100,180,255,0.2)'; color = TC.info; }
        if (isSowOutdoor) { bg = 'rgba(107,255,188,0.2)'; color = '#6bffbc'; }
        if (isHarvest) { bg = 'rgba(255,200,100,0.2)'; color = TC.warning; }

        return `<div style="flex:1; text-align:center; padding:4px 0; border-radius:4px; background:${bg}; color:${color}; font-size:10px; font-weight:${isCurrent ? '700' : '400'}; border:${isCurrent ? '1px solid ' + color : '1px solid transparent'};" title="${isSowIndoor ? t('catalog.month.sowIndoor') : ''} ${isSowOutdoor ? t('catalog.month.sowOutdoor') : ''} ${isHarvest ? t('catalog.month.harvest') : ''}">${label}</div>`;
      }).join('')}
    </div>
    <div style="display:flex; gap:8px; margin-top:4px; font-size:10px; color:${TC.textDim};">
      <span style="color:${TC.info};">&#9632; ${t('catalog.month.sowIndoor')}</span>
      <span style="color:#6bffbc;">&#9632; ${t('catalog.month.sowOutdoor')}</span>
      <span style="color:${TC.warning};">&#9632; ${t('catalog.month.harvest')}</span>
    </div>
  `;
}

function renderClimateSettings() {
  const panel = document.getElementById('climateSettingsPanel');
  if (!panel) return;

  const cityName = climateConfig?.city_name ? escapeHtml(climateConfig.city_name) : '';
  const lat = climateConfig?.latitude?.toFixed(4) ?? '';
  const lon = climateConfig?.longitude?.toFixed(4) ?? '';
  const springFrost = climateConfig?.last_spring_frost ?? '';
  const fallFrost = climateConfig?.first_fall_frost ?? '';

  panel.innerHTML = `
    <h3 style="color:${TC.accent}; margin-top:0;">${t('climate.title')}</h3>
    <div style="font-size:12px; color:${TC.textMuted}; margin-bottom:10px;">
      ${t('climate.hint')}
    </div>
    <div style="display:flex; gap:6px; margin-bottom:10px; align-items:flex-end;">
      <div style="flex:1;">
        <label class="form-label" style="font-size:11px; color:${TC.textMuted};">${t('climate.searchCity')}</label>
        <input id="climateCitySearch" class="form-input" placeholder="${t('climate.searchPlaceholder')}" value="" />
      </div>
      <button id="climateCitySearchBtn" class="btn btn--sm btn--primary" style="height:34px;">${t('climate.searchBtn')}</button>
    </div>
    <div id="climateCityResults" style="display:none; margin-bottom:10px;"></div>
    ${cityName ? `<div style="padding:8px 12px; border-radius:10px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.10); margin-bottom:10px; font-size:13px; color:${TC.textMuted};">
      ${t('climate.currentLocation', `<strong style="color:${TC.text};">${cityName}</strong>`, lat, lon)}
    </div>` : ''}
    <div style="display:grid; grid-template-columns:1fr 1fr; gap:8px; margin-bottom:10px;">
      <div>
        <label class="form-label" style="font-size:11px; color:${TC.textMuted};">${t('climate.springFrost')}</label>
        <input id="climateSpringFrost" class="form-input" placeholder="${t('climate.springFrostPlaceholder')}" value="${escapeHtml(springFrost)}" />
      </div>
      <div>
        <label class="form-label" style="font-size:11px; color:${TC.textMuted};">${t('climate.fallFrost')}</label>
        <input id="climateFallFrost" class="form-input" placeholder="${t('climate.fallFrostPlaceholder')}" value="${escapeHtml(fallFrost)}" />
      </div>
    </div>
    <div style="display:flex; gap:6px;">
      <button id="climateSaveBtn" class="btn btn--sm btn--primary">${t('climate.saveLocation')}</button>
      ${climateConfig ? `<button id="climateRefreshWeather" class="btn btn--sm">${t('climate.refreshWeather')}</button>` : ''}
    </div>
    <div id="climateFeedback" style="margin-top:8px;"></div>
    ${climateConfig ? renderClimateWeatherPreview() : ''}
  `;

  // Event handlers
  document.getElementById('climateCitySearchBtn')?.addEventListener('click', handleCitySearch);
  document.getElementById('climateCitySearch')?.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') handleCitySearch();
  });
  document.getElementById('climateSaveBtn')?.addEventListener('click', handleClimateSave);
  document.getElementById('climateRefreshWeather')?.addEventListener('click', async () => {
    await loadWeather();
    renderClimateSettings();
  });
}

function renderClimateWeatherPreview() {
  if (!weatherData || !weatherData.current) return '';
  const c = weatherData.current;
  return `
    <div style="margin-top:10px; padding:10px; border-radius:10px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.10);">
      <div style="font-size:12px; color:${TC.textMuted}; margin-bottom:4px;">${t('climate.currentWeather')}</div>
      <div style="display:grid; grid-template-columns:1fr 1fr 1fr 1fr; gap:8px; font-size:13px; color:${TC.textMuted};">
        <div>${t('climate.tempLabel')}: <strong style="color:${TC.text};">${c.temperature_2m ?? '-'}&#176;C</strong></div>
        <div>${t('climate.humidityLabel')}: <strong style="color:${TC.text};">${c.relative_humidity_2m ?? '-'}%</strong></div>
        <div>${t('climate.windLabel')}: <strong style="color:${TC.text};">${c.wind_speed_10m ?? '-'} km/h</strong></div>
        <div>${t('climate.rainLabel')}: <strong style="color:${TC.text};">${c.precipitation ?? 0} mm</strong></div>
      </div>
    </div>
  `;
}

async function handleCitySearch() {
  const query = document.getElementById('climateCitySearch')?.value?.trim();
  const resultsDiv = document.getElementById('climateCityResults');
  if (!query || !resultsDiv) return;
  if (!hasConsent('weather')) {
    notifyConsentMissing('weather', 'consent.missingWeatherToast');
    resultsDiv.style.display = '';
    resultsDiv.innerHTML = `<div style="color:${TC.textMuted}; font-size:12px; padding:6px;">${escapeHtml(t('consent.missingWeatherToast'))}</div>`;
    return;
  }

  try {
    const data = await invoke("search_city", { query });
    const results = data?.results;
    if (!results || !results.length) {
      resultsDiv.style.display = '';
      resultsDiv.innerHTML = `<div style="color:${TC.textMuted}; font-size:12px; padding:6px;">${t('climate.noResults')}</div>`;
      return;
    }
    resultsDiv.style.display = '';
    resultsDiv.innerHTML = `
      <div style="border-radius:10px; background:${TC.surface}; border:1px solid rgba(${TC.accentRgb},0.10); overflow:hidden;">
        ${results.map((r, i) => {
          const label = [r.name, r.admin1, r.country].filter(Boolean).join(', ');
          return `<div class="climate-city-option" data-city-idx="${i}" style="padding:8px 12px; cursor:pointer; font-size:13px; color:${TC.textMuted}; border-bottom:1px solid rgba(${TC.accentRgb},0.06);">
            ${escapeHtml(label)} <span style="color:${TC.textDim}; font-size:11px;">(${r.latitude.toFixed(2)}, ${r.longitude.toFixed(2)})</span>
          </div>`;
        }).join('')}
      </div>
    `;
    resultsDiv.querySelectorAll('.climate-city-option').forEach((el) => {
      el.addEventListener('click', () => {
        const idx = parseInt(el.dataset.cityIdx, 10);
        const r = results[idx];
        if (!r) return;
        climateConfig = {
          latitude: r.latitude,
          longitude: r.longitude,
          city_name: [r.name, r.admin1, r.country].filter(Boolean).join(', '),
          last_spring_frost: climateConfig?.last_spring_frost || null,
          first_fall_frost: climateConfig?.first_fall_frost || null,
          timezone: r.timezone || null,
        };
        resultsDiv.style.display = 'none';
        renderClimateSettings();
      });
    });
  } catch (err) {
    if (resultsDiv) {
      resultsDiv.style.display = '';
      resultsDiv.innerHTML = `<div style="color:${TC.danger}; font-size:12px; padding:6px;">${t('action.genericError', escapeHtml(String(err)))}</div>`;
    }
  }
}

async function handleClimateSave() {
  const feedback = document.getElementById('climateFeedback');
  if (!climateConfig || !climateConfig.latitude) {
    if (feedback) feedback.innerHTML = `<div style="color:${TC.danger}; font-size:12px;">${t('climate.selectCityFirst')}</div>`;
    return;
  }
  const springFrost = document.getElementById('climateSpringFrost')?.value?.trim() || null;
  const fallFrost = document.getElementById('climateFallFrost')?.value?.trim() || null;
  climateConfig.last_spring_frost = springFrost;
  climateConfig.first_fall_frost = fallFrost;
  try {
    await saveClimateConfig(climateConfig);
    await loadWeather();
    renderClimateSettings();
    renderWeatherWidget();
    renderCalendarToolbar();
    renderSeasonAlerts();
    if (feedback) feedback.innerHTML = `<div style="color:${TC.accent}; font-size:12px;">${t('climate.locationSaved')}</div>`;
  } catch (err) {
    if (feedback) feedback.innerHTML = `<div style="color:${TC.danger}; font-size:12px;">${t('action.genericError', escapeHtml(String(err)))}</div>`;
  }
}

// ── Growth Analytics & Care Insights ──

function calculatePlantAnalytics(plantId) {
  const plant = plants.find(p => p.plant_id === plantId);
  if (!plant) return null;

  const plantTasks = allTasks.filter(t => t.plant_id === plantId);
  const plantLogs = allLogs.filter(l => l.plant_id === plantId);
  const photos = plantPhotos.filter(p => p.plant_id === plantId);

  // Task completion rate
  const doneTasks = plantTasks.filter(t => t.status === 'done').length;
  const totalTasks = plantTasks.length;
  const completionRate = totalTasks > 0 ? Math.round((doneTasks / totalTasks) * 100) : 0;

  // Watering stats
  const waterLogs = plantLogs.filter(l => l.metrics && l.metrics.water_liters);
  const totalWaterLiters = waterLogs.reduce((sum, l) => sum + (l.metrics.water_liters || 0), 0);

  // Watering frequency analysis
  const waterDates = waterLogs.map(l => new Date(l.created_at).getTime()).sort((a, b) => a - b);
  const waterIntervals = [];
  for (let i = 1; i < waterDates.length; i++) {
    waterIntervals.push((waterDates[i] - waterDates[i-1]) / 3600000);
  }
  const avgWaterInterval = waterIntervals.length > 0
    ? Math.round(waterIntervals.reduce((a, b) => a + b, 0) / waterIntervals.length)
    : null;

  // Harvest stats
  const harvestLogs = plantLogs.filter(l => l.metrics && (l.metrics.harvest_kg || l.metrics.harvest_count));
  const totalHarvestKg = harvestLogs.reduce((sum, l) => sum + (l.metrics.harvest_kg || 0), 0);
  const totalHarvestCount = harvestLogs.reduce((sum, l) => sum + (l.metrics.harvest_count || 0), 0);

  // Plant age
  const ageInDays = Math.floor((Date.now() - new Date(plant.start_date).getTime()) / 86400000);
  const ageWeeks = Math.floor(ageInDays / 7);

  // Activity heatmap data (last 90 days)
  const now = Date.now();
  const activityByDay = {};
  for (let d = 0; d < 90; d++) {
    const dayKey = new Date(now - d * 86400000).toISOString().slice(0, 10);
    const dayTasks = plantTasks.filter(t => normalizeDateKey(t.due_at) === dayKey && t.status === 'done').length;
    const dayLogs = plantLogs.filter(l => normalizeDateKey(l.created_at) === dayKey).length;
    activityByDay[dayKey] = dayTasks + dayLogs;
  }

  // Most common log types
  const logTypeCounts = {};
  plantLogs.forEach(l => {
    const type = String(l.log_type || 'note');
    logTypeCounts[type] = (logTypeCounts[type] || 0) + 1;
  });

  return {
    plant,
    ageInDays,
    ageWeeks,
    totalTasks,
    doneTasks,
    completionRate,
    totalLogs: plantLogs.length,
    photos: photos.length,
    waterLogs: waterLogs.length,
    totalWaterLiters: Math.round(totalWaterLiters * 10) / 10,
    avgWaterInterval,
    harvestLogs: harvestLogs.length,
    totalHarvestKg: Math.round(totalHarvestKg * 10) / 10,
    totalHarvestCount,
    logTypeCounts,
    activityByDay,
  };
}

function renderGrowthAnalytics() {
  const container = document.getElementById('analyticsPanel');
  if (!container) return;
  if (!selectedPlantId) { container.innerHTML = ''; return; }

  const stats = calculatePlantAnalytics(selectedPlantId);
  if (!stats) { container.innerHTML = ''; return; }

  const entry = catalogById(stats.plant.catalog_id);

  container.innerHTML = `
    <div class="section-title">\u{1F4CA} ${t('analytics.title')}</div>

    <!-- Key metrics -->
    <div style="display:grid;grid-template-columns:repeat(auto-fit, minmax(100px, 1fr));gap:8px;margin-bottom:12px;">
      <div style="text-align:center;padding:10px;border-radius:12px;background:${TC.surface};border:1px solid rgba(${TC.accentRgb},0.08);">
        <div style="font-size:22px;font-weight:700;color:${TC.accent};">${stats.ageInDays}</div>
        <div style="font-size:10px;color:${TC.textDim};">${t('analytics.days')}</div>
      </div>
      <div style="text-align:center;padding:10px;border-radius:12px;background:${TC.surface};border:1px solid rgba(${TC.accentRgb},0.08);">
        <div style="font-size:22px;font-weight:700;color:${TC.accent};">${stats.completionRate}%</div>
        <div style="font-size:10px;color:${TC.textDim};">${t('analytics.tasksDone')}</div>
      </div>
      <div style="text-align:center;padding:10px;border-radius:12px;background:${TC.surface};border:1px solid rgba(${TC.accentRgb},0.08);">
        <div style="font-size:22px;font-weight:700;color:${TC.info};">${stats.totalWaterLiters}L</div>
        <div style="font-size:10px;color:${TC.textDim};">${t('analytics.totalWaterLabel')}</div>
      </div>
      <div style="text-align:center;padding:10px;border-radius:12px;background:${TC.surface};border:1px solid rgba(${TC.accentRgb},0.08);">
        <div style="font-size:22px;font-weight:700;color:${TC.warning};">${stats.totalHarvestKg ? stats.totalHarvestKg : stats.totalHarvestCount ? stats.totalHarvestCount : '\u2013'}</div>
        <div style="font-size:10px;color:${TC.textDim};">${stats.totalHarvestKg ? t('analytics.harvestKg') : stats.totalHarvestCount ? t('analytics.harvestCount') : t('analytics.harvest')}</div>
      </div>
      <div style="text-align:center;padding:10px;border-radius:12px;background:${TC.surface};border:1px solid rgba(${TC.accentRgb},0.08);">
        <div style="font-size:22px;font-weight:700;color:${TC.text};">${stats.photos}</div>
        <div style="font-size:10px;color:${TC.textDim};">${t('analytics.photosLabel')}</div>
      </div>
      <div style="text-align:center;padding:10px;border-radius:12px;background:${TC.surface};border:1px solid rgba(${TC.accentRgb},0.08);">
        <div style="font-size:22px;font-weight:700;color:${TC.text};">${stats.totalLogs}</div>
        <div style="font-size:10px;color:${TC.textDim};">${t('analytics.logsLabel')}</div>
      </div>
    </div>

    <!-- Watering insight -->
    ${stats.avgWaterInterval ? `
      <div style="padding:10px 14px;border-radius:10px;background:rgba(100,180,255,0.06);border:1px solid rgba(100,180,255,0.12);margin-bottom:10px;font-size:12px;">
        <span style="color:${TC.info};font-weight:600;">💧 ${t('analytics.wateringRhythm')}:</span>
        <span style="color:${TC.textMuted};"> ${t('analytics.wateringDetail', stats.avgWaterInterval, Math.round(stats.avgWaterInterval / 24), stats.waterLogs)}</span>
        ${entry && stats.avgWaterInterval < (entry.water === 'low' ? 48 : entry.water === 'high' ? 16 : 24)
          ? '<div style="color:' + TC.warning + ';margin-top:4px;">' + t('analytics.waterTooOften', escapeHtml(entry.name_de)) + '</div>'
          : ''}
        ${entry && stats.avgWaterInterval > (entry.water === 'high' ? 72 : entry.water === 'low' ? 200 : 120)
          ? '<div style="color:' + TC.warning + ';margin-top:4px;">' + t('analytics.waterTooRarely', escapeHtml(entry.name_de)) + '</div>'
          : ''}
      </div>
    ` : ''}

    <!-- Activity heatmap (last 90 days) -->
    <div style="margin-bottom:10px;">
      <div style="font-size:11px;color:${TC.textDim};margin-bottom:4px;">${t('analytics.activity90')}</div>
      <div style="display:flex;gap:1px;flex-wrap:wrap;">
        ${Object.entries(stats.activityByDay).reverse().map(([day, count]) => {
          const opacity = count === 0 ? 0.05 : Math.min(0.15 + count * 0.2, 0.8);
          return '<div style="width:8px;height:8px;border-radius:2px;background:rgba(' + TC.accentRgb + ',' + opacity + ');" title="' + escapeHtml(day) + ': ' + t('analytics.activitiesTooltip', count) + '"></div>';
        }).join('')}
      </div>
    </div>

    <!-- Log type distribution -->
    ${Object.keys(stats.logTypeCounts).length > 0 ? `
      <div style="font-size:11px;color:${TC.textDim};margin-bottom:4px;">${t('analytics.logDistribution')}</div>
      <div style="display:flex;gap:6px;flex-wrap:wrap;margin-bottom:10px;">
        ${Object.entries(stats.logTypeCounts).map(([type, count]) =>
          '<span class="cal-badge">' + escapeHtml(type) + ': ' + count + '</span>'
        ).join('')}
      </div>
    ` : ''}
  `;
}

function renderCareInsights() {
  const container = document.getElementById('careInsightsPanel');
  if (!container) return;

  const activePlants = plants.filter(p => isPlantActive(p));
  if (!activePlants.length) { container.innerHTML = ''; return; }

  const insights = [];

  for (const plant of activePlants) {
    const entry = catalogById(plant.catalog_id);
    const pLogs = allLogs.filter(l => l.plant_id === plant.plant_id);
    const waterLogs = pLogs.filter(l => l.metrics && l.metrics.water_liters);

    // Over/under-watering detection (only with >= 2 water logs)
    if (waterLogs.length >= 2) {
      const intervals = [];
      const sorted = [...waterLogs].sort((a, b) => new Date(a.created_at) - new Date(b.created_at));
      for (let i = 1; i < sorted.length; i++) {
        intervals.push((new Date(sorted[i].created_at) - new Date(sorted[i-1].created_at)) / 3600000);
      }
      const avgInterval = intervals.reduce((a, b) => a + b, 0) / intervals.length;

      if (entry) {
        const idealMin = entry.water === 'low' ? 72 : entry.water === 'high' ? 20 : 36;
        if (avgInterval < idealMin * 0.6) {
          insights.push({
            type: 'warning',
            plant: plant.name,
            emoji: entry.emoji || '🌱',
            text: t('insight.overWater.text', Math.round(avgInterval), idealMin),
            tip: t('insight.overWater.tip'),
          });
        }
        if (avgInterval > idealMin * 2.5) {
          insights.push({
            type: 'info',
            plant: plant.name,
            emoji: entry.emoji || '🌱',
            text: t('insight.underWater.text', Math.round(avgInterval)),
            tip: entry.water === 'high' ? t('insight.underWater.tipHigh') : t('insight.underWater.tipLow'),
          });
        }
      }
    }

    // No activity in 14 days
    const lastLog = pLogs.sort((a, b) => new Date(b.created_at) - new Date(a.created_at))[0];
    if (lastLog) {
      const daysSinceActivity = (Date.now() - new Date(lastLog.created_at).getTime()) / 86400000;
      if (daysSinceActivity > 14) {
        insights.push({
          type: 'warning',
          plant: plant.name,
          emoji: (entry && entry.emoji) || '🌱',
          text: t('insight.inactive.text', Math.round(daysSinceActivity)),
          tip: t('insight.inactive.tip'),
        });
      }
    }

    // No photos — plantPhotos only ever holds the selected plant's photos,
    // so this insight is only meaningful for that one plant.
    const plantPhotoCount = plantPhotos.filter(p => p.plant_id === plant.plant_id).length;
    if (plant.plant_id === selectedPlantId && plantPhotoCount === 0) {
      insights.push({
        type: 'tip',
        plant: plant.name,
        emoji: (entry && entry.emoji) || '🌱',
        text: t('insight.noPhotos.text'),
        tip: t('insight.noPhotos.tip'),
      });
    }
  }

  if (!insights.length) {
    container.innerHTML = `
      <div class="section-title">\u{1F4A1} ${t('analytics.insights')}</div>
      <div style="padding:12px;border-radius:12px;background:rgba(${TC.accentRgb},0.06);border:1px solid rgba(${TC.accentRgb},0.1);font-size:13px;color:${TC.textMuted};">
        ${t('analytics.allGood')}
      </div>
    `;
    return;
  }

  container.innerHTML = `
    <div class="section-title">\u{1F4A1} ${t('analytics.insights')} (${insights.length})</div>
    <div style="display:grid;gap:8px;">
      ${insights.map(i => `
        <div style="padding:10px 14px;border-radius:10px;background:${i.type === 'warning' ? 'rgba(255,200,100,0.06)' : i.type === 'info' ? 'rgba(100,180,255,0.06)' : 'rgba(' + TC.accentRgb + ',0.04)'};border:1px solid ${i.type === 'warning' ? 'rgba(255,200,100,0.15)' : i.type === 'info' ? 'rgba(100,180,255,0.12)' : 'rgba(' + TC.accentRgb + ',0.08)'};font-size:12px;">
          <div style="color:${TC.text};">${i.emoji} <strong>${escapeHtml(i.plant)}</strong> ${escapeHtml(i.text)}</div>
          <div style="color:${TC.textDim};margin-top:3px;font-size:11px;">💡 ${escapeHtml(i.tip)}</div>
        </div>
      `).join('')}
    </div>
  `;
}

function renderPhotoGallery() {
  const container = document.getElementById('photoGallery');
  if (!container || !selectedPlantId) { if (container) container.innerHTML = ''; return; }

  const photos = [...plantPhotos].sort((a, b) => new Date(a.taken_at) - new Date(b.taken_at));

  // Calculate growth duration
  const plant = selectedPlant();
  const startDate = plant ? new Date(plant.start_date) : null;
  const daysSinceStart = startDate ? Math.floor((Date.now() - startDate.getTime()) / 86400000) : null;

  // Web: uploading is a paid feature, and after a paid plan ends the photos
  // are deleted on a fixed date \u2014 say so, and offer everything as a download.
  const uploadLocked = !IS_TAURI && webPhotoStatus && !webPhotoStatus.can_upload;
  const deletionAt = !IS_TAURI && webPhotoStatus?.deletion_scheduled_at
    ? new Date(webPhotoStatus.deletion_scheduled_at)
    : null;

  container.innerHTML = `
    <div style="display:flex;justify-content:space-between;align-items:center;gap:8px;flex-wrap:wrap;margin-bottom:10px;">
      <div class="section-title" style="margin:0;">${t('timeline.title')} (${t('timeline.photos', photos.length)}${daysSinceStart != null ? ' \u00b7 ' + t('photo.dayLabel', daysSinceStart) : ''})</div>
      ${uploadLocked
        ? `<span style="font-size:12px;color:${TC.textMuted};">${t('photo.requiresSubscription')}</span>`
        : `<button class="btn btn--sm btn--primary" id="addPhotoBtn">${t('photo.add')}</button>`}
    </div>
    ${deletionAt ? `
      <div role="alert" style="display:flex;align-items:center;justify-content:space-between;gap:10px;flex-wrap:wrap;margin-bottom:12px;padding:10px 12px;border-radius:12px;background:${TC.warningBg};border:1px solid ${TC.warning};color:${TC.text};font-size:13px;">
        <span>${t('photo.deletionBanner', deletionAt.toLocaleDateString(localeTag(), { day: 'numeric', month: 'long', year: 'numeric' }))}</span>
        <button class="btn btn--sm" id="downloadAllPhotos">${t('photo.downloadAll')}</button>
      </div>` : ''}
    ${photos.length >= 2 ? `
      <div style="display:flex;align-items:center;gap:8px;margin-bottom:12px;padding:10px;border-radius:12px;background:rgba(${TC.accentRgb},0.06);border:1px solid rgba(${TC.accentRgb},0.1);">
        <span style="font-size:14px;">📈</span>
        <span style="font-size:12px;color:${TC.textMuted};">
          ${t('photo.first', new Date(photos[0].taken_at).toLocaleDateString(localeTag()))} \u00b7
          ${t('photo.last', new Date(photos[photos.length-1].taken_at).toLocaleDateString(localeTag()))} \u00b7
          ${t('photo.range', Math.ceil((new Date(photos[photos.length-1].taken_at) - new Date(photos[0].taken_at)) / 86400000))}
        </span>
      </div>
    ` : ''}
    ${photos.length ? `
      <div style="position:relative;padding-left:24px;border-left:2px solid rgba(${TC.accentRgb},0.2);">
        ${photos.map((p, i) => {
          const date = new Date(p.taken_at);
          const dayNum = startDate ? Math.floor((date.getTime() - startDate.getTime()) / 86400000) : null;
          const prevDate = i > 0 ? new Date(photos[i-1].taken_at) : null;
          const daysBetween = prevDate ? Math.floor((date.getTime() - prevDate.getTime()) / 86400000) : null;
          return `
            <div style="position:relative;margin-bottom:16px;">
              <div style="position:absolute;left:-29px;top:8px;width:10px;height:10px;border-radius:50%;background:${TC.accent};border:2px solid ${TC.cardSolid};"></div>
              <div style="display:flex;gap:4px;margin-bottom:4px;font-size:11px;color:${TC.textDim};">
                <span>${date.toLocaleDateString(localeTag())}</span>
                ${dayNum != null ? '<span>\u00b7 ' + t('photo.dayLabel', dayNum) + '</span>' : ''}
                ${daysBetween != null && daysBetween > 0 ? '<span>\u00b7 +' + daysBetween + ' ' + t('analytics.days') + '</span>' : ''}
              </div>
              <div style="display:flex;gap:10px;align-items:flex-start;">
                <img src="" data-photo-src="${escapeHtml(p.file_path)}" style="width:140px;height:105px;object-fit:cover;border-radius:10px;border:1px solid rgba(${TC.accentRgb},0.1);background:${TC.surface};" />
                <div style="flex:1;">
                  ${p.caption ? '<div style="font-size:13px;color:' + TC.text + ';margin-bottom:4px;">' + escapeHtml(p.caption) + '</div>' : ''}
                  <button class="btn btn--sm btn--ghost" data-delete-photo="${p.photo_id}" style="font-size:10px;">${t('action.remove')}</button>
                </div>
              </div>
            </div>
          `;
        }).join('')}
      </div>
    ` : `
      <div class="empty-state" style="margin-top:8px;">
        <div style="font-size:24px;margin-bottom:8px;">📷</div>
        <div>${t('photo.empty')}</div>
        <div style="font-size:11px;color:${TC.textDim};margin-top:4px;">${t('photo.emptyTip')}</div>
      </div>
    `}
  `;

  // Load actual photo paths via asset protocol
  container.querySelectorAll('[data-photo-src]').forEach(async (img) => {
    try {
      const fullPath = await invoke("get_photo_path", { photoPath: img.dataset.photoSrc });
      img.src = convertFileSrc(fullPath);
    } catch (_) {
      img.alt = t('photo.notFoundAlt');
    }
  });

  document.getElementById('downloadAllPhotos')?.addEventListener('click', async (e) => {
    const btn = e.currentTarget;
    btn.disabled = true;
    try {
      await invoke("web_photo_archive");
    } catch (err) {
      alert(t('photo.downloadFailed', err?.message || err));
    } finally {
      btn.disabled = false;
    }
  });

  // Add photo button
  document.getElementById('addPhotoBtn')?.addEventListener('click', async () => {
    try {
      const selected = await tauriOpenDialog({
        multiple: false,
        filters: [{ name: t('log.photoDialogFilter'), extensions: ['jpg', 'jpeg', 'png', 'gif', 'webp', 'bmp'] }],
        title: t('log.photoDialogTitle'),
      });
      if (!selected) return; // User cancelled
      const filePath = typeof selected === 'string' ? selected : selected.path;
      if (!filePath) return;
      const caption = prompt(t('photo.captionPrompt')) || '';
      await invoke("add_photo", {
        plantId: selectedPlantId,
        sourcePath: filePath,
        caption: caption || null,
      });
      playNotificationSound('success');
      await loadPlantPhotos();
      renderPhotoGallery();
    } catch (err) {
      alert('Foto konnte nicht gespeichert werden: ' + (err?.message || err));
    }
  });

  // Delete photo buttons
  container.querySelectorAll('[data-delete-photo]').forEach(btn => {
    btn.addEventListener('click', async (e) => {
      e.stopPropagation();
      if (!confirm(t('photo.confirmDelete'))) return;
      try {
        await invoke("delete_photo", { photoId: btn.dataset.deletePhoto });
        await loadPlantPhotos();
        renderPhotoGallery();
      } catch (err) {
        alert('Loeschen fehlgeschlagen: ' + (err?.message || err));
      }
    });
  });
}

async function loadAllTasks() {
  const grouped = await Promise.all(plants.map((plant) => invoke("list_current_tasks", { plantId: plant.plant_id }).catch(() => [])));
  allTasks = grouped.flat();
}

async function loadAllLogs() {
  const grouped = await Promise.all(plants.map((plant) => invoke("list_current_logs", { plantId: plant.plant_id }).catch(() => [])));
  allLogs = grouped.flat();
}

async function refreshAll(options = {}) {
  // Sign-in and sign-out both end in a refresh, so this keeps the account
  // controls in step without touching every auth handler. Local only, no request.
  await refreshWebSessionState();
  const { autoReminderReason = null } = options;
  showLoading(t('misc.loading'));
  try {
    await refreshStatus();
    await loadCatalog();
    await loadProfiles();
    await loadConsents();
    plants = await invoke("list_current_plants");
    if (selectedPlantId && !plants.some((p) => p.plant_id === selectedPlantId)) selectedPlantId = null;
    if (!selectedPlantId && plants.length) selectedPlantId = plants[0].plant_id;
    renderPlants();
    await Promise.all([loadTasks(), loadLogs(), loadAllTasks(), loadAllLogs(), loadDevices(), loadLocations(), loadPlantPhotos(), loadClimateConfig()]);
    await loadLatestReadings();
    refreshExpandedTasks();
    renderTimelinePanel();
    renderPhaseTaskSets();
    updateOnboardingVisibility();
    renderCalendarToolbar();
    renderPlantContextBar();
    renderSensorWidget();
    renderDeviceManagementPanel();
    renderLocationsView();
    renderPhotoGallery();
    renderGrowthAnalytics();
    renderCareInsights();
    renderClimateSettings();
    renderSeasonAlerts();
    renderTipsView();
    renderProfileIndicator();
    loadWeather();
    if (currentView === 'weather') renderWeatherDashboard();
    if (focusedDayKey) showDayDetail(focusedDayKey);
    if (autoReminderReason) {
      await autoCheckDueNotifications(autoReminderReason);
    }
  } finally {
    hideLoading();
  }
}

function friendlyAuthError(err) {
  const msg = String(err?.message || err);
  if (msg.includes("Connection refused") || msg.includes("error sending request") || msg.includes("tcp connect"))
    return t('auth.backendUnreachable');
  return msg;
}

// Auth modal button
$("openAuthModal")?.addEventListener("click", () => openModal('auth'));
$("webLogout")?.addEventListener("click", async () => {
  try {
    await invoke("web_logout");
  } catch (err) {
    logOut(String(err));
  }
  plants = [];
  allTasks = [];
  allLogs = [];
  selectedPlantId = null;
  await refreshWebSessionState();
  await refreshAll({ autoReminderReason: "web_logout" });
  openModal('auth');
});

// Update auth status display
function updateAuthStatusDisplay() {
  const el = document.getElementById('authStatusDisplay');
  if (!el) return;
  if (currentPlanSource === 'account') {
    el.innerHTML = `<div style="display:flex;align-items:center;gap:8px;"><span style="width:8px;height:8px;border-radius:50%;background:${TC.accent};display:inline-block;"></span><span style="color:${TC.text};">${t('auth.status.loggedIn')}</span><span style="color:${TC.textMuted};font-size:12px;">${t('auth.status.planSuffix', escapeHtml(planLabel(currentPlan)))}</span></div>`;
  } else {
    el.innerHTML = `<div style="color:${TC.textMuted};font-size:13px;">${t('auth.status.guest')}</div>`;
  }
}

$("bootstrap")?.addEventListener("click", async () => {
  try {
    logOut(await invoke("bootstrap_local_user"));
    await refreshAll({ autoReminderReason: "guest_bootstrap" });
    await maybeShowConsentModal();
  } catch (err) {
    logOut(t('auth.guestStartFailed', err?.message || err));
    const statusEl = $("status");
    if (statusEl) statusEl.textContent = t('auth.guestStartFailed', err?.message || err);
  }
});

$("register")?.addEventListener("click", async () => {
  const email = $("email")?.value?.trim();
  const password = $("password")?.value;
  const backendUrl = $("backendUrl")?.value?.trim();
  if (!email || !password) {
    logOut(t('auth.emailPasswordRequired'));
    const statusEl = $("status");
    if (statusEl) statusEl.textContent = t('auth.emailPasswordRequired');
    return;
  }
  try {
    const result = await invoke("auth_register", { payload: { email, password, backend_url: backendUrl || null } });
    logOut(result);
    const statusEl = $("status");
    if (statusEl) statusEl.textContent = t('auth.registerSuccess');
    await refreshAll({ autoReminderReason: "auth_change" });
    await maybeShowConsentModal();
  } catch (err) {
    const msg = friendlyAuthError(err);
    logOut(t('auth.registerFailed', msg));
    const statusEl = $("status");
    if (statusEl) statusEl.textContent = t('auth.registerFailed', msg);
  }
});

$("login")?.addEventListener("click", async () => {
  const email = $("email")?.value?.trim();
  const password = $("password")?.value;
  const backendUrl = $("backendUrl")?.value?.trim();
  if (!email || !password) {
    logOut(t('auth.emailPasswordRequired'));
    const statusEl = $("status");
    if (statusEl) statusEl.textContent = t('auth.emailPasswordRequired');
    return;
  }
  try {
    const result = await invoke("auth_login", { payload: { email, password, backend_url: backendUrl || null } });
    logOut(result);
    const statusEl = $("status");
    if (statusEl) statusEl.textContent = t('auth.loginSuccess');
    await refreshAll({ autoReminderReason: "auth_change" });
  } catch (err) {
    const msg = friendlyAuthError(err);
    logOut(t('auth.loginFailed', msg));
    const statusEl = $("status");
    if (statusEl) statusEl.textContent = t('auth.loginFailed', msg);
  }
});

$("me")?.addEventListener("click", async () => {
  try {
    logOut(await invoke("auth_fetch_me"));
    await refreshStatus();
  } catch (err) {
    logOut(t('auth.meFailed', err?.message || err));
  }
});

$("syncNow")?.addEventListener("click", async () => {
  if (!hasConsent('cloud_sync')) {
    notifyConsentMissing('cloud_sync', 'consent.missingSyncToast');
    return;
  }
  showLoading(t('sync.inProgress'));
  try {
    logOut(await invoke("sync_now"));
    await refreshAll({ autoReminderReason: "sync_now" });
  } catch (err) {
    logOut(t('sync.failed', err?.message || err));
  } finally {
    hideLoading();
  }
});

subscriptionPanel?.addEventListener("click", async (event) => {
  const webhookPlan = event.target?.dataset?.checkoutWebhookPlan;
  if (webhookPlan) {
    try {
      setCheckoutState({ status: "idle", plan: webhookPlan, message: t('billing.webhook.simulating', planLabel(webhookPlan)), checkoutUrl: checkoutState.checkoutUrl || null });
      addCheckoutHistoryEntry({ mode: 'webhook', status: 'pending', plan: webhookPlan, message: t('billing.webhook.simulating', planLabel(webhookPlan)) });
      renderSubscriptionPanel();
      const result = await invoke("subscription_webhook_stub_smart", { plan: webhookPlan, checkoutStatus: "paid" });
      setCheckoutState({
        status: "ready",
        plan: webhookPlan,
        message: result?.message || t('billing.webhook.processed', planLabel(webhookPlan)),
        checkoutUrl: checkoutState.checkoutUrl || null,
      });
      addCheckoutHistoryEntry({ mode: 'webhook', status: 'success', plan: webhookPlan, message: result?.message || t('billing.webhook.processed', planLabel(webhookPlan)) , checkoutId: result?.checkout_id || result?.checkoutId || null });
      logOut(result);
      await refreshAll({ autoReminderReason: "subscription_webhook" });
    } catch (err) {
      setCheckoutState({ status: "error", plan: webhookPlan, message: t('billing.webhook.failed', err), checkoutUrl: checkoutState.checkoutUrl || null });
      addCheckoutHistoryEntry({ mode: 'webhook', status: 'error', plan: webhookPlan, message: t('billing.webhook.failed', err) });
      renderSubscriptionPanel();
      logOut(t('billing.webhook.failed', err));
    }
    return;
  }

  const plan = event.target?.dataset?.planAction;
  if (!plan) return;
  if (plan !== 'free' && plan !== 'basic' && !hasConsent('billing_stripe')) {
    notifyConsentMissing('billing_stripe', 'consent.missingBillingToast');
    return;
  }
  const preview = planPreviewFor(plan);
  const confirmed = window.confirm([
    `${t('billing.confirmCheckout.title', preview.title)}${preview.price ? ` (${preview.price})` : ""}`,
    "",
    preview.summary,
    preview.cleanupHint,
    "",
    t('billing.confirmCheckout.question'),
  ].join("\n"));
  if (!confirmed) return;
  showLoading(t('billing.checkout.preparing'));
  try {
    setCheckoutState({ status: "idle", plan, message: t('billing.checkout.pending', preview.title), checkoutUrl: null });
    addCheckoutHistoryEntry({ mode: 'checkout', status: 'pending', plan, message: t('billing.checkout.pending', preview.title) });
    renderSubscriptionPanel();
    const result = await invoke("subscription_checkout_smart", { plan });
    setCheckoutState({
      status: "ready",
      plan,
      message: result?.message || t('billing.checkout.readyExtended', preview.title),
      checkoutUrl: result?.checkout_url || result?.url || null,
    });
    addCheckoutHistoryEntry({ mode: 'checkout', status: 'success', plan, message: result?.message || t('billing.checkout.ready', preview.title), checkoutId: result?.checkout_id || result?.checkoutId || null });
    logOut(result);
    await refreshAll({ autoReminderReason: "plan_change" });
  } catch (err) {
    setCheckoutState({ status: "error", plan, message: t('billing.checkout.failed', err), checkoutUrl: null });
    addCheckoutHistoryEntry({ mode: 'checkout', status: 'error', plan, message: t('billing.checkout.failed', err) });
    renderSubscriptionPanel();
    logOut(t('billing.planChangeFailed', err));
  } finally {
    hideLoading();
  }
});

phaseTaskSetBar?.addEventListener("click", async (event) => {
  const key = event.target?.dataset?.phaseTaskSet;
  if (!key) return;
  await createTaskSet(key);
});

// Init
loadPlantFilterPrefs();
applyPlantFilterPrefsToControls();
renderAppNav();
renderCalendarToolbar();
renderFab();
renderOnboardingOverlayText();
applyPageView();

// FAB events
document.getElementById('fabMain')?.addEventListener('click', toggleFab);
document.getElementById('fabMenu')?.addEventListener('click', (e) => {
  const action = e.target.closest('[data-fab-action]')?.dataset.fabAction;
  if (action) { closeFab(); openModal(action); }
});

// Modal events
document.getElementById('modalClose')?.addEventListener('click', closeModal);
document.getElementById('modalOverlay')?.addEventListener('click', (e) => {
  if (e.target === e.currentTarget) closeModal();
});

// Onboarding submit
document.getElementById('onboardSubmit')?.addEventListener('click', async () => {
  const name = $("onboardPlantName")?.value?.trim();
  if (!name) {
    const err = document.getElementById('onboardError');
    if (err) { err.textContent = t('onboard.nameRequired'); err.style.display = ''; }
    return;
  }
  try {
    const plant = await invoke("create_plant_smart", { payload: {
      name,
      color_tag: $("onboardPlantColor")?.value || '#0B3D2E',
      phase: $("onboardPlantPhase")?.value || 'veg',
      phase_week: 1,
    }});
    selectedPlantId = plant.plant_id;
    await refreshAll({ autoReminderReason: "plant_create" });
  } catch (err) {
    const errEl = document.getElementById('onboardError');
    if (errEl) { errEl.textContent = t('action.genericError', err); errEl.style.display = ''; }
  }
});

// Keyboard shortcuts
document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') {
    if (activeModal) closeModal();
    else if (fabOpen) closeFab();
    else if (focusedDayKey) closeDayDetail();
  }
});

// Keep existing filter bindings
bindPlantFilterControls();

// Load data. In the browser, first exchange the refresh cookie for a session
// so the initial load already sees the user's data.
const startupSession = IS_TAURI ? Promise.resolve() : invoke("bootstrap_local_user").catch(() => {});
startupSession
  .then(() => refreshWebSessionState())
  .then(() => refreshAll({ autoReminderReason: "app_start" }))
  .then(() => {
    // Show first-run consent modal if no consent records exist yet.
    try { maybeShowConsentModal(); } catch (_) {}
    // The browser has no local database, so it needs an account to store anything.
    if (!IS_TAURI && !webSignedIn) openModal('auth');
  }).catch((err) => {
  const statusEl = document.getElementById('status');
  if (statusEl) statusEl.textContent = t('app.initError', err);
  logOut(String(err));
  // Still render calendar and show onboarding even if invoke failed
  renderCalendarToolbar();
  renderTimelinePanel();
  updateOnboardingVisibility();
});


function bindPlantFilterControls() {
  $("plantSearch")?.addEventListener("input", (event) => {
    plantSearchQuery = event.target.value || "";
    persistPlantFilterPrefs();
    renderPlants();
  });
  $("plantPhaseFilter")?.addEventListener("change", (event) => {
    plantPhaseFilter = event.target.value || "all";
    persistPlantFilterPrefs();
    renderPlants();
  });
  $("plantStateFilter")?.addEventListener("change", (event) => {
    plantStateFilter = event.target.value || "all";
    persistPlantFilterPrefs();
    renderPlants();
  });
  $("plantSort")?.addEventListener("change", (event) => {
    plantSort = event.target.value || "updated_desc";
    persistPlantFilterPrefs();
    renderPlants();
  });
}


loadCheckoutHistory();

// Phase 4b: Background notification check (every 5 minutes)
// Guard against duplicate intervals from Vite HMR reloads
if (!window.__notifyIntervalSet) {
  window.__notifyIntervalSet = true;
  setInterval(async () => {
    try {
      const sent = await invoke("check_and_send_notifications");
      if (sent > 0) {
        playNotificationSound('reminder');
        console.log(`[notifications] sent ${sent}`);
      }
    } catch (_) {}
  }, 5 * 60 * 1000);

  setTimeout(async () => {
    try {
      const sent = await invoke("check_and_send_notifications");
      if (sent > 0) playNotificationSound('reminder');
    } catch (_) {}
  }, 10_000);
}

// Weather auto-logging interval (every 60 minutes)
if (!window.__weatherLogIntervalSet) {
  window.__weatherLogIntervalSet = true;
  setInterval(async () => {
    if (!climateConfig?.latitude) return;
    if (!hasConsent('weather')) return;
    try {
      await invoke("fetch_and_log_weather", {
        lat: climateConfig.latitude,
        lon: climateConfig.longitude,
      });
      // Also log for locations with coordinates
      for (const loc of locations) {
        if (loc.latitude && loc.longitude) {
          await invoke("fetch_and_log_weather", {
            locationId: loc.location_id,
            lat: loc.latitude,
            lon: loc.longitude,
          });
        }
      }
    } catch (_) {}
  }, 60 * 60 * 1000);
}


// Retention sweep on startup.
//
// Pruning previously ran only when the user *changed* the retention setting, so
// a user who never touched it kept every row forever. `device_readings` had no
// caller at all and grew unbounded with every sensor poll.
if (!window.__retentionSweepDone) {
  window.__retentionSweepDone = true;
  setTimeout(async () => {
    const days = getStoredRetentionDays();
    try {
      await invoke("prune_weather_history_cmd", { days });
    } catch (err) {
      console.warn("weather history prune failed", err);
    }
    try {
      await invoke("prune_old_readings", { days });
    } catch (err) {
      console.warn("device reading prune failed", err);
    }
  }, 5_000);
}
