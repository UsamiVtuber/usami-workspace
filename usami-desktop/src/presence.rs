//! Presença do Usami Desktop no Windows.
//!
//! Uma thread própria com message loop Win32 cuida do ícone da bandeja, do
//! atalho global Ctrl+Alt+U e dos toggles de sempre-no-topo e autostart. A UI
//! do GPUI só entrega o HWND da janela principal e continua dona do seu loop.
//!
//! Regras de escopo: nada aqui inicia rede, serviço ou processo; o autostart
//! grava apenas em `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` e só
//! quando o usuário liga o item do menu.

#![cfg(target_os = "windows")]

use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use gpui::Window;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::Foundation::{
    ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, HWND, LPARAM, LRESULT, POINT, WPARAM,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ, RRF_RT_REG_SZ, RegCloseKey, RegCreateKeyW,
    RegDeleteValueW, RegGetValueW, RegOpenKeyExW, RegSetValueExW,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey, VK_U,
};
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CS_HREDRAW, CS_VREDRAW, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
    DestroyMenu, DestroyWindow, DispatchMessageW, GCLP_HICON, GCLP_HICONSM, GWL_EXSTYLE,
    GetClassLongPtrW, GetCursorPos, GetMessageW, GetWindowLongPtrW, HWND_NOTOPMOST, HWND_TOPMOST,
    IDI_APPLICATION, IsIconic, IsWindowVisible, LoadIconW, MF_CHECKED, MF_SEPARATOR, MF_STRING,
    MSG, PostMessageW, PostQuitMessage, RegisterClassW, RegisterWindowMessageW, SW_HIDE,
    SW_RESTORE, SW_SHOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetForegroundWindow, SetWindowPos,
    ShowWindow, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage,
    WM_APP, WM_CLOSE, WM_CONTEXTMENU, WM_DESTROY, WM_HOTKEY, WM_LBUTTONDBLCLK, WM_NULL,
    WM_RBUTTONUP, WNDCLASSW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

const TRAY_CALLBACK: u32 = WM_APP + 1;
const HOTKEY_ID: i32 = 1;
const TRAY_UID: u32 = 1;
const WINDOW_CLASS: &str = "UsamiPresenceWindow";
const WINDOW_TITLE: &str = "Usami Presence";
const TRAY_TOOLTIP: &str = "Usami Desktop - Ctrl+Alt+U";

const MENU_TOGGLE_WINDOW: usize = 1;
const MENU_TOPMOST: usize = 2;
const MENU_AUTOSTART: usize = 3;
const MENU_EXIT: usize = 4;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "Usami Desktop";

static APP_WINDOW: AtomicIsize = AtomicIsize::new(0);
static HIDDEN_WINDOW: AtomicIsize = AtomicIsize::new(0);
static TRAY_ICON: AtomicIsize = AtomicIsize::new(0);
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);
static THREAD: OnceLock<Mutex<Option<JoinHandle<()>>>> = OnceLock::new();

#[derive(Clone, Copy)]
struct Ready {
    tray: bool,
    hotkey: bool,
}

/// Entrega o HWND da janela principal e sobe a thread de presença.
///
/// Chamado uma vez, logo após `open_window`. Espera no máximo 2 s pelo
/// resultado do registro para registrar falhas no log; não altera a janela.
pub fn start(window: &Window) {
    let Some(app) = window_hwnd(window) else {
        eprintln!("[Usami Presence] janela sem HWND; bandeja desativada");
        return;
    };

    let slot = THREAD.get_or_init(|| Mutex::new(None));
    let mut guard = match slot.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if guard.is_some() {
        return;
    }

    APP_WINDOW.store(app as isize, Ordering::SeqCst);
    let app_id = app as isize;
    let (ready_tx, ready_rx) = mpsc::channel();
    match thread::Builder::new()
        .name("usami-presence".into())
        .spawn(move || run(app_id, ready_tx))
    {
        Ok(handle) => *guard = Some(handle),
        Err(err) => {
            eprintln!("[Usami Presence] não subi a thread: {err}");
            return;
        }
    }
    drop(guard);

    match ready_rx.recv_timeout(Duration::from_secs(2)) {
        Ok(Ok(ready)) => {
            if ready.tray {
                println!("[Usami Presence] bandeja e atalho Ctrl+Alt+U prontos");
            } else {
                eprintln!("[Usami Presence] o ícone da bandeja não foi criado");
            }
            if !ready.hotkey {
                eprintln!(
                    "[Usami Presence] Ctrl+Alt+U indisponível (outra instância ou atalho em uso)"
                );
            }
        }
        Ok(Err(err)) => eprintln!("[Usami Presence] falha ao iniciar: {err}"),
        Err(_) => eprintln!("[Usami Presence] thread não respondeu em 2 s"),
    }
}

/// Encerra a thread de presença: remove o ícone, desregistra o atalho e fecha
/// a janela oculta. Idempotente; pode ser chamado no fechamento da janela.
pub fn shutdown() {
    let hidden = HIDDEN_WINDOW.swap(0, Ordering::SeqCst);
    if hidden != 0 {
        unsafe { PostMessageW(hidden as HWND, WM_CLOSE, 0, 0) };
    }

    let Some(slot) = THREAD.get() else {
        return;
    };
    let handle = match slot.lock() {
        Ok(mut guard) => guard.take(),
        Err(poisoned) => poisoned.into_inner().take(),
    };
    if let Some(handle) = handle {
        let _ = handle.join();
    }
}

fn run(app: isize, ready: Sender<Result<Ready, String>>) {
    let hidden = match create_hidden_window() {
        Ok(hidden) => hidden,
        Err(err) => {
            let _ = ready.send(Err(err));
            return;
        }
    };
    HIDDEN_WINDOW.store(hidden as isize, Ordering::SeqCst);

    let icon = load_tray_icon(app as HWND);
    TRAY_ICON.store(icon as isize, Ordering::SeqCst);
    let tray = add_tray_icon(hidden);

    let hotkey = unsafe {
        RegisterHotKey(
            hidden,
            HOTKEY_ID,
            MOD_CONTROL | MOD_ALT | MOD_NOREPEAT,
            VK_U as u32,
        )
    } != 0;

    unsafe {
        TASKBAR_CREATED.store(
            RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()),
            Ordering::Relaxed,
        );
    }

    let _ = ready.send(Ok(Ready { tray, hotkey }));

    let mut msg = MSG::default();
    while unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) } > 0 {
        unsafe {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn create_hidden_window() -> Result<HWND, String> {
    let class = wide(WINDOW_CLASS);
    let title = wide(WINDOW_TITLE);
    let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
    if instance.is_null() {
        return Err("GetModuleHandleW retornou nulo".into());
    }

    let wnd_class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(window_proc),
        hInstance: instance,
        lpszClassName: class.as_ptr(),
        ..Default::default()
    };
    unsafe { RegisterClassW(&wnd_class) };

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        )
    };
    if hwnd.is_null() {
        return Err("CreateWindowExW retornou nulo".into());
    }
    Ok(hwnd)
}

fn window_hwnd(window: &Window) -> Option<HWND> {
    // A chamada não qualificada cairia no método inerente do GPUI, que devolve
    // um `AnyWindowHandle`; o trait é quem expõe o HWND nativo.
    let handle = HasWindowHandle::window_handle(window).ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(win32) => Some(win32.hwnd.get() as HWND),
        _ => None,
    }
}

fn app_window() -> HWND {
    APP_WINDOW.load(Ordering::SeqCst) as HWND
}

/// `true` quando a janela principal está minimizada ou escondida na bandeja.
/// A UI usa isso para marcar presença ausente mesmo quando o foco já se perdeu
/// (o `observe_window_activation` sozinho não distingue alt-tab de minimizar).
pub fn app_is_minimized_or_hidden() -> bool {
    let hwnd = app_window();
    if hwnd.is_null() {
        return false;
    }
    unsafe { IsIconic(hwnd) != 0 || IsWindowVisible(hwnd) == 0 }
}

fn load_tray_icon(app: HWND) -> *mut core::ffi::c_void {
    // A classe do GPUI já carrega o ícone do executável; ler a classe evita
    // SendMessage entre threads. Sem ícone, cai no ícone padrão do Windows.
    let big = unsafe { GetClassLongPtrW(app, GCLP_HICON) } as *mut core::ffi::c_void;
    if !big.is_null() {
        return big;
    }
    let small = unsafe { GetClassLongPtrW(app, GCLP_HICONSM) } as *mut core::ffi::c_void;
    if !small.is_null() {
        return small;
    }
    unsafe { LoadIconW(std::ptr::null_mut(), IDI_APPLICATION) }
}

fn add_tray_icon(hwnd: HWND) -> bool {
    let mut nid = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_UID,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
        uCallbackMessage: TRAY_CALLBACK,
        hIcon: TRAY_ICON.load(Ordering::SeqCst) as *mut core::ffi::c_void,
        ..Default::default()
    };
    for (slot, unit) in nid.szTip.iter_mut().zip(TRAY_TOOLTIP.encode_utf16()) {
        *slot = unit;
    }
    let added = unsafe { Shell_NotifyIconW(NIM_ADD, &nid) };
    added != 0
}

fn remove_tray_icon(hwnd: HWND) {
    let nid = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_UID,
        ..Default::default()
    };
    unsafe { Shell_NotifyIconW(NIM_DELETE, &nid) };
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        TRAY_CALLBACK => {
            match (lparam as u32) & 0xffff {
                WM_RBUTTONUP | WM_CONTEXTMENU => unsafe { show_tray_menu(hwnd) },
                WM_LBUTTONDBLCLK => unsafe { toggle_window(app_window()) },
                _ => {}
            }
            0
        }
        WM_HOTKEY if wparam as i32 == HOTKEY_ID => {
            unsafe { toggle_window(app_window()) };
            0
        }
        WM_CLOSE => {
            unsafe {
                remove_tray_icon(hwnd);
                UnregisterHotKey(hwnd, HOTKEY_ID);
                DestroyWindow(hwnd);
            }
            0
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            0
        }
        _ => {
            let taskbar_created = TASKBAR_CREATED.load(Ordering::Relaxed);
            if taskbar_created != 0 && msg == taskbar_created {
                // O Explorer reiniciou: os ícones da bandeja se perdem.
                add_tray_icon(hwnd);
                return 0;
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
    }
}

unsafe fn show_tray_menu(hwnd: HWND) {
    let app = app_window();
    if app.is_null() {
        return;
    }

    let shown = unsafe { IsWindowVisible(app) != 0 && IsIconic(app) == 0 };
    let topmost = (unsafe { GetWindowLongPtrW(app, GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST) != 0;
    let autostart = autostart_enabled();

    let menu = unsafe { CreatePopupMenu() };
    if menu.is_null() {
        return;
    }

    let toggle = wide(if shown {
        "Ocultar janela"
    } else {
        "Mostrar janela"
    });
    unsafe { AppendMenuW(menu, MF_STRING, MENU_TOGGLE_WINDOW, toggle.as_ptr()) };

    let topmost_label = wide("Sempre no topo");
    let topmost_flags = MF_STRING | if topmost { MF_CHECKED } else { 0 };
    unsafe { AppendMenuW(menu, topmost_flags, MENU_TOPMOST, topmost_label.as_ptr()) };

    let autostart_label = wide("Iniciar com o Windows");
    let autostart_flags = MF_STRING | if autostart { MF_CHECKED } else { 0 };
    unsafe {
        AppendMenuW(
            menu,
            autostart_flags,
            MENU_AUTOSTART,
            autostart_label.as_ptr(),
        )
    };

    unsafe {
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
    }

    let exit = wide("Sair");
    unsafe { AppendMenuW(menu, MF_STRING, MENU_EXIT, exit.as_ptr()) };

    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point) };
    // Sem SetForegroundWindow o menu pode não fechar ao clicar fora.
    unsafe { SetForegroundWindow(hwnd) };
    let command = unsafe {
        TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY,
            point.x,
            point.y,
            0,
            hwnd,
            std::ptr::null(),
        )
    } as usize;
    unsafe {
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(menu);
    }

    match command {
        MENU_TOGGLE_WINDOW => unsafe { toggle_window(app) },
        MENU_TOPMOST => unsafe { toggle_topmost(app) },
        MENU_AUTOSTART => toggle_autostart(),
        MENU_EXIT => {
            unsafe { PostMessageW(app, WM_CLOSE, 0, 0) };
        }
        _ => {}
    }
}

unsafe fn toggle_window(hwnd: HWND) {
    if hwnd.is_null() {
        return;
    }
    unsafe {
        if IsWindowVisible(hwnd) != 0 && IsIconic(hwnd) == 0 {
            ShowWindow(hwnd, SW_HIDE);
            return;
        }
        ShowWindow(hwnd, SW_SHOW);
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        SetForegroundWindow(hwnd);
    }
}

unsafe fn toggle_topmost(hwnd: HWND) {
    let is_topmost = (unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST) != 0;
    let after = if is_topmost {
        HWND_NOTOPMOST
    } else {
        HWND_TOPMOST
    };
    unsafe {
        SetWindowPos(
            hwnd,
            after,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

fn toggle_autostart() {
    if autostart_enabled() {
        if let Err(err) = disable_autostart() {
            eprintln!("[Usami Presence] não removi o autostart: {err}");
        }
    } else if let Err(err) = enable_autostart() {
        eprintln!("[Usami Presence] não ativei o autostart: {err}");
    }
}

fn autostart_enabled() -> bool {
    reg_value_exists(RUN_KEY, RUN_VALUE)
}

fn enable_autostart() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|err| err.to_string())?;
    let command = quote_command(&exe);
    reg_write_string(RUN_KEY, RUN_VALUE, &command)
}

fn disable_autostart() -> Result<(), String> {
    reg_delete_value(RUN_KEY, RUN_VALUE)
}

fn quote_command(path: &Path) -> Vec<u16> {
    let mut command = vec![u16::from(b'"')];
    command.extend(path.as_os_str().encode_wide());
    command.push(u16::from(b'"'));
    command.push(0);
    command
}

fn reg_value_exists(subkey: &str, value: &str) -> bool {
    let subkey = wide(subkey);
    let value = wide(value);
    let rc = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    rc == ERROR_SUCCESS
}

fn reg_write_string(subkey: &str, value: &str, data: &[u16]) -> Result<(), String> {
    let subkey = wide(subkey);
    let value = wide(value);
    let mut key: HKEY = std::ptr::null_mut();
    let rc = unsafe { RegCreateKeyW(HKEY_CURRENT_USER, subkey.as_ptr(), &mut key) };
    if rc != ERROR_SUCCESS {
        return Err(format!("RegCreateKeyW = {rc}"));
    }
    let bytes = std::mem::size_of_val(data) as u32;
    let rc = unsafe {
        RegSetValueExW(
            key,
            value.as_ptr(),
            0,
            REG_SZ,
            data.as_ptr() as *const u8,
            bytes,
        )
    };
    unsafe { RegCloseKey(key) };
    if rc != ERROR_SUCCESS {
        return Err(format!("RegSetValueExW = {rc}"));
    }
    Ok(())
}

fn reg_delete_value(subkey: &str, value: &str) -> Result<(), String> {
    let subkey = wide(subkey);
    let value = wide(value);
    let mut key: HKEY = std::ptr::null_mut();
    let rc = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            KEY_SET_VALUE,
            &mut key,
        )
    };
    if rc == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    if rc != ERROR_SUCCESS {
        return Err(format!("RegOpenKeyExW = {rc}"));
    }
    let rc = unsafe { RegDeleteValueW(key, value.as_ptr()) };
    unsafe { RegCloseKey(key) };
    if rc != ERROR_SUCCESS && rc != ERROR_FILE_NOT_FOUND {
        return Err(format!("RegDeleteValueW = {rc}"));
    }
    Ok(())
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::System::Registry::RegDeleteKeyW;

    #[test]
    fn command_is_quoted_and_terminated() {
        let command = quote_command(Path::new(r"Usami Desktop\usami-desktop.exe"));
        assert_eq!(command.first().copied(), Some(u16::from(b'"')));
        assert_eq!(
            command.get(command.len() - 2).copied(),
            Some(u16::from(b'"'))
        );
        assert_eq!(command.last().copied(), Some(0));
    }

    #[test]
    fn registry_round_trip_stays_under_hkcu() {
        const TEST_KEY: &str = r"Software\Usami\DesktopPresenceTest";
        const TEST_VALUE: &str = "Usami Desktop Test";
        let data = wide("usami-desktop.exe");

        reg_write_string(TEST_KEY, TEST_VALUE, &data).expect("grava valor de teste");
        assert!(reg_value_exists(TEST_KEY, TEST_VALUE));
        reg_delete_value(TEST_KEY, TEST_VALUE).expect("remove valor de teste");
        assert!(!reg_value_exists(TEST_KEY, TEST_VALUE));

        let key = wide(TEST_KEY);
        unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, key.as_ptr()) };
    }
}
