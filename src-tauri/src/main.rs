#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{path::PathBuf, sync::Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

static RESIZING: AtomicBool = AtomicBool::new(false);

type FrameRect = (f64, f64, f64, f64);

struct AppState {
    hotkey: Mutex<Shortcut>,
    mark_hotkey: Mutex<Shortcut>,
    restore_hotkey: Mutex<Shortcut>,
    dark_mode: Mutex<bool>,
    silent_start: Mutex<bool>,
    autostart: Mutex<bool>,
    marked_rect: Mutex<Option<FrameRect>>,
    config_path: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct Config {
    hotkey: String,
    #[serde(default = "default_mark_hotkey")]
    mark_hotkey: String,
    #[serde(default = "default_restore_hotkey")]
    restore_hotkey: String,
    #[serde(default)]
    dark_mode: bool,
    #[serde(default)]
    silent_start: bool,
    #[serde(default)]
    autostart: bool,
    #[serde(default)]
    marked_rect: Option<FrameRect>,
}

fn default_mark_hotkey() -> String {
    "alt+m".into()
}

fn default_restore_hotkey() -> String {
    "alt+`".into()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey: "ctrl+space".into(),
            mark_hotkey: default_mark_hotkey(),
            restore_hotkey: default_restore_hotkey(),
            dark_mode: false,
            silent_start: false,
            autostart: false,
            marked_rect: None,
        }
    }
}

fn parse_shortcut_or(s: &str, default: &str) -> Shortcut {
    s.parse().unwrap_or_else(|_| default.parse().unwrap())
}

fn load_config(path: &PathBuf) -> Config {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_config(state: &AppState) {
    let path = &state.config_path;
    let cfg = Config {
        hotkey: state.hotkey.lock().unwrap().to_string(),
        mark_hotkey: state.mark_hotkey.lock().unwrap().to_string(),
        restore_hotkey: state.restore_hotkey.lock().unwrap().to_string(),
        dark_mode: *state.dark_mode.lock().unwrap(),
        silent_start: *state.silent_start.lock().unwrap(),
        autostart: *state.autostart.lock().unwrap(),
        marked_rect: *state.marked_rect.lock().unwrap(),
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(s) = serde_json::to_string_pretty(&cfg) {
        let _ = std::fs::write(path, s);
    }
}

fn toggle_main(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        match win.is_visible() {
            Ok(true) => {
                let _ = win.hide();
            }
            _ => {
                let _ = win.show();
                let _ = win.set_focus();
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn window_rect_physical(window: &tauri::WebviewWindow) -> Option<(i32, i32, i32, i32)> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

    let hwnd = window.hwnd().ok()?;
    let mut rect = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut rect) }.ok()?;
    Some((
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top,
    ))
}

#[cfg(target_os = "windows")]
fn apply_window_rect(window: &tauri::WebviewWindow, rect: Option<FrameRect>) {
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_TOP, SWP_NOACTIVATE, SWP_NOZORDER,
    };

    let Some((x, y, w, h)) = rect else { return };
    let Ok(hwnd) = window.hwnd() else { return };
    let s = window.scale_factor().unwrap_or(1.0);
    let _ = unsafe {
        SetWindowPos(
            hwnd,
            Some(HWND_TOP),
            (x * s) as i32,
            (y * s) as i32,
            (w * s) as i32,
            (h * s) as i32,
            SWP_NOZORDER | SWP_NOACTIVATE,
        )
    };
}

fn is_main_visible(app: &AppHandle) -> bool {
    app.get_webview_window("main")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}

fn mark_frame_inner(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        #[cfg(target_os = "windows")]
        if let Some((x, y, w, h)) = window_rect_physical(&win) {
            let s = win.scale_factor().unwrap_or(1.0);
            let rect = (x as f64 / s, y as f64 / s, w as f64 / s, h as f64 / s);
            *app.state::<AppState>().marked_rect.lock().unwrap() = Some(rect);
            save_config(&app.state::<AppState>());
            show_mark_notification();
        }
    }
}

#[cfg(target_os = "windows")]
fn show_mark_notification() {
    use tauri_winrt_notification::Toast;

    let in_target_dir = tauri::utils::platform::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.display().to_string()))
        .map(|d| {
            d.ends_with("target\\debug")
                || d.ends_with("target\\release")
                || d.ends_with("target/debug")
                || d.ends_with("target/release")
        })
        .unwrap_or(false);

    let app_id = if in_target_dir {
        Toast::POWERSHELL_APP_ID
    } else {
        "com.textplasticine.desktop"
    };

    let _ = Toast::new(app_id).text1("已记录编辑框的状态").show();
}

fn restore_frame_inner(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let rect = *app.state::<AppState>().marked_rect.lock().unwrap();
        #[cfg(target_os = "windows")]
        apply_window_rect(&win, rect);
        let _ = win.show();
        let _ = win.set_focus();
    }
}

#[tauri::command]
fn set_default_rect(
    state: State<'_, AppState>,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) {
    let mut marked = state.marked_rect.lock().unwrap();
    if marked.is_none() {
        *marked = Some((x, y, w, h));
        drop(marked);
        save_config(&state);
    }
}

#[tauri::command]
fn start_corner_resize(window: tauri::WebviewWindow) {
    if RESIZING.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        #[cfg(target_os = "windows")]
        corner_resize_loop(&window);
        RESIZING.store(false, Ordering::SeqCst);
    });
}

#[cfg(target_os = "windows")]
fn corner_resize_loop(window: &tauri::WebviewWindow) {
    use std::thread::sleep;
    use std::time::Duration;
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Dwm::DwmFlush;
    use windows::Win32::Media::{timeBeginPeriod, timeEndPeriod};
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetCursorPos, GetWindowRect, SetWindowPos, HWND_TOP, SWP_NOACTIVATE, SWP_NOZORDER,
    };

    let Ok(hwnd) = window.hwnd() else { return };
    let mut start = unsafe { std::mem::zeroed() };
    if unsafe { GetCursorPos(&mut start) }.is_err() {
        return;
    }
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
        return;
    }
    let (start_w, start_h) = (rect.right - rect.left, rect.bottom - rect.top);

    let dpi = unsafe { GetDpiForWindow(hwnd) } as f64;
    let scale = dpi / 96.0;
    let min_w = (260.0 * scale) as i32;
    let min_h = (100.0 * scale) as i32;

    unsafe { timeBeginPeriod(1) };
    let mut last = (rect.left, rect.top, start_w, start_h);

    loop {
        let mut cur = unsafe { std::mem::zeroed() };
        if unsafe { GetCursorPos(&mut cur) }.is_err() {
            break;
        }
        let w = (start_w + (cur.x - start.x)).max(min_w);
        let h = (start_h + (cur.y - start.y)).max(min_h);
        let current = (rect.left, rect.top, w, h);
        if current != last {
            let _ = unsafe {
                SetWindowPos(
                    hwnd,
                    Some(HWND_TOP),
                    rect.left,
                    rect.top,
                    w,
                    h,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                )
            };
            last = current;
        }
        if unsafe { GetAsyncKeyState(VK_LBUTTON.0 as i32) } >= 0 {
            break;
        }
        if unsafe { DwmFlush() }.is_err() {
            sleep(Duration::from_millis(16));
        }
    }
    unsafe { timeEndPeriod(1) };
}

fn register_all(app: &AppHandle) {
    let state = app.state::<AppState>();
    let gs = app.global_shortcut();
    let keys = [
        *state.hotkey.lock().unwrap(),
        *state.mark_hotkey.lock().unwrap(),
        *state.restore_hotkey.lock().unwrap(),
    ];
    for k in keys {
        if !gs.is_registered(k) {
            let _ = gs.register(k);
        }
    }
}

#[tauri::command]
fn suspend_hotkeys(app: AppHandle) {
    let state = app.state::<AppState>();
    let gs = app.global_shortcut();
    let _ = gs.unregister(*state.hotkey.lock().unwrap());
    let _ = gs.unregister(*state.mark_hotkey.lock().unwrap());
    let _ = gs.unregister(*state.restore_hotkey.lock().unwrap());
}

#[tauri::command]
fn restore_hotkeys(app: AppHandle) {
    register_all(&app);
}

fn open_settings(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.show();
        let _ = win.set_focus();
        return;
    }
    let win = tauri::WebviewWindowBuilder::new(
        app,
        "settings",
        tauri::WebviewUrl::App("settings.html".into()),
    )
    .title("设置")
    .inner_size(400.0, 430.0)
    .resizable(false)
    .maximizable(false)
    .always_on_top(true)
    .center()
    .build();

    if let Ok(win) = win {
        #[cfg(target_os = "windows")]
        remove_maximize_button(&win);
    }
}

#[cfg(target_os = "windows")]
fn remove_maximize_button(win: &tauri::WebviewWindow) {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_STYLE, SWP_FRAMECHANGED,
        SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_MAXIMIZEBOX,
    };

    if let Ok(hwnd) = win.hwnd() {
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
            let style = style & !(WS_MAXIMIZEBOX.0 as isize);
            SetWindowLongPtrW(hwnd, GWL_STYLE, style);
            let _ = SetWindowPos(
                hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
            );
        }
    }
}

#[tauri::command]
fn get_shortcut(state: State<'_, AppState>, kind: String) -> String {
    match kind.as_str() {
        "mark" => state.mark_hotkey.lock().unwrap().to_string(),
        "restore" => state.restore_hotkey.lock().unwrap().to_string(),
        _ => state.hotkey.lock().unwrap().to_string(),
    }
}

#[tauri::command]
fn set_shortcut(
    app: AppHandle,
    state: State<'_, AppState>,
    kind: String,
    hotkey: String,
) -> Result<(), String> {
    let new: Shortcut = hotkey
        .parse()
        .map_err(|_| format!("无效的快捷键: {hotkey}"))?;
    let slot = match kind.as_str() {
        "mark" => &state.mark_hotkey,
        "restore" => &state.restore_hotkey,
        _ => &state.hotkey,
    };
    let old = *slot.lock().unwrap();
    if new == old {
        register_all(&app);
        return Ok(());
    }
    let gs = app.global_shortcut();
    let _ = gs.unregister(old);
    if let Err(e) = gs.register(new) {
        register_all(&app);
        return Err(format!("注册失败（可能与其他程序冲突）: {e}"));
    }
    *slot.lock().unwrap() = new;
    register_all(&app);
    save_config(&state);
    Ok(())
}

#[tauri::command]
fn get_dark_mode(state: State<'_, AppState>) -> bool {
    *state.dark_mode.lock().unwrap()
}

#[tauri::command]
fn set_dark_mode(app: AppHandle, state: State<'_, AppState>, dark: bool) {
    {
        let mut m = state.dark_mode.lock().unwrap();
        *m = dark;
    }
    save_config(&state);
    let _ = app.emit("dark-mode", dark);
}

#[tauri::command]
fn get_silent_start(state: State<'_, AppState>) -> bool {
    *state.silent_start.lock().unwrap()
}

#[tauri::command]
fn set_silent_start(state: State<'_, AppState>, silent: bool) {
    {
        let mut m = state.silent_start.lock().unwrap();
        *m = silent;
    }
    save_config(&state);
}

#[tauri::command]
fn get_autostart(state: State<'_, AppState>) -> bool {
    *state.autostart.lock().unwrap()
}

#[tauri::command]
fn set_autostart(state: State<'_, AppState>, enable: bool) {
    {
        let mut m = state.autostart.lock().unwrap();
        *m = enable;
    }
    #[cfg(target_os = "windows")]
    set_autostart_registry(enable);
    save_config(&state);
}

#[derive(Serialize)]
struct StartupConfig {
    silent_start: bool,
    marked_rect: Option<FrameRect>,
}

#[tauri::command]
fn get_startup_config(state: State<'_, AppState>) -> StartupConfig {
    StartupConfig {
        silent_start: *state.silent_start.lock().unwrap(),
        marked_rect: *state.marked_rect.lock().unwrap(),
    }
}

#[cfg(target_os = "windows")]
fn set_autostart_registry(enable: bool) {
    use windows::core::{w, PCWSTR};
    use windows::Win32::System::Registry::{
        RegDeleteKeyValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ,
    };

    const SUBKEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
    const NAME: PCWSTR = w!("TextPlasticine");

    unsafe {
        if enable {
            if let Ok(exe) = std::env::current_exe() {
                let path: Vec<u16> = exe
                    .to_string_lossy()
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();
                let _ = RegSetKeyValueW(
                    HKEY_CURRENT_USER,
                    SUBKEY,
                    NAME,
                    REG_SZ.0,
                    Some(path.as_ptr() as *const std::ffi::c_void),
                    (path.len() * 2) as u32,
                );
            }
        } else {
            let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, SUBKEY, NAME);
        }
    }
}

fn tray_icon_image() -> tauri::image::Image<'static> {
    tauri::image::Image::from_bytes(include_bytes!("../assets/tray.png"))
        .expect("failed to load tray icon")
}

#[cfg(target_os = "windows")]
fn setup_app_user_model_id() {
    use windows::core::{w, PCWSTR};
    use windows::Win32::System::Registry::{RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ};
    use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;

    const AUMID: &str = "com.textplasticine.desktop";

    unsafe {
        let id: Vec<u16> = AUMID
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let _ = SetCurrentProcessExplicitAppUserModelID(PCWSTR(id.as_ptr()));

        let subkey: Vec<u16> = format!("Software\\Classes\\AppUserModelId\\{AUMID}")
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let name: Vec<u16> = "TextPlasticine"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let _ = RegSetKeyValueW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            PCWSTR::null(),
            REG_SZ.0,
            Some(name.as_ptr() as *const std::ffi::c_void),
            (name.len() * 2) as u32,
        );

        if let Ok(exe) = std::env::current_exe() {
            let icon: Vec<u16> = format!("{},0", exe.to_string_lossy())
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let _ = RegSetKeyValueW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey.as_ptr()),
                w!("IconUri"),
                REG_SZ.0,
                Some(icon.as_ptr() as *const std::ffi::c_void),
                (icon.len() * 2) as u32,
            );
        }
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        let state = app.state::<AppState>();
                        if *shortcut == *state.hotkey.lock().unwrap() {
                            toggle_main(app);
                        } else if *shortcut == *state.mark_hotkey.lock().unwrap() {
                            if is_main_visible(app) {
                                mark_frame_inner(app);
                            }
                        } else if *shortcut == *state.restore_hotkey.lock().unwrap() {
                            if is_main_visible(app) {
                                restore_frame_inner(app);
                            }
                        }
                    }
                })
                .build(),
        )
        .setup(|app| {
            #[cfg(target_os = "windows")]
            setup_app_user_model_id();
            let config_path = app.path().app_config_dir()?.join("config.json");
            let cfg = load_config(&config_path);
            let hotkey = parse_shortcut_or(&cfg.hotkey, "ctrl+space");
            let mark_hotkey = parse_shortcut_or(&cfg.mark_hotkey, "alt+m");
            let restore_hotkey = parse_shortcut_or(&cfg.restore_hotkey, "alt+`");
            app.global_shortcut().register(hotkey)?;
            app.global_shortcut().register(mark_hotkey)?;
            app.global_shortcut().register(restore_hotkey)?;
            #[cfg(target_os = "windows")]
            if cfg.autostart {
                set_autostart_registry(true);
            }
            app.manage(AppState {
                hotkey: Mutex::new(hotkey),
                mark_hotkey: Mutex::new(mark_hotkey),
                restore_hotkey: Mutex::new(restore_hotkey),
                dark_mode: Mutex::new(cfg.dark_mode),
                silent_start: Mutex::new(cfg.silent_start),
                autostart: Mutex::new(cfg.autostart),
                marked_rect: Mutex::new(cfg.marked_rect),
                config_path,
            });

            let toggle = MenuItemBuilder::with_id("toggle", "显示/隐藏").build(app)?;
            let settings = MenuItemBuilder::with_id("settings", "设置").build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "退出").build(app)?;
            let sep1 = PredefinedMenuItem::separator(app)?;
            let sep2 = PredefinedMenuItem::separator(app)?;
            let menu = MenuBuilder::new(app)
                .items(&[&toggle, &sep1, &settings, &sep2, &quit])
                .build()?;

            TrayIconBuilder::with_id("tray")
                .icon(tray_icon_image())
                .tooltip("TextPlasticine")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "toggle" => toggle_main(app),
                    "settings" => open_settings(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_main(tray.app_handle());
                    }
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| match window.label() {
            "main" => {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
            "settings" => {
                if let tauri::WindowEvent::Destroyed = event {
                    register_all(window.app_handle());
                }
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            get_shortcut,
            set_shortcut,
            get_dark_mode,
            set_dark_mode,
            get_silent_start,
            set_silent_start,
            get_autostart,
            set_autostart,
            get_startup_config,
            suspend_hotkeys,
            restore_hotkeys,
            set_default_rect,
            start_corner_resize
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
