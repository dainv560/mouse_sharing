"""
Mouse Share - Android Quick Bridge (Zero-Install Bridge via ADB)
---------------------------------------------------------------
Chạy script này trên PC để kết nối điện thoại Android vào Mouse Share Server
ngay lập tức mà không cần phải build file APK.

Yêu cầu:
1. Điện thoại Android đã bật "USB Debugging" hoặc "Wireless Debugging" (Gỡ lỗi qua Wi-Fi).
2. Máy tính có cài đặt 'adb' (Android SDK Platform-Tools).
3. Python 3.x.
"""

import socket
import struct
import subprocess
import sys
import threading
import time

SERVER_HOST = "127.0.0.1"
SERVER_PORT = 8421
PACKET_SIZE = 16


def get_android_screen_size():
    """Lấy độ phân giải màn hình của thiết bị Android qua ADB."""
    try:
        output = subprocess.check_output(
            ["adb", "shell", "wm", "size"], text=True, stderr=subprocess.PIPE
        )
        for line in output.strip().split("\n"):
            if "Physical size:" in line:
                size_str = line.split("Physical size:")[1].strip()
                w, h = map(int, size_str.split("x"))
                return w, h
    except Exception as e:
        print(f"[!] Không thể tự động lấy kích thước màn hình qua ADB: {e}")
    # Kích thước mặc định nếu không lấy được
    return 1080, 2400


class AndroidBridge:

    def __init__(self, server_host, server_port):
        self.server_host = server_host
        self.server_port = server_port
        self.running = False
        self.width = 1080
        self.height = 2400
        self.adb_process = None
        self.last_x = 0
        self.last_y = 0
        self.is_down = False

    def start_adb_shell(self):
        """Khởi động tiến trình adb shell chạy ngầm để gửi lệnh siêu tốc."""
        self.adb_process = subprocess.Popen(
            ["adb", "shell"],
            stdin=subprocess.PIPE,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            text=True,
            bufsize=1,
        )

    def send_adb_cmd(self, cmd: str):
        if self.adb_process and self.adb_process.stdin:
            try:
                self.adb_process.stdin.write(cmd + "\n")
                self.adb_process.stdin.flush()
            except Exception:
                pass

    def run(self):
        print("============================================================")
        print("  MOUSE SHARE - ANDROID QUICK BRIDGE")
        print("============================================================")

        # Kiểm tra thiết bị ADB
        try:
            devices = subprocess.check_output(
                ["adb", "devices"], text=True
            ).strip()
            lines = [l for l in devices.split("\n")[1:] if l.strip()]
            if not lines:
                print(
                    "[!] Không tìm thấy thiết bị Android nào qua ADB!"
                )
                print(
                    "    Vui lòng bật 'USB Debugging' trên điện thoại và cắm cáp hoặc kết nối Wi-Fi (adb connect <IP>:5555)."
                )
                return
            print(f"[*] Đã nhận diện thiết bị Android: {lines[0]}")
        except FileNotFoundError:
            print("[!] Lỗi: Không tìm thấy lệnh 'adb' trong PATH.")
            print(
                "    Vui lòng cài đặt Android Platform-Tools hoặc dùng Android Client APK."
            )
            return

        self.width, self.height = get_android_screen_size()
        print(f"[*] Độ phân giải màn hình Android: {self.width}x{self.height}")

        self.start_adb_shell()

        while True:
            try:
                print(
                    f"[*] Đang kết nối tới Mouse Share Server {self.server_host}:{self.server_port}..."
                )
                sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
                sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
                sock.connect((self.server_host, self.server_port))
                print("[*] Đã kết nối TCP thành công!")

                # Gửi Handshake (type = 1, width: i32, height: i32)
                handshake_pkt = bytearray(PACKET_SIZE)
                handshake_pkt[0] = 1
                struct.pack_into("<i", handshake_pkt, 1, self.width)
                struct.pack_into("<i", handshake_pkt, 5, self.height)
                sock.sendall(handshake_pkt)

                # Nhận HandshakeAck (type = 2)
                ack = sock.recv(PACKET_SIZE)
                if not ack or ack[0] != 2:
                    print("[!] Handshake thất bại. Thử lại sau 2 giây...")
                    sock.close()
                    time.sleep(2)
                    continue

                server_w, server_h = struct.unpack_from("<ii", ack, 1)
                print(
                    f"[+] BẮT TAY THÀNH CÔNG! (Màn hình máy chủ: {server_w}x{server_h})"
                )
                print(
                    "[+] Điện thoại Android đã sẵn sàng nhận điều khiển từ chuột và bàn phím máy tính!"
                )

                self.running = True

                while self.running:
                    pkt_id_bytes = sock.recv(1)
                    if not pkt_id_bytes:
                        break
                    pkt_id = pkt_id_bytes[0]

                    if pkt_id == 3:  # MouseMoveAbs
                        rest = sock.recv(PACKET_SIZE - 1)
                        if len(rest) < PACKET_SIZE - 1:
                            break
                        x, y = struct.unpack_from("<ii", rest, 0)
                        self.last_x, self.last_y = x, y

                    elif pkt_id == 4:  # MouseButton
                        rest = sock.recv(PACKET_SIZE - 1)
                        if len(rest) < PACKET_SIZE - 1:
                            break
                        btn = rest[0]
                        is_down = rest[1] != 0

                        if btn == 1:  # Left click = Touch tap / drag
                            if is_down:
                                self.is_down = True
                            else:
                                if self.is_down:
                                    self.send_adb_cmd(
                                        f"input tap {self.last_x} {self.last_y}"
                                    )
                                    self.is_down = False
                        elif btn == 2:  # Right click = Back button trên Android
                            if not is_down:
                                self.send_adb_cmd("input keyevent 4")  # KEYCODE_BACK

                    elif pkt_id == 5:  # MouseWheel
                        rest = sock.recv(PACKET_SIZE - 1)
                        if len(rest) < PACKET_SIZE - 1:
                            break
                        (delta,) = struct.unpack_from("<h", rest, 0)
                        # Cuộn trang bằng swipe
                        swipe_dist = 300 if delta > 0 else -300
                        y2 = max(
                            100, min(self.height - 100, self.last_y + swipe_dist)
                        )
                        self.send_adb_cmd(
                            f"input swipe {self.last_x} {self.last_y} {self.last_x} {y2} 150"
                        )

                    elif pkt_id == 6:  # Key
                        rest = sock.recv(PACKET_SIZE - 1)
                        if len(rest) < PACKET_SIZE - 1:
                            break
                        vk, scan, flags = struct.unpack_from("<HHI", rest, 0)
                        is_down = rest[8] != 0

                        if is_down:
                            self.handle_android_key(vk)

                    elif pkt_id == 8:  # ReleaseControl
                        self.is_down = False

                    elif pkt_id == 10:  # ClipboardText
                        len_bytes = sock.recv(4)
                        (t_len,) = struct.unpack("<I", len_bytes)
                        text_data = sock.recv(t_len).decode(
                            "utf-8", errors="ignore"
                        )
                        print(
                            f"[+] Đồng bộ văn bản sang Android ({len(text_data)} ký tự)"
                        )
                        # Gửi vào clipboard Android
                        safe_text = text_data.replace('"', '\\"')
                        self.send_adb_cmd(
                            f'am broadcast -a clipper.set -e text "{safe_text}"'
                        )

                    elif pkt_id == 11:  # ClipboardFile
                        hdr = sock.recv(6)
                        n_len, f_len = struct.unpack("<HI", hdr)
                        fname = sock.recv(n_len).decode(
                            "utf-8", errors="ignore"
                        )
                        # Bỏ qua data file lớn để giữ kết nối nhẹ nhàng
                        bytes_left = f_len
                        while bytes_left > 0:
                            chunk = sock.recv(min(4096, bytes_left))
                            if not chunk:
                                break
                            bytes_left -= len(chunk)
                        print(f"[+] Đã nhận thông tin file: {fname}")

                    elif pkt_id in (1, 2, 7):
                        sock.recv(PACKET_SIZE - 1)

            except Exception as e:
                print(f"[!] Mất kết nối tới Server: {e}. Đang thử lại sau 3s...")
                time.sleep(3)

    def handle_android_key(self, vk: int):
        # Mapping các phím thông dụng sang Android KeyEvent
        vk_map = {
            0x08: 67,  # Backspace
            0x0D: 66,  # Enter
            0x1B: 111,  # Escape -> Back
            0x20: 62,  # Space
            0x25: 21,  # Left
            0x26: 19,  # Up
            0x27: 22,  # Right
            0x28: 20,  # Down
            0x2E: 112,  # Delete
            0x5B: 3,  # Windows key -> Home
        }
        if vk in vk_map:
            self.send_adb_cmd(f"input keyevent {vk_map[vk]}")
        elif 0x30 <= vk <= 0x39:  # 0 - 9
            self.send_adb_cmd(f"input keyevent {vk - 0x30 + 7}")
        elif 0x41 <= vk <= 0x5A:  # A - Z
            self.send_adb_cmd(f"input keyevent {vk - 0x41 + 29}")


if __name__ == "__main__":
    host = sys.argv[1] if len(sys.argv) > 1 else SERVER_HOST
    port = int(sys.argv[2]) if len(sys.argv) > 2 else SERVER_PORT
    bridge = AndroidBridge(host, port)
    bridge.run()
