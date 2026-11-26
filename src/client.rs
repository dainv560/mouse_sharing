use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

use crate::clipboard::{set_clipboard_file, set_clipboard_text, start_clipboard_monitor};
use crate::input::{send_keyboard, send_mouse_button, send_mouse_move, send_mouse_wheel};
use crate::protocol::Packet;

static CLIENT_RUNNING: AtomicBool = AtomicBool::new(false);

pub fn stop_client() {
    CLIENT_RUNNING.store(false, Ordering::SeqCst);
}

pub fn run_client(
    server_addr: &str,
    on_connected: Option<Arc<dyn Fn(i32, i32) + Send + Sync>>,
    on_disconnected: Option<Arc<dyn Fn() + Send + Sync>>,
) -> std::io::Result<()> {
    CLIENT_RUNNING.store(true, Ordering::SeqCst);

    println!("============================================================");
    println!("  MOUSE SHARE - MÁY KHÁCH (CLIENT / RECEIVER)");
    println!("============================================================");
    println!("- Địa chỉ máy chủ mục tiêu: {}", server_addr);

    let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    println!("- Độ phân giải màn hình máy này: {}x{}", screen_w, screen_h);
    println!("============================================================");

    while CLIENT_RUNNING.load(Ordering::SeqCst) {
        println!("[Client] Đang kết nối tới máy chủ {}...", server_addr);
        match TcpStream::connect(server_addr) {
            Ok(mut stream) => {
                let _ = stream.set_nodelay(true);
                println!("[Client] Đã kết nối TCP! Đang gửi Handshake...");

                let handshake = Packet::Handshake { width: screen_w, height: screen_h };
                if let Err(e) = handshake.send(&mut stream) {
                    eprintln!("[Client] Lỗi gửi Handshake: {}. Thử lại...", e);
                    thread::sleep(Duration::from_secs(2));
                    continue;
                }

                let server_screen = match Packet::recv(&mut stream) {
                    Ok(Packet::HandshakeAck { width, height }) => {
                        println!("[Client] Nhận HandshakeAck từ máy chủ! (Màn hình máy chủ: {}x{})", width, height);
                        (width, height)
                    }
                    Ok(other) => {
                        eprintln!("[Client] Nhận gói tin bất thường: {:?}", other);
                        thread::sleep(Duration::from_secs(2));
                        continue;
                    }
                    Err(e) => {
                        eprintln!("[Client] Lỗi nhận HandshakeAck: {}. Thử lại...", e);
                        thread::sleep(Duration::from_secs(2));
                        continue;
                    }
                };

                if let Some(ref cb) = on_connected {
                    cb(server_screen.0, server_screen.1);
                }

                println!("------------------------------------------------------------");
                println!("[Client] >> SẴN SÀNG NHẬN ĐIỀU KHIỂN & CHIA SẺ CLIPBOARD <<");
                println!("------------------------------------------------------------");

                // Track physical mouse button states so we never send spurious button-up events
                let mut is_lbutton_down = false;
                let mut is_rbutton_down = false;
                let mut is_mbutton_down = false;

                // Start Clipboard Monitor on Client to send local clipboard to Server
                let session_alive = Arc::new(AtomicBool::new(true));
                let session_alive_monitor = session_alive.clone();
                let mut cb_write_stream = match stream.try_clone() {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("[Client] Không clone được stream: {}", e);
                        continue;
                    }
                };
                let cb_monitor_handle = start_clipboard_monitor(session_alive_monitor, move |pkt| {
                    pkt.send(&mut cb_write_stream).is_ok()
                });

                while CLIENT_RUNNING.load(Ordering::SeqCst) {
                    stream.set_read_timeout(Some(Duration::from_millis(500))).ok();
                    match Packet::recv(&mut stream) {
                        Ok(Packet::MouseMoveAbs { x, y }) => {
                            send_mouse_move(x, y, screen_w, screen_h);
                        }
                        Ok(Packet::MouseButton { button, is_down }) => {
                            match button {
                                1 => is_lbutton_down = is_down,
                                2 => is_rbutton_down = is_down,
                                3 => is_mbutton_down = is_down,
                                _ => {}
                            }
                            send_mouse_button(button, is_down);
                        }
                        Ok(Packet::MouseWheel { delta }) => {
                            send_mouse_wheel(delta);
                        }
                        Ok(Packet::Key { vk, scan, flags, is_down }) => {
                            send_keyboard(vk, scan, flags, is_down);
                        }
                        Ok(Packet::ReleaseControl) => {
                            // Only release buttons that were actually held down!
                            if is_lbutton_down {
                                send_mouse_button(1, false);
                                is_lbutton_down = false;
                            }
                            if is_rbutton_down {
                                send_mouse_button(2, false);
                                is_rbutton_down = false;
                            }
                            if is_mbutton_down {
                                send_mouse_button(3, false);
                                is_mbutton_down = false;
                            }
                        }
                        Ok(Packet::ClipboardText { text }) => {
                            println!("[Client] Nhận văn bản clipboard từ Máy Chủ ({} ký tự)", text.chars().count());
                            set_clipboard_text(&text);
                        }
                        Ok(Packet::ClipboardFile { filename, data }) => {
                            println!("[Client] Nhận file clipboard từ Máy Chủ: {} ({} bytes)", filename, data.len());
                            if let Some(target) = set_clipboard_file(&filename, &data) {
                                println!("[Client] Đã lưu file vào: {:?}", target);
                            }
                        }
                        Ok(Packet::Heartbeat) => {}
                        Ok(other) => {
                            eprintln!("[Client] Gói tin không xác định: {:?}", other);
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut || e.kind() == std::io::ErrorKind::WouldBlock => {
                            continue;
                        }
                        Err(e) => {
                            println!("[Client] Mất kết nối với máy chủ: {}.", e);
                            if is_lbutton_down {
                                send_mouse_button(1, false);
                            }
                            if is_rbutton_down {
                                send_mouse_button(2, false);
                            }
                            if is_mbutton_down {
                                send_mouse_button(3, false);
                            }
                            if let Some(ref cb) = on_disconnected {
                                cb();
                            }
                            break;
                        }
                    }
                }

                session_alive.store(false, Ordering::SeqCst);
                let _ = cb_monitor_handle.join();
            }
            Err(e) => {
                println!("[Client] Không thể kết nối tới máy chủ ({}). Đang thử lại...", e);
            }
        }
        if CLIENT_RUNNING.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_secs(2));
        }
    }
    Ok(())
}
