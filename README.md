# Rename RX

Rename RX is a native batch file renamer for Linux, written in Rust with
[egui](https://github.com/emilk/egui)/[eframe](https://github.com/emilk/egui/tree/main/crates/eframe).
It is a reimplementation of the macOS app **Rename X**, with an Adwaita-style
interface that follows the system light/dark preference.

> Status: Phase 3 — transactional rename with undo. Per-rule settings and
> date/time rules are next.

## Requirements

- Rust 1.98 or newer
- Linux with X11 or Wayland, and OpenGL

## Run

```sh
cargo run -p rename-rx
cargo run -p rename-rx -- path/to/file1 path/to/file2
```

You can also drag files onto the window or press `Ctrl+O`. Select rows with a
click, `Ctrl`/`Cmd`+click, or `Shift`+click. `Delete` removes the selected
entries and `Ctrl+A` selects all of them.

The rules panel applies every checked rule to the live preview. `Validity`
opens the report of errors and warnings; errors block renaming and warnings
ask for confirmation. `Rename` (`Ctrl+Enter`) stages every change in the same
directory and rolls back on failure. The last successful rename can be undone
with `Ctrl+Z` or **Undo Rename** in the menu.

## Build a test AppImage

```sh
./packaging/build-appimage.sh
```

The result is `dist/Rename_RX-x86_64.AppImage`. The script builds a release
binary and downloads `appimagetool` into `dist/.tools` on the first run. A
minimal desktop entry and icon are embedded only because `appimagetool`
requires them; they are not installed on the system.

If your system does not have FUSE, extract and run it:

```sh
./dist/Rename_RX-x86_64.AppImage --appimage-extract-and-run
```

To package a binary built elsewhere (for example in an older container for
wider glibc compatibility):

```sh
RENAME_RX_SKIP_BUILD=1 RENAME_RX_BINARY=/path/to/rename-rx \
  ./packaging/build-appimage.sh
```

## Workspace

| Crate | Purpose |
| --- | --- |
| `crates/rename-rx` | egui/eframe application, Adwaita theme, dialogs, file table |
| `crates/rename-rx-core` | UI-independent file model, rules, preview, validation and rename transaction |

## Roadmap

- [x] Phase 0 — window shell, Adwaita light/dark theme, system theme detection,
      Settings and About dialogs
- [x] Phase 1 — Add Files, drag and drop, file table, selection, keyboard shortcuts
- [x] Phase 2 — rule engine, live preview, validity report
- [x] Phase 3 — transactional rename with undo
- [ ] Phase 4 — per-rule settings, date/time rules
- [ ] Phase 5 — polish and accessibility
- [ ] Phase 6 — AppImage packaging and release CI

## License

Not chosen yet.
