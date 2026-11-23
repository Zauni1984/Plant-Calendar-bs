// Structured data helpers (schema.org JSON-LD).
import { SITE, APP_URL, PLANS } from "../i18n.js";

const strip = (html) => String(html).replace(/<[^>]+>/g, "");

export function faqJsonLd(items) {
  if (!items?.length) return [];
  return [{
    "@type": "FAQPage",
    mainEntity: items.map((i) => ({ "@type": "Question", name: strip(i.q), acceptedAnswer: { "@type": "Answer", text: strip(i.a) } })),
  }];
}

export function appJsonLd(lang, description) {
  return [{
    "@type": "SoftwareApplication",
    "@id": `${SITE}/#app`,
    name: "Sowling",
    applicationCategory: "LifestyleApplication",
    applicationSubCategory: lang === "de" ? "Pflanzenkalender" : "Plant calendar",
    operatingSystem: "Web, Windows, Android, iOS",
    url: APP_URL,
    description,
    inLanguage: ["de", "en"],
    publisher: { "@id": `${SITE}/#org` },
    offers: PLANS.map((p) => ({
      "@type": "Offer",
      name: p.name[lang],
      price: p.price.toFixed(2),
      priceCurrency: "EUR",
      ...(p.price > 0 ? { priceSpecification: { "@type": "UnitPriceSpecification", price: p.price.toFixed(2), priceCurrency: "EUR", billingDuration: "P1M", referenceQuantity: { "@type": "QuantitativeValue", value: 1, unitCode: "MON" } } } : {}),
    })),
  }];
}

export function articleJsonLd({ title, description, url, lang, published, updated }) {
  return [{
    "@type": "Article",
    headline: title,
    description,
    inLanguage: lang,
    mainEntityOfPage: new URL(url, SITE).href,
    datePublished: published.toISOString().slice(0, 10),
    dateModified: updated.toISOString().slice(0, 10),
    author: { "@id": `${SITE}/#org` },
    publisher: { "@id": `${SITE}/#org` },
    image: `${SITE}/og/sowling.png`,
  }];
}

export function howToJsonLd({ title, description, url, minutes, steps, lang }) {
  return [{
    "@type": "HowTo",
    name: title,
    description,
    inLanguage: lang,
    totalTime: `PT${minutes}M`,
    url: new URL(url, SITE).href,
    tool: [{ "@type": "HowToTool", name: "Sowling" }],
    step: steps.map((s, i) => ({ "@type": "HowToStep", position: i + 1, name: s.name, text: strip(s.text), url: `${new URL(url, SITE).href}#step-${i + 1}` })),
  }];
}
