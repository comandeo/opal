# opal

A minimalist Markdown editor: markup is shown only for the element under the cursor, the text column is centered, typewriter mode, autosave. GTK4, no web engine.

## Building

Debian: `sudo apt install libgtk-4-dev pkg-config` · FreeBSD: `pkg install gtk4 rust pkgconf`

```
scripts/install.sh
```

The script builds opal and installs it for the current user in `~/.local`: the program (`~/.local/bin/opal`), a launcher entry for application menus and launchers (fuzzel, rofi, wofi…) and an icon. To install elsewhere, run `scripts/install.sh /usr/local` (needs write access). How to uninstall is described at the top of the script.

## Usage

`opal file.md` opens a file (if it doesn't exist, it is created on the first save); `opal` starts a new document.

When started from a terminal, opal returns to the shell at once and the window lives on its own, so the terminal can be closed. To keep the editor attached to the terminal (for example, to see its messages), run `OPAL_FOREGROUND=1 opal file.md`.

| Keys | |
|---|---|
| Ctrl+S | save now (for a new document, choose a name) |
| Ctrl+O / Ctrl+N | open / new document |
| Ctrl+T | toggle typewriter mode |
| Ctrl+Q | close |
| Ctrl+click | open a link |

Saving is automatic: one second after you pause typing, and on close.

Autocorrect while typing: `--` → `—`, `"…"` → `«…»`, quotes inside quotes → `„…“`. Nothing is replaced inside code. Ctrl+Z right after a replacement restores what you typed.

## Settings

`~/.config/opal/config`:

```
column_width = 70        # column width in characters
font = Serif 14
mono_font = Monospace 12
typewriter = true
theme = light            # light | dark
autocorrect = true       # replace dashes and quotes while typing
```

opal draws on the CPU (GTK's `cairo` renderer): this way the window appears faster and no GPU drivers are loaded. Another renderer can be chosen the usual GTK way: `GSK_RENDERER=vulkan opal`.

## Tests

`cargo test --lib` covers the logic; `cargo test --test gtk_editor` and `cargo test --test gtk_window` open windows and need a display.
