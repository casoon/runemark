---
title: Overview
description: What runemark does, where it stops, and how this documentation is organised.
order: 0
---

runemark gives related command-line tools one consistent way to communicate progress, status,
findings and next steps. Applications pass their own data to semantic building blocks; runemark
owns layout, wrapping, symbols and the colour policy.

## What it covers

- Semantic tones (title, muted, info, success, warning, error) and verdicts.
- A TTY-aware colour policy that honours `NO_COLOR`, with explicit `always` and `never` modes.
- Reports with metrics, grouped findings and next steps, in compact or detailed form.
- Error blocks, file-change previews and clickable locations in compatible terminals.
- Progress for long-running work (optional `progress` feature in Rust).
- Grouped, keyboard-driven selection (optional `select` feature in Rust, Unix only).

## Where it stops

| runemark owns | Applications own |
| --- | --- |
| Terminal colour policy and semantic tones | CLI arguments and configuration |
| Compact human-readable report layout | Domain finding types and business rules |
| Detail levels and next-step blocks | JSON, SARIF, Markdown, HTML and other artifacts |
| Line-oriented terminal presentation | Logging, tracing, free-text prompts and full-screen TUIs |
| Grouped interactive selection (`select`, Unix) | What the entries mean, how they are grouped and ordered |

## How the docs are organised

- **Getting started**: [install](getting-started/installation/) the crate or the npm package and
  render a [first output](getting-started/quickstart/).
- **Guides**: colour policy, reports, errors and diffs, progress, selection, the Node contract
  and the security model.
- **Reference**: an [overview of the public API](reference/api/). Item-level documentation lives
  on [docs.rs](https://docs.rs/runemark).

Every output shown in the [showcase](../showcase/) is rendered by runemark during the site build.
