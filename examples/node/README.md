# Node.js examples

These examples use the local package build so they can be run from this
repository before npm publication. The build needs Rust 1.88 or newer and
generates the gitignored `native.cjs` loader the examples require.

```bash
npm --prefix packages/runemark ci
npm --prefix packages/runemark run build
node examples/node/status.cjs
node examples/node/report.cjs
node examples/node/progress.cjs
```

In an application, replace `require('../../packages/runemark')` with
`require('@casoon/runemark')` after installing the package.
