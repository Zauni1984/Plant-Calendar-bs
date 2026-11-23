// robots.txt: everything may be crawled; points to the sitemap.
export const GET = () => new Response(`User-agent: *\nAllow: /\n\nSitemap: https://sowling.com/sitemap.xml\n`, { headers: { "Content-Type": "text/plain; charset=utf-8" } });
