// Screenshots of the web app for the website, in German and English.
// Needs the app (vite, http://localhost:1420) and a local backend with the
// demo accounts from seed_demo.py. Uses the installed Chrome or Edge.
//
//   DEMO_PW=... node scripts/screenshots.mjs
import fs from "node:fs";
import path from "node:path";
import puppeteer from "puppeteer-core";

const APP = process.env.APP_URL || "http://localhost:1420";
const PW = process.env.DEMO_PW;
if (!PW) throw new Error("set DEMO_PW");
const BROWSERS = [
  "C:/Program Files/Google/Chrome/Application/chrome.exe",
  "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
];
const executablePath = BROWSERS.find((p) => fs.existsSync(p));
const OUT = path.resolve("src/assets/screens");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function selectTomato(page) {
  await page.evaluate(() => {
    const sel = document.getElementById("plantContextSelect");
    const opt = [...(sel?.options || [])].find((o) => /Tomat/.test(o.text));
    if (opt) { sel.value = opt.value; sel.dispatchEvent(new Event("change", { bubbles: true })); }
  });
  await sleep(600);
}

async function session(browser, lang, viewport) {
  // Own context per session: the refresh cookie of one account must not
  // sign the next session in as the same account.
  const context = await browser.createBrowserContext();
  const page = await context.newPage();
  await page.setViewport(viewport);
  await page.evaluateOnNewDocument((l) => {
    localStorage.setItem("sowling_lang", l);
    // First-run dialogs that would cover the app in a screenshot.
    localStorage.setItem("pc_web_pending_consents", JSON.stringify({ cloud_sync: true, weather: true, devices: true, billing_stripe: true }));
    localStorage.setItem("pc_install_banner_dismissed", "1");
  }, lang);
  await page.goto(APP, { waitUntil: "networkidle2" });
  await sleep(1500);
  const signedIn = await page.evaluate(() => document.getElementById("openAuthModal")?.style.display === "none");
  if (!signedIn) {
    await page.evaluate(() => {
      if (document.getElementById("modalOverlay")?.style.display === "none") document.getElementById("openAuthModal")?.click();
    });
    await page.waitForSelector("#authModalEmail", { visible: true });
    await page.type("#authModalEmail", `demo-${lang}@example.test`);
    await page.type("#authModalPassword", PW);
    await page.click("#authModalLogin");
    await sleep(3500);
  }
  return page;
}

async function shot(page, lang, name, clip) {
  await page.evaluate(() => document.activeElement?.blur());
  const dir = path.join(OUT, lang);
  fs.mkdirSync(dir, { recursive: true });
  await page.screenshot({ path: path.join(dir, `${name}.png`), ...(clip ? { clip } : {}) });
  console.log(lang, name);
}

const view = (page, v) => page.evaluate((x) => document.querySelector(`[data-view="${x}"]`)?.click(), v);

async function modalClip(page) {
  return page.evaluate(() => {
    const r = document.querySelector("#modalOverlay .modal-content").getBoundingClientRect();
    const pad = 24;
    return { x: Math.max(0, r.x - pad), y: Math.max(0, r.y - pad), width: r.width + pad * 2, height: Math.min(r.height + pad * 2, window.innerHeight - Math.max(0, r.y - pad)) };
  });
}

const browser = await puppeteer.launch({ executablePath, headless: true, args: ["--lang=de-DE"] });
try {
  for (const lang of ["de", "en"]) {
    const desktop = { width: 1440, height: 900, deviceScaleFactor: 1.5 };
    const page = await session(browser, lang, desktop);

    await selectTomato(page);
    await view(page, "calendar"); await sleep(800);
    await page.evaluate(() => document.getElementById("timelineModeDay")?.click());
    await sleep(1200);
    await shot(page, lang, "calendar-day");
    await page.evaluate(() => document.getElementById("timelineModeWeek")?.click());
    await sleep(1200);
    await shot(page, lang, "calendar");
    await page.evaluate(() => document.getElementById("timelineModeMonth")?.click());
    await sleep(800);
    await shot(page, lang, "calendar-month");
    await view(page, "plants"); await sleep(1200);
    await shot(page, lang, "plants");
    await view(page, "weather"); await sleep(5000);
    await shot(page, lang, "weather");
    await view(page, "tips"); await sleep(1200);
    await shot(page, lang, "tips");
    await view(page, "billing"); await sleep(1500);
    await shot(page, lang, "plans");

    // Fertilizer log with two products.
    await view(page, "calendar"); await sleep(600);
    await page.evaluate(() => document.querySelector('[data-fab-action="log"]').click());
    await sleep(500);
    await page.evaluate(() => {
      const sel = document.getElementById("logType");
      sel.value = "feeding"; sel.dispatchEvent(new Event("change"));
      const plant = document.getElementById("logPlantSelect");
      const tomato = [...plant.options].find((o) => /Tomat/.test(o.text));
      if (tomato) plant.value = tomato.value;
      const set = (row, k, v) => { row.querySelector(`[data-feed="${k}"]`).value = v; };
      let row = document.querySelector("#logFeeds [data-feed-row]");
      set(row, "brand", "Hanfjack"); set(row, "product", "Blüte"); set(row, "amount", "3");
      document.querySelector("#logFeeds [data-feed-add]").click();
      row = document.querySelectorAll("#logFeeds [data-feed-row]")[1];
      set(row, "brand", "BioBizz"); set(row, "product", "Top·Max"); set(row, "amount", "1");
      document.getElementById("logWaterLiters").value = "2";
      document.getElementById("logPh").value = "6.4";
      document.getElementById("logEc").value = "1.6";
      document.getElementById("logText").value = "";
    });
    await sleep(300);
    await shot(page, lang, "feeding-log", await modalClip(page));
    await page.evaluate(() => document.getElementById("modalCancel").click());

    // Tree catalog.
    await page.evaluate(() => document.querySelector('[data-fab-action="plant"]').click());
    await sleep(500);
    await page.evaluate(() => {
      document.querySelector('#catalogKind [data-kind="trees"]').click();
      const sel = document.getElementById("plantCatalogSelect");
      sel.value = "apple_elstar"; sel.dispatchEvent(new Event("change"));
    });
    await sleep(500);
    await shot(page, lang, "tree-catalog", await modalClip(page));
    await page.evaluate(() => document.getElementById("modalCancel").click());
    await page.browserContext().close();

    // Phone.
    const phone = await session(browser, lang, { width: 390, height: 844, deviceScaleFactor: 2, isMobile: true, hasTouch: true });
    await selectTomato(phone);
    await view(phone, "calendar"); await sleep(800);
    await phone.evaluate(() => document.getElementById("timelineModeDay")?.click());
    await sleep(1200);
    await shot(phone, lang, "phone-calendar");
    await view(phone, "plants"); await sleep(1200);
    await shot(phone, lang, "phone-plants");
    await phone.browserContext().close();
  }
} finally {
  await browser.close();
}
