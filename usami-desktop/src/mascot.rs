//! Modo mascote "desktop mate": a mesma janela do app vira um avatar pequeno,
//! sem moldura, sempre no topo e fora da barra de tarefas/Alt+Tab.
//!
//! O pedido de toggle vem da thread de presença (bandeja e Ctrl+Alt+M) por um
//! flag atômico, consumido pelo tick da UI. Toda a cirurgia de estilo/geometria
//! é Win32 porque o GPUI 0.2.2 não troca decoração em runtime no Windows.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

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

/// Origem do pedido de toggle. Minimizar dentro do mascote e o atalho
/// durante uma partida têm efeitos diferentes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToggleRequest {
    Minimize,
    User,
}

static TOGGLE_REQUESTED: AtomicU8 = AtomicU8::new(0);
static ACTIVE: AtomicBool = AtomicBool::new(false);
static ENABLED: AtomicBool = AtomicBool::new(true);
static FOLLOW: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "windows")]
static GAZE_STARTED: AtomicBool = AtomicBool::new(false);
static GAZE_STOP: AtomicBool = AtomicBool::new(false);

pub(crate) fn request_toggle(source: ToggleRequest) {
    let value = match source {
        ToggleRequest::Minimize => 1,
        ToggleRequest::User => 2,
    };
    TOGGLE_REQUESTED.store(value, Ordering::SeqCst);
}

pub(crate) fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::SeqCst);
}

pub(crate) fn is_enabled() -> bool {
    ENABLED.load(Ordering::SeqCst)
}

/// Espelho em memória da preferência persistida: o laço de olhar roda fora da
/// thread da UI e não pode consultar o `UiPrefs`.
pub(crate) fn set_follow(follow: bool) {
    FOLLOW.store(follow, Ordering::SeqCst);
}

pub(crate) fn follow_enabled() -> bool {
    FOLLOW.load(Ordering::SeqCst)
}

/// Uma única thread de olhar por processo; a partir da segunda chamada não
/// cria nada.
#[cfg(target_os = "windows")]
fn claim_gaze_slot() -> bool {
    !GAZE_STARTED.swap(true, Ordering::SeqCst)
}

/// Encerra o laço de olhar no fechamento do app (idempotente).
pub(crate) fn stop_gaze_loop() {
    GAZE_STOP.store(true, Ordering::SeqCst);
}

/// Converte o cursor da tela para a coordenada normalizada do conteúdo da
/// janela (`-1..=1`), com clamp nas bordas. Usa `+y` para cima, que é a
/// convenção do `look_at_normalized` do `gpui-vrm`.
pub(crate) fn normalize_gaze(
    cursor_x: i32,
    cursor_y: i32,
    origin_x: i32,
    origin_y: i32,
    width: i32,
    height: i32,
) -> (f32, f32) {
    let width = width.max(1) as f32;
    let height = height.max(1) as f32;
    let nx = (((cursor_x - origin_x) as f32 / width) * 2.0 - 1.0).clamp(-1.0, 1.0);
    let ny = (1.0 - ((cursor_y - origin_y) as f32 / height) * 2.0).clamp(-1.0, 1.0);
    (nx, ny)
}

/// Posição do menu de contexto: abre para cima perto da base e nunca sai da
/// janela nem cobre a barra de mensagem.
pub(crate) fn menu_position(
    x: f32,
    y: f32,
    menu_w: f32,
    menu_h: f32,
    window_w: f32,
    window_h: f32,
    bottom_reserve: f32,
) -> (f32, f32) {
    let max_x = (window_w - menu_w - 8.0).max(8.0);
    let max_y = (window_h - menu_h - bottom_reserve).max(8.0);
    let y = if y + menu_h > window_h - bottom_reserve {
        y - menu_h
    } else {
        y
    };
    (x.clamp(8.0, max_x), y.clamp(8.0, max_y))
}

pub(crate) fn take_toggle_request() -> Option<ToggleRequest> {
    match TOGGLE_REQUESTED.swap(0, Ordering::SeqCst) {
        1 => Some(ToggleRequest::Minimize),
        2 => Some(ToggleRequest::User),
        _ => None,
    }
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
    use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
    use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
    use windows_sys::Win32::Graphics::Gdi::{
        ClientToScreen, GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GWL_STYLE, GetClientRect, GetCursorPos, GetSystemMetrics, GetWindowLongPtrW,
        GetWindowRect, HWND_NOTOPMOST, HWND_TOPMOST, IsIconic, IsWindowVisible, IsZoomed,
        SM_CXSCREEN, SM_CYSCREEN, SW_HIDE, SW_MAXIMIZE, SW_RESTORE, SW_SHOW, SWP_FRAMECHANGED,
        SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SetWindowLongPtrW, SetWindowPos,
        ShowWindow, ShowWindowAsync, WS_CAPTION, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
        WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_SYSMENU, WS_THICKFRAME,
    };

    static SAVED_STYLE: AtomicIsize = AtomicIsize::new(0);
    static SAVED_EX_STYLE: AtomicIsize = AtomicIsize::new(0);
    static SAVED_MAXIMIZED: AtomicBool = AtomicBool::new(false);
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
            let was_maximized = IsZoomed(hwnd) != 0;
            if IsIconic(hwnd) != 0 || was_maximized {
                ShowWindow(hwnd, SW_RESTORE);
            }

            let restore = fallback.or_else(current_rect)?;
            SAVED_MAXIMIZED.store(was_maximized, Ordering::SeqCst);
            *SAVED_RECT.lock().unwrap_or_else(|p| p.into_inner()) = Some(restore);

            let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
            let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SAVED_STYLE.store(style, Ordering::SeqCst);
            SAVED_EX_STYLE.store(ex_style, Ordering::SeqCst);

            let frameless = style & !(FRAME_STYLES as isize);
            SetWindowLongPtrW(hwnd, GWL_STYLE, frameless);
            let mascot_ex =
                ((ex_style as u32) | WS_EX_TOPMOST | WS_EX_TOOLWINDOW) & !WS_EX_APPWINDOW;
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

    /// Devolve se a janela estava maximizada antes de virar mascote. O
    /// `ShowWindowAsync` deixa o `WM_SIZE` do maximize para depois do update
    /// atual, quando o GPUI já pode ressincronizar o viewport.
    pub(super) fn exit() -> bool {
        let Some(hwnd) = hwnd() else {
            return false;
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

            let restore = SAVED_RECT.lock().unwrap_or_else(|p| p.into_inner()).take();
            let was_maximized = SAVED_MAXIMIZED.swap(false, Ordering::SeqCst);
            let Some((x, y, width, height)) = restore else {
                return false;
            };
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
            if was_maximized {
                ShowWindowAsync(hwnd, SW_MAXIMIZE);
            }
            was_maximized
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

    /// Cursor em coordenadas normalizadas do conteúdo da janela (`-1..=1`),
    /// a mesma convenção que o handler de mouse do `gpui-vrm` usa.
    pub(super) fn cursor_gaze() -> Option<(f32, f32)> {
        let hwnd = hwnd()?;
        unsafe {
            if IsWindowVisible(hwnd) == 0 || IsIconic(hwnd) != 0 {
                return None;
            }
            let mut cursor: POINT = zeroed();
            let mut client: RECT = zeroed();
            if GetCursorPos(&mut cursor) == 0 || GetClientRect(hwnd, &mut client) == 0 {
                return None;
            }
            let mut origin = POINT { x: 0, y: 0 };
            if ClientToScreen(hwnd, &mut origin) == 0 {
                return None;
            }
            Some(super::normalize_gaze(
                cursor.x,
                cursor.y,
                origin.x,
                origin.y,
                client.right - client.left,
                client.bottom - client.top,
            ))
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
pub(crate) fn exit() -> bool {
    win32::exit()
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn exit() -> bool {
    false
}

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

/// Segue o cursor em toda a tela enquanto o mascote está ativo e a preferência
/// está ligada. Roda fora da UI a ~60 fps: o `gpui-vrm` já suaviza o alvo no
/// laço de render, então aqui basta amostrar e enviar.
#[cfg(target_os = "windows")]
pub(crate) fn start_gaze_loop(controller: gpui_vrm::VrmController) {
    if !claim_gaze_slot() {
        return;
    }
    std::thread::Builder::new()
        .name("usami-mascot-gaze".into())
        .spawn(move || {
            let mut following = false;
            loop {
                if GAZE_STOP.load(Ordering::SeqCst) {
                    if following {
                        controller.look_at_normalized(0.0, 0.0);
                    }
                    return;
                }
                if is_active() && follow_enabled() {
                    if let Some((nx, ny)) = win32::cursor_gaze() {
                        controller.look_at_normalized(nx, ny);
                        following = true;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(16));
                } else {
                    if following {
                        controller.look_at_normalized(0.0, 0.0);
                        following = false;
                    }
                    // Fora do mascote não há nada para amostrar: o laço dorme
                    // fundo em vez de acordar a 60 fps à toa.
                    std::thread::sleep(std::time::Duration::from_millis(250));
                }
            }
        })
        .ok();
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn start_gaze_loop(_controller: gpui_vrm::VrmController) {}

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

    #[test]
    fn gaze_maps_the_window_to_minus_one_through_one() {
        let (nx, ny) = normalize_gaze(290, 480, 100, 200, 380, 560);
        assert!((nx - 0.0).abs() < 1e-6, "centro x = {nx}");
        assert!((ny - 0.0).abs() < 1e-6, "centro y = {ny}");

        // Canto superior esquerdo: x à esquerda, y para cima.
        assert_eq!(normalize_gaze(100, 200, 100, 200, 380, 560), (-1.0, 1.0));
        // Canto inferior direito: x à direita, y para baixo.
        assert_eq!(normalize_gaze(480, 760, 100, 200, 380, 560), (1.0, -1.0));
    }

    #[test]
    fn gaze_clamps_outside_the_window_and_survives_zero_size() {
        assert_eq!(normalize_gaze(9999, -9999, 100, 200, 380, 560), (1.0, 1.0));
        let (nx, ny) = normalize_gaze(10, 10, 0, 0, 0, 0);
        assert_eq!((nx, ny), (1.0, -1.0));
    }

    #[test]
    fn menu_opens_down_when_it_fits() {
        let (x, y) = menu_position(100.0, 100.0, 210.0, 172.0, 380.0, 560.0, 56.0);
        assert_eq!((x, y), (100.0, 100.0));
    }

    #[test]
    fn menu_flips_up_near_the_bottom_and_keeps_off_the_composer() {
        let (_, y) = menu_position(100.0, 400.0, 210.0, 172.0, 380.0, 560.0, 56.0);
        assert_eq!(y, 228.0);
        let (_, y) = menu_position(100.0, 520.0, 210.0, 172.0, 380.0, 560.0, 56.0);
        assert_eq!(y, 560.0 - 172.0 - 56.0);
    }

    #[test]
    fn menu_stays_inside_the_horizontal_bounds() {
        assert_eq!(
            menu_position(300.0, 100.0, 210.0, 172.0, 380.0, 560.0, 56.0).0,
            162.0
        );
        assert_eq!(
            menu_position(-40.0, 100.0, 210.0, 172.0, 380.0, 560.0, 56.0).0,
            8.0
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn gaze_loop_has_a_single_instance() {
        assert!(claim_gaze_slot());
        assert!(!claim_gaze_slot());
        stop_gaze_loop();
        assert!(GAZE_STOP.load(Ordering::SeqCst));
    }
}
