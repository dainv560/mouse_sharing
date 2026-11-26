# 📱 Mouse Share - Hướng dẫn kết nối Thiết bị Android (Client)

Dự án hiện đã hỗ trợ **Một Máy Chủ (Server) kết nối đồng thời NHIỀU Máy Khách (Multi-Client)** bao gồm cả máy tính Windows và điện thoại/máy tính bảng **Android**.

---

## 🌟 Mô hình kết nối Đa Thiết Bị (Multi-Screen Matrix)

Bạn có thể bố trí các máy xung quanh Máy Chủ ở 4 hướng:
```
                    [ 📱 Điện thoại Android (Trên) ]
[ 📱 Tablet Android (Trái) ]  [ 💻 PC MÁY CHỦ (Giữa) ]  [ 💻 Laptop Windows (Phải) ]
                    [ 💻 Máy tính phụ (Dưới) ]
```
- Khi rê chuột lên mép trên $\rightarrow$ Điều khiển điện thoại Android.
- Rê chuột sang mép trái $\rightarrow$ Điều khiển máy tính bảng Android.
- Rê chuột sang mép phải $\rightarrow$ Điều khiển Laptop Windows.
- Phím tắt **`Scroll Lock`** hoặc **`Pause`** trên bàn phím: Tự động đổi xoay vòng qua các máy đang kết nối!
- Copy văn bản ở bất kỳ máy nào $\rightarrow$ Tất cả các máy khác đều nhận được qua Clipboard!

---

## 🚀 Cách 1: Dùng script `android_quick_bridge.py` (Chạy ngay không cần cài APK)

Đây là cách nhanh nhất (chỉ mất 10 giây) dành cho lập trình viên hoặc người dùng có sẵn cáp kết nối hoặc Wi-Fi ADB:

### Bước 1: Chuẩn bị trên điện thoại Android
1. Vào *Cài đặt* $\rightarrow$ *Thông tin điện thoại* $\rightarrow$ Chạm 7 lần vào *Số bản dựng (Build Number)* để mở menu **Tùy chọn nhà phát triển**.
2. Bật mục **Gỡ lỗi USB (USB Debugging)** hoặc **Gỡ lỗi không dây (Wireless Debugging)**.

### Bước 2: Khởi động trên Máy tính
1. Mở file `mouse_share.exe` trên PC và bấm **[ ▶ BẮT ĐẦU CHẠY ]**.
2. Mở PowerShell/CMD trong thư mục `mouse_share\android` và chạy:
   ```powershell
   python android_quick_bridge.py 127.0.0.1 8421
   ```
3. Script sẽ tự động nhận diện điện thoại qua ADB, đo độ phân giải màn hình và kết nối vào Server.
4. Bạn chỉ cần rê chuột chạm mép màn hình PC là có thể click, cuộn trang và gõ phím trên Android ngay lập tức!

---

## 📦 Cách 2: Cài ứng dụng Native App trên Android (Standalone APK)

Nếu muốn Android tự kết nối qua Wi-Fi mà không cần cắm cáp hay chạy script trên máy tính:

### Cấu trúc mã nguồn ứng dụng Android:
Toàn bộ mã nguồn dự án Android nằm trong thư mục `android/app`:
- **`MainActivity.kt`**: Giao diện nhập IP Máy Chủ PC và nút bấm Kết nối.
- **`MouseShareService.kt`**: Dịch vụ Trợ năng (`AccessibilityService`), hiển thị con trỏ chuột ảo trên màn hình Android, nhận lệnh từ PC và tự động kích hoạt thao tác chạm (Tap), vuốt (Swipe), nút Back/Home/Recents và đồng bộ Clipboard.
- **`NetworkClient.kt`**: Kết nối TCP Socket với `mouse_share.exe` qua cổng `8421`.

### Các bước biên dịch và cài đặt:
1. Mở thư mục `mouse_share/android` bằng **Android Studio**.
2. Kết nối điện thoại và bấm **Run 'app'** (hoặc `Build` $\rightarrow$ `Build APK`).
3. Mở ứng dụng **Mouse Share** trên điện thoại:
   - Bấm nút **"Cấp quyền Dịch vụ Trợ năng"** và kích hoạt cho `Mouse Share Control Service`.
   - Nhập IP LAN của Máy Chủ PC (ví dụ: `192.168.1.15`).
   - Bấm **"KẾT NỐI TỚI PC"**.
4. Con trỏ chuột sẽ xuất hiện trên màn hình Android và sẵn sàng nhận điều khiển!
