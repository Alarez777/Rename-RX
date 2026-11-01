# Rename RX — agent instructions

Rename RX is a native batch file renamer for Linux: an egui/eframe desktop
application with a UI-independent core crate.

## Rust skills

This project follows [Beautiful Rust](https://github.com/morganlinton/BeautifulRust).

- Load the `rust-best-practices` skill before writing, changing, refactoring, or
  reviewing Rust code.
- Load the `rust-review` skill when the task is an audit, a review, or a
  prioritized list of findings.
- Do not use the `rust-port` skill here. This is a Rust-only codebase; there is
  no code being ported from another language.

## Workspace layout

| Crate | Kind | Rules |
| --- | --- | --- |
| `crates/rename-rx-core` | library | Follow the library/API rules: documented public items, `# Errors`/`# Panics`, stable public API, no egui dependency. |
| `crates/rename-rx` | binary | Follow the application rules: concrete types, direct code, no speculative API. Keep UI behavior unchanged unless the task asks for it. |

The core crate owns the file model, rule pipeline, validation, and transactional
rename. It must never depend on egui. The binary owns egui widgets, dialogs,
themes, settings, and desktop/file-manager integration.

## Required checks

Run these before handing work back:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo doc --no-deps -p rename-rx-core
```

Fix findings instead of silencing them. When a lint is genuinely wrong for one
item, use the narrowest `#[expect(clippy::lint, reason = "...")]`. Do not add
`#![deny(warnings)]` to source; CI turns warnings into errors.

## Lint policy

The Beautiful Rust lint configuration lives in the workspace:

- `clippy.toml` sets the thresholds (`max-fn-params-bools = 0`, test
  exceptions).
- `[workspace.lints]` in `Cargo.toml` is inherited by both crates through
  `[lints] workspace = true`.

Deliberate deviations, kept in `Cargo.toml` with a comment:

- `exhaustive_enums` is off: this is an internal workspace, and exhaustive
  matches make a new rule mode fail compilation instead of silently falling
  through a wildcard arm.

The binary initializes `env_logger` in `main`; use the `log` macros for
diagnostics instead of `println!` or `eprintln!`. The core library does not
print or log.

## Behavior invariants

- The live preview and the real rename use the same rule pipeline.
- Rename is transactional: every source is staged first, then finalized, with
  rollback on failure. Do not replace this with a per-file rename loop.
- Validation output is deterministic. Do not introduce `HashMap` iteration into
  user-visible ordering; use `BTreeMap` or sort before emitting.
- Settings and presets are stored under `~/.config/rename-rx/` and read
  defensively: old files with missing fields must keep working through
  `#[serde(default)]`.
- Do not change visible UI text, shortcut behavior, or the on-disk rename
  semantics unless the task asks for it.

## Documentation

- Public items in `rename-rx-core` get `///` docs. Result-returning functions
  get `# Errors`; reachable panics get `# Panics`.
- Comments explain why, not what. `// SAFETY:` on any `unsafe` block.
- UI-only helpers in the binary do not need rustdoc.
