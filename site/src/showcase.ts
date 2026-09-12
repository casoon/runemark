import { createRequire } from 'node:module';
import { ansiToHtml } from '@casoon/pages-theme/ansi';
import type { ShowcaseExample } from '@casoon/pages-theme/showcase';
import type {
  DiffInput,
  ErrorBlockInput,
  RenderOptions,
  ReportInput,
  Verdict,
} from '@casoon/runemark';

// Load the native binding with Node at build time; the bundler must not follow it into the
// platform-specific .node files.
const { renderDiff, renderError, renderReport, renderStatus }: typeof import('@casoon/runemark') =
  createRequire(import.meta.url)('@casoon/runemark');

// Every output on this site is rendered at build time by @casoon/runemark – the same Rust
// renderer as the crate – and converted from ANSI to HTML for the page. The data mirrors
// examples/report_demo.rs.
const colour = { color: 'always', isTerminal: true, symbols: 'unicode' } as const;

// JSON with unquoted keys, so the input panel reads like the JavaScript a caller writes.
const js = (value: unknown) => JSON.stringify(value, null, 2).replace(/"([A-Za-z]+)":/g, '$1:');

const verdicts: Array<[Verdict, string]> = [
  ['passed', 'Configuration validated cleanly'],
  ['warning', '3 non-critical findings detected'],
  ['failed', 'Build failed: 2 errors'],
  ['action-required', 'Sign in to continue the deployment'],
  ['skipped', 'Link check disabled for drafts'],
  ['info', 'Using cached results from 09:41'],
];

const audit: ReportInput = {
  schemaVersion: 1,
  title: 'Site audit v1.2.0',
  verdict: 'warning',
  metrics: [
    { key: 'Errors', value: '0' },
    { key: 'Warnings', value: '4', trend: 'negative', delta: '+2' },
    { key: 'Score', value: '94/100', trend: 'positive', delta: '+5%' },
  ],
  groups: [
    {
      title: 'Accessibility violations',
      findings: [
        {
          message: 'Image missing alt attribute',
          tone: 'warning',
          ruleId: 'a11y/img-alt',
          confidence: 'high',
          badge: { label: 'quick win', tone: 'success' },
          location: { kind: 'file', value: 'src/pages/index.astro', line: 42, column: 10 },
          remedy: 'Add an alt="…" description to the <img> tag.',
        },
        {
          message: 'Low contrast ratio on hero button',
          tone: 'warning',
          ruleId: 'a11y/contrast',
          location: { kind: 'file', value: 'src/components/Hero.astro', line: 18 },
        },
      ],
    },
  ],
  nextSteps: [{ text: 'Run the auto-fixer for formatting issues', command: 'audit --fix' }],
};

const detailed: ReportInput = { ...audit, detailLevel: 'detailed' };

// Long messages without locations: runemark wraps them with a hanging indent.
const narrow: ReportInput = {
  schemaVersion: 1,
  title: 'Content audit',
  verdict: 'warning',
  metrics: audit.metrics,
  groups: [
    {
      title: 'Navigation',
      findings: [
        {
          message:
            'The main navigation contains more links than can be reached comfortably with a keyboard',
          tone: 'warning',
          ruleId: 'ux/nav-size',
        },
        {
          message: 'Two landmarks share the label "Menu", so screen reader users cannot tell them apart',
          tone: 'warning',
          ruleId: 'a11y/landmark-label',
        },
      ],
    },
  ],
};

// Start page: the compact audit without badges and locations, so every line fits the panel.
const heroReport: ReportInput = {
  ...audit,
  groups: audit.groups?.map((group) => ({
    ...group,
    findings: group.findings?.map(({ message, tone, ruleId }) => ({ message, tone, ruleId })),
  })),
};

const missingToolchain: ErrorBlockInput = {
  heading: 'Required toolchain is unavailable',
  explanation: 'The configured version is not installed.',
  remedy: 'Install the required toolchain:',
  commands: ['toolchain install stable'],
};

const generated: DiffInput = {
  changes: [
    { action: 'added', path: 'src/components/Footer.astro', delta: '+1.2 kB' },
    { action: 'modified', path: 'src/layouts/Layout.astro' },
    { action: 'renamed', path: 'src/styles/site.css' },
    { action: 'deleted', path: 'src/legacy/Footer.jsx' },
  ],
};

const report = (input: ReportInput, width: number) =>
  renderReport(input, { ...colour, width } satisfies RenderOptions);

/** Compact report for the start page. */
export const hero = report(heroReport, 58);

/** Raw ANSI per example, for pages that show an output outside the showcase. */
export const ansi = {
  verdicts: verdicts.map(([verdict, message]) => renderStatus(verdict, message, colour)).join('\n'),
  'report-compact': report(audit, 80),
  'report-detailed': report(detailed, 80),
  'report-narrow': report(narrow, 44),
  'error-block': renderError(missingToolchain, colour),
  'file-changes': renderDiff(generated, colour),
};

const examples_: Array<Omit<ShowcaseExample, 'output'> & { slug: keyof typeof ansi }> = [
  {
    slug: 'verdicts',
    title: 'Verdicts',
    description: 'One status line per verdict: symbol, colour and wording come from runemark.',
    file: 'site/src/showcase.ts',
    tags: ['renderStatus', 'verdict'],
    input: {
      lang: 'js',
      code: verdicts
        .map(([verdict, message]) => `renderStatus('${verdict}', '${message}')`)
        .join('\n'),
    },
  },
  {
    slug: 'report-compact',
    title: 'Compact report',
    description: 'Verdict, metrics with trends, grouped findings and a next step, at 80 columns.',
    file: 'site/src/showcase.ts',
    tags: ['renderReport', 'compact'],
    input: { lang: 'js', code: `renderReport(${js(audit)}, { width: 80 })` },
  },
  {
    slug: 'report-detailed',
    title: 'Detailed report',
    description: 'The same report with detailLevel "detailed": rule IDs, locations and remedies.',
    file: 'site/src/showcase.ts',
    tags: ['renderReport', 'detailed'],
    input: { lang: 'js', code: `renderReport({ ...audit, detailLevel: 'detailed' }, { width: 80 })` },
  },
  {
    slug: 'report-narrow',
    title: 'Narrow terminal',
    description: 'At 44 columns summary metrics stack and long messages wrap with a hanging indent.',
    file: 'site/src/showcase.ts',
    tags: ['renderReport', 'width'],
    input: { lang: 'js', code: `renderReport(${js(narrow)}, { width: 44 })` },
  },
  {
    slug: 'error-block',
    title: 'Error block',
    description: 'What went wrong, why, and the command that fixes it.',
    file: 'site/src/showcase.ts',
    tags: ['renderError'],
    input: { lang: 'js', code: `renderError(${js(missingToolchain)})` },
  },
  {
    slug: 'file-changes',
    title: 'File changes',
    description: 'A generator summary: added, modified, renamed and deleted files.',
    file: 'site/src/showcase.ts',
    tags: ['renderDiff'],
    input: { lang: 'js', code: `renderDiff(${js(generated)})` },
  },
];

export const examples: ShowcaseExample[] = examples_.map((example) => ({
  ...example,
  output: { html: ansiToHtml(ansi[example.slug]), kind: 'terminal' },
}));
