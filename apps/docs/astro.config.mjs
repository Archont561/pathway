import { fileURLToPath } from "node:url";
import starlight from "@astrojs/starlight";
import { defineConfig } from "astro/config";
import { version } from "./src/version.ts";

// The workspace manifest reader lives at the repository root, outside this
// project, and Vite will not resolve a module from outside the project root
// without being told. An absolute alias is the smallest way to say so, and
// declaring it in one place is better than each import site computing a
// `../../..` path back out of the app and hoping it stays correct.
const versionScript = fileURLToPath(new URL("../../scripts/version.ts", import.meta.url));

// Deployed to GitHub Pages as a project site, so the site lives under the
// /pathway subpath: https://archont561.github.io/pathway/
const site = "https://archont561.github.io";
const base = "/pathway";

export default defineConfig({
  site,
  base,
  server: {
    allowedHosts: "all"
  },
  vite: {
    resolve: {
      alias: {
        "@workspace/version": versionScript
      }
    }
  },
  integrations: [
    starlight({
      title: "pathway",
      description: "A native, pathlib-inspired filesystem API for TypeScript, Bun, Node and Rust",
      // Array form, since Starlight 0.33: an object keyed by network is
      // rejected, and the error surfaces as a config-validation message rather
      // than anything that names the version that changed.
      social: [
        {
          icon: "github",
          label: "GitHub",
          href: "https://github.com/Archont561/pathway"
        }
      ],
      // The version the rest of the site quotes, taken from the environment the
      // build ran in. A head meta tag rather than visible chrome on purpose:
      // every page inherits it, so there is one place to change, and a scraper
      // or a reader who wants it can have it without it shouting.
      head: [
        { tag: "meta", attrs: { name: "pathway-version", content: version } },
        {
          tag: "meta",
          attrs: { name: "pathway-docset", content: `v${version}` }
        }
      ],
      editLink: {
        baseUrl: "https://github.com/Archont561/pathway/edit/main/apps/docs/"
      },
      sidebar: [
        {
          label: "Getting Started",
          items: [
            { label: "Installation", link: "/getting-started/installation" },
            { label: "Quick Start", link: "/getting-started/quick-start" }
          ]
        },
        {
          label: "Concepts",
          items: [{ label: "Architecture", link: "/concepts/architecture" }]
        },
        {
          label: "Guides",
          items: [{ label: "Contributing", link: "/guides/contributing" }]
        }
      ]
    })
  ]
});
