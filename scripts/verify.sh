#!/usr/bin/env bash
# Local counterpart of the Ubuntu jobs in .github/workflows/ci.yml.
# Runs every check CI runs, in the same order, and stops at the first failure.
# macOS and Windows runners are not covered; CI still runs those.
#
#   scripts/verify.sh            everything (Rust, release metadata, Node)
#   scripts/verify.sh --rust     skip the Node section
set -euo pipefail

cd "$(dirname "$0")/.."

run_node=1
case "${1:-}" in
  "") ;;
  --rust) run_node=0 ;;
  *) echo "usage: scripts/verify.sh [--rust]" >&2; exit 2 ;;
esac

section() { printf '\n==> %s\n' "$1"; }

# --- Rust (jobs: format, clippy, test, docs, package) ----------------------
section "Formatting"
cargo fmt --check

section "Clippy"
cargo clippy --all-targets --all-features -- -D warnings

section "Test (all features)"
cargo test --all-features

# CI also tests on the MSRV (1.85); needs `rustup toolchain install 1.85`.
section "Test on MSRV 1.85"
if command -v rustup >/dev/null 2>&1 && rustup toolchain list | grep -q '^1\.85'; then
  cargo +1.85 test --all-features
else
  echo "skipped: Rust 1.85 toolchain not installed (rustup toolchain install 1.85)"
fi

section "Documentation"
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features

section "Package"
cargo package --locked

# --- Release metadata (job: metadata) --------------------------------------
section "Release metadata"
node scripts/check-release-metadata.mjs

# --- Node binding (job: node; needs Rust 1.88 and Node 20 or newer) --------
if [ "$run_node" -eq 1 ]; then
  if command -v npm >/dev/null 2>&1; then
    section "Clippy Node binding"
    cargo clippy --manifest-path bindings/node/Cargo.toml --all-targets --all-features -- -D warnings

    section "Test Node binding (Rust)"
    cargo test --manifest-path bindings/node/Cargo.toml

    cd packages/runemark

    section "Install Node dependencies"
    npm ci

    section "Build and test Node package"
    npm test

    section "TypeScript contract typecheck"
    npm run test:types

    section "Consumer tarball test"
    npm run test:contract

    section "Pack check"
    npm run pack:check
  else
    section "Node binding"
    echo "skipped: npm not found"
  fi
fi

printf '\nAll checks passed.\n'
