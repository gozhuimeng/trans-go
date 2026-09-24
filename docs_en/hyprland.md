# Hyprland / Wayland Desktop Integration

## The limitation, stated up front

Wayland deliberately **does not allow any application to listen to global hotkeys**.

It has nothing to do with how well pot or transgo is built — it is a protocol-level rule.
X11 let applications grab global hotkeys with `XGrabKey`; Wayland removed that capability so
a malicious app can't act as a keylogger, and the compositor holds it instead.

So "register a global hotkey" under Wayland has exactly one correct shape: let the desktop
environment own the binding and have it invoke your command.

transgo is designed around that shape. The CLI is the integration point: no port, no daemon
polling in the background.

---

## Basic bindings

```bash
# ~/.config/hypr/hyprland.conf

# Translate the clipboard, write the result back to the clipboard
bind = SUPER, T, exec, wl-paste | transgo | wl-copy

# Translate the clipboard and show a notification
bind = SUPER SHIFT, T, exec, notify-send "译文" "$(wl-paste | transgo)"

# GUI window
bind = SUPER, V, exec, transgo gui --clip   # opens pre-filled from the clipboard
bind = SUPER, G, exec, transgo gui          # opens a blank window
```

Run `hyprctl reload` to apply.

---

## Handy scripts

### Translate and copy the result

```bash
#!/usr/bin/env bash
# ~/.local/bin/trans-copy
set -euo pipefail

result="$(wl-paste --no-newline | transgo)"
[ -n "$result" ] || exit 0

printf '%s' "$result" | wl-copy
notify-send -t 4000 "译文（已复制）" "$result"
```

### Translate into a scrollable notification

Long text gets truncated by `notify-send`; write it to a temp file and open a floating window
instead:

```bash
#!/usr/bin/env bash
# ~/.local/bin/trans-show
set -euo pipefail

wl-paste --no-newline | transgo > /tmp/transgo-out.txt
kitty --class transgo-float -e less /tmp/transgo-out.txt
```

With a window rule to float `less` near the cursor:

```bash
# ~/.config/hypr/hyprland.conf
windowrulev2 = float, class:^(transgo-float)$
windowrulev2 = size 640 480, class:^(transgo-float)$
windowrulev2 = center, class:^(transgo-float)$
```

### Pick a language interactively

Use `rofi` as a language picker, then translate:

```bash
#!/usr/bin/env bash
# ~/.local/bin/trans-pick
set -euo pipefail

lang="$(transgo lang | tail -n +3 | rofi -dmenu -p '译成')"
[ -n "$lang" ] || exit 0
code="${lang%%[[:space:]]*}"        # take the first column: the language code

result="$(wl-paste --no-newline | transgo -t "$code")"
notify-send -t 6000 "译文 → ${code}" "$result"
```

### In pipelines

stdout carries the translation only and metadata goes to stderr, so pipelines compose
freely:

```bash
# straight into an editor
wl-paste | transgo | nvim -

# translate a file line by line
while IFS= read -r line; do
  printf '%s\n' "$(transgo "$line")"
done < words.txt

# pick a past translation with fzf
history | fzf | transgo
```

---

## GUI window rules

The GUI is a standalone window; window rules control it precisely. Match on **title** (the
window title is `transgo`; the class is empty in some environments and a class match then
misses):

```bash
# float the translation window near the cursor
windowrulev2 = float, title:^(transgo)$
windowrulev2 = pin, title:^(transgo)$
windowrulev2 = stayfocused, title:^(transgo)$
```

Two entry points, two ways to summon it:

| Key | Command | Behavior |
|---|---|---|
| `SUPER + G` | `transgo gui` | Open a blank window |
| `SUPER + V` | `transgo gui --clip` | Open pre-filled with the clipboard |

> transgo listens to neither the clipboard nor keys. `--clip` simply reads the clipboard once
> at startup and fills the input box — and only when the hotkey explicitly asks for it. No
> background daemon, no surprise pop-ups.

---

## Why not a daemon plus a port

Tools in the pot family expose a local port to work around "applications can't get global
hotkeys": something external (a script or the desktop environment) sends the trigger over
HTTP.

transgo needs no such layer:

| Approach | Problems |
|---|---|
| Daemon + HTTP port | One more resident process, one more listening port reachable by other local processes, more state to manage |
| Daemon + D-Bus | Still resident, and harder to debug |
| **Direct CLI invocation** (transgo) | Stateless, no daemon, no listener. Cold start is just one process launch |

When cross-process state is genuinely needed — feeding translations back into a GUI window —
a Unix socket does the job: the GUI's singleton messages (focusing an already-open window on
a second invocation, sending `--clip` text in to be translated) travel exactly this way. It
stays an internal GUI detail and does not change how the CLI is used.

---

## Companion tools

Common partners for transgo on Wayland:

| Tool | Purpose | Notes |
|---|---|---|
| `wl-clipboard` | `wl-paste` / `wl-copy` | Clipboard access |
| `wtype` | Simulate keystrokes to the focused window | Selection translation needs it to simulate Ctrl+C |
| `grim` + `slurp` | Screenshot + region selection | Screenshot OCR translation |
| `tesseract` | OCR | Screenshot OCR translation |
| `libnotify` | `notify-send` | Translation notifications |

```console
$ sudo pacman -S wl-clipboard wtype grim slurp tesseract
```
