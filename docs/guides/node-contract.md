---
title: Node contract
description: What the npm package guarantees, how the report schema is versioned, and what stays internal.
order: 5
---

`@casoon/runemark` is a native Node-API addon around the Rust renderer. Consumers work with plain
objects and strings, never with Rust types or builders.

## Stable boundary

- Classes `RunemarkConsole` (semantic text), `RunemarkReport` (an immutable report) and
  `RunemarkProgress` (a progress lifecycle), plus the helpers `renderStatus`, `renderReport`,
  `renderError`, `renderDiff` and `createProgress`.
- Rendering returns strings; your code writes them. `write*` methods accept any object with a
  `write(text)` method.
- The string literals in `index.d.ts` are the contract. The native binding validates tones,
  verdicts, locations and file actions, so invalid input fails at the package boundary instead of
  rendering a partial report.

## Report schema versions

`ReportInput` carries `schemaVersion`. Version `1` is the only accepted shape. New optional fields
may be added without a new schema version; renaming, changing the meaning of, or removing a field
requires a new schema version and a new major package version.

## Compatibility

- Node.js 20 or newer, Node-API 8.
- The native binding builds with Rust 1.88; the `runemark` crate keeps Rust 1.85 as its minimum.
- The native loader is private: import only `@casoon/runemark`.
- Output matches the Rust crate for the same input and options.
