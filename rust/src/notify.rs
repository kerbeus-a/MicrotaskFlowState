//! Gentle "look at me" for the check-in timer: a short taskbar nudge that stops on its own.
//! (eframe's RequestUserAttention alone keeps going until the window is activated.)

use eframe::egui;

const INTERVAL_MS: u32 = 500;

/// Flash the taskbar entry `times` times (Windows) or raise a brief urgency hint (Linux),
/// then clear it.
pub fn flash_taskbar(ctx: &egui::Context, times: u32) {
    imp::flash_taskbar(ctx, times);
}

#[cfg(windows)]
mod imp {
    use super::{egui, INTERVAL_MS};
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

    pub fn flash_taskbar(_ctx: &egui::Context, times: u32) {
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
    use super::{egui, INTERVAL_MS};

    /// Linux has no "flash N times" API. Set the urgency hint (taskbar highlight on KDE/XFCE,
    /// "ready" notice on GNOME) for about as long as the Windows flashes, then clear it.
    pub fn flash_taskbar(ctx: &egui::Context, times: u32) {
        ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
            egui::UserAttentionType::Informational,
        ));
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(
                (INTERVAL_MS * 2 * times + 200) as u64,
            ));
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                egui::UserAttentionType::Reset,
            ));
            ctx.request_repaint();
        });
    }
}

#[cfg(windows)]
pub use imp::set_window;
