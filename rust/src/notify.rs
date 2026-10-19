//! Gentle "look at me" for the check-in timer: flash the taskbar button a few times, then stop.
//! (eframe's RequestUserAttention flashes until the window is activated.)

#[cfg(windows)]
mod imp {
    use std::sync::OnceLock;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        FlashWindowEx, FLASHWINFO, FLASHW_STOP, FLASHW_TRAY,
    };

    static HWND: OnceLock<isize> = OnceLock::new();

    pub fn set_window(hwnd: isize) {
        let _ = HWND.set(hwnd);
    }

    fn flash(flags: u32, count: u32, interval_ms: u32) {
        let Some(&hwnd) = HWND.get() else { return };
        let info = FLASHWINFO {
            cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
            hwnd: hwnd as _,
            dwFlags: flags,
            uCount: count,
            dwTimeout: interval_ms,
        };
        // SAFETY: valid struct for our own window handle.
        unsafe {
            FlashWindowEx(&info);
        }
    }

    pub fn flash_taskbar(times: u32) {
        const INTERVAL_MS: u32 = 500;
        flash(FLASHW_TRAY, times, INTERVAL_MS);
        // Windows leaves the button highlighted after the last flash; clear it once done.
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(
                (INTERVAL_MS * 2 * times + 200) as u64,
            ));
            flash(FLASHW_STOP, 0, 0);
        });
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn flash_taskbar(_times: u32) {}
}

pub use imp::*;
