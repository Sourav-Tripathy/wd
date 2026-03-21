# wd

A lightweight word-lookup daemon and CLI tool for Linux.

## Features

- **CLI mode** — Type `wd <word>` and get a clean, formatted definition instantly
- **Daemon mode** — Runs silently in the background:
  - Auto-detects text selection in PDF viewers (Evince/Okular) and shows a popup
  - Global hotkey (`Ctrl+Alt+W`) to look up the current text selection from any app
- **Offline-first** — Uses a local WordNet database with automatic Wiktionary fallback
- **PDF annotation** — Save definitions as annotations directly in your PDF (`Ctrl+Alt+S`)
- **Lightweight** — Under 4 MB RAM at idle, zero CPU between lookups

## Install

### From source

```bash
# Install build dependencies
sudo apt install libgtk-4-dev libdbus-1-dev

# Build
cargo build --release

# Install binary
sudo cp target/release/wd /usr/bin/wd

# Install systemd service
mkdir -p ~/.config/systemd/user
cp debian/wd.service ~/.config/systemd/user/
systemctl --user enable --now wd.service
```

### From .deb package

```bash
sudo apt install ./wd_0.1.0_amd64.deb
```

### WordNet data

wd works best with WordNet installed locally:

```bash
sudo apt install wordnet-base
```

If `wordnet-base` is not installed, the post-install script downloads the data automatically.

## Usage

### CLI

```bash
# Look up a word
wd think

# Output:
# think  (verb)
#
#   1. Judge or regard; look upon; judge.
#
#      "She thinks he is a saint"
#
#   2. Expect, believe, or suppose.
#
#      "I thought to find her in a bad state"
#
#   3. Use or exercise the mind or one's power of reason.
#
#      "Think before you speak"
#
# [WordNet]
```

### Daemon

```bash
# Start the daemon (usually handled by systemd)
wd --daemon

# Or enable the systemd service
systemctl --user enable --now wd.service
```

## Hotkeys

| Hotkey | Action |
|--------|--------|
| `Ctrl+Alt+W` | Look up the current text selection |
| `Ctrl+Alt+S` | Save definition as PDF annotation (while popup is open) |
| `Escape` | Dismiss the popup |

## Configuration

Edit `~/.config/wd/config.toml` (created with defaults on first run):

```toml
lookup_hotkey = "Ctrl+Alt+W"
annotate_hotkey = "Ctrl+Alt+S"
pdf_auto_trigger = true
popup_timeout_ms = 0
popup_font_size = 13
max_definitions = 3
annotate_include_example = true
```

## Lookup Resolution

1. Normalise input (lowercase, strip punctuation, trim)
2. Check local WordNet index (including morphological variants)
3. If not found, silently query Wiktionary REST API
4. Source label (`[WordNet]` or `[Wiktionary]`) shown in output

## Current Limitations & Known Issues

`wd` currently relies heavily on X11 APIs (via `x11rb`) for global hotkeys, window identification, and popup placement. Because of this, using it under **Wayland** introduces several quirks:

1. **Wayland Auto-Trigger Issue**: If you run Evince or Okular natively under Wayland, the `wd` daemon cannot detect that the text selection came from a PDF viewer (Wayland hides the application identity behind an "Xwayland proxy window", stripping the application class).
   - **Workaround:** For the auto-trigger to recognize your PDF viewer, run it using XWayland (e.g., `GDK_BACKEND=x11 evince`). 
   - **Alternative:** You can always just highlight text and use the manual `Ctrl+Alt+W` hotkey (this bypasses the application check).
2. **Wayland Global Hotkeys**: On strict Wayland compositors (like GNOME), global hotkeys (`Ctrl+Alt+W`) will only fire if an X11/XWayland window is currently in focus. Less restrictive compositors (like KDE Plasma) usually allow global X11 shortcuts out-of-the-box.
3. **Cursor Snapping**: GTK4 natively hands over all window positioning to the Wayland compositor. To force the definition popup to render near your cursor, we use a manual X11 coordinate override. **For this to work cleanly, the daemon must be run with `GDK_BACKEND=x11`.**
4. **Linux Only**: Heavily coupled to D-Bus, X11, and Linux filesystem structures.

## Contributing

Contributions are incredibly welcome! I am specifically looking for help migrating `wd` to natively support the modern Wayland desktop ecosystem.

**Open Issues you can help solve:**
- **Native Wayland Selection Watcher:** Implement a Wayland-native text selection monitor (e.g., using the `wlr-data-control` protocol) to replace the current `XFixes` dependency. 
- **Wayland Global Hotkeys:** Support Wayland global shortcuts (e.g., using `ext-session-lock-v1` or compositor-specific DBus APIs) instead of X11-specific global keyboard grabs.
- **GTK4 Notification Placement:** Find a clean, native approach to placing GTK4 popup windows precisely at the cursor coordinates within a strict Wayland environment.

If you enjoy system-level Rust programming, feel free to dive in and open a PR!

## License

MIT
