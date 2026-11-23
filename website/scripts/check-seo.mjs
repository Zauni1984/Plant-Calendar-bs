// Checks the built site (dist/): title and description present and unique,
// one <h1>, canonical and hreflang, JSON-LD parses, internal links resolve.
//   npm run build && node scripts/check-seo.mjs
import fs from "node:fs";
import path from "node:path";

const DIST = "dist";
const files = [];
(function walk(dir) {
  for (const f of fs.readdirSync(dir)) {
    const p = path.join(dir, f);
    if (fs.statSync(p).isDirectory()) walk(p);
    else if (f.endsWith(".html")) files.push(p);
  }
})(DIST);

const problems = [];
const titles = new Map();
const descs = new Map();
const exists = (url) => {
  const clean = url.split("#")[0].split("?")[0];
  const p = path.join(DIST, clean);
  return fs.existsSync(p) && (fs.statSync(p).isFile() || fs.existsSync(path.join(p, "index.html")));
};

for (const file of files) {
  const html = fs.readFileSync(file, "utf8");
  const rel = "/" + path.relative(DIST, file).replace(/\\/g, "/").replace(/index\.html$/, "");
  const get = (re) => html.match(re)?.[1];
  const title = get(/<title>([^<]*)<\/title>/);
  const desc = get(/<meta name="description" content="([^"]*)"/);
  const noindex = html.includes('content="noindex');
  if (!title) problems.push(`${rel}: no title`);
  if (!desc) problems.push(`${rel}: no description`);
  if (title && title.length > 70) problems.push(`${rel}: title ${title.length} chars`);
  if (desc && (desc.length < 70 || desc.length > 170)) problems.push(`${rel}: description ${desc.length} chars`);
  if (!noindex) {
    titles.set(title, [...(titles.get(title) || []), rel]);
    descs.set(desc, [...(descs.get(desc) || []), rel]);
    if (!html.includes('rel="canonical"')) problems.push(`${rel}: no canonical`);
    if (!html.includes('hreflang="de"') || !html.includes('hreflang="en"')) problems.push(`${rel}: hreflang incomplete`);
  }
  const h1 = (html.match(/<h1[\s>]/g) || []).length;
  if (h1 !== 1) problems.push(`${rel}: ${h1} <h1>`);
  for (const m of html.matchAll(/<script type="application\/ld\+json">([\s\S]*?)<\/script>/g)) {
    try { JSON.parse(m[1]); } catch { problems.push(`${rel}: JSON-LD does not parse`); }
  }
  for (const m of html.matchAll(/href="(\/[^"]*)"/g)) {
    const url = m[1];
    if (url.startsWith("//") || url.startsWith("/_astro/")) continue;
    if (!exists(url)) problems.push(`${rel}: broken link ${url}`);
  }
  for (const m of html.matchAll(/<img [^>]*>/g)) {
    if (!/ alt="/.test(m[0])) problems.push(`${rel}: <img> without alt`);
  }
}
for (const [t, pages] of titles) if (pages.length > 1) problems.push(`duplicate title "${t}": ${pages.join(", ")}`);
for (const [d, pages] of descs) if (pages.length > 1) problems.push(`duplicate description: ${pages.join(", ")}`);

console.log(`${files.length} pages checked`);
console.log(problems.length ? problems.join("\n") : "no problems");
process.exitCode = problems.length ? 1 : 0;
