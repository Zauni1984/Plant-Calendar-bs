// Icons and the social preview image from the logo in ../files.
//   node scripts/brand-assets.mjs
import fs from "node:fs";
import sharp from "sharp";

const owl = "../files/2.png"; // owl without lettering, cream background
const out = "public";
fs.mkdirSync(`${out}/icons`, { recursive: true });
fs.mkdirSync(`${out}/og`, { recursive: true });

// Round icon on cream: cut the square logo to a circle-friendly crop.
const square = sharp(owl).resize(1024, 1024, { fit: "cover" });
const buf = await square.png().toBuffer();
for (const [name, size] of [["icons/sowling-64.png", 64], ["icons/sowling-192.png", 192], ["icons/sowling-512.png", 512], ["icons/apple-touch-icon.png", 180], ["favicon-32.png", 32]]) {
  await sharp(buf).resize(size, size).png({ compressionLevel: 9 }).toFile(`${out}/${name}`);
}
await sharp(buf).resize(48, 48).toFile(`${out}/favicon.ico.png`);
fs.renameSync(`${out}/favicon.ico.png`, `${out}/favicon.png`);

// Open Graph image 1200 × 630: owl on the left, name and claim on the right.
const logo = await sharp(buf).resize(470, 470).png().toBuffer();
const text = Buffer.from(`<svg width="1200" height="630" xmlns="http://www.w3.org/2000/svg">
  <rect width="1200" height="630" fill="#fbf8f1"/>
  <rect x="0" y="560" width="1200" height="70" fill="#2c4a2a"/>
  <text x="560" y="250" font-family="Segoe UI, Arial, sans-serif" font-size="104" font-weight="800" fill="#2c4a2a">Sowling</text>
  <text x="564" y="330" font-family="Segoe UI, Arial, sans-serif" font-size="40" fill="#3f6b3a">Pflanzenkalender &amp; Garten-App</text>
  <text x="564" y="385" font-family="Segoe UI, Arial, sans-serif" font-size="32" fill="#5b6757">Gießen · Düngen · Ernten – zur richtigen Zeit</text>
  <text x="60" y="606" font-family="Segoe UI, Arial, sans-serif" font-size="28" font-weight="600" fill="#ffffff">sowling.com</text>
</svg>`);
await sharp(text).composite([{ input: logo, left: 60, top: 60 }]).png({ compressionLevel: 9 }).toFile(`${out}/og/sowling.png`);

fs.writeFileSync(`${out}/site.webmanifest`, JSON.stringify({
  name: "Sowling",
  short_name: "Sowling",
  start_url: "https://app.sowling.com/",
  display: "browser",
  background_color: "#fbf8f1",
  theme_color: "#2c4a2a",
  icons: [
    { src: "/icons/sowling-192.png", sizes: "192x192", type: "image/png" },
    { src: "/icons/sowling-512.png", sizes: "512x512", type: "image/png" },
  ],
}, null, 2));
console.log("ok");
