# Vendored UI security audit

Audit date: 2026-08-07

This directory contains source snapshots, not Git or registry dependencies. The
application consumes them exclusively through local `path` dependencies.

## egui-shadcn

- Upstream: https://github.com/pjankiewicz/egui-shadcn
- Package version: `0.1.0`
- Commit: `fa5ceeed623eea983765fa4f886dd610d8b39470`
- Git tree: `2d5eabedf0003e07a20b29af2d8875db47870773`
- Commit date: 2026-06-13T08:31:53+02:00
- License: MIT (`egui-shadcn/LICENSE`)
- Audited scope: all 263 tracked upstream files, including `src`, examples,
  fonts, the icon generator, the Web demo, manifests, README, and license.
- Vendored scope: the complete runtime `src`, required fonts, README, and
  license. Examples, Web demo, and the Python icon generator are deliberately
  excluded from both packaging and the build graph.
- Local compatibility change: the upstream `egui = "0.33"` dependency is
  pinned to `egui = "=0.32.3"`; compatibility edits are kept as ordinary Git
  diffs in this repository.

## egui_flex

- Registry package: `egui_flex 0.5.0`
- crates.io checksum:
  `849265cb6869179fc14abab0a725afbeee36006397f8cbfd3c42b0b7a08eba2b`
- Source repository: https://github.com/lucasmerlin/hello_egui
- Release commit: `8fac8abb42161a6e27a4488c90e6b1ff0f9932f5`
- Git tree: `efab692c5e821013bddbbd83238a33c7b0ee2058`
- Source path: `crates/egui_flex`
- License: MIT (`egui_flex/LICENSE`)
- Verification: the registry runtime source matched the release commit; only
  Cargo's generated package metadata differed.
- Local compatibility change: the upstream `egui = "0.33.0"` dependency is
  pinned to `egui = "=0.32.3"`.

## Findings

No malicious or release-blocking behavior was found in either runtime library.

- No `unsafe` blocks, FFI, dynamic loading, sockets, HTTP clients, process
  execution, filesystem access, environment/credential reads, build scripts,
  or procedural macros exist in the runtime source.
- Neither vendored package has a build script or build dependency.
- `egui-shadcn` embeds two static Geist TTF files and generated Lucide SVG path
  data. Font SHA-256 values:
  - Geist Bold: `f3e7e77fe8ed83dd2696c123b87d6f58a95e0fa7d8c31b4f7dec2ba0abdbdafa`
  - Geist Regular: `ec77e679049dfbdc9eea027a32c8c579c589b93a79283d56310c9f75003c356a`
- Generated icon data contains 1,671 static entries and no scripts, external
  URLs, entities, images, or foreign-object payloads.
- The custom SVG parser only receives compile-time icon strings in this app.
- `egui_flex` contains debug-only panic checks for duplicate egui IDs and two
  internal invariant unwraps. These are availability risks after programmer
  misuse, not privilege or data-exfiltration paths.

## Complete local source delta

The vendored runtime is intentionally kept byte-for-byte equal to the recorded
upstream snapshots except for the following reviewable compatibility changes:

- Five `egui-shadcn` modal/sheet call sites use the egui 0.32 name
  `screen_rect()` instead of egui 0.33's `viewport_rect()`.
- Two `egui_flex` let-chain expressions are written as nested `if` statements
  because this project builds with Rust 1.86. The control flow is identical.
- One `#[expect(deprecated)]` is `#[allow(deprecated)]` because the expected
  warning is not emitted by egui 0.32.
- One unused upstream `Resizable::initial_fraction` field has a local
  `#[allow(dead_code)]`; no code path is changed.
- The vendored `Textarea` accepts an explicit `id_salt` for its internal
  `ScrollArea`, preventing duplicate egui IDs when multiple textareas appear in
  equivalent nested card layouts.
- The vendored full-width `Button` allocates the full available row, keeps
  icon/text content left-aligned with theme padding, uses a pointing cursor,
  emits egui `WidgetInfo` button/selected semantics for accessibility, exposes
  explicit height/padding/radius overrides, and preserves distinct selected,
  hover, and pressed states.
- The vendored `ScrollArea` exposes stable IDs, optional framing, bottom
  sticking, auto-shrink configuration, and exact available-area allocation so
  full-page and nested shadcn scrollers do not fall back to raw egui containers.
- The vendored `Typography` exposes color, monospace, italic, wrapping,
  truncation, explicit size/line-height, and weight presentation options.
  `PropertyRow` uses an exact-width two-column shadcn layout with centered
  single-line rows and explicit first-line alignment for multiline values.
  `ToggleGroup` exposes the audited component-size tokens for compact tab-like
  controls. `Alert` can fill its parent width for preview and validation panels.
  `Textarea` supports relaxed web-style leading, and Card uses the opaque theme
  border rather than a translucent foreground stroke. Badges/alerts add
  semantic success, warning, info, and destructive colors; compact component
  text stays in the readable 13–14 px range.
- Self-painted Label, Badge, Switch, Select, ToggleGroup, Slider, and
  AlertDialog controls emit egui accessibility metadata. Switch marks its
  response changed when toggled so consumers can reliably persist the new
  state. Slider supports arrow, Home, and End keys; confirmation dialogs support
  Escape cancellation and an accessible close action.
- The two local Cargo manifests remove all examples/dev dependencies, forbid
  unsafe code, pin egui exactly to `0.32.3`, and connect the libraries by a
  local path.

No other vendored runtime source changes are permitted without updating this
section and re-running the full audit.

## Update policy

Do not auto-update these packages. For an update, fetch a full 40-character
commit, record its tree hash, audit the complete repository and dependency diff,
then replace the runtime snapshot in a dedicated commit. Never change the path
dependencies to a branch, tag, Git URL, or registry range.

Release verification must include:

```text
cargo build --frozen
cargo test --frozen
```

`--frozen` makes Cargo use the committed lock file without network access.
