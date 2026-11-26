use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;

pub fn send_mouse_move(x: i32, y: i32, screen_w: i32, screen_h: i32) {
    unsafe {
        // Also call SetCursorPos for immediate and smooth UI responsiveness
        windows_sys::Win32::UI::WindowsAndMessaging::SetCursorPos(x, y);

        // Normalize to 0..65535 for SendInput
        let norm_x = if screen_w > 0 { (x * 65535) / screen_w } else { 0 };
        let norm_y = if screen_h > 0 { (y * 65535) / screen_h } else { 0 };

        let mut input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: norm_x,
                    dy: norm_y,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        SendInput(1, &mut input, std::mem::size_of::<INPUT>() as i32);
    }
}

pub fn send_mouse_button(button: u8, is_down: bool) {
    let dw_flags = match button {
        1 => if is_down { MOUSEEVENTF_LEFTDOWN } else { MOUSEEVENTF_LEFTUP },
        2 => if is_down { MOUSEEVENTF_RIGHTDOWN } else { MOUSEEVENTF_RIGHTUP },
        3 => if is_down { MOUSEEVENTF_MIDDLEDOWN } else { MOUSEEVENTF_MIDDLEUP },
        4 => if is_down { MOUSEEVENTF_XDOWN } else { MOUSEEVENTF_XUP },
        _ => return,
    };

    unsafe {
        let mut input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: if button == 4 { 1 } else { 0 },
                    dwFlags: dw_flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        SendInput(1, &mut input, std::mem::size_of::<INPUT>() as i32);
    }
}

pub fn send_mouse_wheel(delta: i16) {
    unsafe {
        let mut input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: delta as u32,
                    dwFlags: MOUSEEVENTF_WHEEL,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        SendInput(1, &mut input, std::mem::size_of::<INPUT>() as i32);
    }
}

pub fn send_keyboard(vk: u16, scan: u16, flags: u32, is_down: bool) {
    unsafe {
        let mut dw_flags = if is_down { 0 } else { KEYEVENTF_KEYUP };
        if (flags & 1) != 0 {
            dw_flags |= KEYEVENTF_EXTENDEDKEY;
        }

        let mut input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: scan,
                    dwFlags: dw_flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        SendInput(1, &mut input, std::mem::size_of::<INPUT>() as i32);
    }
}
