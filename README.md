# Rename RX

Rename RX is a native batch file renamer for Linux, written in Rust with
[egui](https://github.com/emilk/egui)/[eframe](https://github.com/emilk/egui/tree/main/crates/eframe).
It is a reimplementation of the macOS app **Rename X**, with an Adwaita-style
interface that follows the system light/dark preference.

> Status: Phase 0 — application shell. File selection and the rename engine
> land in the next milestones.

## Requirements

- Rust 1.98 or newer
- Linux with X11 or Wayland, and OpenGL

## Run

```sh
cargo run -p rename-rx
```

## Workspace

| Crate | Purpose |
| --- | --- |
| `crates/rename-rx` | egui/eframe application, Adwaita theme, dialogs |
| `crates/rename-rx-core` | UI-independent rename engine, validation, transactions |

## Roadmap

- [x] Phase 0 — window shell, Adwaita light/dark theme, system theme detection,
      Settings and About dialogs
- [ ] Phase 1 — Add Files, drag and drop, file table
- [ ] Phase 2 — rule engine, live preview, validity report
- [ ] Phase 3 — transactional rename with undo
- [ ] Phase 4 — per-rule settings, date/time rules
- [ ] Phase 5 — polish and accessibility
- [ ] Phase 6 — AppImage packaging and release CI

## License

Not chosen yet.
