# Problems to Fix — STATUS: RESOLVED

All reported mechanical glitches have been successfully resolved. Below is a record of the problems and their respective solutions.

---

## 1. Popup Location Glitch (Wayland / XWayland) — [RESOLVED]

### Problem:
When triggered, the definition popup blipped/flashed at the top-left corner of the screen `(0,0)` for a brief moment before snapping to the correct coordinate space alongside the cursor.

### Solution:
* **Robust Window Identification:** Upgraded the X11 Breadth-First-Search (BFS) window matching logic to search for `_NET_WM_NAME` and `WM_CLASS` in addition to the legacy `WM_NAME`. This ensures the popup window is located reliably and quickly by the daemon.
* **Centered Above Positioning:** Modified the placement logic to position the popup window centered horizontally relative to the cursor/selection (`target_x = cx - width / 2`) and vertically above it (`target_y = cy - height - 15`).
* **Boundary Clamping:** Integrated root window geometry checks (`conn.get_geometry(root)`) to query screen dimensions and clamp coordinates inside screen boundaries.
* **Top-of-Screen Fallback:** If the cursor is near the top edge of the screen, the positioning automatically flips below the cursor (`target_y = cy + 20`) to prevent it from rendering off-screen.
* **Window Accumulation Prevention:** Implemented a thread-local static tracker in `popup.rs` to close and destroy any previously active definition popup before displaying a new one, preventing dock/task list clutter.

---

## 2. PDF Auto-Selection Ignored — [RESOLVED]

### Problem:
Highlighting text inside a PDF viewer (like Evince/Okular) under Wayland did not automatically trigger the definition popup.

### Solution:
* **Corrected Hierarchy Traversal:** Fixed a logic bug in `is_pdf_viewer_window` (in `selection.rs`) where the loop returned `false` on the first sub-window that had a non-matching class or PID. It now correctly queries the parent window and traverses up the X11 window hierarchy until a matching PDF viewer class (`evince`/`okular`) or the root window is reached.
* **Automated XWayland Overrides:** Added a native helper `setup_wayland_pdf_compatibility` to the daemon's startup loop in `daemon.rs`. When running on a Wayland session, it automatically configures user-level desktop overrides in `~/.local/share/applications/` to run Evince (with `GDK_BACKEND=x11`) and Okular (with `QT_QPA_PLATFORM=xcb`) under XWayland, allowing the X11 selection watcher to capture focus and copy events natively.
