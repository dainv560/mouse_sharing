use std::env;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::DataExchange::*;
use windows_sys::Win32::System::Memory::*;

use crate::protocol::Packet;

pub const CF_UNICODETEXT: u32 = 13;
pub const CF_HDROP: u32 = 15;

#[repr(C)]
#[derive(Clone, Copy)]
#[allow(non_snake_case)]
pub struct DROPFILES {
    pub pFiles: u32,
    pub pt: POINT,
    pub fNC: BOOL,
    pub fWide: BOOL,
}

static LAST_PROCESSED_HASH: Mutex<u64> = Mutex::new(0);

pub fn compute_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

pub fn set_last_hash(hash: u64) {
    if let Ok(mut lock) = LAST_PROCESSED_HASH.lock() {
        *lock = hash;
    }
}

pub fn get_last_hash() -> u64 {
    LAST_PROCESSED_HASH.lock().map(|h| *h).unwrap_or(0)
}

pub fn get_download_dir() -> PathBuf {
    if let Ok(userprofile) = env::var("USERPROFILE") {
        let p = PathBuf::from(userprofile).join("Downloads").join("MouseShare");
        let _ = fs::create_dir_all(&p);
        return p;
    }
    let p = env::temp_dir().join("MouseShare");
    let _ = fs::create_dir_all(&p);
    p
}

pub fn get_clipboard_text() -> Option<String> {
    unsafe {
        if IsClipboardFormatAvailable(CF_UNICODETEXT) == 0 {
            return None;
        }
        if OpenClipboard(0) == 0 {
            return None;
        }
        let handle = GetClipboardData(CF_UNICODETEXT);
        if handle == 0 {
            CloseClipboard();
            return None;
        }
        let ptr = GlobalLock(handle as _) as *const u16;
        if ptr.is_null() {
            CloseClipboard();
            return None;
        }
        let mut len = 0;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(ptr, len);
        let s = String::from_utf16_lossy(slice);
        GlobalUnlock(handle as _);
        CloseClipboard();
        Some(s)
    }
}

pub fn set_clipboard_text(text: &str) -> bool {
    let hash = compute_hash(&format!("text:{}", text));
    set_last_hash(hash);

    unsafe {
        if OpenClipboard(0) == 0 {
            return false;
        }
        EmptyClipboard();
        let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes_len = utf16.len() * std::mem::size_of::<u16>();
        let h_mem = GlobalAlloc(GMEM_MOVEABLE, bytes_len);
        if h_mem.is_null() {
            CloseClipboard();
            return false;
        }
        let dest = GlobalLock(h_mem) as *mut u16;
        if !dest.is_null() {
            std::ptr::copy_nonoverlapping(utf16.as_ptr(), dest, utf16.len());
            GlobalUnlock(h_mem);
            SetClipboardData(CF_UNICODETEXT, h_mem as _);
        }
        CloseClipboard();
        true
    }
}

pub fn get_clipboard_file() -> Option<(String, Vec<u8>)> {
    unsafe {
        if IsClipboardFormatAvailable(CF_HDROP) == 0 {
            return None;
        }
        if OpenClipboard(0) == 0 {
            return None;
        }
        let handle = GetClipboardData(CF_HDROP);
        if handle == 0 {
            CloseClipboard();
            return None;
        }
        let ptr = GlobalLock(handle as _) as *const u8;
        if ptr.is_null() {
            CloseClipboard();
            return None;
        }
        let drop = &*(ptr as *const DROPFILES);
        let mut file_path: Option<PathBuf> = None;

        if drop.fWide != 0 {
            let curr = ptr.add(drop.pFiles as usize) as *const u16;
            let mut len = 0;
            while *curr.add(len) != 0 {
                len += 1;
            }
            if len > 0 {
                let slice = std::slice::from_raw_parts(curr, len);
                let path_str = String::from_utf16_lossy(slice);
                file_path = Some(PathBuf::from(path_str));
            }
        }
        GlobalUnlock(handle as _);
        CloseClipboard();

        if let Some(path) = file_path {
            if path.is_file() {
                if let Ok(meta) = fs::metadata(&path) {
                    if meta.len() <= 500 * 1024 * 1024 {
                        let filename = path.file_name()?.to_string_lossy().to_string();
                        if let Ok(data) = fs::read(&path) {
                            return Some((filename, data));
                        }
                    }
                }
            }
        }
        None
    }
}

pub fn set_clipboard_file(filename: &str, data: &[u8]) -> Option<PathBuf> {
    let dir = get_download_dir();
    let target = dir.join(filename);
    if fs::write(&target, data).is_err() {
        return None;
    }

    let hash = compute_hash(&format!("file:{}:{}", filename, data.len()));
    set_last_hash(hash);

    if set_clipboard_paths(&[&target]) {
        Some(target)
    } else {
        None
    }
}

pub fn set_clipboard_paths(paths: &[&Path]) -> bool {
    if paths.is_empty() {
        return false;
    }

    let mut file_bytes: Vec<u16> = Vec::new();
    for p in paths {
        let s = p.to_string_lossy();
        file_bytes.extend(s.encode_utf16());
        file_bytes.push(0);
    }
    file_bytes.push(0);

    let drop_size = std::mem::size_of::<DROPFILES>();
    let total_size = drop_size + file_bytes.len() * std::mem::size_of::<u16>();

    unsafe {
        if OpenClipboard(0) == 0 {
            return false;
        }
        EmptyClipboard();

        let h_mem = GlobalAlloc(GMEM_MOVEABLE, total_size);
        if h_mem.is_null() {
            CloseClipboard();
            return false;
        }
        let dest = GlobalLock(h_mem) as *mut u8;
        if !dest.is_null() {
            let drop_header = DROPFILES {
                pFiles: drop_size as u32,
                pt: POINT { x: 0, y: 0 },
                fNC: 0,
                fWide: 1,
            };
            std::ptr::copy_nonoverlapping(
                &drop_header as *const DROPFILES as *const u8,
                dest,
                drop_size,
            );
            let files_dest = dest.add(drop_size) as *mut u16;
            std::ptr::copy_nonoverlapping(file_bytes.as_ptr(), files_dest, file_bytes.len());
            GlobalUnlock(h_mem);
            SetClipboardData(CF_HDROP, h_mem as _);
        }
        CloseClipboard();
        true
    }
}

pub fn start_clipboard_monitor<F>(is_running: Arc<AtomicBool>, mut send_fn: F) -> thread::JoinHandle<()>
where
    F: FnMut(Packet) -> bool + Send + 'static,
{
    thread::spawn(move || {
        while is_running.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(350));

            // Check files first
            if let Some((filename, data)) = get_clipboard_file() {
                let hash = compute_hash(&format!("file:{}:{}", filename, data.len()));
                if hash != get_last_hash() {
                    set_last_hash(hash);
                    println!("[Clipboard] Phát hiện file mới: {} ({} bytes). Đang đồng bộ...", filename, data.len());
                    if !send_fn(Packet::ClipboardFile { filename, data }) {
                        break;
                    }
                    continue;
                }
            }

            // Check text
            if let Some(text) = get_clipboard_text() {
                if !text.is_empty() {
                    let hash = compute_hash(&format!("text:{}", text));
                    if hash != get_last_hash() {
                        set_last_hash(hash);
                        println!("[Clipboard] Phát hiện văn bản mới ({} ký tự). Đang đồng bộ...", text.chars().count());
                        if !send_fn(Packet::ClipboardText { text }) {
                            break;
                        }
                    }
                }
            }
        }
    })
}
