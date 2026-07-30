# Hướng dẫn cài cliphist

Clipboard history cho Linux Mint / Cinnamon, dùng giống `Win+V` trên Windows.
Lưu cả text và ảnh, mở panel bằng `Super+V`, giữ tối đa 20 bản ghi.

---

## 1. Cài công cụ build

```bash
sudo apt update
sudo apt install cargo build-essential pkg-config libgtk-3-dev xdotool
```

---

## 2. Build

```bash
tar xzf cliphist-rs.tar.gz
cd cliphist-rs
cargo build --release
```

Lần đầu mất khoảng **2–4 phút** (compile SQLite + toàn bộ gtk-rs, rồi LTO).
Xong sẽ thấy:

```
    Finished release [optimized] target(s) in 2m21s
```


---

## 3. Cài binary

```bash
install -Dm755 target/release/cliphist ~/.local/bin/cliphist
```

Kiểm tra `~/.local/bin` có trong `PATH` chưa:

```bash
cliphist --help
```

Nếu báo `command not found`, thêm vào `~/.zshrc` (Mint mặc định dùng bash thì là
`~/.bashrc`) rồi mở terminal mới:

```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
```

---

## 4. Cài autostart + hotkey

```bash
cliphist install
```

Lệnh này làm 2 việc:

- ghi `~/.config/autostart/cliphist-lite.desktop` để daemon tự chạy khi đăng nhập
- gán `Super+V` qua `gsettings org.cinnamon.desktop.keybindings`, tự tìm slot
  `customN` còn trống nên không đè shortcut sẵn có của bạn

Muốn phím khác: `cliphist install --hotkey '<Super>c'`.

Xem lại trong **Settings → Keyboard → Shortcuts → Custom Shortcuts**, sẽ thấy
mục "Clipboard History".

---

## 5. Chạy daemon lần đầu

Autostart chỉ có hiệu lực từ lần đăng nhập sau, nên lần này chạy tay:

```bash
nohup cliphist daemon >/tmp/cliphist.log 2>&1 &
```

Kiểm tra:

```bash
cliphist doctor
```

Kỳ vọng thấy `DISPLAY` có giá trị, `xdotool` có đường dẫn, `daemon = đang chạy`,
`X11 clipboard = OK`.

Rồi copy vài đoạn text và một tấm ảnh, xem có vào chưa:

```bash
cliphist list
```

Có dữ liệu thì bấm **Super+V** — panel sẽ mở cạnh con trỏ chuột.

---

## 6. Dùng panel

| Phím | Việc |
|---|---|
| gõ chữ | lọc (nhiều từ khoá, phải khớp tất cả) |
| `↑` `↓` | di chuyển |
| `PgUp` `PgDn` | nhảy 5 mục |
| `Enter` hoặc click vào card | chép và tự dán |
| `Alt+1`…`Alt+9` | chọn nhanh mục thứ N |
| `Ctrl+P` | ghim / bỏ ghim |
| `Delete` | xoá mục đang chọn |
| `Esc`, click ra ngoài, hoặc `Super+V` lần nữa | đóng |

Mục đã ghim luôn nằm trên đầu và **không bị xoá khi đầy**.

---

## 7. Cấu hình

```bash
cliphist set max-entries 20    # số bản ghi giữ lại (mặc định 20, ghim không tính)
cliphist set blur-close off    # click ra ngoài KHÔNG đóng panel
cliphist set blur-close on     # bật lại
```

Cả hai áp dụng ngay, không cần restart daemon. `max-entries` khi hạ xuống sẽ xoá
bớt bản ghi cũ luôn tại thời điểm chạy lệnh.

File cấu hình: `~/.config/cliphist-lite/settings.conf`.

### Loại trừ nội dung không muốn lưu

`~/.config/cliphist-lite/ignore.txt`, mỗi dòng một regex, nội dung khớp sẽ không
được lưu. File reload nóng, không cần restart. Mẫu có sẵn, chỉ cần bỏ `#`:

```
^ghp_[A-Za-z0-9]{36}$
^sk-[A-Za-z0-9]{20,}$
-----BEGIN [A-Z ]*PRIVATE KEY-----
```

Crate `regex` của Rust **không hỗ trợ** lookahead / lookbehind / backreference —
pattern kiểu `(?=...)` sẽ bị bỏ qua kèm cảnh báo trong log.

### Tạm dừng

```bash
cliphist pause     # trước khi share màn hình
cliphist resume
```

---

## 8. Các lệnh khác

```bash
cliphist list -n 20      # in lịch sử
cliphist get 12          # in nội dung mục 12 (ảnh: in đường dẫn file)
cliphist copy 12         # chép mục 12 vào clipboard
cliphist wipe            # xoá hết, giữ mục đã ghim
cliphist wipe --all      # xoá sạch
cliphist pick --stay     # mở panel, click ra ngoài không đóng
cliphist doctor          # kiểm tra môi trường
cliphist uninstall       # gỡ autostart
```

---

## 9. Chuyển từ bản Python

DB tương thích, schema tự migrate, lịch sử cũ giữ nguyên. Chỉ cần tắt bản cũ:

```bash
pkill -f 'cliphist.py daemon'
rm -f ~/.config/autostart/cliphist-lite.desktop
cliphist install
nohup cliphist daemon >/tmp/cliphist.log 2>&1 &
```

Hotkey cũ trỏ tới script Python sẽ được `cliphist install` ghi đè vì nó nhận ra
slot có chữ `cliphist`.

Lưu ý: bản Python giữ 500 text + 60 ảnh, bản này giữ 20. Lần copy đầu tiên sau
khi đổi sẽ trim xuống 20 — muốn giữ lại mục nào thì ghim (`Ctrl+P`) trước.

---

## 10. Gặp lỗi

| Hiện tượng | Cách xử lý |
|---|---|
| `The system library 'gtk+-3.0' was not found` | thiếu `libgtk-3-dev pkg-config` |
| `linker 'cc' not found` | thiếu `build-essential` |
| `feature 'edition2024' is required` | `Cargo.lock` bị mất hoặc đã `cargo update` — giải nén lại tarball |
| `command not found: cargo` | `sudo apt install cargo` |
| Super+V không mở gì | `cliphist doctor`; xem hotkey trong Settings → Keyboard → Shortcuts; thử chạy tay `cliphist pick` |
| Panel mở nhưng lịch sử trống | daemon chưa chạy: `cliphist doctor`, xem `/tmp/cliphist.log` |
| Chọn xong không tự dán | thiếu `xdotool`, hoặc app đích không nhận `Ctrl+V` (terminal thường là `Ctrl+Shift+V`) |
| Panel đóng sai lúc | `CLIPHIST_DEBUG=1 cliphist pick` để xem toạ độ click; hoặc `cliphist set blur-close off` |
| `daemon đã chạy rồi (pid N)` | đúng như vậy; muốn khởi động lại thì `kill N` trước |

Log daemon: `/tmp/cliphist.log` (nếu chạy bằng lệnh `nohup` ở trên).

---

## 11. Dữ liệu lưu ở đâu

```
~/.local/share/cliphist-lite/history.db     SQLite, chmod 600
~/.local/share/cliphist-lite/images/        ảnh, tên = sha256, chmod 600
~/.config/cliphist-lite/settings.conf       cấu hình
~/.config/cliphist-lite/ignore.txt          regex loại trừ
```

`history.db` là **plaintext**. Nó chmod 600 trong `$HOME`, nhưng mọi tiến trình
chạy dưới user của bạn đều đọc được — coi nó như một file secret. Xoá sạch:

```bash
cliphist uninstall
pkill -f 'cliphist daemon'
rm -rf ~/.local/share/cliphist-lite ~/.config/cliphist-lite ~/.local/bin/cliphist
```

Hotkey phải gỡ tay trong Settings → Keyboard → Shortcuts.
