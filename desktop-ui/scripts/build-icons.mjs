// Rasterise the SVG icon masters into the PNGs that browsers and phones need.
// Run after editing public/icons/*.svg:   npm run icons
//
//   icon-192.png / icon-512.png   manifest "any"      (rounded, transparent corners)
//   icon-maskable-512.png         manifest "maskable" (full bleed; the OS crops it)
//   apple-touch-icon.png (180)    iOS home screen — must be opaque, iOS rounds it
//   favicon-32.png                browsers without SVG favicon support

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { Resvg } from "@resvg/resvg-js";

const dir = join(dirname(fileURLToPath(import.meta.url)), "..", "public", "icons");
const svg = (name) => readFileSync(join(dir, name), "utf8");

const outputs = [
  ["icon.svg", 192, "icon-192.png"],
  ["icon.svg", 512, "icon-512.png"],
  ["icon-maskable.svg", 512, "icon-maskable-512.png"],
  ["icon-maskable.svg", 180, "apple-touch-icon.png"],
  ["icon.svg", 32, "favicon-32.png"],
];

for (const [source, size, target] of outputs) {
  const png = new Resvg(svg(source), { fitTo: { mode: "width", value: size } }).render().asPng();
  writeFileSync(join(dir, target), png);
  console.log(`${target.padEnd(24)} ${size}x${size}  ${png.length} bytes`);
}
