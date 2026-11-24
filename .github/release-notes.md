# Rename RX 0.2.2

A lightweight, fast batch file renamer for Linux, written in Rust and inspired
by the workflow of **Rename X** for macOS.

## Downloads

- `Rename_RX-x86_64.AppImage` — Intel/AMD 64-bit
- `Rename_RX-aarch64.AppImage` — ARM 64-bit
- Matching `.zsync` files for delta updates

The AppImages are built on Debian 12 and require glibc **2.35 or newer**.

## What's new in 0.2.2

- **Rounded window corners:** the integrated Adwaita headerbar and footer now
  have rounded corners on composited desktops. X11 sessions without a
  compositor fall back to square corners instead of black transparent corners.
- **AppImage runtime:** images now embed uruntime v0.8.5, which preserves
  `ARGV0` when FUSE is unavailable or a mount retries. The build downloads and
  verifies the runtime's published SHA-256.

## Highlights

- Live preview, validity report, transactional rename and undo.
- Rules: New Name, Replace, Regular Expression, Insert, Keep, Remove, Move,
  Serialize, Prefix, Suffix, Extension, Case and Sort.
- **Duplicate rule rows:** press `+` on a rule to add another instance directly
  below it. Duplicates show `−` and can be removed independently. The source
  rule is enabled when a duplicate is added.
- **Toggle from the name:** double-click any rule name to enable or disable it.
- **Rename summary:** after renaming, a dialog reports the result and offers
  **Undo Rename** directly.
- **Automatic Serialize padding:** the width follows the batch and respects a
  configurable minimum, keeping conventions such as `s01e08`.
- Editing a rule's controls or settings enables the rule automatically.
- Per-rule options for Replace, Regular Expression and Serialize.
- Saved presets.
- Adwaita light/dark/system theme and a rounded custom headerbar.
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
