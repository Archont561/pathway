import { defineCollection } from "astro:content";
import { docsLoader, i18nLoader } from "@astrojs/starlight/loaders";
import { docsSchema, i18nSchema } from "@astrojs/starlight/schema";

export const collections = {
  docs: defineCollection({ loader: docsLoader(), schema: docsSchema() }),
  // Starlight reads UI strings from this collection, and warns at build time
  // when it is absent — the site is English-only, so `src/content/i18n/` stays
  // empty and every string is the default.
  i18n: defineCollection({ loader: i18nLoader(), schema: i18nSchema() })
};
