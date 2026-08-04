# cliphist — Quick guide / Hướng dẫn nhanh

Clipboard history for Linux, used like `Win+V`. Records text and images, opens
with `Super+V`, keeps the last 20 entries. Works on **Cinnamon/X11** and
**GNOME/Wayland**.

---

## 🇬🇧 Quick guide (English)

**Install & run — the easy way (recommended)**
```bash
sudo apt install cargo build-essential pkg-config libgtk-3-dev fonts-inter fonts-jetbrains-mono
./install.sh        # detects X11/Wayland, installs helpers, binds Super+V, starts the daemon
```
On Wayland, `install.sh` adds you to the `input` group for auto-paste — **log out
and back in once** after the first run. Then `./update.sh` handles every later
rebuild.

**Manual install**
```bash
# X11 (Cinnamon):   sudo apt install xdotool xclip
# Wayland (GNOME):  sudo apt install ydotool wl-clipboard
cargo build --release
ln -sfn "$PWD/target/release/cliphist" ~/.local/bin/cliphist   # symlink → no re-copy on updates
cliphist install                       # autostart + Super+V hotkey (Cinnamon only)
nohup cliphist daemon >~/.local/share/cliphist-lite/daemon.log 2>&1 &
cliphist doctor                        # check everything is OK
```

**First build — add `~/.local/bin` to PATH**
If `cliphist` reports `command not found` after install, `~/.local/bin` isn't on
your PATH. Add it once (Ubuntu/Mint use bash by default), then reopen the terminal:
```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```
Or just run it by full path this session: `~/.local/bin/cliphist doctor`.

**GNOME instead of Cinnamon?** `cliphist install` binds Super+V via Cinnamon only
— `./install.sh` handles GNOME, or add it by hand: **Settings → Keyboard → View
and Customize Shortcuts → Custom Shortcuts → +**, Command
`~/.local/bin/cliphist pick --paste`, shortcut `Super+V`.

**Wayland auto-paste needs ydotool** — `install.sh` sets it up; manually it's
`sudo apt install ydotool`, join the `input` group + a `/dev/uinput` udev rule,
re-login, and run `ydotoold` (see INSTALL.md §4b).

**Everyday use**
- `Super+V` → open the panel next to the cursor · `Enter` copy · `Alt+1..9` quick pick · `Ctrl+P` pin · `Del` delete · `Esc` close
- `cliphist list` show history · `cliphist pause` / `cliphist resume` stop/resume recording · `cliphist wipe [--all]` clear
- Keep secrets out: add regex lines to `~/.config/cliphist-lite/ignore.txt` (hot-reloaded)

**Files it uses**: data `~/.local/share/cliphist-lite/`, config `~/.config/cliphist-lite/`, autostart `~/.config/autostart/cliphist-lite.desktop`.

---

## 🇻🇳 Hướng dẫn nhanh (Tiếng Việt)

**Cài & chạy — cách nhanh (khuyên dùng)**
```bash
sudo apt install cargo build-essential pkg-config libgtk-3-dev fonts-inter fonts-jetbrains-mono
./install.sh        # tự nhận X11/Wayland, cài helper, gán Super+V, chạy daemon
```
Trên Wayland, `install.sh` thêm bạn vào group `input` để auto-paste chạy được —
**đăng xuất/đăng nhập lại 1 lần** sau lần chạy đầu. Sau đó mọi lần build lại chỉ
cần `./update.sh`.

**Cài thủ công**
```bash
# X11 (Cinnamon):   sudo apt install xdotool xclip
# Wayland (GNOME):  sudo apt install ydotool wl-clipboard
cargo build --release
ln -sfn "$PWD/target/release/cliphist" ~/.local/bin/cliphist   # symlink → update khỏi copy lại
cliphist install                       # tự khởi động + phím tắt Super+V (chỉ Cinnamon)
nohup cliphist daemon >~/.local/share/cliphist-lite/daemon.log 2>&1 &
cliphist doctor                        # kiểm tra mọi thứ OK
```

**Build lần đầu — thêm `~/.local/bin` vào PATH**
Nếu gõ `cliphist` bị báo `command not found`, tức `~/.local/bin` chưa có trong
PATH. Thêm một lần (Ubuntu/Mint mặc định dùng bash), rồi mở lại terminal:
```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```
Hoặc chạy bằng đường dẫn đầy đủ trong phiên này: `~/.local/bin/cliphist doctor`.

**Đang dùng GNOME chứ không phải Cinnamon?** `cliphist install` chỉ gán Super+V
cho Cinnamon — `./install.sh` lo được GNOME, hoặc gán tay: **Settings → Keyboard
→ View and Customize Shortcuts → Custom Shortcuts → +**, Command
`~/.local/bin/cliphist pick --paste`, phím tắt `Super+V`.

**Auto-paste trên Wayland cần ydotool** — `install.sh` tự lo; làm tay thì
`sudo apt install ydotool`, thêm group `input` + udev rule cho `/dev/uinput`,
đăng nhập lại, rồi chạy `ydotoold` (xem INSTALL.md §4b).

**Dùng hằng ngày**
- `Super+V` → mở bảng cạnh con trỏ · `Enter` dán · `Alt+1..9` chọn nhanh · `Ctrl+P` ghim · `Del` xóa · `Esc` đóng
- `cliphist list` xem lịch sử · `cliphist pause` / `cliphist resume` tạm dừng/ghi tiếp · `cliphist wipe [--all]` xóa sạch
- Chặn nội dung nhạy cảm: thêm dòng regex vào `~/.config/cliphist-lite/ignore.txt` (tự nạp lại, không cần restart)

**Thư mục tool dùng**: dữ liệu `~/.local/share/cliphist-lite/`, cấu hình `~/.config/cliphist-lite/`, autostart `~/.config/autostart/cliphist-lite.desktop`.

---

## 🔄 Update after code changes / Cập nhật khi sửa code

Rust is compiled, so any `.rs` change needs a rebuild. The first build is slow
(2–4 min); later ones only recompile what changed (a few seconds). Never run
`cargo update` — `Cargo.lock` is pinned for rustc 1.75.
/ Rust là code biên dịch nên sửa `.rs` phải build lại. Lần đầu lâu (2–4 phút),
các lần sau chỉ vài giây. Đừng chạy `cargo update` (phá `Cargo.lock` đã ghim).

**The easy way / Cách nhanh** — with the binary symlinked (install.sh does this),
one command rebuilds and restarts the daemon: / với binary đã symlink (install.sh
làm sẵn), một lệnh build lại + restart daemon:
```bash
./update.sh            # build --release + restart daemon
./update.sh --debug    # build nhanh hơn nhiều khi đang sửa tới lui
```

- **UI change only (`ui.rs`)** — no restart needed; just press `Super+V` again.
  / **Chỉ sửa UI** — không cần restart, bấm lại `Super+V` là thấy. (`update.sh`
  restart luôn cho chắc.)
- **Changed `daemon.rs` / `db.rs`** — the daemon must restart; `update.sh` does
  it. / **Sửa phần ghi history** — phải restart daemon, `update.sh` tự làm.

**By hand / Làm tay** (symlink already in place / đã symlink sẵn):
```bash
cargo build --release && pkill -f 'cliphist .*daemon'; nohup ~/.local/bin/cliphist daemon >~/.local/share/cliphist-lite/daemon.log 2>&1 &
```
> No `install`/`cp` step: the symlink at `~/.local/bin/cliphist` already points at
> the fresh binary. / Không cần `install`/`cp`: symlink `~/.local/bin/cliphist`
> đã trỏ sẵn tới binary mới.

---

## 🗑️ Full uninstall / Gỡ toàn bộ

```bash
# 1. Dừng daemon / stop the daemon
pkill -f 'cliphist .*daemon'

# 2. Xóa autostart + binary / remove autostart entry + binary
rm -f ~/.config/autostart/cliphist-lite.desktop
rm -f ~/.local/bin/cliphist

# 3. Xóa dữ liệu + cấu hình (lịch sử, ảnh, settings) / remove data + config
rm -rf ~/.local/share/cliphist-lite ~/.config/cliphist-lite

# 4. Xóa socket còn sót / remove leftover socket
rm -f "${XDG_RUNTIME_DIR:-/tmp}"/cliphist-lite*.sock /tmp/cliphist-lite-*.sock
```

**Super+V hotkey** — remove it via GUI / gỡ bằng GUI cho chắc:
**Settings → Keyboard → Shortcuts → Custom Shortcuts** → select the cliphist entry
(**"Clipboard History"** on Cinnamon, **"cliphist"** on GNOME) → delete.

> Keep your history? Skip step 3. / Muốn **giữ lại lịch sử** thì bỏ qua bước 3.
>
> The `apt` packages (gtk, xdotool/ydotool, wl-clipboard, fonts…) are shared
> system deps — only remove them if nothing else needs them. / Các gói `apt` là
> dependency hệ thống, chỉ gỡ nếu chắc chắn không dùng cho việc khác.
