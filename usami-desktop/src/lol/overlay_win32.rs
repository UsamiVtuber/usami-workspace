//! Controle nativo Win32 para o modo overlay transparente e click-through
//! (`WS_EX_TRANSPARENT | WS_EX_LAYERED | WS_EX_TOPMOST`) sem bloquear o GPUI.

#[cfg(target_os = "windows")]
mod win32 {
    use std::sync::atomic::{AtomicIsize, Ordering};
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, HWND_NOTOPMOST, HWND_TOPMOST, SWP_FRAMECHANGED,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowLongPtrW, SetWindowPos, WS_EX_LAYERED,
        WS_EX_TOPMOST, WS_EX_TRANSPARENT,
    };

    static SAVED_EX_STYLE: AtomicIsize = AtomicIsize::new(0);
    static HAS_SAVED_STYLE: AtomicIsize = AtomicIsize::new(0);

    pub fn apply_overlay_styles(overlay_active: bool, click_through: bool) {
        let Some(hwnd) = crate::presence::app_hwnd() else {
            return;
        };
        unsafe {
            let current = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            if HAS_SAVED_STYLE
                .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                SAVED_EX_STYLE.store(current, Ordering::SeqCst);
            }

            if overlay_active {
                let mut target = current | (WS_EX_LAYERED as isize) | (WS_EX_TOPMOST as isize);
                if click_through {
                    target |= WS_EX_TRANSPARENT as isize;
                } else {
                    target &= !(WS_EX_TRANSPARENT as isize);
                }
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, target);
                SetWindowPos(
                    hwnd,
                    HWND_TOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                );
            } else {
                let base = SAVED_EX_STYLE.load(Ordering::SeqCst);
                let restored = base & !(WS_EX_TRANSPARENT as isize);
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, restored);
                let insert_after: HWND = if (restored as u32 & WS_EX_TOPMOST) != 0 {
                    HWND_TOPMOST
                } else {
                    HWND_NOTOPMOST
                };
                SetWindowPos(
                    hwnd,
                    insert_after,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                );
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod win32 {
    pub fn apply_overlay_styles(_overlay_active: bool, _click_through: bool) {}
}

pub fn apply_overlay_styles(overlay_active: bool, click_through: bool) {
    win32::apply_overlay_styles(overlay_active, click_through);
}
