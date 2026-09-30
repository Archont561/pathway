# pathway docs

The documentation site: [Astro](https://astro.build) with
[Starlight](https://starlight.astro.build), deployed to GitHub Pages.

Run it through pixi, so it gets the same locked environment as everything else:

```sh
pixi run docs-dev      # dev server
pixi run docs-build    # production build into dist/
pixi run docs-preview  # serve the built site
```

The version quoted throughout the site is **read from the environment**, not
written into the site: the pixi tasks export `PATHWAY_VERSION` (resolved from
`[workspace.package] version` in the root `Cargo.toml` by `scripts/version.ts`),
and `src/version.ts` falls back to reading that same manifest when the variable
did not survive the trip — for example when turbo runs the build directly.

## Known build warnings

Three warnings on a clean build, all expected and none of them ours to fix:

- `The collection "i18n" does not exist or is empty` — the site is
  English-only, so `src/content/i18n/` has no files. The collection is defined
  anyway, so a translator has somewhere to put one; adding
  `src/content/i18n/<lang>.json` silences it.
- `Could not render /404 from route /[...slug]` — Starlight builds the 404 page
  from `src/content/docs/404.mdx`, which is the documented arrangement;
  `dist/404.html` is produced and correct.
- `MODULE_LEVEL_DIRECTIVE ... "use astro:head-inject" may not be preserved` —
  emitted by Vite for every MDX page by Astro itself, not by anything in this
  app.