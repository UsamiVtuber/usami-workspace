//! Modo mascote "desktop mate": a mesma janela do app vira um avatar pequeno,
//! sem moldura, sempre no topo e fora da barra de tarefas/Alt+Tab.
//!
//! O pedido de toggle vem da thread de presença (bandeja e Ctrl+Alt+M) por um
//! flag atômico, consumido pelo tick da UI. Toda a cirurgia de estilo/geometria
//! é Win32 porque o GPUI 0.2.2 não troca decoração em runtime no Windows.

use std::sync::atomic::{AtomicBool, Ordering};

/// Retângulo físico de janela: (x, y, largura, altura), em pixels de tela.
pub(crate) type WinRect = (i32, i32, i32, i32);

/// Tamanho lógico da janela quando ela é só o avatar.
pub(crate) const MASCOT_WIDTH: f32 = 380.0;
pub(crate) const MASCOT_HEIGHT: f32 = 560.0;

/// Encolher abaixo disto (largura ou altura) entra sozinho no modo mascote.
/// É o mesmo mínimo que `WindowPrefs::is_sane` aceita: abaixo dele a janela
/// já não é uma janela de app utilizável.
pub(crate) const SNAP_WIDTH: f32 = 640.0;
pub(crate) const SNAP_HEIGHT: f32 = 480.0;

static TOGGLE_REQUESTED: AtomicBool = AtomicBool::new(false);
static ACTIVE: AtomicBool = AtomicBool::new(false);
static ENABLED: AtomicBool = AtomicBool::new(true);

pub(crate) fn request_toggle() {
    TOGGLE_REQUESTED.store(true, Ordering::SeqCst);
}

pub(crate) fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::SeqCst);
}

pub(crate) fn is_enabled() -> bool {
    ENABLED.load(Ordering::SeqCst)
}

pub(crate) fn take_toggle_request() -> bool {
    TOGGLE_REQUESTED.swap(false, Ordering::SeqCst)
}

pub(crate) fn is_active() -> bool {
    ACTIVE.load(Ordering::SeqCst)
}

pub(crate) fn set_active(active: bool) {
    ACTIVE.store(active, Ordering::SeqCst);
}

pub(crate) fn should_snap(width: f32, height: f32) -> bool {
    width < SNAP_WIDTH || height < SNAP_HEIGHT
}

#[cfg(target_os = "windows")]
mod win32 {
    use super::{MASCOT_HEIGHT, MASCOT_WIDTH, WinRect};
    use std::mem::{size_of, zeroed};
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicIsize, Ordering};
    use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GWL_STYLE, GetCursorPos, GetSystemMetrics, GetWindowLongPtrW, GetWindowRect,
        HWND_NOTOPMOST, HWND_TOPMOST, IsIconic, IsZoomed, SM_CXSCREEN, SM_CYSCREEN,
        SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SW_HIDE,
        SW_RESTORE, SW_SHOW, SetWindowLongPtrW, SetWindowPos, ShowWindow, WS_CAPTION,
        WS_EX_APPWINDOW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_MAXIMIZEBOX, WS_MINIMIZEBOX,
        WS_SYSMENU, WS_THICKFRAME,
    };

    static SAVED_STYLE: AtomicIsize = AtomicIsize::new(0);
    static SAVED_EX_STYLE: AtomicIsize = AtomicIsize::new(0);
    static SAVED_RECT: Mutex<Option<WinRect>> = Mutex::new(None);

    const FRAME_STYLES: u32 =
        WS_CAPTION | WS_THICKFRAME | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX;

    fn hwnd() -> Option<HWND> {
        crate::presence::app_hwnd()
    }

    pub(super) fn current_rect() -> Option<WinRect> {
        let hwnd = hwnd()?;
        unsafe {
            let mut rect: RECT = zeroed();
            if GetWindowRect(hwnd, &mut rect) == 0 {
                return None;
            }
            Some((
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
            ))
        }
    }

    fn work_area(hwnd: HWND) -> (i32, i32, i32, i32) {
        unsafe {
            let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            let mut info: MONITORINFO = zeroed();
            info.cbSize = size_of::<MONITORINFO>() as u32;
            if !monitor.is_null() && GetMonitorInfoW(monitor, &mut info) != 0 {
                return (
                    info.rcWork.left,
                    info.rcWork.top,
                    info.rcWork.right,
                    info.rcWork.bottom,
                );
            }
            (
                0,
                0,
                GetSystemMetrics(SM_CXSCREEN),
                GetSystemMetrics(SM_CYSCREEN),
            )
        }
    }

    /// Esconde e mostra de novo: é o que faz o Explorer recalcular se a janela
    /// entra ou sai da barra de tarefas depois da troca de `WS_EX_TOOLWINDOW`.
    fn refresh_taskbar(hwnd: HWND) {
        unsafe {
            ShowWindow(hwnd, SW_HIDE);
            ShowWindow(hwnd, SW_SHOW);
        }
    }

    pub(super) fn enter(scale: f32, fallback: Option<WinRect>) -> Option<WinRect> {
        let hwnd = hwnd()?;
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        unsafe {
            if IsIconic(hwnd) != 0 || IsZoomed(hwnd) != 0 {
                ShowWindow(hwnd, SW_RESTORE);
            }

            let restore = fallback.or_else(current_rect)?;
            *SAVED_RECT.lock().unwrap_or_else(|p| p.into_inner()) = Some(restore);

            let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
            let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SAVED_STYLE.store(style, Ordering::SeqCst);
            SAVED_EX_STYLE.store(ex_style, Ordering::SeqCst);

            let frameless = style & !(FRAME_STYLES as isize);
            SetWindowLongPtrW(hwnd, GWL_STYLE, frameless);
            let mascot_ex = ((ex_style as u32) | WS_EX_TOPMOST | WS_EX_TOOLWINDOW) & !WS_EX_APPWINDOW;
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, mascot_ex as isize);
            refresh_taskbar(hwnd);

            let width = (MASCOT_WIDTH * scale).round() as i32;
            let height = (MASCOT_HEIGHT * scale).round() as i32;
            let margin = (24.0 * scale).round() as i32;
            let (_, _, work_right, work_bottom) = work_area(hwnd);
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                work_right - width - margin,
                work_bottom - height - margin,
                width,
                height,
                SWP_NOACTIVATE | SWP_FRAMECHANGED | SWP_SHOWWINDOW,
            );
            Some(restore)
        }
    }

    pub(super) fn exit() {
        let Some(hwnd) = hwnd() else {
            return;
        };
        unsafe {
            let style = SAVED_STYLE.load(Ordering::SeqCst);
            let ex_style = SAVED_EX_STYLE.load(Ordering::SeqCst);
            if style != 0 {
                SetWindowLongPtrW(hwnd, GWL_STYLE, style);
            }
            if ex_style != 0 {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style);
            }
            refresh_taskbar(hwnd);

            let restore = SAVED_RECT
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .take();
            if let Some((x, y, width, height)) = restore {
                let insert_after = if (ex_style as u32 & WS_EX_TOPMOST) != 0 {
                    HWND_TOPMOST
                } else {
                    HWND_NOTOPMOST
                };
                SetWindowPos(
                    hwnd,
                    insert_after,
                    x,
                    y,
                    width,
                    height,
                    SWP_NOACTIVATE | SWP_FRAMECHANGED | SWP_SHOWWINDOW,
                );
            }
        }
    }

    /// Distância do canto superior esquerdo da janela até o cursor no momento
    /// do `mouse down`; é o que mantém o avatar "grudado" no ponteiro.
    pub(super) fn grab_offset() -> Option<(i32, i32)> {
        let hwnd = hwnd()?;
        unsafe {
            let mut cursor: POINT = zeroed();
            let mut rect: RECT = zeroed();
            if GetCursorPos(&mut cursor) == 0 || GetWindowRect(hwnd, &mut rect) == 0 {
                return None;
            }
            Some((cursor.x - rect.left, cursor.y - rect.top))
        }
    }

    pub(super) fn move_with_cursor(grab: (i32, i32)) {
        let Some(hwnd) = hwnd() else {
            return;
        };
        unsafe {
            let mut cursor: POINT = zeroed();
            if GetCursorPos(&mut cursor) == 0 {
                return;
            }
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                cursor.x - grab.0,
                cursor.y - grab.1,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }

}

#[cfg(target_os = "windows")]
pub(crate) fn enter(scale: f32, fallback: Option<WinRect>) -> Option<WinRect> {
    win32::enter(scale, fallback)
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn enter(_scale: f32, _fallback: Option<WinRect>) -> Option<WinRect> {
    None
}

#[cfg(target_os = "windows")]
pub(crate) fn exit() {
    win32::exit();
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn exit() {}

#[cfg(target_os = "windows")]
pub(crate) fn current_rect() -> Option<WinRect> {
    win32::current_rect()
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn current_rect() -> Option<WinRect> {
    None
}

#[cfg(target_os = "windows")]
pub(crate) fn grab_offset() -> Option<(i32, i32)> {
    win32::grab_offset()
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn grab_offset() -> Option<(i32, i32)> {
    None
}

#[cfg(target_os = "windows")]
pub(crate) fn move_with_cursor(grab: (i32, i32)) {
    win32::move_with_cursor(grab);
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn move_with_cursor(_grab: (i32, i32)) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_uses_the_same_minimum_the_prefs_accept() {
        assert!(!should_snap(SNAP_WIDTH, SNAP_HEIGHT));
        assert!(should_snap(SNAP_WIDTH - 1.0, SNAP_HEIGHT));
        assert!(should_snap(SNAP_WIDTH, SNAP_HEIGHT - 1.0));
        assert!(!should_snap(1440.0, 900.0));
    }
}
