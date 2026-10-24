#!/usr/bin/env sh
# Renders the hero scene to assets/hero.svg — an animated SVG with no script,
# which honours prefers-reduced-motion and works in the README as on the site.
#
# The example is built first so the recording shows the program and not cargo
# compiling it.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)

cargo build --quiet --features progress --example hero_demo
# node-pty is what `exec:` runs the command through; castwright needs it present.
npx -y -p @casoon/castwright@0.4.0 -p shiki -p node-pty castwright build \
  "$root/demo/casts/hero.terminal.yaml" \
  -o "$root/assets" --format svg --loop-delay 6000 --allow-exec
