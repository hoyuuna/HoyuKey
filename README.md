# HoyuKey

Bộ gõ tiếng Việt cho Windows được viết bằng Rust.

---

## Tính năng

- **Kiểu gõ**: Hỗ trợ 2 kiểu gõ thông dụng là **Telex** và **VNI**.
- **Bỏ dấu**:
  - Hỗ trợ cả 2 cách đặt dấu: chuẩn mới (`hòa`, `thủy`) và chuẩn cũ (`hoà`, `thuỷ`).
  - Cho phép đặt dấu tự do ở cuối từ (ví dụ: `hoangf` -> `hoàng`).
  - Gõ lặp phím dấu để quay về ký tự gốc (ví dụ: gõ `as` thành `á`, nhấn tiếp `s` thành `as`).
- **Khôi phục từ tiếng Anh**: Tự động nhận biết các từ tiếng Anh hoặc từ viết tắt để trả về đúng chữ gốc mà không bị nuốt ký tự hay dính dấu.
- **Phím tắt chuyển chế độ**: Chuyển nhanh giữa tiếng Việt và tiếng Anh bằng `Ctrl + Shift` hoặc `Alt + Z`.
- **Khay hệ thống**:
  - Click chuột trái vào icon khay để chuyển nhanh Việt / Anh.
  - Click chuột phải để bật menu cấu hình nhanh hoặc mở bảng điều khiển.
- **Tiện ích hệ thống**:
  - Tuỳ chọn khởi động cùng Windows.
  - Điều chỉnh mức tiêu thụ RAM chạy nền tùy theo nhu cầu (512 KB, 1 MB, 2 MB hoặc không giới hạn).
  - Tự động kiểm tra và cập nhật phiên bản mới từ GitHub Releases.

---

## Tải về và sử dụng

1. Vào mục [Releases](https://github.com/hoyuuna/HoyuKey/releases) trên repository này.
2. Tải về file `HoyuKey.exe` ở phiên bản mới nhất.
3. Chạy trực tiếp file `HoyuKey.exe` để sử dụng (không cần cài đặt).

---

## Tự biên dịch từ mã nguồn

Yêu cầu máy đã cài sẵn Rust (stable toolchain) và Visual Studio C++ Build Tools:

```powershell
# Chạy kiểm tra mã nguồn
cargo check

# Chạy bộ test tính năng bộ gõ
cargo test

# Biên dịch bản phát hành tối ưu
cargo build --release
```

File sau khi biên dịch nằm tại `target/release/hoyukey.exe`.

---

## Giấy phép

Phát hành theo giấy phép MIT.
