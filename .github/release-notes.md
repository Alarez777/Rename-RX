# Rename RX 0.2.1

A lightweight, fast batch file renamer for Linux, written in Rust and inspired
by the workflow of **Rename X** for macOS.

## Downloads

- `Rename_RX-x86_64.AppImage` — Intel/AMD 64-bit
- `Rename_RX-aarch64.AppImage` — ARM 64-bit
- Matching `.zsync` files for delta updates

The AppImages are built on Debian 12 and require glibc **2.35 or newer**.

## What's new in 0.2.1

- **Rename summary:** a dialog reports how many files were renamed, including
  any that could not be reloaded, and offers **Undo Rename** directly.
- **Automatic Serialize padding:** the width follows the batch and never drops
  below a configurable minimum (default 2), so a short series keeps a
  convention such as `s01e08`. The last manual width is remembered while
  automatic mode is on.
- **Enable on edit:** typing in a rule's field or changing its spinbox, combo
  or settings now enables that rule automatically.
- **Single instance:** opening files from another launch reuses and focuses
  the existing window instead of starting a second one.
- **Resizable file table:** the name column can be resized and the table has a
  grid.
- **Spinbox repeat:** holding the arrow buttons repeats the step.
- **New AppImage runtime:** images now embed uruntime v0.8.3, downloaded and
  verified against its published SHA-256 during the build.

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
