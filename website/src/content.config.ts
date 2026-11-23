import { defineCollection, z } from "astro:content";
import { glob } from "astro/loaders";

const faq = z.array(z.object({ q: z.string(), a: z.string() })).default([]);

// Gardening articles: /ratgeber/<slug>/ and /en/blog/<slug>/.
const ratgeber = defineCollection({
  loader: glob({ pattern: "**/*.md", base: "./src/content/ratgeber" }),
  schema: z.object({
    title: z.string().max(70),
    description: z.string(),
    translationKey: z.string(),
    lang: z.enum(["de", "en"]),
    published: z.coerce.date(),
    updated: z.coerce.date(),
    readingMinutes: z.number(),
    keywords: z.array(z.string()).default([]),
    faq,
  }),
});

// How-tos for the app: /anleitungen/<slug>/ and /en/guides/<slug>/.
// `steps` feed both the numbered list and the HowTo structured data.
const anleitungen = defineCollection({
  loader: glob({ pattern: "**/*.md", base: "./src/content/anleitungen" }),
  schema: z.object({
    title: z.string().max(70),
    description: z.string(),
    translationKey: z.string(),
    lang: z.enum(["de", "en"]),
    order: z.number(),
    icon: z.string(),
    minutes: z.number(),
    updated: z.coerce.date(),
    steps: z.array(z.object({ name: z.string(), text: z.string() })),
    faq,
  }),
});

export const collections = { ratgeber, anleitungen };
