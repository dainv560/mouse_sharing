#![windows_subsystem = "windows"]

use std::env;
use std::net::UdpSocket;

use windows_sys::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};

mod client;
mod clipboard;
mod gui;
mod input;
mod protocol;
mod server;

use protocol::DEFAULT_PORT;
use server::ScreenPosition;

fn get_local_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    Some(socket.local_addr().ok()?.ip().to_string())
}

fn print_banner() {
    println!(r#"
  __  __                       ____  _                     
 |  \/  | ___  _   _ ___  ___ / ___|| |__   __ _ _ __ ___  
 | |\/| |/ _ \| | | / __|/ _ \\___ \| '_ \ / _` | '__/ _ \ 
 | |  | | (_) | |_| \__ \  __/ ___) | | | | (_| | | |  __/ 
 |_|  |_|\___/ \__,_|___/\___||____/|_| |_|\__,_|_|  \___| 
    Chia sẻ chuột & bàn phím qua mạng LAN (Windows 11)
"#);
}

fn print_help() {
    println!("SỬ DỤNG:");
    println!("  1. Mở GIAO DIỆN ĐỒ HỌA (GUI) trực quan:");
    println!("     mouse_share.exe");
    println!("     mouse_share.exe --gui");
    println!();
    println!("  2. Chạy dòng lệnh trên MÁY CHỦ (máy có chuột & phím vật lý):");
    println!("     mouse_share.exe --server [--port 8421] [--pos right|left|top|bottom]");
    println!("     Ví dụ: mouse_share.exe --server --pos right");
    println!();
    println!("  3. Chạy dòng lệnh trên MÁY KHÁCH (máy nhận điều khiển):");
    println!("     mouse_share.exe --client <IP_MÁY_CHỦ>[:CỔNG]");
    println!("     Ví dụ: mouse_share.exe --client 192.168.1.50");
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();

    // If run without arguments or with --gui, open purely GUI window (no console)
    if args.len() == 1 || (args.len() == 2 && (args[1] == "--gui" || args[1] == "-g")) {
        gui::run_gui();
        return Ok(());
    }

    // If command line arguments were passed, attach to parent console so CLI output works
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }

    print_banner();

    let mut i = 1;
    let mut is_server = false;
    let mut client_target = None;
    let mut port = DEFAULT_PORT;
    let mut position = ScreenPosition::Right;

    while i < args.len() {
        match args[i].as_str() {
            "--server" | "-s" => {
                is_server = true;
            }
            "--client" | "-c" => {
                if i + 1 < args.len() {
                    client_target = Some(args[i + 1].clone());
                    i += 1;
                } else {
                    eprintln!("Lỗi: Thiếu địa chỉ IP máy chủ sau --client");
                    std::process::exit(1);
                }
            }
            "--port" | "-p" => {
                if i + 1 < args.len() {
                    port = args[i + 1].parse().unwrap_or(DEFAULT_PORT);
                    i += 1;
                }
            }
            "--pos" => {
                if i + 1 < args.len() {
                    match args[i + 1].to_lowercase().as_str() {
                        "left" => position = ScreenPosition::Left,
                        "top" => position = ScreenPosition::Top,
                        "bottom" => position = ScreenPosition::Bottom,
                        _ => position = ScreenPosition::Right,
                    }
                    i += 1;
                }
            }
            "--gui" | "-g" => {
                gui::run_gui();
                return Ok(());
            }
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            other => {
                if other.contains('.') || other.contains(':') {
                    client_target = Some(other.to_string());
                } else {
                    eprintln!("Tham số không hợp lệ: {}", other);
                    print_help();
                    std::process::exit(1);
                }
            }
        }
        i += 1;
    }

    if is_server {
        if let Some(ip) = get_local_ip() {
            println!("[Gợi ý] Địa chỉ IP LAN của máy này là: {}", ip);
        }
        server::run_server(port, position, None)?;
        return Ok(());
    }

    if let Some(target) = client_target {
        let full_addr = if target.contains(':') {
            target
        } else {
            format!("{}:{}", target, port)
        };
        client::run_client(&full_addr, None, None)?;
        return Ok(());
    }

    gui::run_gui();
    Ok(())
}
