# Security policy

## Supported versions

Until the first stable release, only the latest published `0.x` version is
supported.

## Reporting a vulnerability

Please use the project's private security-advisory channel once it is
published. Do not disclose a suspected vulnerability publicly before a
maintainer has had a chance to investigate it.

Include the affected version, Rust version, operating system, terminal/runtime
context, a minimal reproduction, and the potential impact.

## Terminal escape and trust model

How runemark neutralizes control characters, validates hyperlink targets, and
keeps the Rust and Node bindings in parity is documented at
<https://casoon.github.io/runemark/docs/guides/security/>.
