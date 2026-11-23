// Printable plant labels with a QR code. Scanning a label opens the plant in
// the web app (https://app.sowling.com/?pflanze=<id>); the app then selects it
// after sign-in. Works offline: the QR code is generated here, printing goes
// through the browser's / WebView's print dialog.
//
// Plant passport (Gärtnerei plan): EU plant passport for trade under
// Regulation (EU) 2016/2031 and Implementing Regulation (EU) 2017/2313 –
// EU flag, "Pflanzenpass / Plant Passport", A botanical name, B registration
// number of the operator, C traceability code, D country of origin. The
// regulation sets no minimum size (its models are examples), so it also fits
// on 12, 18 or 24 mm label tape. The QR code only complements code C.
import qrcode from "qrcode-generator";

export const LABEL_APP_URL = "https://app.sowling.com";

export const plantLabelUrl = (plantId) => `${LABEL_APP_URL}/?pflanze=${encodeURIComponent(plantId)}`;

/** Label formats: page size, grid and the size of one label (mm). */
export const LABEL_FORMATS = {
  "a4-24": { page: "A4", cols: 3, rows: 8, w: 70, h: 37 },        // e.g. Avery Zweckform 3474
  "a4-8": { page: "A4", cols: 2, rows: 4, w: 105, h: 74 },        // large labels for stakes and benches
  "roll-62x29": { page: "62mm 29mm", cols: 1, rows: 1, w: 62, h: 29 }, // Brother DK-11209
  "roll-50x30": { page: "50mm 30mm", cols: 1, rows: 1, w: 50, h: 30 }, // common thermal labels
  "tape-24": { page: "75mm 24mm", cols: 1, rows: 1, w: 75, h: 24 },   // label tape 24 mm (e.g. Brother TZe)
  "tape-18": { page: "70mm 18mm", cols: 1, rows: 1, w: 70, h: 18 },   // label tape 18 mm
  "tape-12": { page: "64mm 12mm", cols: 1, rows: 1, w: 64, h: 12 },   // label tape 12 mm
};

/** QR code as an SVG string (black on white, quiet zone of 2 modules). */
export function qrSvg(text) {
  const qr = qrcode(0, "M");
  qr.addData(text);
  qr.make();
  const n = qr.getModuleCount();
  const q = 2;
  let d = "";
  for (let r = 0; r < n; r++) {
    for (let c = 0; c < n; c++) if (qr.isDark(r, c)) d += `M${c + q},${r + q}h1v1h-1z`;
  }
  const size = n + q * 2;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${size} ${size}" shape-rendering="crispEdges"><rect width="${size}" height="${size}" fill="#fff"/><path d="${d}" fill="#000"/></svg>`;
}

/** EU flag (3:2, twelve stars), colour or black-and-white as the regulation allows. */
export function euFlagSvg(mono = false) {
  const blue = mono ? "#fff" : "#003399";
  const star = mono ? "#000" : "#FFCC00";
  const stars = Array.from({ length: 12 }, (_, i) => {
    const a = (i * Math.PI) / 6;
    const cx = 30 + 13.3 * Math.sin(a);
    const cy = 20 - 13.3 * Math.cos(a);
    const pts = Array.from({ length: 10 }, (_, k) => {
      const r = k % 2 ? 1.05 : 2.6;
      const t = (k * Math.PI) / 5 - Math.PI / 2;
      return `${(cx + r * Math.cos(t)).toFixed(2)},${(cy + r * Math.sin(t)).toFixed(2)}`;
    }).join(" ");
    return `<polygon points="${pts}" fill="${star}"/>`;
  }).join("");
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 60 40"><rect width="60" height="40" fill="${blue}"${mono ? ' stroke="#000" stroke-width="1.5"' : ""}/>${stars}</svg>`;
}

const esc = (v) => String(v ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));

/** Default traceability code (C): year and the first 8 characters of the plant id. */
export const defaultTraceCode = (plant) => `${new Date(plant.created_at || Date.now()).getFullYear()}-${String(plant.plant_id).slice(0, 8).toUpperCase()}`;

/**
 * Print markup and CSS.
 * `items`: [{ id, name, variety, location, started, latin, trace }],
 * `show`: which lines to print, `texts`: { footer, title, passport },
 * `passport`: null for normal labels, or { reg, origin, mono, qr } for the
 * EU plant passport.
 */
export function labelSheet({ items, format, show, texts, passport = null }) {
  const f = LABEL_FORMATS[format] || LABEL_FORMATS["a4-24"];
  const perPage = f.cols * f.rows;
  const big = f.h >= 60;
  const tiny = f.h <= 18;
  const pad = big ? 5 : tiny ? 1 : 2.2;
  const qrSize = f.h - pad * 2;
  const normal = (it) => `
    <div class="label">
      <div class="qr">${qrSvg(plantLabelUrl(it.id))}</div>
      <div class="text">
        <div class="name">${esc(it.name)}</div>
        ${show.variety && it.variety ? `<div class="line">${esc(it.variety)}</div>` : ""}
        ${show.started && it.started ? `<div class="line">${esc(it.started)}</div>` : ""}
        ${show.location && it.location ? `<div class="line">${esc(it.location)}</div>` : ""}
        ${show.brand && !tiny ? `<div class="brand">${esc(texts.footer)}</div>` : ""}
      </div>
    </div>`;
  const pass = (it) => `
    <div class="label pp">
      <div class="pp-head">
        <div class="flag">${euFlagSvg(passport.mono)}</div>
        <div class="pp-title">${esc(texts.passport)}</div>
      </div>
      <div class="pp-body">
        <div><b>A</b> <i>${esc(it.latin || it.name)}</i></div>
        <div><b>B</b> ${esc(passport.reg)}</div>
        <div><b>C</b> ${esc(it.trace)}</div>
        <div><b>D</b> ${esc(passport.origin)}</div>
      </div>
      ${passport.qr ? `<div class="qr">${qrSvg(plantLabelUrl(it.id))}</div>` : ""}
    </div>`;
  const pages = [];
  for (let i = 0; i < items.length; i += perPage) pages.push(items.slice(i, i + perPage));
  // Passport text size scales with the label height: 4 lines plus margins.
  const ppLine = Math.max(4.2, Math.min(9, (f.h - pad * 2) / 4 * 2.2));
  const css = `
    @page { size: ${f.page}; margin: 0; }
    #labelPrint { display: none; }
    @media print {
      body > *:not(#labelPrint) { display: none !important; }
      html, body { margin: 0 !important; padding: 0 !important; background: #fff !important; }
      #labelPrint { display: block; font-family: Arial, Helvetica, sans-serif; color: #000; }
    }
    #labelPrint, #labelPrint * { box-sizing: border-box; color: #000; font-family: Arial, Helvetica, sans-serif; }
    #labelPrint .page { width: ${f.page === "A4" ? "210mm" : `${f.w}mm`}; height: ${f.page === "A4" ? "297mm" : `${f.h}mm`}; display: grid;
      grid-template-columns: repeat(${f.cols}, ${f.w}mm); grid-auto-rows: ${f.h}mm; justify-content: center; align-content: ${f.page === "A4" ? "center" : "start"};
      page-break-after: always; break-after: page; overflow: hidden; background: #fff; }
    #labelPrint .page:last-child { page-break-after: auto; break-after: auto; }
    #labelPrint .label { display: flex; gap: ${big ? 4 : tiny ? 1.2 : 2}mm; align-items: center; padding: ${pad}mm; overflow: hidden; }
    #labelPrint .qr { flex: 0 0 ${qrSize}mm; height: ${qrSize}mm; max-width: 48%; }
    #labelPrint .qr svg { width: 100%; height: 100%; display: block; }
    #labelPrint .text { min-width: 0; display: flex; flex-direction: column; gap: 0.6mm; }
    #labelPrint .name { font-weight: 700; font-size: ${big ? 16 : tiny ? 7 : 10}pt; line-height: 1.1; overflow-wrap: anywhere; }
    #labelPrint .line { font-size: ${big ? 11 : tiny ? 5 : 7}pt; line-height: 1.15; overflow-wrap: anywhere; }
    #labelPrint .brand { font-size: ${big ? 8 : 5.5}pt; color: #444; margin-top: 0.8mm; }
    /* Plant passport: a framed rectangle that stands out from anything else. */
    #labelPrint .pp { border: 0.3mm solid #000; margin: ${tiny ? 0.3 : 0.6}mm; padding: ${tiny ? 0.6 : 1.2}mm ${tiny ? 0.9 : 1.6}mm; gap: ${tiny ? 1.2 : 2}mm; }
    #labelPrint .pp .qr { flex-basis: ${f.h - (tiny ? 2.4 : 4.4)}mm; height: ${f.h - (tiny ? 2.4 : 4.4)}mm; }
    #labelPrint .pp-head { flex: 0 0 auto; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 0.5mm; width: ${tiny ? 11 : big ? 26 : 15}mm; }
    #labelPrint .flag { width: ${tiny ? 7 : big ? 22 : 11}mm; }
    #labelPrint .flag svg { width: 100%; height: auto; display: block; }
    #labelPrint .pp-title { font-size: ${Math.max(3.6, ppLine * 0.72).toFixed(1)}pt; font-weight: 700; text-align: center; line-height: 1.05; }
    #labelPrint .pp-body { flex: 1 1 auto; min-width: 0; font-size: ${ppLine.toFixed(1)}pt; line-height: 1.12; }
    #labelPrint .pp-body div { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
    #labelPrint .pp-body b { display: inline-block; min-width: 1.1em; }`;
  const body = pages.map((p) => `<div class="page">${p.map(passport ? pass : normal).join("")}</div>`).join("");
  return { css, body, count: items.length, pages: pages.length };
}

/** Print the labels from this document: everything else is hidden while
 *  printing. (A frame would be blocked by the app's CSP frame-ancestors.) */
export function printLabels({ css, body }) {
  document.getElementById("labelPrint")?.remove();
  document.getElementById("labelPrintStyle")?.remove();
  const style = document.createElement("style");
  style.id = "labelPrintStyle";
  style.textContent = css;
  const box = document.createElement("div");
  box.id = "labelPrint";
  box.innerHTML = body;
  document.head.appendChild(style);
  document.body.appendChild(box);
  const cleanup = () => { box.remove(); style.remove(); window.removeEventListener("afterprint", cleanup); };
  window.addEventListener("afterprint", cleanup);
  setTimeout(() => window.print(), 50);
}
