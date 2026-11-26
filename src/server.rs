use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::clipboard::{set_clipboard_file, set_clipboard_text, start_clipboard_monitor};
use crate::protocol::Packet;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScreenPosition {
    Right,
    Left,
    Top,
    Bottom,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActiveTarget {
    Local,
    Remote(ScreenPosition),
}

#[derive(Clone)]
pub struct ClientSlot {
    pub id: usize,
    pub address: String,
    pub width: i32,
    pub height: i32,
    pub _position: ScreenPosition,
    pub tx: Sender<Packet>,
}

pub struct ServerContext {
    pub active: ActiveTarget,
    pub local_w: i32,
    pub local_h: i32,
    pub center_x: i32,
    pub center_y: i32,
    pub remote_x: i32,
    pub remote_y: i32,

    pub slot_right: Option<ClientSlot>,
    pub slot_left: Option<ClientSlot>,
    pub slot_top: Option<ClientSlot>,
    pub slot_bottom: Option<ClientSlot>,
}

impl ServerContext {
    pub fn get_slot(&self, pos: ScreenPosition) -> Option<&ClientSlot> {
        match pos {
            ScreenPosition::Right => self.slot_right.as_ref(),
            ScreenPosition::Left => self.slot_left.as_ref(),
            ScreenPosition::Top => self.slot_top.as_ref(),
            ScreenPosition::Bottom => self.slot_bottom.as_ref(),
        }
    }

    pub fn set_slot(&mut self, pos: ScreenPosition, slot: Option<ClientSlot>) {
        match pos {
            ScreenPosition::Right => self.slot_right = slot,
            ScreenPosition::Left => self.slot_left = slot,
            ScreenPosition::Top => self.slot_top = slot,
            ScreenPosition::Bottom => self.slot_bottom = slot,
        }
    }

    pub fn broadcast(&self, pkt: Packet) {
        if let Some(ref s) = self.slot_right { let _ = s.tx.send(pkt.clone()); }
        if let Some(ref s) = self.slot_left { let _ = s.tx.send(pkt.clone()); }
        if let Some(ref s) = self.slot_top { let _ = s.tx.send(pkt.clone()); }
        if let Some(ref s) = self.slot_bottom { let _ = s.tx.send(pkt); }
    }

    pub fn active_slot(&self) -> Option<&ClientSlot> {
        match self.active {
            ActiveTarget::Local => None,
            ActiveTarget::Remote(pos) => self.get_slot(pos),
        }
    }

    #[allow(dead_code)]
    pub fn client_count(&self) -> usize {
        let mut count = 0;
        if self.slot_right.is_some() { count += 1; }
        if self.slot_left.is_some() { count += 1; }
        if self.slot_top.is_some() { count += 1; }
        if self.slot_bottom.is_some() { count += 1; }
        count
    }
}

static SERVER_CTX: Mutex<Option<ServerContext>> = Mutex::new(None);
static SERVER_RUNNING: AtomicBool = AtomicBool::new(false);
static HOOK_THREAD_ID: AtomicU32 = AtomicU32::new(0);
static NEXT_CLIENT_ID: AtomicU32 = AtomicU32::new(1);

pub fn get_connected_clients_info() -> Vec<(ScreenPosition, String, i32, i32)> {
    let mut list = Vec::new();
    if let Ok(lock) = SERVER_CTX.lock() {
        if let Some(ref ctx) = *lock {
            if let Some(ref s) = ctx.slot_right { list.push((ScreenPosition::Right, s.address.clone(), s.width, s.height)); }
            if let Some(ref s) = ctx.slot_left { list.push((ScreenPosition::Left, s.address.clone(), s.width, s.height)); }
            if let Some(ref s) = ctx.slot_top { list.push((ScreenPosition::Top, s.address.clone(), s.width, s.height)); }
            if let Some(ref s) = ctx.slot_bottom { list.push((ScreenPosition::Bottom, s.address.clone(), s.width, s.height)); }
        }
    }
    list
}

pub fn stop_server() {
    SERVER_RUNNING.store(false, Ordering::SeqCst);
    let tid = HOOK_THREAD_ID.swap(0, Ordering::SeqCst);
    if tid != 0 {
        unsafe { PostThreadMessageW(tid, WM_QUIT, 0, 0) };
    }
    if let Ok(mut lock) = SERVER_CTX.lock() {
        if let Some(ctx) = lock.as_mut() {
            ctx.active = ActiveTarget::Local;
            ctx.slot_right = None;
            ctx.slot_left = None;
            ctx.slot_top = None;
            ctx.slot_bottom = None;
        }
    }
}

pub fn run_server(
    port: u16,
    preferred_position: ScreenPosition,
    on_client_change: Option<Arc<dyn Fn() + Send + Sync>>,
) -> std::io::Result<()> {
    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr)?;
    listener.set_nonblocking(true)?;

    SERVER_RUNNING.store(true, Ordering::SeqCst);

    println!("============================================================");
    println!("  MOUSE SHARE - MULTI-CLIENT SERVER (Windows & Android)");
    println!("============================================================");
    println!("- Status: Đang chờ các máy khách (Client) kết nối...");
    println!("- Cổng lắng nghe: {}", port);
    println!("- Hỗ trợ kết nối đồng thời nhiều máy ở 4 hướng: Trái, Phải, Trên, Dưới.");
    println!("- Phím tắt chuyển đổi: Nhấn [Scroll Lock] hoặc [Pause] để đổi máy.");
    println!("============================================================");

    let local_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let local_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };

    {
        let mut lock = SERVER_CTX.lock().unwrap();
        *lock = Some(ServerContext {
            active: ActiveTarget::Local,
            local_w,
            local_h,
            center_x: local_w / 2,
            center_y: local_h / 2,
            remote_x: 0,
            remote_y: 0,
            slot_right: None,
            slot_left: None,
            slot_top: None,
            slot_bottom: None,
        });
    }

    // Start local clipboard monitor that broadcasts to all connected clients
    let server_alive = Arc::new(AtomicBool::new(true));
    let server_alive_monitor = server_alive.clone();
    let cb_monitor_handle = start_clipboard_monitor(server_alive_monitor, move |pkt| {
        if let Ok(lock) = SERVER_CTX.lock() {
            if let Some(ref ctx) = *lock {
                ctx.broadcast(pkt);
            }
        }
        true
    });

    // Spawn accept loop thread to handle multiple clients in parallel
    let accept_alive = server_alive.clone();
    let change_cb = on_client_change.clone();
    let accept_handle = thread::spawn(move || {
        while accept_alive.load(Ordering::SeqCst) && SERVER_RUNNING.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut stream, peer)) => {
                    println!("[Server] Máy khách mới kết nối từ: {}", peer);
                    let _ = stream.set_nodelay(true);
                    let _ = stream.set_nonblocking(false);

                    let handshake = match Packet::recv(&mut stream) {
                        Ok(Packet::Handshake { width, height }) => (width, height),
                        Ok(other) => {
                            eprintln!("[Server] Gói tin bắt tay không hợp lệ từ {}: {:?}", peer, other);
                            continue;
                        }
                        Err(e) => {
                            eprintln!("[Server] Lỗi Handshake từ {}: {}", peer, e);
                            continue;
                        }
                    };

                    let (remote_w, remote_h) = handshake;
                    println!("[Server] Máy khách [{}] màn hình: {}x{}", peer, remote_w, remote_h);

                    if let Err(e) = (Packet::HandshakeAck { width: local_w, height: local_h }).send(&mut stream) {
                        eprintln!("[Server] Gửi HandshakeAck thất bại: {}", e);
                        continue;
                    }

                    let client_id = NEXT_CLIENT_ID.fetch_add(1, Ordering::SeqCst) as usize;
                    let (tx, rx) = channel::<Packet>();

                    // Determine which slot to assign
                    let assigned_pos = {
                        let mut lock = SERVER_CTX.lock().unwrap();
                        let ctx = lock.as_mut().unwrap();

                        // Try preferred position first, then others
                        let mut candidate_positions = vec![preferred_position];
                        for &pos in &[
                            ScreenPosition::Right,
                            ScreenPosition::Left,
                            ScreenPosition::Top,
                            ScreenPosition::Bottom,
                        ] {
                            if !candidate_positions.contains(&pos) {
                                candidate_positions.push(pos);
                            }
                        }

                        let mut chosen = None;
                        for pos in candidate_positions {
                            if ctx.get_slot(pos).is_none() {
                                chosen = Some(pos);
                                break;
                            }
                        }

                        if let Some(pos) = chosen {
                            let slot = ClientSlot {
                                id: client_id,
                                address: peer.to_string(),
                                width: remote_w,
                                height: remote_h,
                                _position: pos,
                                tx: tx.clone(),
                            };
                            ctx.set_slot(pos, Some(slot));
                            println!("[Server] Đã gán máy [{}] vào vị trí: {:?}", peer, pos);
                            pos
                        } else {
                            println!("[Server] Đã đủ 4 máy khách! Từ chối kết nối từ: {}", peer);
                            continue;
                        }
                    };

                    if let Some(ref cb) = change_cb {
                        cb();
                    }

                    // Spawn writer thread for this client
                    let mut write_stream = match stream.try_clone() {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("[Server] Không clone được stream: {}", e);
                            continue;
                        }
                    };
                    thread::spawn(move || {
                        while let Ok(pkt) = rx.recv() {
                            if pkt.send(&mut write_stream).is_err() {
                                break;
                            }
                        }
                    });

                    // Spawn reader thread for this client
                    let mut read_stream = match stream.try_clone() {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("[Server] Không clone được stream: {}", e);
                            continue;
                        }
                    };
                    let on_disc = change_cb.clone();
                    thread::spawn(move || {
                        while let Ok(pkt) = Packet::recv(&mut read_stream) {
                            match pkt {
                                Packet::ClipboardText { text } => {
                                    println!("[Server] Nhận văn bản clipboard từ máy [{}]: {} ký tự", peer, text.chars().count());
                                    set_clipboard_text(&text);
                                    // Forward to other clients
                                    if let Ok(lock) = SERVER_CTX.lock() {
                                        if let Some(ref ctx) = *lock {
                                            ctx.broadcast(Packet::ClipboardText { text });
                                        }
                                    }
                                }
                                Packet::ClipboardFile { filename, data } => {
                                    println!("[Server] Nhận file clipboard từ máy [{}]: {} ({} bytes)", peer, filename, data.len());
                                    if let Some(saved) = set_clipboard_file(&filename, &data) {
                                        println!("[Server] Đã lưu file vào: {:?}", saved);
                                    }
                                    if let Ok(lock) = SERVER_CTX.lock() {
                                        if let Some(ref ctx) = *lock {
                                            ctx.broadcast(Packet::ClipboardFile { filename, data });
                                        }
                                    }
                                }
                                Packet::Heartbeat => {}
                                _ => {}
                            }
                        }

                        println!("[Server] Máy khách [{}] ({:?}) đã ngắt kết nối.", peer, assigned_pos);
                        let mut reset_cursor_pos = None;
                        if let Ok(mut lock) = SERVER_CTX.lock() {
                            if let Some(ctx) = lock.as_mut() {
                                if let Some(slot) = ctx.get_slot(assigned_pos) {
                                    if slot.id == client_id {
                                        ctx.set_slot(assigned_pos, None);
                                        if ctx.active == ActiveTarget::Remote(assigned_pos) {
                                            ctx.active = ActiveTarget::Local;
                                            reset_cursor_pos = Some((ctx.center_x, ctx.center_y));
                                        }
                                    }
                                }
                            }
                        }
                        if let Some((cx, cy)) = reset_cursor_pos {
                            unsafe { SetCursorPos(cx, cy) };
                        }
                        if let Some(ref cb) = on_disc {
                            cb();
                        }
                    });
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(50));
                }
                Err(e) => {
                    eprintln!("[Server] Lỗi kết nối: {}", e);
                    thread::sleep(Duration::from_millis(500));
                }
            }
        }
    });

    // Run input hook loop on this thread
    run_hook_loop();

    server_alive.store(false, Ordering::SeqCst);
    let _ = accept_handle.join();
    let _ = cb_monitor_handle.join();

    Ok(())
}

fn run_hook_loop() {
    unsafe {
        let tid = GetCurrentThreadId();
        HOOK_THREAD_ID.store(tid, Ordering::SeqCst);

        let mouse_hook = SetWindowsHookExW(WH_MOUSE_LL, Some(low_level_mouse_proc), 0, 0);
        let kbd_hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(low_level_keyboard_proc), 0, 0);

        if mouse_hook == 0 || kbd_hook == 0 {
            eprintln!("[Server] Lỗi cài đặt Windows Hook!");
            if mouse_hook != 0 { UnhookWindowsHookEx(mouse_hook); }
            if kbd_hook != 0 { UnhookWindowsHookEx(kbd_hook); }
            HOOK_THREAD_ID.store(0, Ordering::SeqCst);
            return;
        }

        println!("[Server] Đã kích hoạt Hook chuột và bàn phím (Sẵn sàng điều khiển đa màn hình).");

        let mut msg: MSG = std::mem::zeroed();
        while SERVER_RUNNING.load(Ordering::SeqCst) {
            let ret = GetMessageW(&mut msg, 0, 0, 0);
            if ret <= 0 {
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        UnhookWindowsHookEx(mouse_hook);
        UnhookWindowsHookEx(kbd_hook);
        HOOK_THREAD_ID.store(0, Ordering::SeqCst);
        println!("[Server] Đã giải phóng Hook.");
    }
}

unsafe extern "system" fn low_level_mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        return unsafe { CallNextHookEx(0, code, wparam, lparam) };
    }

    let ms = unsafe { *(lparam as *const MSLLHOOKSTRUCT) };
    let msg = wparam as u32;

    enum MouseAction {
        PassThrough,
        Consume,
        SetPosAndSend(i32, i32, Sender<Packet>, Packet),
        SetPosAndRelease(i32, i32, Sender<Packet>),
        SendOnly(Sender<Packet>, Packet),
    }

    let action = {
        let mut lock = match SERVER_CTX.lock() {
            Ok(l) => l,
            Err(_) => return unsafe { CallNextHookEx(0, code, wparam, lparam) },
        };
        let ctx = match lock.as_mut() {
            Some(c) => c,
            None => return unsafe { CallNextHookEx(0, code, wparam, lparam) },
        };

        match ctx.active {
            ActiveTarget::Local => {
                if msg == WM_MOUSEMOVE {
                    // 1. Right edge -> Remote(Right)
                    if ms.pt.x >= ctx.local_w - 1 {
                        if let Some(slot) = ctx.slot_right.as_ref() {
                            ctx.active = ActiveTarget::Remote(ScreenPosition::Right);
                            ctx.remote_x = 10;
                            ctx.remote_y = (ms.pt.y * slot.height) / ctx.local_h;
                            MouseAction::SetPosAndSend(
                                ctx.center_x,
                                ctx.center_y,
                                slot.tx.clone(),
                                Packet::MouseMoveAbs { x: ctx.remote_x, y: ctx.remote_y },
                            )
                        } else {
                            MouseAction::PassThrough
                        }
                    }
                    // 2. Left edge -> Remote(Left)
                    else if ms.pt.x <= 0 {
                        if let Some(slot) = ctx.slot_left.as_ref() {
                            ctx.active = ActiveTarget::Remote(ScreenPosition::Left);
                            ctx.remote_x = (slot.width - 11).max(0);
                            ctx.remote_y = (ms.pt.y * slot.height) / ctx.local_h;
                            MouseAction::SetPosAndSend(
                                ctx.center_x,
                                ctx.center_y,
                                slot.tx.clone(),
                                Packet::MouseMoveAbs { x: ctx.remote_x, y: ctx.remote_y },
                            )
                        } else {
                            MouseAction::PassThrough
                        }
                    }
                    // 3. Top edge -> Remote(Top)
                    else if ms.pt.y <= 0 {
                        if let Some(slot) = ctx.slot_top.as_ref() {
                            ctx.active = ActiveTarget::Remote(ScreenPosition::Top);
                            ctx.remote_x = (ms.pt.x * slot.width) / ctx.local_w;
                            ctx.remote_y = (slot.height - 11).max(0);
                            MouseAction::SetPosAndSend(
                                ctx.center_x,
                                ctx.center_y,
                                slot.tx.clone(),
                                Packet::MouseMoveAbs { x: ctx.remote_x, y: ctx.remote_y },
                            )
                        } else {
                            MouseAction::PassThrough
                        }
                    }
                    // 4. Bottom edge -> Remote(Bottom)
                    else if ms.pt.y >= ctx.local_h - 1 {
                        if let Some(slot) = ctx.slot_bottom.as_ref() {
                            ctx.active = ActiveTarget::Remote(ScreenPosition::Bottom);
                            ctx.remote_x = (ms.pt.x * slot.width) / ctx.local_w;
                            ctx.remote_y = 10;
                            MouseAction::SetPosAndSend(
                                ctx.center_x,
                                ctx.center_y,
                                slot.tx.clone(),
                                Packet::MouseMoveAbs { x: ctx.remote_x, y: ctx.remote_y },
                            )
                        } else {
                            MouseAction::PassThrough
                        }
                    } else {
                        MouseAction::PassThrough
                    }
                } else {
                    MouseAction::PassThrough
                }
            }
            ActiveTarget::Remote(current_pos) => {
                let slot = match ctx.get_slot(current_pos) {
                    Some(s) => s.clone(),
                    None => {
                        ctx.active = ActiveTarget::Local;
                        return unsafe { CallNextHookEx(0, code, wparam, lparam) };
                    }
                };

                match msg {
                    WM_MOUSEMOVE => {
                        let dx = ms.pt.x - ctx.center_x;
                        let dy = ms.pt.y - ctx.center_y;
                        if dx == 0 && dy == 0 {
                            // Synthetic warp event resetting cursor to center: absorb it.
                            MouseAction::Consume
                        } else {
                            ctx.remote_x += dx;
                            ctx.remote_y += dy;

                            let switch_back = match current_pos {
                                ScreenPosition::Right => ctx.remote_x < 0,
                                ScreenPosition::Left => ctx.remote_x >= slot.width,
                                ScreenPosition::Top => ctx.remote_y >= slot.height,
                                ScreenPosition::Bottom => ctx.remote_y < 0,
                            };

                            if switch_back {
                                ctx.active = ActiveTarget::Local;
                                let (local_x, local_y) = match current_pos {
                                    ScreenPosition::Right => (ctx.local_w - 15, (ctx.remote_y * ctx.local_h) / slot.height),
                                    ScreenPosition::Left => (15, (ctx.remote_y * ctx.local_h) / slot.height),
                                    ScreenPosition::Top => ((ctx.remote_x * ctx.local_w) / slot.width, 15),
                                    ScreenPosition::Bottom => ((ctx.remote_x * ctx.local_w) / slot.width, ctx.local_h - 15),
                                };
                                MouseAction::SetPosAndRelease(
                                    local_x.clamp(0, ctx.local_w - 1),
                                    local_y.clamp(0, ctx.local_h - 1),
                                    slot.tx.clone(),
                                )
                            } else {
                                ctx.remote_x = ctx.remote_x.clamp(0, slot.width - 1);
                                ctx.remote_y = ctx.remote_y.clamp(0, slot.height - 1);
                                MouseAction::SetPosAndSend(
                                    ctx.center_x,
                                    ctx.center_y,
                                    slot.tx.clone(),
                                    Packet::MouseMoveAbs { x: ctx.remote_x, y: ctx.remote_y },
                                )
                            }
                        }
                    }
                    WM_LBUTTONDOWN => MouseAction::SendOnly(slot.tx.clone(), Packet::MouseButton { button: 1, is_down: true }),
                    WM_LBUTTONUP => MouseAction::SendOnly(slot.tx.clone(), Packet::MouseButton { button: 1, is_down: false }),
                    WM_RBUTTONDOWN => MouseAction::SendOnly(slot.tx.clone(), Packet::MouseButton { button: 2, is_down: true }),
                    WM_RBUTTONUP => MouseAction::SendOnly(slot.tx.clone(), Packet::MouseButton { button: 2, is_down: false }),
                    WM_MBUTTONDOWN => MouseAction::SendOnly(slot.tx.clone(), Packet::MouseButton { button: 3, is_down: true }),
                    WM_MBUTTONUP => MouseAction::SendOnly(slot.tx.clone(), Packet::MouseButton { button: 3, is_down: false }),
                    WM_XBUTTONDOWN => MouseAction::SendOnly(slot.tx.clone(), Packet::MouseButton { button: 4, is_down: true }),
                    WM_XBUTTONUP => MouseAction::SendOnly(slot.tx.clone(), Packet::MouseButton { button: 4, is_down: false }),
                    WM_MOUSEWHEEL => {
                        let delta = (ms.mouseData >> 16) as i16;
                        MouseAction::SendOnly(slot.tx.clone(), Packet::MouseWheel { delta })
                    }
                    _ => MouseAction::Consume,
                }
            }
        }
    };

    // Execute Win32 calls and channel operations safely OUTSIDE the mutex lock
    match action {
        MouseAction::PassThrough => unsafe { CallNextHookEx(0, code, wparam, lparam) },
        MouseAction::Consume => 1,
        MouseAction::SetPosAndSend(px, py, tx, pkt) => {
            unsafe { SetCursorPos(px, py) };
            let _ = tx.send(pkt);
            1
        }
        MouseAction::SetPosAndRelease(px, py, tx) => {
            unsafe { SetCursorPos(px, py) };
            let _ = tx.send(Packet::ReleaseControl);
            1
        }
        MouseAction::SendOnly(tx, pkt) => {
            let _ = tx.send(pkt);
            1
        }
    }
}

unsafe extern "system" fn low_level_keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        return unsafe { CallNextHookEx(0, code, wparam, lparam) };
    }

    let kb = unsafe { *(lparam as *const KBDLLHOOKSTRUCT) };
    let vk = kb.vkCode as u16;
    let scan = kb.scanCode as u16;
    let flags = kb.flags;
    let is_down = wparam as u32 == WM_KEYDOWN || wparam as u32 == WM_SYSKEYDOWN;

    enum KbAction {
        PassThrough,
        Consume,
        SendKey(Sender<Packet>, Packet),
        SwitchToLocal(i32, i32),
        SwitchToRemote(i32, i32, Sender<Packet>, Packet),
    }

    let action = {
        let mut lock = match SERVER_CTX.lock() {
            Ok(l) => l,
            Err(_) => return unsafe { CallNextHookEx(0, code, wparam, lparam) },
        };
        let ctx = match lock.as_mut() {
            Some(c) => c,
            None => return unsafe { CallNextHookEx(0, code, wparam, lparam) },
        };

        // Hotkey Toggle: Scroll Lock (0x91) or Pause (0x13) to cycle through clients
        if (vk == 0x91 || vk == 0x13) && is_down {
            let targets = [
                ActiveTarget::Local,
                ActiveTarget::Remote(ScreenPosition::Right),
                ActiveTarget::Remote(ScreenPosition::Left),
                ActiveTarget::Remote(ScreenPosition::Top),
                ActiveTarget::Remote(ScreenPosition::Bottom),
            ];

            let current_idx = targets.iter().position(|t| *t == ctx.active).unwrap_or(0);
            let mut next_target = ActiveTarget::Local;

            for offset in 1..=targets.len() {
                let cand = targets[(current_idx + offset) % targets.len()];
                match cand {
                    ActiveTarget::Local => {
                        next_target = ActiveTarget::Local;
                        break;
                    }
                    ActiveTarget::Remote(pos) => {
                        if ctx.get_slot(pos).is_some() {
                            next_target = ActiveTarget::Remote(pos);
                            break;
                        }
                    }
                }
            }

            ctx.active = next_target;
            match ctx.active {
                ActiveTarget::Local => {
                    println!("[Server] Phím tắt: Trở về MÁY CHỦ.");
                    ctx.broadcast(Packet::ReleaseControl);
                    KbAction::SwitchToLocal(ctx.center_x, ctx.center_y)
                }
                ActiveTarget::Remote(pos) => {
                    println!("[Server] Phím tắt: Chuyển sang máy khách vị trí {:?}", pos);
                    if let Some(slot) = ctx.get_slot(pos) {
                        let (sw, sh, slot_tx) = (slot.width, slot.height, slot.tx.clone());
                        ctx.remote_x = sw / 2;
                        ctx.remote_y = sh / 2;
                        KbAction::SwitchToRemote(
                            ctx.center_x,
                            ctx.center_y,
                            slot_tx,
                            Packet::MouseMoveAbs { x: ctx.remote_x, y: ctx.remote_y },
                        )
                    } else {
                        KbAction::Consume
                    }
                }
            }
        } else if let Some(slot) = ctx.active_slot() {
            KbAction::SendKey(slot.tx.clone(), Packet::Key { vk, scan, flags, is_down })
        } else {
            KbAction::PassThrough
        }
    };

    match action {
        KbAction::PassThrough => unsafe { CallNextHookEx(0, code, wparam, lparam) },
        KbAction::Consume => 1,
        KbAction::SendKey(tx, pkt) => {
            let _ = tx.send(pkt);
            1
        }
        KbAction::SwitchToLocal(cx, cy) => {
            unsafe { SetCursorPos(cx, cy) };
            1
        }
        KbAction::SwitchToRemote(cx, cy, tx, pkt) => {
            unsafe { SetCursorPos(cx, cy) };
            let _ = tx.send(pkt);
            1
        }
    }
}
