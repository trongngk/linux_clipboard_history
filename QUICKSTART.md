# cliphist — Quick guide / Hướng dẫn nhanh

Clipboard history for Linux Mint / Cinnamon (X11), used like `Win+V`.
Records text and images, opens with `Super+V`, keeps the last 20 entries.

---

## 🇬🇧 Quick guide (English)

**Install & run**
```bash
sudo apt install cargo build-essential pkg-config libgtk-3-dev xdotool fonts-inter fonts-jetbrains-mono
cargo build --release
install -Dm755 target/release/cliphist ~/.local/bin/cliphist
cliphist install                       # autostart + Super+V hotkey (Cinnamon)
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

**GNOME instead of Cinnamon?** `cliphist install` binds Super+V via Cinnamon only.
On GNOME, add it by hand: **Settings → Keyboard → View and Customize Shortcuts →
Custom Shortcuts → +**, Command `~/.local/bin/cliphist pick --paste`, shortcut `Super+V`.

**Everyday use**
- `Super+V` → open the panel next to the cursor · `Enter` copy · `Alt+1..9` quick pick · `Ctrl+P` pin · `Del` delete · `Esc` close
- `cliphist list` show history · `cliphist pause` / `cliphist resume` stop/resume recording · `cliphist wipe [--all]` clear
- Keep secrets out: add regex lines to `~/.config/cliphist-lite/ignore.txt` (hot-reloaded)

**Files it uses**: data `~/.local/share/cliphist-lite/`, config `~/.config/cliphist-lite/`, autostart `~/.config/autostart/cliphist-lite.desktop`.

---

## 🇻🇳 Hướng dẫn nhanh (Tiếng Việt)

**Cài & chạy**
```bash
sudo apt install cargo build-essential pkg-config libgtk-3-dev xdotool fonts-inter fonts-jetbrains-mono
cargo build --release
install -Dm755 target/release/cliphist ~/.local/bin/cliphist
cliphist install                       # tự khởi động + phím tắt Super+V (Cinnamon)
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
cho Cinnamon. Trên GNOME hãy gán tay: **Settings → Keyboard → View and Customize
Shortcuts → Custom Shortcuts → +**, Command `~/.local/bin/cliphist pick --paste`,
phím tắt `Super+V`.

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

**Rebuild + reinstall the binary / Build lại + ghi đè binary**
```bash
cargo build --release
install -Dm755 target/release/cliphist ~/.local/bin/cliphist
```

- **UI change only (`ui.rs`)** — no restart; just press `Super+V` again (the
  panel runs from the `pick` command). / **Chỉ sửa UI** — không cần restart, bấm
  lại `Super+V` là thấy.
- **Changed `daemon.rs` / `db.rs`** — restart the daemon so the background
  process picks up the new binary / **Sửa phần ghi history** — restart daemon:
  ```bash
  pkill -f 'cliphist .*daemon'
  nohup ~/.local/bin/cliphist daemon >~/.local/share/cliphist-lite/daemon.log 2>&1 &
  ```

One-liner (build → install → restart daemon) / Gộp một dòng:
```bash
cargo build --release && install -Dm755 target/release/cliphist ~/.local/bin/cliphist && pkill -f 'cliphist .*daemon'; nohup ~/.local/bin/cliphist daemon >~/.local/share/cliphist-lite/daemon.log 2>&1 &
```
> If `cargo build` fails, `&&` stops before installing, so a broken build never
> overwrites the working binary. / Nếu build lỗi, `&&` dừng lại nên binary đang
> chạy không bị ghi đè bản hỏng.

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

**Super+V hotkey (Cinnamon)** — remove it via GUI / gỡ bằng GUI cho chắc:
**Settings → Keyboard → Shortcuts → Custom Shortcuts** → select **"Clipboard History"** → delete.

> Keep your history? Skip step 3. / Muốn **giữ lại lịch sử** thì bỏ qua bước 3.
>
> The `apt` packages (gtk, xdotool, fonts…) are shared system deps — only remove
> them if nothing else needs them. / Các gói `apt` là dependency hệ thống, chỉ gỡ
> nếu chắc chắn không dùng cho việc khác.
