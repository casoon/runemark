// @ts-check
import casoonPages from '@casoon/pages-theme';
import { defineConfig } from 'astro/config';

// Project page: https://casoon.github.io/runemark/ — `base` is the GitHub Pages path.
export default defineConfig({
  site: 'https://casoon.github.io/runemark',
  base: '/runemark/',
  integrations: [
    casoonPages({
      name: 'runemark',
      description:
        'Consistent, human-readable terminal presentation for Rust and Node.js command-line tools.',
      repo: 'casoon/runemark',
      version: '0.3.1',
      license: 'MIT',
      packages: [
        { label: 'crates.io', href: 'https://crates.io/crates/runemark' },
        { label: 'npm', href: 'https://www.npmjs.com/package/@casoon/runemark' },
        { label: 'docs.rs', href: 'https://docs.rs/runemark' },
      ],
      docsGroups: {
        'getting-started': 'Getting started',
        guides: 'Guides',
        reference: 'Reference',
      },
    }),
  ],
});
