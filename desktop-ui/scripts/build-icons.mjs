// Build every app icon from the logo master:   npm run icons
//
//   master:  icon-src/sowling-icon.png  (square, ideally 1024 px or larger)
//
//   public/icons/icon-192.png, icon-512.png   manifest "any": rounded square,
//                                             transparent corners
//   public/icons/icon-maskable-512.png        manifest "maskable": full bleed;
//                                             Android crops it to a circle or
//                                             squircle, so the logo sits inside
//                                             the safe zone
//   public/icons/apple-touch-icon.png (180)   iOS: must be opaque, iOS rounds it
//   public/icons/favicon-32.png               browser tab
//   src-tauri/app-icon.png (1024)             input for `npx tauri icon`
//                                             (Windows/macOS/Linux icon set)
//
// The master's own background colour is sampled from its corner and used to
// fill the padded variants, so they match whatever the logo was drawn on.

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { Resvg } from "@resvg/resvg-js";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const master = readFileSync(join(root, "icon-src", "sowling-icon.png"));
const href = `data:image/png;base64,${master.toString("base64")}`;

function render(svg, size) {
  return new Resvg(svg, { fitTo: { mode: "width", value: size } }).render();
}

// Background colour from the master's top-left corner.
const probe = render(`<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64">
  <image href="${href}" width="64" height="64"/></svg>`, 64).pixels;
const bg = `rgb(${probe[0]},${probe[1]},${probe[2]})`;

/**
 * One icon variant on a 100x100 canvas.
 *   scale:   logo size as a fraction of the canvas
 *   radius:  corner radius in % of the canvas (null = square, opaque)
 */
function variant({ scale, radius }) {
  const s = 100 * scale;
  const o = (100 - s) / 2;
  const clip = radius == null ? "" : `<clipPath id="c"><rect width="100" height="100" rx="${radius}"/></clipPath>`;
  const use = radius == null ? "" : ` clip-path="url(#c)"`;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="100" height="100">
  <defs>${clip}</defs>
  <g${use}>
    <rect width="100" height="100" fill="${bg}"/>
    <image href="${href}" x="${o}" y="${o}" width="${s}" height="${s}"/>
  </g>
</svg>`;
}

// Rounded tile with a little margin, so ears and leaves are not cut off.
const tile = variant({ scale: 0.92, radius: 22 });
// Android masks to as little as the inner 80 % circle.
const maskable = variant({ scale: 0.74, radius: null });
// iOS rounds the corners itself (~22 %); keep the logo clear of them.
const apple = variant({ scale: 0.86, radius: null });

const outputs = [
  [tile, 192, "public/icons/icon-192.png"],
  [tile, 512, "public/icons/icon-512.png"],
  [maskable, 512, "public/icons/icon-maskable-512.png"],
  [apple, 180, "public/icons/apple-touch-icon.png"],
  [tile, 32, "public/icons/favicon-32.png"],
  [tile, 1024, "src-tauri/app-icon.png"],
];

console.log(`background ${bg}`);
for (const [svg, size, target] of outputs) {
  const png = render(svg, size).asPng();
  writeFileSync(join(root, target), png);
  console.log(`${target.padEnd(36)} ${size}x${size}  ${(png.length / 1024).toFixed(0)} KB`);
}
console.log("\nDesktop icon set: npx tauri icon src-tauri/app-icon.png");
