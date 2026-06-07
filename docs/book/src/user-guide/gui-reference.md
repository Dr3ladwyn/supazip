# GUI reference

The SupaZip desktop application is built with [eframe](https://github.com/emilk/egui/tree/master/eframe) / [egui](https://github.com/emilk/egui) and provides a PeaZip-style interface for browsing, extracting, creating, and testing archives.

## Window layout

```
+----------------------------------------------------------+
| Menu bar: File  Edit  View  Help                         |
+----------------------------------------------------------+
| Toolbar: [Open...] [Recent v] [Extract] [Create...]      |
|          [Test]                              [Cancel]    |
+----------------------------------------------------------+
|                                                          |
|  Central panel                                           |
|  - Heading: "<path> (<backend>)"                         |
|  - Striped grid: # | Name | Size | Encrypted            |
|                                                          |
+----------------------------------------------------------+
| Status bar: [spinner] <status text>                      |
+----------------------------------------------------------+
```

## Toolbar

| Button | Description |
|--------|-------------|
| **Open…** | Open a file dialog to pick an archive. Filtered to `.zip`, `.7z`, `.tar`, `.tar.gz`, `.tar.xz`. |
| **Recent ▾** | Dropdown of the 10 most recently opened archives. Entries are pruned lazily when the menu opens. Click **Clear recent** to empty the list. |
| **Extract** | Extract the currently open archive. Prompts for a destination folder. Disabled when no archive is loaded. |
| **Create…** | Create a new archive. Prompts for the target path and input files. |
| **Test** | Verify the integrity of the currently open archive. Disabled when no archive is loaded. |
| **Cancel** | Cancel the in-flight operation. Only visible while an operation is running. |

## Entry grid

The central panel displays a four-column striped grid:

| Column | Description |
|--------|-------------|
| `#` | 0-based entry index (right-aligned, 4 characters) |
| `Name` | Entry path within the archive (left-aligned, expands) |
| `Size` | Uncompressed size in bytes (right-aligned, 12 characters) |
| `Encrypted` | `yes` or `-` (left-aligned) |

Rows are striped with alternating background colours for readability.

## Context menu

Right-click an entry row to open the context menu:

| Action | Description |
|--------|-------------|
| **Extract here…** | Extract the selected entry to a chosen directory. |
| **Extract to…** | Extract to a different destination. If the archive is encrypted, a password dialog opens first. |
| **Test entry** | Check the entry's integrity (whole-archive test; per-entry test is a planned feature). |
| **Copy path** | Copy the entry's in-archive path to the clipboard. |

## Menu bar

### File

| Item | Shortcut | Description |
|------|----------|-------------|
| Open… | `Ctrl+O` | Open an archive file. |
| Close | `Ctrl+W` | Close the current archive. |
| Extract… | `Ctrl+E` | Extract the current archive. |
| Create… | — | Create a new archive. |
| Test integrity | — | Test the current archive. |
| Quit | `Ctrl+Q` | Exit the application. |

### Edit

| Item | Description |
|------|-------------|
| Copy path | Uses the context menu path; the Edit menu item is a placeholder. |

### View

| Item | Description |
|------|-------------|
| Debug overlay | Toggle a debug overlay that dumps the current `AppState`. |

### Help

| Item | Shortcut | Description |
|------|----------|-------------|
| About | `F1` | Show the About modal. |

## Drag and drop

Drop archive files onto the main window to open them. A semi-transparent overlay appears while dragging. Multiple files can be dropped simultaneously; each is opened in sequence.

On platforms where the system DnD is filtered (e.g. some Wayland compositors), the overlay appears but the drop event may not arrive. Use **File → Open** as a fallback.

## Password dialog

When an encrypted archive is opened, a modal password dialog appears:

- The dialog has a password input field with a show/hide toggle.
- Submit the password to retry the operation (open, extract, create, or test).
- Press **Cancel** to dismiss without entering a password.

The dialog is context-aware: it knows which operation requested the password (Open, Extract, Create, Test) and re-dispatches the correct worker after submission.

## Progress dialog

Long-running operations (extract, create, test) display a modal progress dialog:

- Shows the current operation name and a progress bar.
- A **Cancel** button interrupts the operation via the shared cancellation flag.
- The dialog disappears automatically when the operation completes or is cancelled.

## Status bar

The bottom status bar shows:

- A **spinner** while an operation is in flight.
- The current status text: `"Open an archive to get started."` (idle), `"loaded"` (after open), `"extracting…"` (in progress), `"error: <msg>"` (on failure).

## Keyboard shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+O` | Open archive |
| `Ctrl+W` | Close archive |
| `Ctrl+E` | Extract archive |
| `Ctrl+Q` | Quit |
| `F1` | About |
| `Esc` | Cancel in-flight operation / close dialog |
