// `tsconfig.json` excludes `vendor/`, where the synced theme lives, so the
// declarations for its virtual modules are not picked up on their own. A page
// that imports one — `index.astro` reads the configured version out of
// `virtual:casoon-pages/config` — needs them referenced here.
/// <reference path="../vendor/pages-theme/virtual.d.ts" />
