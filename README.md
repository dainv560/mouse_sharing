# 🖱️ Mouse Share - Chia sẻ Chuột, Bàn phím & Clipboard (Đa Màn Hình: Windows & Android)

Phần mềm siêu nhẹ (~1.4 MB, tiêu tốn < 15 MB RAM, 0% CPU), viết bằng **Rust** cho **Windows 11 & Windows 10**, hỗ trợ **Một Máy Chủ (Server) kết nối đồng thời NHIỀU Máy Khách (Multi-Client)** bao gồm cả máy tính Windows và thiết bị **Android** trong mạng LAN.

---

## ✨ Tính năng nổi bật

1. **Hỗ trợ Đa Thiết Bị đồng thời (1 Server kết nối nhiều Client)**:
   - Một Máy Chủ có thể kết nối đồng thời với tối đa 4 thiết bị ở 4 hướng:
     ```
                         [ 📱 Điện thoại Android (Trên) ]
     [ 📱 Tablet Android (Trái) ]  [ 💻 PC MÁY CHỦ (Giữa) ]  [ 💻 Laptop Windows (Phải) ]
                         [ 💻 Máy tính phụ (Dưới) ]
     ```
   - Rê chuột tự do xuyên qua tất cả các màn hình theo vị trí đã bố trí.
   - Phím tắt **`Scroll Lock`** hoặc **`Pause`**: Chuyển đổi xoay vòng qua các máy đang kết nối.

2. **Hỗ trợ Thiết Bị Android**:
   - Điều khiển điện thoại hoặc tablet Android bằng chuột và bàn phím máy tính.
   - Click chuột trái = Chạm cảm ứng (Tap); Click chuột phải = Nút Quay lại (Back); Phím Windows = Về màn hình chính (Home); Lăn chuột = Cuộn trang (Swipe).
   - Xem chi tiết tại thư mục [android/README_ANDROID.md](file:///d:/Workspaces/RUST/mouse_share/android/README_ANDROID.md).

3. **Đồng bộ Clipboard Đa Chiều (Văn bản & File)**:
   - **Văn bản**: Nhấn `Ctrl + C` copy đoạn văn bản, đường link, mã code trên máy A $\rightarrow$ Nhấn `Ctrl + V` paste ngay trên máy B!
   - **Tập tin (File)**: Nhấn `Ctrl + C` copy một file bất kỳ (ảnh, PDF, word, zip...) trong Windows Explorer trên máy A $\rightarrow$ Sang máy B mở thư mục bất kỳ nhấn `Ctrl + V` để paste file ngay lập tức!
   - File nhận được tự động lưu an toàn vào thư mục:
     ```
     C:\Users\<Tên_Bạn>\Downloads\MouseShare\
     ```
   - Cơ chế phát hiện trùng lặp thông minh (Hash-based Deduplication), loại bỏ hoàn toàn hiện tượng lặp vòng lặp (echo loop).

2. **Giao diện đồ họa (GUI) trực quan & hiện đại**:
   - Chọn chế độ **Máy Chủ (Host)** hoặc **Máy Khách (Client)** chỉ với 1 click.
   - Tự động hiển thị địa chỉ IP LAN của máy.
   - Nút bấm **[ ▶ BẮT ĐẦU CHẠY ]** / **[ ⏹ DỪNG LẠI ]** kèm thông báo trạng thái kết nối thời gian thực.

3. **Cài đặt vị trí Màn hình 4 hướng (Trên / Dưới / Trái / Phải)**:
   - Sơ đồ tương tác trực quan:
     ```
                 [ ⬆ Ở TRÊN ]
     [ ⬅ BÊN TRÁI ] [ 💻 MÁY CHỦ ] [ ➡ BÊN PHẢI ]
                 [ ⬇ Ở DƯỚI ]
     ```
   - Click chọn vị trí của Máy Khách so với Máy Chủ:
     - **➡ BÊN PHẢI**: Rê chuột chạm mép phải để sang máy khách (mặc định).
     - **⬅ BÊN TRÁI**: Rê chuột chạm mép trái để sang máy khách.
     - **⬆ Ở TRÊN**: Rê chuột chạm mép trên cùng để sang máy khách.
     - **⬇ Ở DƯỚI**: Rê chuột chạm mép dưới cùng để sang máy khách.
   - Đổi vị trí tức thì ngay cả khi Server đang chạy mà không cần khởi động lại.

4. **Độ trễ điều khiển cực thấp (< 1ms)**:
   - Kết nối TCP Socket với `TCP_NODELAY`.
   - Gói tin điều khiển nhị phân cố định 16 bytes/packet, tách biệt với luồng dữ liệu file/clipboard dung lượng lớn.

5. **Mô phỏng đầy đủ Chuột & Bàn phím**:
   - Chuột: Di chuyển mượt mà, Click trái/phải/giữa/phụ, Con lăn cuộn trang (Wheel).
   - Bàn phím: Tương thích đầy đủ `VirtualKey` và `ScanCode`, gõ tiếng Việt (Unikey/EVKey) và phím tắt (`Ctrl+C`, `Ctrl+V`, `Alt+Tab`, `Win key`...).

6. **Phím nóng khẩn cấp**:
   - Nhấn **`Scroll Lock`** hoặc **`Pause / Break`** để chuyển đổi qua lại giữa 2 máy tức thì.
   - Tự động thu hồi quyền kiểm soát về máy chủ nếu mất mạng hoặc máy khách tắt.

---

## 🚀 Hướng dẫn sử dụng nhanh

Cả 2 máy tính kết nối chung mạng Wi-Fi hoặc dây LAN.

### 1. Trên Máy Chủ (Máy cắm chuột & bàn phím):
1. Nhấp đúp mở file `mouse_share.exe`.
2. Chọn **MÁY CHỦ (Host)** (đã được chọn sẵn).
3. Chọn vị trí màn hình máy khách: **`⬆ Ở TRÊN`**, **`⬅ BÊN TRÁI`**, **`➡ BÊN PHẢI`**, hoặc **`⬇ Ở DƯỚI`**.
4. Bấm **[ ▶ BẮT ĐẦU CHẠY ]**.
5. Ghi nhớ địa chỉ `IP LAN của máy này`, ví dụ: `192.168.1.15`.

### 2. Trên Máy Khách (Máy nhận điều khiển):
1. Copy file `mouse_share.exe` sang máy thứ 2 và nhấp đúp mở.
2. Chọn **MÁY KHÁCH (Client)**.
3. Nhập địa chỉ IP của Máy Chủ (ví dụ `192.168.1.15`).
4. Bấm **[ ▶ BẮT ĐẦU CHẠY ]**.

### 3. Trải nghiệm Clipboard & File Sharing:
- **Sao chép văn bản**: Nhấn `Ctrl + C` bất kỳ đoạn text nào ở máy này $\rightarrow$ Nhấn `Ctrl + V` ở máy kia!
- **Sao chép file**: Chọn một file trong File Explorer trên máy A, nhấn `Ctrl + C` $\rightarrow$ Chuột sang máy B, mở thư mục cần dán rồi nhấn `Ctrl + V`! File sẽ xuất hiện ngay lập tức.
