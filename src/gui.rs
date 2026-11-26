use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::client::{run_client, stop_client};
use crate::protocol::DEFAULT_PORT;
use crate::server::{get_connected_clients_info, run_server, stop_server, ScreenPosition};

const SS_CENTER: u32 = 0x00000001;
const SS_CENTERIMAGE: u32 = 0x00000200;
const BST_CHECKED: usize = 1;
const BM_SETCHECK: u32 = 0x00F1;

const ID_RADIO_SERVER: i32 = 101;
const ID_RADIO_CLIENT: i32 = 102;

const ID_POS_TOP: i32 = 201;
const ID_POS_LEFT: i32 = 202;
const ID_POS_RIGHT: i32 = 203;
const ID_POS_BOTTOM: i32 = 204;

const ID_EDIT_PORT: i32 = 301;
const ID_EDIT_IP: i32 = 302;
const ID_BTN_START: i32 = 401;
const ID_BTN_MINIMIZE_TRAY: i32 = 402;

const WM_TRAYICON: u32 = WM_USER + 100;
const IDM_TRAY_SHOW: usize = 2001;
const IDM_TRAY_EXIT: usize = 2002;

static IS_RUNNING: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy)]
struct GuiState {
    _hwnd: HWND,
    is_server: bool,
    position: ScreenPosition,
    lbl_pos_status: HWND,
    edit_ip: HWND,
    edit_port: HWND,
    btn_start: HWND,
    lbl_status: HWND,
    grp_pos: HWND,
}

static GUI_STATE: Mutex<Option<GuiState>> = Mutex::new(None);

fn get_local_ip() -> String {
    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = socket.local_addr() {
                return addr.ip().to_string();
            }
        }
    }
    "127.0.0.1".to_string()
}

fn to_wstring(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn run_gui() {
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class_name = to_wstring("MouseShareGuiClass");

        let wnd_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: LoadIconW(0, IDI_APPLICATION),
            hCursor: LoadCursorW(0, IDC_ARROW),
            hbrBackground: (COLOR_BTNFACE + 1) as HBRUSH,
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };

        RegisterClassW(&wnd_class);

        let title = to_wstring("Mouse Share - Cài Đặt Màn Hình & Kết Nối (Windows 11)");
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            560,
            640,
            0,
            0,
            instance,
            std::ptr::null(),
        );

        if hwnd == 0 {
            eprintln!("Không thể tạo cửa sổ giao diện!");
            return;
        }

        // Add System Tray icon
        setup_tray_icon(hwnd);

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

unsafe fn setup_tray_icon(hwnd: HWND) {
    let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    nid.uCallbackMessage = WM_TRAYICON;
    nid.hIcon = unsafe { LoadIconW(0, IDI_APPLICATION) };
    let tip = to_wstring("Mouse Share - Đang chạy");
    let copy_len = tip.len().min(nid.szTip.len());
    nid.szTip[..copy_len].copy_from_slice(&tip[..copy_len]);
    unsafe { Shell_NotifyIconW(NIM_ADD, &nid) };
}

unsafe fn remove_tray_icon(hwnd: HWND) {
    let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    unsafe { Shell_NotifyIconW(NIM_DELETE, &nid) };
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            init_gui_controls(hwnd);
            0
        }
        WM_SYSCOMMAND => {
            let cmd = (wparam & 0xFFF0) as u32;
            if cmd == SC_MINIMIZE {
                // Minimize to system tray
                unsafe { ShowWindow(hwnd, SW_HIDE) };
                return 0;
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_TRAYICON => {
            let event = (lparam & 0xFFFF) as u32;
            match event {
                WM_LBUTTONUP | WM_LBUTTONDBLCLK => {
                    // Restore window
                    unsafe {
                        ShowWindow(hwnd, SW_SHOW);
                        ShowWindow(hwnd, SW_RESTORE);
                        SetForegroundWindow(hwnd);
                    }
                }
                WM_RBUTTONUP => {
                    // Show context menu
                    let mut pt: POINT = unsafe { std::mem::zeroed() };
                    unsafe {
                        GetCursorPos(&mut pt);
                        let h_menu = CreatePopupMenu();
                        let show_str = to_wstring("🖥️ Mở giao diện Mouse Share");
                        let exit_str = to_wstring("❌ Thoát ứng dụng");
                        AppendMenuW(h_menu, MF_STRING, IDM_TRAY_SHOW, show_str.as_ptr());
                        AppendMenuW(h_menu, MF_SEPARATOR, 0, std::ptr::null());
                        AppendMenuW(h_menu, MF_STRING, IDM_TRAY_EXIT, exit_str.as_ptr());

                        SetForegroundWindow(hwnd);
                        TrackPopupMenu(
                            h_menu,
                            TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
                            pt.x,
                            pt.y,
                            0,
                            hwnd,
                            std::ptr::null(),
                        );
                        PostMessageW(hwnd, WM_NULL, 0, 0);
                        DestroyMenu(h_menu);
                    }
                }
                _ => {}
            }
            0
        }
        WM_COMMAND => {
            let id = (wparam & 0xFFFF) as usize;
            if id == IDM_TRAY_SHOW {
                unsafe {
                    ShowWindow(hwnd, SW_SHOW);
                    ShowWindow(hwnd, SW_RESTORE);
                    SetForegroundWindow(hwnd);
                }
                return 0;
            } else if id == IDM_TRAY_EXIT {
                stop_server();
                stop_client();
                unsafe {
                    remove_tray_icon(hwnd);
                    DestroyWindow(hwnd);
                }
                return 0;
            }

            handle_command(hwnd, id as i32);
            0
        }
        WM_DESTROY => {
            stop_server();
            stop_client();
            unsafe {
                remove_tray_icon(hwnd);
                PostQuitMessage(0);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn create_font(size: i32, bold: bool) -> HFONT {
    let font_name = to_wstring("Segoe UI");
    unsafe {
        CreateFontW(
            size,
            0,
            0,
            0,
            if bold { FW_BOLD as i32 } else { FW_NORMAL as i32 },
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            (DEFAULT_PITCH | FF_DONTCARE) as u32,
            font_name.as_ptr(),
        )
    }
}

fn init_gui_controls(hwnd: HWND) {
    let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
    let font = create_font(18, false);
    let bold_font = create_font(19, true);
    let title_font = create_font(22, true);

    let btn_class = to_wstring("BUTTON");
    let static_class = to_wstring("STATIC");
    let edit_class = to_wstring("EDIT");

    unsafe {
        // Title
        let title_lbl = CreateWindowExW(
            0,
            static_class.as_ptr(),
            to_wstring("🖱️ MOUSE SHARE - CHIA SẺ CHUỘT, PHÍM & CLIPBOARD").as_ptr(),
            WS_CHILD | WS_VISIBLE | SS_CENTER,
            20,
            15,
            500,
            28,
            hwnd,
            0,
            instance,
            std::ptr::null(),
        );
        SendMessageW(title_lbl, WM_SETFONT, title_font as WPARAM, 1);

        // Group Box: Chế độ
        let grp_mode = CreateWindowExW(
            0,
            btn_class.as_ptr(),
            to_wstring(" 1. Chọn chế độ hoạt động ").as_ptr(),
            WS_CHILD | WS_VISIBLE | (BS_GROUPBOX as u32),
            20,
            50,
            500,
            70,
            hwnd,
            0,
            instance,
            std::ptr::null(),
        );
        SendMessageW(grp_mode, WM_SETFONT, bold_font as WPARAM, 1);

        // Radio: Máy Chủ
        let rdo_server = CreateWindowExW(
            0,
            btn_class.as_ptr(),
            to_wstring("MÁY CHỦ (Host - máy cắm chuột & bàn phím)").as_ptr(),
            WS_CHILD | WS_VISIBLE | (BS_AUTORADIOBUTTON as u32),
            40,
            75,
            340,
            24,
            hwnd,
            ID_RADIO_SERVER as HMENU,
            instance,
            std::ptr::null(),
        );
        SendMessageW(rdo_server, WM_SETFONT, font as WPARAM, 1);
        SendMessageW(rdo_server, BM_SETCHECK, BST_CHECKED, 0);

        // Radio: Máy Khách
        let rdo_client = CreateWindowExW(
            0,
            btn_class.as_ptr(),
            to_wstring("MÁY KHÁCH (Client - nhận điều khiển)").as_ptr(),
            WS_CHILD | WS_VISIBLE | (BS_AUTORADIOBUTTON as u32),
            40,
            95,
            340,
            24,
            hwnd,
            ID_RADIO_CLIENT as HMENU,
            instance,
            std::ptr::null(),
        );
        SendMessageW(rdo_client, WM_SETFONT, font as WPARAM, 1);

        // Group Box: Vị trí màn hình
        let grp_pos = CreateWindowExW(
            0,
            btn_class.as_ptr(),
            to_wstring(" 2. Cài đặt vị trí Máy Khách so với Máy Chủ ").as_ptr(),
            WS_CHILD | WS_VISIBLE | (BS_GROUPBOX as u32),
            20,
            130,
            500,
            180,
            hwnd,
            0,
            instance,
            std::ptr::null(),
        );
        SendMessageW(grp_pos, WM_SETFONT, bold_font as WPARAM, 1);

        // Top
        let btn_top = CreateWindowExW(
            0,
            btn_class.as_ptr(),
            to_wstring("⬆ Ở TRÊN").as_ptr(),
            WS_CHILD | WS_VISIBLE | (BS_PUSHBUTTON as u32),
            220,
            155,
            100,
            32,
            hwnd,
            ID_POS_TOP as HMENU,
            instance,
            std::ptr::null(),
        );
        SendMessageW(btn_top, WM_SETFONT, font as WPARAM, 1);

        // Left
        let btn_left = CreateWindowExW(
            0,
            btn_class.as_ptr(),
            to_wstring("⬅ Ở BÊN TRÁI").as_ptr(),
            WS_CHILD | WS_VISIBLE | (BS_PUSHBUTTON as u32),
            90,
            195,
            115,
            32,
            hwnd,
            ID_POS_LEFT as HMENU,
            instance,
            std::ptr::null(),
        );
        SendMessageW(btn_left, WM_SETFONT, font as WPARAM, 1);

        // Center static: MÁY CHỦ
        let center_lbl = CreateWindowExW(
            0,
            static_class.as_ptr(),
            to_wstring("💻 MÁY CHỦ").as_ptr(),
            WS_CHILD | WS_VISIBLE | SS_CENTER | SS_CENTERIMAGE | WS_BORDER,
            215,
            195,
            110,
            32,
            hwnd,
            0,
            instance,
            std::ptr::null(),
        );
        SendMessageW(center_lbl, WM_SETFONT, bold_font as WPARAM, 1);

        // Right
        let btn_right = CreateWindowExW(
            0,
            btn_class.as_ptr(),
            to_wstring("➡ Ở BÊN PHẢI").as_ptr(),
            WS_CHILD | WS_VISIBLE | (BS_PUSHBUTTON as u32),
            335,
            195,
            115,
            32,
            hwnd,
            ID_POS_RIGHT as HMENU,
            instance,
            std::ptr::null(),
        );
        SendMessageW(btn_right, WM_SETFONT, font as WPARAM, 1);

        // Bottom
        let btn_bottom = CreateWindowExW(
            0,
            btn_class.as_ptr(),
            to_wstring("⬇ Ở DƯỚI").as_ptr(),
            WS_CHILD | WS_VISIBLE | (BS_PUSHBUTTON as u32),
            220,
            235,
            100,
            32,
            hwnd,
            ID_POS_BOTTOM as HMENU,
            instance,
            std::ptr::null(),
        );
        SendMessageW(btn_bottom, WM_SETFONT, font as WPARAM, 1);

        // Position Description Label
        let lbl_pos_status = CreateWindowExW(
            0,
            static_class.as_ptr(),
            to_wstring("Đang chọn: ➡ BÊN PHẢI (Rê chuột chạm mép phải để sang máy khách)").as_ptr(),
            WS_CHILD | WS_VISIBLE | SS_CENTER,
            30,
            275,
            480,
            24,
            hwnd,
            0,
            instance,
            std::ptr::null(),
        );
        SendMessageW(lbl_pos_status, WM_SETFONT, bold_font as WPARAM, 1);

        // Group Box: Kết nối mạng
        let grp_net = CreateWindowExW(
            0,
            btn_class.as_ptr(),
            to_wstring(" 3. Thông tin kết nối mạng ").as_ptr(),
            WS_CHILD | WS_VISIBLE | (BS_GROUPBOX as u32),
            20,
            320,
            500,
            110,
            hwnd,
            0,
            instance,
            std::ptr::null(),
        );
        SendMessageW(grp_net, WM_SETFONT, bold_font as WPARAM, 1);

        let local_ip = get_local_ip();
        let local_ip_text = format!("IP LAN của máy này: {}  (Cổng: {})", local_ip, DEFAULT_PORT);
        let lbl_local_ip = CreateWindowExW(
            0,
            static_class.as_ptr(),
            to_wstring(&local_ip_text).as_ptr(),
            WS_CHILD | WS_VISIBLE,
            40,
            348,
            450,
            22,
            hwnd,
            0,
            instance,
            std::ptr::null(),
        );
        SendMessageW(lbl_local_ip, WM_SETFONT, font as WPARAM, 1);

        let lbl_ip_prompt = CreateWindowExW(
            0,
            static_class.as_ptr(),
            to_wstring("Nhập IP Máy Chủ (dành cho Máy Khách):").as_ptr(),
            WS_CHILD | WS_VISIBLE,
            40,
            375,
            270,
            22,
            hwnd,
            0,
            instance,
            std::ptr::null(),
        );
        SendMessageW(lbl_ip_prompt, WM_SETFONT, font as WPARAM, 1);

        let edit_ip = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            edit_class.as_ptr(),
            to_wstring("192.168.1.").as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | (ES_AUTOHSCROLL as u32),
            40,
            398,
            250,
            24,
            hwnd,
            ID_EDIT_IP as HMENU,
            instance,
            std::ptr::null(),
        );
        SendMessageW(edit_ip, WM_SETFONT, font as WPARAM, 1);
        EnableWindow(edit_ip, 0);

        let lbl_port = CreateWindowExW(
            0,
            static_class.as_ptr(),
            to_wstring("Cổng:").as_ptr(),
            WS_CHILD | WS_VISIBLE,
            320,
            375,
            60,
            22,
            hwnd,
            0,
            instance,
            std::ptr::null(),
        );
        SendMessageW(lbl_port, WM_SETFONT, font as WPARAM, 1);

        let edit_port = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            edit_class.as_ptr(),
            to_wstring(&DEFAULT_PORT.to_string()).as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | (ES_AUTOHSCROLL as u32),
            320,
            398,
            80,
            24,
            hwnd,
            ID_EDIT_PORT as HMENU,
            instance,
            std::ptr::null(),
        );
        SendMessageW(edit_port, WM_SETFONT, font as WPARAM, 1);

        // Buttons: Start / Stop & Minimize to Tray
        let btn_start = CreateWindowExW(
            0,
            btn_class.as_ptr(),
            to_wstring("▶ BẮT ĐẦU CHẠY").as_ptr(),
            WS_CHILD | WS_VISIBLE | (BS_PUSHBUTTON as u32) | WS_TABSTOP,
            100,
            445,
            170,
            42,
            hwnd,
            ID_BTN_START as HMENU,
            instance,
            std::ptr::null(),
        );
        SendMessageW(btn_start, WM_SETFONT, bold_font as WPARAM, 1);

        let btn_min = CreateWindowExW(
            0,
            btn_class.as_ptr(),
            to_wstring("⬇ ẨN XUỐNG KHAY").as_ptr(),
            WS_CHILD | WS_VISIBLE | (BS_PUSHBUTTON as u32) | WS_TABSTOP,
            290,
            445,
            170,
            42,
            hwnd,
            ID_BTN_MINIMIZE_TRAY as HMENU,
            instance,
            std::ptr::null(),
        );
        SendMessageW(btn_min, WM_SETFONT, bold_font as WPARAM, 1);

        // Status Label
        let lbl_status = CreateWindowExW(
            0,
            static_class.as_ptr(),
            to_wstring("Trạng thái: Chưa khởi động. Bấm 'Bắt đầu chạy' để kích hoạt.").as_ptr(),
            WS_CHILD | WS_VISIBLE | SS_CENTER,
            20,
            500,
            500,
            24,
            hwnd,
            0,
            instance,
            std::ptr::null(),
        );
        SendMessageW(lbl_status, WM_SETFONT, font as WPARAM, 1);

        // Bottom Help Text
        let lbl_hint = CreateWindowExW(
            0,
            static_class.as_ptr(),
            to_wstring("Bấm [-] thu nhỏ hoặc 'Ẩn xuống khay' để ẩn xuống góc thanh Taskbar.").as_ptr(),
            WS_CHILD | WS_VISIBLE | SS_CENTER,
            20,
            530,
            500,
            24,
            hwnd,
            0,
            instance,
            std::ptr::null(),
        );
        SendMessageW(lbl_hint, WM_SETFONT, font as WPARAM, 1);

        let mut lock = GUI_STATE.lock().unwrap();
        *lock = Some(GuiState {
            _hwnd: hwnd,
            is_server: true,
            position: ScreenPosition::Right,
            lbl_pos_status,
            edit_ip,
            edit_port,
            btn_start,
            lbl_status,
            grp_pos,
        });
    }
}

fn update_position_text(pos: ScreenPosition) {
    if let Ok(lock) = GUI_STATE.lock() {
        if let Some(ref state) = *lock {
            let text = match pos {
                ScreenPosition::Right => "Đang chọn: ➡ BÊN PHẢI (Rê chuột chạm mép phải để sang máy khách)",
                ScreenPosition::Left => "Đang chọn: ⬅ BÊN TRÁI (Rê chuột chạm mép trái để sang máy khách)",
                ScreenPosition::Top => "Đang chọn: ⬆ Ở TRÊN (Rê chuột chạm mép trên để sang máy khách)",
                ScreenPosition::Bottom => "Đang chọn: ⬇ Ở DƯỚI (Rê chuột chạm mép dưới để sang máy khách)",
            };
            unsafe { SetWindowTextW(state.lbl_pos_status, to_wstring(text).as_ptr()) };
        }
    }
}

pub fn update_status_text(text: &str) {
    if let Ok(lock) = GUI_STATE.lock() {
        if let Some(ref state) = *lock {
            unsafe { SetWindowTextW(state.lbl_status, to_wstring(text).as_ptr()) };
        }
    }
}

fn handle_command(hwnd: HWND, id: i32) {
    match id {
        ID_RADIO_SERVER => {
            if let Ok(mut lock) = GUI_STATE.lock() {
                if let Some(ref mut state) = *lock {
                    state.is_server = true;
                    unsafe {
                        EnableWindow(state.edit_ip, 0);
                        EnableWindow(state.grp_pos, 1);
                    }
                }
            }
        }
        ID_RADIO_CLIENT => {
            if let Ok(mut lock) = GUI_STATE.lock() {
                if let Some(ref mut state) = *lock {
                    state.is_server = false;
                    unsafe {
                        EnableWindow(state.edit_ip, 1);
                        EnableWindow(state.grp_pos, 0);
                    }
                }
            }
        }
        ID_POS_TOP => {
            if let Ok(mut lock) = GUI_STATE.lock() {
                if let Some(ref mut state) = *lock {
                    state.position = ScreenPosition::Top;
                }
            }
            update_position_text(ScreenPosition::Top);
        }
        ID_POS_LEFT => {
            if let Ok(mut lock) = GUI_STATE.lock() {
                if let Some(ref mut state) = *lock {
                    state.position = ScreenPosition::Left;
                }
            }
            update_position_text(ScreenPosition::Left);
        }
        ID_POS_RIGHT => {
            if let Ok(mut lock) = GUI_STATE.lock() {
                if let Some(ref mut state) = *lock {
                    state.position = ScreenPosition::Right;
                }
            }
            update_position_text(ScreenPosition::Right);
        }
        ID_POS_BOTTOM => {
            if let Ok(mut lock) = GUI_STATE.lock() {
                if let Some(ref mut state) = *lock {
                    state.position = ScreenPosition::Bottom;
                }
            }
            update_position_text(ScreenPosition::Bottom);
        }
        ID_BTN_START => {
            toggle_start_stop(hwnd);
        }
        ID_BTN_MINIMIZE_TRAY => {
            unsafe { ShowWindow(hwnd, SW_HIDE) };
        }
        _ => {}
    }
}

fn toggle_start_stop(_hwnd: HWND) {
    let state = {
        let lock = GUI_STATE.lock().unwrap();
        match *lock {
            Some(s) => s,
            None => return,
        }
    };

    if IS_RUNNING.load(Ordering::SeqCst) {
        // Stop
        IS_RUNNING.store(false, Ordering::SeqCst);
        stop_server();
        stop_client();
        unsafe { SetWindowTextW(state.btn_start, to_wstring("▶ BẮT ĐẦU CHẠY").as_ptr()) };
        update_status_text("Trạng thái: Đã dừng.");
    } else {
        // Start
        IS_RUNNING.store(true, Ordering::SeqCst);
        unsafe { SetWindowTextW(state.btn_start, to_wstring("⏹ DỪNG LẠI").as_ptr()) };

        let mut port_buf = [0u16; 16];
        unsafe { GetWindowTextW(state.edit_port, port_buf.as_mut_ptr(), 16) };
        let port_str = String::from_utf16_lossy(&port_buf);
        let port: u16 = port_str.trim_matches(char::from(0)).trim().parse().unwrap_or(DEFAULT_PORT);

        let is_server = state.is_server;
        let pos = state.position;

        if is_server {
            update_status_text("Trạng thái: Máy Chủ đang chạy, chờ các Máy Khách kết nối...");
            let on_client_change = Arc::new(|| {
                let clients = get_connected_clients_info();
                if clients.is_empty() {
                    update_status_text("Trạng thái: Máy Chủ đang chạy, chờ các Máy Khách kết nối...");
                } else {
                    let details: Vec<String> = clients.iter().map(|(pos, addr, w, h)| {
                        let pos_name = match pos {
                            ScreenPosition::Right => "Phải",
                            ScreenPosition::Left => "Trái",
                            ScreenPosition::Top => "Trên",
                            ScreenPosition::Bottom => "Dưới",
                        };
                        format!("{}: {} ({}x{})", pos_name, addr, w, h)
                    }).collect();
                    update_status_text(&format!("Đã kết nối {} máy [ {} ]", clients.len(), details.join(" | ")));
                }
            });

            thread::spawn(move || {
                if let Err(e) = run_server(port, pos, Some(on_client_change)) {
                    eprintln!("Lỗi Server: {}", e);
                    update_status_text(&format!("Lỗi Máy Chủ: {}", e));
                    IS_RUNNING.store(false, Ordering::SeqCst);
                    if let Ok(lock) = GUI_STATE.lock() {
                        if let Some(ref st) = *lock {
                            unsafe { SetWindowTextW(st.btn_start, to_wstring("▶ BẮT ĐẦU CHẠY").as_ptr()) };
                        }
                    }
                }
            });
        } else {
            let mut ip_buf = [0u16; 64];
            unsafe { GetWindowTextW(state.edit_ip, ip_buf.as_mut_ptr(), 64) };
            let ip_str = String::from_utf16_lossy(&ip_buf);
            let ip_trimmed = ip_str.trim_matches(char::from(0)).trim().to_string();

            if ip_trimmed.is_empty() || ip_trimmed == "192.168.1." {
                update_status_text("Lỗi: Hãy nhập địa chỉ IP hợp lệ của Máy Chủ!");
                IS_RUNNING.store(false, Ordering::SeqCst);
                unsafe { SetWindowTextW(state.btn_start, to_wstring("▶ BẮT ĐẦU CHẠY").as_ptr()) };
                return;
            }

            let target = if ip_trimmed.contains(':') {
                ip_trimmed
            } else {
                format!("{}:{}", ip_trimmed, port)
            };

            update_status_text(&format!("Trạng thái: Đang kết nối tới Máy Chủ {}...", target));

            let on_connected = Arc::new(|w: i32, h: i32| {
                let msg = format!("Trạng thái: ĐÃ KẾT NỐI VỚI MÁY CHỦ! (Màn hình máy chủ: {}x{})", w, h);
                update_status_text(&msg);
            });
            let on_disconnected = Arc::new(|| {
                update_status_text("Trạng thái: Mất kết nối tới Máy Chủ. Đang thử kết nối lại...");
            });

            thread::spawn(move || {
                if let Err(e) = run_client(&target, Some(on_connected), Some(on_disconnected)) {
                    eprintln!("Lỗi Client: {}", e);
                    update_status_text(&format!("Lỗi Máy Khách: {}", e));
                    IS_RUNNING.store(false, Ordering::SeqCst);
                    if let Ok(lock) = GUI_STATE.lock() {
                        if let Some(ref st) = *lock {
                            unsafe { SetWindowTextW(st.btn_start, to_wstring("▶ BẮT ĐẦU CHẠY").as_ptr()) };
                        }
                    }
                }
            });
        }
    }
}
