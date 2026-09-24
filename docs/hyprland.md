# Hyprland / Wayland 桌面集成

## 先说清楚那个限制

Wayland 出于安全设计，**不允许任何应用自行监听全局按键**。

这跟 pot、transgo 的实现好坏无关，是协议层面的规定。X11 时代应用可以 `XGrabKey` 抢全局热键，
Wayland 为了防止恶意应用做键盘记录器，把这个能力收走了，改由合成器统一持有。

所以「注册全局快捷键」这件事在 Wayland 下只有一个正确做法：让桌面环境去绑按键，
触发时调用你的命令。

transgo 就是按这个思路设计的，CLI 就是集成入口：不需要开端口，也不需要守护进程在后台轮询。

---

## 基本绑定

```bash
# ~/.config/hypr/hyprland.conf

# 翻译剪贴板内容，结果写回剪贴板
bind = SUPER, T, exec, wl-paste | transgo | wl-copy

# 翻译剪贴板内容并弹出通知
bind = SUPER SHIFT, T, exec, notify-send "译文" "$(wl-paste | transgo)"

# 图形界面窗口
bind = SUPER, V, exec, transgo gui --clip   # 唤出并预填剪贴板
bind = SUPER, G, exec, transgo gui          # 唤出空白窗口
```

改完 `hyprctl reload` 生效。

---

## 实用脚本

### 翻译并复制结果

```bash
#!/usr/bin/env bash
# ~/.local/bin/trans-copy
set -euo pipefail

result="$(wl-paste --no-newline | transgo)"
[ -n "$result" ] || exit 0

printf '%s' "$result" | wl-copy
notify-send -t 4000 "译文（已复制）" "$result"
```

### 翻译并弹出可滚动的通知

文本较长时 `notify-send` 会被截断，可以落成临时文件再开浮窗：

```bash
#!/usr/bin/env bash
# ~/.local/bin/trans-show
set -euo pipefail

wl-paste --no-newline | transgo > /tmp/transgo-out.txt
kitty --class transgo-float -e less /tmp/transgo-out.txt
```

配合窗口规则让 `less` 浮在鼠标附近：

```bash
# ~/.config/hypr/hyprland.conf
windowrulev2 = float, class:^(transgo-float)$
windowrulev2 = size 640 480, class:^(transgo-float)$
windowrulev2 = center, class:^(transgo-float)$
```

### 交互式选语种

用 `rofi` 做一个语种选择器，选完再译：

```bash
#!/usr/bin/env bash
# ~/.local/bin/trans-pick
set -euo pipefail

lang="$(transgo lang | tail -n +3 | rofi -dmenu -p '译成')"
[ -n "$lang" ] || exit 0
code="${lang%%[[:space:]]*}"        # 取第一列的语种代码

result="$(wl-paste --no-newline | transgo -t "$code")"
notify-send -t 6000 "译文 → ${code}" "$result"
```

### 管道里用

stdout 只有译文本身，元信息一律走 stderr，所以管道里可以随意拼：

```bash
# 译完直接进编辑器
wl-paste | transgo | nvim -

# 批量翻译一个文件的每一行
while IFS= read -r line; do
  printf '%s\n' "$(transgo "$line")"
done < words.txt

# 走 fzf 挑一个历史译文
history | fzf | transgo
```

---

## GUI 窗口规则

GUI 是独立窗口，用窗口规则控制即可。识别用标题匹配（窗口 title 是 `transgo`；
class 在部分环境下为空，按 class 匹配会落空）：

```bash
# 让翻译窗口浮在鼠标附近
windowrulev2 = float, title:^(transgo)$
windowrulev2 = pin, title:^(transgo)$
windowrulev2 = stayfocused, title:^(transgo)$
```

两个入口对应两种唤起方式：

| 快捷键 | 命令 | 行为 |
|---|---|---|
| `SUPER + G` | `transgo gui` | 唤出空白窗口 |
| `SUPER + V` | `transgo gui --clip` | 唤出并预填剪贴板内容 |

> transgo 不监听剪贴板、不监听按键。`--clip` 只是启动时读一次剪贴板内容填进输入框，
> 由你在快捷键里显式要求。没有后台常驻、没有意外弹窗。

---

## 为什么不做成守护进程 + 端口

pot 一类工具提供本地端口，是为了绕开「应用拿不到全局按键」这个限制：
让外部（脚本或桌面环境）通过 HTTP 把触发请求发进来。

transgo 不需要这一层：

| 方案 | 问题 |
|---|---|
| 守护进程 + HTTP 端口 | 多一个常驻进程、多一个可能被本机其他进程访问的监听端口、多了状态要管 |
| 守护进程 + D-Bus | 同样要常驻，且调试门槛更高 |
| **CLI 直接调起**（transgo） | 无状态、无常驻、无监听。冷启动就是一个进程的启动时间 |

需要跨进程状态时用的是 Unix socket：GUI 的单例消息（二次唤起聚焦已有窗口、
`--clip` 把新文本送进去接着翻）就走这条路，属于 GUI 内部的实现细节，不影响 CLI 的用法。

---

## 相关工具

这些是 transgo 在 Wayland 下常用的搭档：

| 工具 | 用途 | 备注 |
|---|---|---|
| `wl-clipboard` | `wl-paste` / `wl-copy` | 读写剪贴板 |
| `wtype` | 向焦点窗口模拟按键 | 划词翻译需要，用来模拟 Ctrl+C |
| `grim` + `slurp` | 截屏 + 区域选择 | 截图 OCR 翻译 |
| `tesseract` | OCR 识别 | 截图 OCR 翻译 |
| `libnotify` | `notify-send` | 译文通知 |

```console
$ sudo pacman -S wl-clipboard wtype grim slurp tesseract
```
