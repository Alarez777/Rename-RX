# Rename RX 0.2.0

A lightweight, fast batch file renamer for Linux, written in Rust and inspired
by the workflow of **Rename X** for macOS.

## Downloads

- `Rename_RX-x86_64.AppImage` — Intel/AMD 64-bit
- `Rename_RX-aarch64.AppImage` — ARM 64-bit
- Matching `.zsync` files for delta updates

The AppImages are built on Debian 12 and require glibc **2.35 or newer**.

## Highlights

- Live preview, validity report, transactional rename and undo.
- Rules: New Name, Replace, Regular Expression, Insert, Keep, Remove, Move,
  Serialize, Prefix, Suffix, Extension, Case and Sort.
- **Duplicate rule rows:** press `+` on a rule to add another instance directly
  below it. Duplicates show `−` and can be removed independently. The source
  rule is enabled when a duplicate is added.
- **Toggle from the name:** double-click any rule name to enable or disable it.
- Per-rule options for Replace, Regular Expression and Serialize.
- Saved presets.
- Adwaita light/dark/system theme and a custom headerbar.
- Desktop integration for GNOME.
- Context-menu actions for Nemo, Nautilus, Thunar and Dolphin.

## Updating

Each AppImage embeds its zsync update information. Tools compatible with
AppImageUpdate can update to a newer release without downloading the whole
image. Rename RX does not include an in-app updater yet.

## Notes

- Date/Time and Part rules are not implemented yet.
- Please report issues on GitHub.
- Licensed under the MIT License.
