# Problems to Fix

## 1. Popup Location Glitch (Wayland / XWayland)
The global hotkey (`Ctrl+Alt+W`) works everywhere successfully now, but there is a visual glitch regarding the popup rendering. When triggered, the popup blips at the top-left corner of the screen for a brief moment before snapping to the correct coordinate space alongside the cursor.

### What the Current Implementation Does:
The following steps are currently executed mechanically in `popup.rs` to attempt mitigating this issue:
1. `popup::show(...)` receives the tracked `x` and `y` coordinates from the daemon.
2. A `gtk4::ApplicationWindow` is instantiated and populated with the parsed word definitions.
3. The code explicitly calls `window.set_opacity(0.0);` to try and make it invisible.
4. `window.present();` is fired, which signals GTK/the Desktop Environment to attach and map the underlying window surface natively (which implicitly drops it at 0,0).
5. A 50-millisecond delay is triggered via `glib::timeout_add_local_once`.
6. After 50ms, `move_window_to_cursor_x11(x, y)` is executed synchronously:
   - It performs an X11 Breadth-First-Search (BFS) querying the root tree for windows bearing `WM_NAME == "wd"`.
   - Natively overrides the X11 spatial positioning by assigning `configure_window(...)` coordinates.
7. Finally, another nested 50-millisecond delay triggers, which tries to complete the sequence by running `window.set_opacity(1.0);`.

## 2. PDF Auto-Selection Ignored
Auto-detecting definitions by simply highlighting text inside a PDF viewer (like Evince/Okular) currently does not trigger the popup at all. (To test word lookups inside PDFs, users are currently forced to highlight the word and hit the manual global hotkey `Ctrl+Alt+W` instead).

### What the Current Implementation Does:
The following is mechanically executed in `selection.rs` dynamically intercepting primary highlights seamlessly to distinguish PDFs:
1. `SelectionWatcher` loops using `x11rb::wait_for_event()` waiting exclusively for `XFixesSelectionNotify` events indicating the Linux `PRIMARY` text selection updated.
2. Upon detection, it uses `get_selection_owner()` to find the specific X11 Window ID possessing clipboard ownership.
3. In case the selection was performed under Wayland (where copy events are routed via a generic "proxy window"), it actively interrogates `_NET_ACTIVE_WINDOW` on the X11 backend to identify the current GUI application in focus.
4. It passes both the `selection_owner` and the `active_window` into `is_pdf_viewer_window`.
5. `is_pdf_viewer_window` implements a double-layered check up to 10 nodes deep in the hierarchy:
   - **Stage 1**: Reads `WM_CLASS` attributes validating boundaries against known values (`evince`, `okular`). 
   - **Stage 2**: If no classes register, queries `_NET_WM_PID`. If a process-ID exists, it reads `/proc/<pid>/comm` dynamically scanning for target binary names identically bounding PDF implementations.
6. Only if one of the windows officially registers as a PDF viewer does it call `convert_selection` loading the UTF-8 text string.
7. Text length bounds are trimmed and clamped (`take(4).count() <= 3`).
8. The `SelectionEvent` is emitted to `daemon.rs` containing the word, alongside global `pointer.root_x` and `pointer.root_y` tracked from the exact triggering millisecond.
