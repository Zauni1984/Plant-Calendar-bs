// sitemap.xml with hreflang alternates for every page that has a translation.
import { getCollection } from "astro:content";
import { PAGES, SITE, sectionPath } from "../i18n.js";

const abs = (p) => new URL(p, SITE).href;

export async function GET() {
  const groups = Object.values(PAGES).map((alt) => ({ alt, lastmod: null }));
  for (const [collection, section] of [["anleitungen", "guides"], ["ratgeber", "blog"]]) {
    const entries = await getCollection(collection);
    const byKey = {};
    for (const e of entries) {
      byKey[e.data.translationKey] ??= { alt: {}, lastmod: e.data.updated };
      byKey[e.data.translationKey].alt[e.data.lang] = sectionPath(section, e.data.lang, e.id.split("/").pop());
    }
    groups.push(...Object.values(byKey));
  }
  const urls = groups.flatMap(({ alt, lastmod }) => Object.values(alt).map((loc) => `  <url>
    <loc>${abs(loc)}</loc>${lastmod ? `\n    <lastmod>${lastmod.toISOString().slice(0, 10)}</lastmod>` : ""}
${Object.entries(alt).map(([l, href]) => `    <xhtml:link rel="alternate" hreflang="${l}" href="${abs(href)}"/>`).join("\n")}${alt.de ? `\n    <xhtml:link rel="alternate" hreflang="x-default" href="${abs(alt.de)}"/>` : ""}
  </url>`));
  const xml = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">
${urls.join("\n")}
</urlset>
`;
  return new Response(xml, { headers: { "Content-Type": "application/xml; charset=utf-8" } });
}
