# ⌨️ HoyuKey - Bộ gõ Tiếng Việt cho Windows viết bằng Rust

**HoyuKey** là ứng dụng gõ tiếng Việt siêu nhẹ, tốc độ cao dành cho hệ điều hành Windows, được xây dựng hoàn toàn bằng ngôn ngữ lập trình **Rust**.

Ứng dụng hoạt động tương tự như UniKey / EVKey: chạy ngầm dưới khay hệ thống (System Tray), dung lượng cực nhỏ, tiêu tốn rất ít RAM và không cần cài đặt thêm bất kỳ thư viện cồng kềnh nào.

---

## ✨ Tính năng nổi bật

- **Đầy đủ kiểu gõ phổ biến**:
  - **Telex**: `s`, `f`, `r`, `x`, `j`, `z`, `aa` (`â`), `ee` (`ê`), `oo` (`ô`), `dd` (`đ`), `w` (`ư`, `ơ`, `ă`), `[uo]w` (`ươ`).
  - **VNI**: `1-5` (dấu thanh), `0` (xóa dấu), `6` (mũ), `7` (móc), `8` (trăng), `9` (`đ`).
- **Bỏ dấu thông minh**:
  - Hỗ trợ chuẩn mới (`hóa`, `hòa`, `thủy`) và chuẩn cũ (`hoá`, `hoà`, `huỷ`).
  - Đặt dấu tự do ở cuối từ (ví dụ: `hoangf` ➔ `hoàng`, `chuyeenr` ➔ `chuyển`).
  - Khôi phục ký tự gốc khi gõ lặp phím dấu (ví dụ: gõ `as` ➔ `á`, gõ tiếp `s` ➔ `as`).
- **Tích hợp khay hệ thống (System Tray)**:
  - Biểu tượng `V` (màu đỏ) khi bật tiếng Việt và `E` (màu xanh) khi tắt.
  - Click chuột trái để chuyển nhanh chế độ Việt ⇄ Anh.
  - Click chuột phải để mở menu tùy chọn nhanh.
  - Click đúp chuột trái để mở **Bảng điều khiển HoyuKey**.
- **Phím tắt chuyển đổi toàn cục**:
  - Hỗ trợ `Ctrl + Shift` hoặc `Alt + Z`.
- **An toàn & Mượt mà**:
  - Tự động nhận biết các tổ hợp phím tắt như `Ctrl + C`, `Ctrl + V`, `Alt + Tab` để không can thiệp sai.
  - Cơ chế chống chạy đè (Single instance) giúp bảo vệ hệ thống không bị xung đột hook.

---

## 🚀 Tải về và Sử dụng

Bạn có thể tải file `.exe` đã được biên dịch tự động qua **GitHub Actions**:
1. Vào tab **Actions** trên GitHub repository này.
2. Chọn build mới nhất và tải file zip chứa `HoyuKey.exe` trong mục **Artifacts**.
3. Chạy trực tiếp `HoyuKey.exe` (không cần cài đặt).

---

## 🛠️ Biên dịch từ mã nguồn

Nếu máy tính của bạn đã có môi trường Rust và Visual Studio C++ Build Tools:

```powershell
# Kiểm tra code
cargo check

# Chạy unit tests cho Engine tiếng Việt
cargo test

# Build bản Release tối ưu hóa kích thước
cargo build --release
```

File thực thi sau khi build nằm tại `target/release/hoyukey.exe`.

---

## 📄 Bản quyền

Phát triển bởi **Hoyuuna**. Được phát hành theo giấy phép mã nguồn mở MIT.
