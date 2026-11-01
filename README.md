# Rename RX

Rename RX is a **lightweight, fast batch file renamer for Linux**, written in
Rust with [egui](https://github.com/emilk/egui) and
[eframe](https://github.com/emilk/egui/tree/main/crates/eframe). It is inspired
by the workflow of **Rename X** for macOS: a compact window, one list of rules,
and a live preview of every new name.

The interface follows the system light/dark preference and uses Adwaita-style
widgets and a custom headerbar.

## Download

Grab the AppImage for your architecture from the
[latest release](https://github.com/Alarez777/Rename-RX/releases/latest):

- `Rename_RX-x86_64.AppImage` — Intel/AMD 64-bit
- `Rename_RX-aarch64.AppImage` — ARM 64-bit

```sh
chmod +x Rename_RX-x86_64.AppImage
./Rename_RX-x86_64.AppImage
```

If FUSE is not available:

```sh
./Rename_RX-x86_64.AppImage --appimage-extract-and-run
```

The AppImages are built on Debian 12 and need glibc **2.35 or newer**.

### Automatic updates

Each AppImage embeds its update information, and every release ships the
matching `.zsync` file. Tools compatible with AppImageUpdate
([AppImageUpdate](https://github.com/AppImageCommunity/AppImageUpdate),
Gear Lever, etc.) can download only the changed blocks and update in place.

Rename RX does not yet include its own “check for updates” button; updates are
handled by those external tools.

## Features

### Batch workflow

- Add files with the picker, drag and drop, or command-line arguments.
- Multi-select with click, `Ctrl`/`Cmd`+click and `Shift`+click.
- Live preview of every resulting file name.
- Validity report with errors and warnings; errors block renaming.
- Transactional rename: all changes are staged, then applied, with rollback on
  failure.
- One-level undo through `Ctrl+Z` or **Undo Rename** in the menu.

### Rename rules

Rules are applied from top to bottom, with per-rule options where needed:

- New Name
- Replace (case-sensitive and whole-word options)
- Regular Expression (ignore-case option, capture groups)
- Insert (before/after, counted from either end)
- Keep (keep a range of characters)
- Remove (remove a range of characters)
- Move (move a range to another position)
- Serialize (start, step, offset, padding, prefix/suffix, before/after)
- Prefix
- Suffix
- Extension
- Case (lowercase, title case, uppercase)
- Sort (original order, name, created, modified; ascending/descending)

The live preview uses the same pipeline as the actual rename.

### Tool visibility

The **Tools** button next to **Presets** splits the rules into **Visible** and
**Hidden** columns. Unchecking a tool moves it to **Hidden** and turns it off
while keeping its values; checking it moves it back to **Visible**. Both columns
are alphabetical, and the choice is remembered in
`~/.config/rename-rx/settings.json`. Loading a preset automatically shows every
tool that preset uses.

### Presets

The **Presets** view saves the complete rule configuration under a name:

- Select a preset and click **Load**, or double-click it.
- **Delete** asks for confirmation.
- Presets are stored in `~/.config/rename-rx/presets.json` and stay compatible
  when new rules are added.

### Desktop and file managers

- **Settings → Desktop integration** installs a user-level launcher and icon so
  GNOME groups Rename RX windows under the correct dock icon.
- **Settings → File manager integration** adds a “Rename with Rename RX” /
  “Renombrar con Rename RX” entry for:

  - Nemo
  - Nautilus (requires `python3-nautilus` / `nautilus-python`)
  - Thunar
  - Dolphin

### Appearance

- Adwaita-style dark and light themes, or follow the system preference.
- Custom Adwaita headerbar with minimize, maximize/restore, close, dragging and
  resize edges.
- Window style selector ready for future platform-specific styles.

## Keyboard shortcuts

| Shortcut | Action |
| --- | --- |
| `Ctrl+O` | Add files |
| `Ctrl+A` | Select all files |
| `Delete` | Remove selected files from the list |
| `Ctrl+Enter` | Rename |
| `Ctrl+Z` | Undo the last rename |
| `Escape` | Close the active dialog / leave the field |

## Run from source

Requirements:

- Rust 1.98 or newer
- Linux with X11 or Wayland, and OpenGL

```sh
cargo run -p rename-rx
cargo run -p rename-rx -- path/to/file1 path/to/file2
```

## Build an AppImage

```sh
./packaging/build-appimage.sh
```

The script builds a release binary on the host and downloads `appimagetool`
into `dist/.tools` on the first run. To package a binary built elsewhere (for
example in Debian 12 for older glibc compatibility):

```sh
RENAME_RX_SKIP_BUILD=1 \
RENAME_RX_BINARY=/path/to/rename-rx \
RENAME_RX_ARCH=x86_64 \
./packaging/build-appimage.sh
```

For ARM:

```sh
RENAME_RX_SKIP_BUILD=1 \
RENAME_RX_BINARY=/path/to/rename-rx-aarch64 \
RENAME_RX_ARCH=aarch64 \
./packaging/build-appimage.sh
```

The packaged AppImages are written to `dist/` together with their `.zsync`
files.

## Workspace

| Crate | Purpose |
| --- | --- |
| `crates/rename-rx` | egui/eframe application, Adwaita theme, dialogs, file table, presets |
| `crates/rename-rx-core` | UI-independent file model, rules, preview, validation and rename transaction |

## Roadmap

- [x] Window shell, Adwaita light/dark theme, system theme detection
- [x] File selection, drag and drop, table, selection and shortcuts
- [x] Rule engine, live preview, validity report
- [x] Transactional rename with undo
- [x] Keep and Move rules
- [x] Presets
- [x] Desktop and file-manager integration
- [x] x86_64 and aarch64 AppImages with zsync updates
- [ ] Date/Time and Part rules
- [ ] Further polish and accessibility
- [ ] Automated release pipeline

## License

MIT — see [LICENSE](LICENSE).

Rename RX is an independent project inspired by Rename X for macOS and is not
affiliated with its author.
