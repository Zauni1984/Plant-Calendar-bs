// Languages and the page map. German lives at the root, English under /en/.
// Every page has a key; the same key in both languages links them as
// translations (hreflang, language switch, sitemap).

export const LANGS = ["de", "en"];
export const SITE = "https://sowling.com";
export const APP_URL = "https://app.sowling.com";
export const COMPANY = "BlockSocial UG (haftungsbeschränkt)";

export const PAGES = {
  home: { de: "/", en: "/en/" },
  features: { de: "/funktionen/", en: "/en/features/" },
  pricing: { de: "/preise/", en: "/en/pricing/" },
  desktop: { de: "/desktop-app/", en: "/en/desktop-app/" },
  nurseries: { de: "/gaertnereien/", en: "/en/nurseries/" },
  guides: { de: "/anleitungen/", en: "/en/guides/" },
  blog: { de: "/ratgeber/", en: "/en/blog/" },
  faq: { de: "/faq/", en: "/en/faq/" },
  imprint: { de: "/impressum/", en: "/en/imprint/" },
  privacy: { de: "/datenschutz/", en: "/en/privacy/" },
  withdrawal: { de: "/widerruf/", en: "/en/withdrawal/" },
  terms: { de: "/agb/", en: "/en/terms/" },
};

export const path = (key, lang) => PAGES[key][lang];

/** Base path of a content section ("guides" → /anleitungen/ or /en/guides/). */
export const sectionPath = (section, lang, slug) => `${PAGES[section][lang]}${slug}/`;

export const T = {
  de: {
    locale: "de_DE",
    nav: { features: "Funktionen", pricing: "Preise", guides: "Anleitungen", blog: "Ratgeber", faq: "FAQ", nurseries: "Für Gärtnereien", desktop: "Desktop-App" },
    openApp: "App öffnen",
    startFree: "Kostenlos starten",
    skip: "Zum Inhalt springen",
    menu: "Menü",
    langName: "Deutsch",
    otherLang: "English",
    footer: {
      tagline: "Der Pflanzenkalender für Garten, Balkon, Gewächshaus und Gärtnerei.",
      product: "Produkt",
      help: "Hilfe",
      legal: "Rechtliches",
      imprint: "Impressum",
      privacy: "Datenschutz",
      withdrawal: "Widerrufsrecht",
      terms: "AGB",
      madeIn: "Entwickelt in Bayern.",
    },
    breadcrumbHome: "Start",
    readMore: "Weiterlesen",
    minutes: (n) => `${n} Min. Lesezeit`,
    updated: "Aktualisiert",
    faqTitle: "Häufige Fragen",
    related: "Passend dazu",
  },
  en: {
    locale: "en_GB",
    nav: { features: "Features", pricing: "Pricing", guides: "Guides", blog: "Blog", faq: "FAQ", nurseries: "For nurseries", desktop: "Desktop app" },
    openApp: "Open the app",
    startFree: "Start for free",
    skip: "Skip to content",
    menu: "Menu",
    langName: "English",
    otherLang: "Deutsch",
    footer: {
      tagline: "The plant calendar for gardens, balconies, greenhouses and nurseries.",
      product: "Product",
      help: "Help",
      legal: "Legal",
      imprint: "Imprint",
      privacy: "Privacy",
      withdrawal: "Right of withdrawal",
      terms: "Terms",
      madeIn: "Made in Bavaria, Germany.",
    },
    breadcrumbHome: "Home",
    readMore: "Read more",
    minutes: (n) => `${n} min read`,
    updated: "Updated",
    faqTitle: "Frequently asked questions",
    related: "Related",
  },
};

/** Plans as in the app (shared/src/domain.rs plant_limit, app price labels). */
export const PLANS = [
  { id: "free", name: { de: "Free", en: "Free" }, price: 0, plants: 1 },
  { id: "basic", name: { de: "Basic", en: "Basic" }, price: 0.99, plants: 3 },
  { id: "pro", name: { de: "Pro", en: "Pro" }, price: 5.99, plants: 100, highlight: true },
  { id: "nursery", name: { de: "Gärtnerei", en: "Nursery" }, price: 50, plants: null },
];

export const formatPrice = (value, lang) =>
  new Intl.NumberFormat(lang === "de" ? "de-DE" : "en-IE", { style: "currency", currency: "EUR", minimumFractionDigits: value % 1 ? 2 : 0 }).format(value);
