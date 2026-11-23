# sowling.com – Webseite

Statische Webseite (Astro), Deutsch unter `/`, Englisch unter `/en/`.
Fertige HTML-Dateien für den Hostinger-Webspace.

## Bauen und ansehen

```bash
cd website
npm install
npm run build              # schreibt alles nach dist/
node scripts/check-seo.mjs # Titel, Beschreibungen, H1, hreflang, Links prüfen
npm run preview            # http://localhost:4321
```

## Hochladen (Hostinger)

Den **Inhalt** von `dist/` (nicht den Ordner selbst) nach `public_html` der
Domain sowling.com hochladen, z. B. im hPanel über **Dateien → Dateimanager**
(ZIP hochladen und entpacken) oder per FTP. Die `.htaccess` liegt mit in
`dist/` (HTTPS, www → ohne www, 404-Seite, Caching).

Danach in der Google Search Console die Domain bestätigen und
`https://sowling.com/sitemap.xml` einreichen.

## Inhalte ändern

| Was | Wo |
|---|---|
| Seitentexte (Start, Funktionen, Preise …) | `src/views/*.astro`, je ein `de`- und `en`-Block |
| Anleitungen | `src/content/anleitungen/de|en/*.md` (Schritte im Kopf der Datei) |
| Ratgeber-Artikel | `src/content/ratgeber/de|en/*.md` |
| Impressum, Datenschutz, Widerruf | `../docs/IMPRESSUM_*.md`, `../docs/PRIVACY_POLICY_*.md`, `../docs/WIDERRUF_*.md` |
| Preise und Tarife | `src/i18n.js` (`PLANS`) und `src/components/Plans.astro` |
| Adressen der Seiten | `src/i18n.js` (`PAGES`) |

Ein neuer Artikel braucht beide Sprachen mit demselben `translationKey`, dann
verknüpfen sich Sprachumschalter, hreflang und Sitemap automatisch.

## Screenshots erneuern

Braucht die App (vite, Port 1420) und ein lokales Backend mit Datenbank:

```bash
python scripts/seed_demo.py de demo-de@example.test PASSWORT
python scripts/seed_demo.py en demo-en@example.test PASSWORT
DEMO_PW=PASSWORT node scripts/screenshots.mjs
```

Die Demo-Konten brauchen einen Tarif mit mehr als einer Pflanze (z. B. per
SQL `voucher_plan = 'pro'`). Bilder landen in `src/assets/screens/de|en/`.
