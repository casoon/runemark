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

## Terminal escape security & trust model

Runemark is designed to safely format data for terminal emulators, including
potentially untrusted finding messages, URLs, and filenames.

- **Visible text sanitization:** Control characters such as ESC (`\x1b`),
  BEL (`\x07`), DEL (`\x7f`), and C0/C1 control codes are neutralized into
  caret notation (e.g. `^[`) or stripped before being emitted. Newlines and tabs
  are preserved; carriage returns are neutralized because they can overwrite
  existing terminal output.
- **Hyperlink validation:** `Location::Url` only creates OSC-8 terminal hyperlinks
  for explicit `http://` and `https://` schemes. Unrecognized or unsafe schemes
  (such as `javascript:`, `file:`, or `data:`) render as plain styled text
  without a hyperlink.
- **OSC-8 target protection:** Any OSC-8 target containing control characters,
  escape characters, or embedded termination sequences (`\x1b\\`, `\x07`) is
  rejected and suppressed as a hyperlink.
- **Parity:** Both Rust and Node.js (`@casoon/runemark`) bindings adhere to the
  same security guarantees.
