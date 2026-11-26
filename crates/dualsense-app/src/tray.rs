use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::JoinHandle;

use dualsense_core::DualSenseManager;
use tokio::sync::watch;

/// Opens the DualSense Studio UI as a standalone Desktop App window (Edge/Chrome `--app` mode),
/// falling back to the default system browser if no Chromium runtime is found.
pub fn open_browser(url: &str) {
    #[cfg(target_os = "windows")]
    {
        let app_arg = format!("--app={url}");
        let mut candidates: Vec<PathBuf> = Vec::new();
        for env_key in ["ProgramFiles(x86)", "ProgramFiles", "LocalAppData"] {
            if let Ok(base) = std::env::var(env_key) {
                let p = PathBuf::from(&base);
                candidates.push(p.join(r"Microsoft\Edge\Application\msedge.exe"));
                candidates.push(p.join(r"Google\Chrome\Application\chrome.exe"));
                candidates.push(p.join(r"BraveSoftware\Brave-Browser\Application\brave.exe"));
            }
        }
        for bin in candidates {
            if bin.is_file()
                && std::process::Command::new(&bin)
                    .args([&app_arg, "--window-size=1280,840"])
                    .spawn()
                    .is_ok()
            {
                return;
            }
        }
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let app_arg = format!("--app={url}");
        for bin in [
            "google-chrome",
            "chromium",
            "chromium-browser",
            "microsoft-edge",
        ] {
            if std::process::Command::new(bin)
                .args([&app_arg, "--window-size=1280,840"])
                .spawn()
                .is_ok()
            {
                return;
            }
        }
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
}

/// Hides the attached Windows console window when running in `--tray` background mode.
pub fn hide_console_window_if_windows() {
    #[cfg(target_os = "windows")]
    windows_tray::hide_console_window();
}

/// Formats the system tray tooltip text (kept under 127 chars for Win32 `NOTIFYICONDATAW`).
pub fn format_tray_tooltip(connected: bool, controller_name: &str, api_addr: SocketAddr) -> String {
    let status = if connected {
        controller_name
    } else {
        "Disconnected"
    };
    let full = format!("DualSense Studio ({status}) - http://{api_addr}");
    if full.chars().count() <= 120 {
        full
    } else {
        let mut truncated: String = full.chars().take(117).collect();
        truncated.push_str("...");
        truncated
    }
}

/// Generates a 32x32 BGRA icon buffer depicting a DualSense-inspired badge (blue gradient + white pad).
pub fn build_tray_icon_bgra() -> Vec<u8> {
    const SIZE: usize = 32;
    let mut buf = vec![0u8; SIZE * SIZE * 4];
    let center = 15.5f32;

    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();
            let idx = (y * SIZE + x) * 4;

            if dist <= 14.5 {
                // Outer circle: PlayStation Cobalt Blue (#00439C -> #0070D1)
                let t = (y as f32 / SIZE as f32).clamp(0.0, 1.0);
                let mut r = (0.0 + 18.0 * (1.0 - t)) as u8;
                let mut g = (67.0 + 45.0 * (1.0 - t)) as u8;
                let mut b = (156.0 + 68.0 * (1.0 - t)) as u8;
                let a = if dist > 13.5 {
                    ((14.5 - dist) * 255.0).clamp(0.0, 255.0) as u8
                } else {
                    255
                };

                // Stylized DualSense lightbar V-arc & grips in white
                let in_left_grip = (7..=12).contains(&x) && (12..=22).contains(&y);
                let in_right_grip = (19..=24).contains(&x) && (12..=22).contains(&y);
                let in_bridge = (11..=20).contains(&x) && (12..=18).contains(&y);
                let in_lightbar = (12..=19).contains(&x) && (10..=11).contains(&y);

                if in_lightbar {
                    // Cyan lightbar accent
                    r = 56;
                    g = 210;
                    b = 255;
                } else if in_left_grip || in_right_grip || in_bridge {
                    r = 244;
                    g = 247;
                    b = 252;
                }

                // Win32 DIB / CreateBitmap 32bpp expects BGRA
                buf[idx] = b;
                buf[idx + 1] = g;
                buf[idx + 2] = r;
                buf[idx + 3] = a;
            }
        }
    }

    buf
}

/// Handle to the running background system tray thread.
pub struct TrayHandle {
    #[cfg(target_os = "windows")]
    hwnd_raw: Arc<std::sync::atomic::AtomicIsize>,
    thread: Option<JoinHandle<()>>,
}

impl TrayHandle {
    pub fn start(
        manager: Arc<DualSenseManager>,
        api_addr: SocketAddr,
        config_path: PathBuf,
        shutdown_tx: watch::Sender<bool>,
    ) -> Self {
        #[cfg(target_os = "windows")]
        {
            let hwnd_raw = Arc::new(std::sync::atomic::AtomicIsize::new(0));
            let hwnd_ref = hwnd_raw.clone();
            let thread = std::thread::Builder::new()
                .name("dualsense-tray".to_string())
                .spawn(move || {
                    windows_tray::run_tray_loop(
                        manager,
                        api_addr,
                        config_path,
                        shutdown_tx,
                        hwnd_ref,
                    );
                })
                .ok();
            Self { hwnd_raw, thread }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = (manager, api_addr, config_path, shutdown_tx);
            if std::env::var_os("DISPLAY").is_some()
                || std::env::var_os("WAYLAND_DISPLAY").is_some()
            {
                tracing::info!(
                    "System tray UI is available via web dashboard at http://{}",
                    api_addr
                );
            } else {
                tracing::debug!("Headless Linux session detected; skipping desktop tray icon");
            }
            Self { thread: None }
        }
    }

    pub fn stop(&mut self) {
        #[cfg(target_os = "windows")]
        {
            let raw = self.hwnd_raw.load(std::sync::atomic::Ordering::SeqCst);
            if raw != 0 {
                windows_tray::post_close(raw);
            }
        }
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for TrayHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(target_os = "windows")]
mod windows_tray {
    use std::ffi::c_void;
    use std::net::SocketAddr;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicIsize, Ordering};
    use std::sync::{Arc, OnceLock};

    use dualsense_core::DualSenseManager;
    use tokio::sync::watch;
    use tracing::{info, warn};

    use super::{build_tray_icon_bgra, format_tray_tooltip, open_browser};

    type HWND = isize;
    type HMENU = isize;
    type HICON = isize;
    type HBITMAP = isize;
    type HINSTANCE = isize;
    type WPARAM = usize;
    type LPARAM = isize;
    type LRESULT = isize;
    type UINT = u32;
    type DWORD = u32;
    type BOOL = i32;

    const WM_DESTROY: UINT = 0x0002;
    const WM_CLOSE: UINT = 0x0010;
    const WM_COMMAND: UINT = 0x0111;
    const WM_TIMER: UINT = 0x0113;
    const WM_LBUTTONUP: UINT = 0x0202;
    const WM_LBUTTONDBLCLK: UINT = 0x0203;
    const WM_RBUTTONUP: UINT = 0x0205;
    const WM_APP: UINT = 0x8000;
    const WM_TRAYICON: UINT = WM_APP + 1;

    const NIM_ADD: DWORD = 0x0000_0000;
    const NIM_MODIFY: DWORD = 0x0000_0001;
    const NIM_DELETE: DWORD = 0x0000_0002;
    const NIF_MESSAGE: UINT = 0x0000_0001;
    const NIF_ICON: UINT = 0x0000_0002;
    const NIF_TIP: UINT = 0x0000_0004;

    const MF_STRING: UINT = 0x0000;
    const MF_GRAYED: UINT = 0x0001;
    const MF_CHECKED: UINT = 0x0008;
    const MF_SEPARATOR: UINT = 0x0800;

    const TPM_LEFTALIGN: UINT = 0x0000;
    const TPM_BOTTOMALIGN: UINT = 0x0020;
    const TPM_RIGHTBUTTON: UINT = 0x0002;
    const SW_HIDE: i32 = 0;

    const ID_OPEN_UI: usize = 1001;
    const ID_STATUS: usize = 1002;
    const ID_PROFILE_CLASSIC: usize = 1010;
    const ID_PROFILE_FPS: usize = 1011;
    const ID_PROFILE_RACING: usize = 1012;
    const ID_PROFILE_FLIGHT: usize = 1013;
    const ID_TOGGLE_MIC: usize = 1020;
    const ID_TOGGLE_UDP: usize = 1021;
    const ID_TOGGLE_CEMUHOOK: usize = 1022;
    const ID_TOGGLE_AUTOSTART: usize = 1023;
    const ID_CHECK_UPDATES: usize = 1030;
    const ID_QUIT: usize = 1099;

    const TIMER_REFRESH_ID: usize = 1;

    #[repr(C)]
    struct WNDCLASSW {
        style: UINT,
        lpfn_wnd_proc: unsafe extern "system" fn(HWND, UINT, WPARAM, LPARAM) -> LRESULT,
        cb_cls_extra: i32,
        cb_wnd_extra: i32,
        h_instance: HINSTANCE,
        h_icon: HICON,
        h_cursor: isize,
        hbr_background: isize,
        lpsz_menu_name: *const u16,
        lpsz_class_name: *const u16,
    }

    #[repr(C)]
    struct POINT {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    struct MSG {
        hwnd: HWND,
        message: UINT,
        w_param: WPARAM,
        l_param: LPARAM,
        time: DWORD,
        pt: POINT,
        l_private: DWORD,
    }

    #[repr(C)]
    struct NOTIFYICONDATAW {
        cb_size: DWORD,
        h_wnd: HWND,
        u_id: UINT,
        u_flags: UINT,
        u_callback_message: UINT,
        h_icon: HICON,
        sz_tip: [u16; 128],
    }

    #[repr(C)]
    struct ICONINFO {
        f_icon: BOOL,
        x_hotspot: DWORD,
        y_hotspot: DWORD,
        hbm_mask: HBITMAP,
        hbm_color: HBITMAP,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleHandleW(lp_module_name: *const u16) -> HINSTANCE;
        fn GetConsoleWindow() -> HWND;
    }

    #[link(name = "user32")]
    extern "system" {
        fn RegisterClassW(lp_wnd_class: *const WNDCLASSW) -> u16;
        fn CreateWindowExW(
            dw_ex_style: DWORD,
            lp_class_name: *const u16,
            lp_window_name: *const u16,
            dw_style: DWORD,
            x: i32,
            y: i32,
            n_width: i32,
            n_height: i32,
            h_wnd_parent: HWND,
            h_menu: HMENU,
            h_instance: HINSTANCE,
            lp_param: *const c_void,
        ) -> HWND;
        fn DefWindowProcW(hwnd: HWND, msg: UINT, w_param: WPARAM, l_param: LPARAM) -> LRESULT;
        fn GetMessageW(lp_msg: *mut MSG, hwnd: HWND, min: UINT, max: UINT) -> BOOL;
        fn TranslateMessage(lp_msg: *const MSG) -> BOOL;
        fn DispatchMessageW(lp_msg: *const MSG) -> LRESULT;
        fn PostMessageW(hwnd: HWND, msg: UINT, w_param: WPARAM, l_param: LPARAM) -> BOOL;
        fn PostQuitMessage(exit_code: i32);
        fn DestroyWindow(hwnd: HWND) -> BOOL;
        fn ShowWindow(hwnd: HWND, n_cmd_show: i32) -> BOOL;
        fn SetForegroundWindow(hwnd: HWND) -> BOOL;
        fn GetCursorPos(lp_point: *mut POINT) -> BOOL;
        fn CreatePopupMenu() -> HMENU;
        fn AppendMenuW(
            h_menu: HMENU,
            u_flags: UINT,
            u_id_new_item: usize,
            lp_new_item: *const u16,
        ) -> BOOL;
        fn TrackPopupMenu(
            h_menu: HMENU,
            u_flags: UINT,
            x: i32,
            y: i32,
            n_reserved: i32,
            hwnd: HWND,
            prc_rect: *const c_void,
        ) -> BOOL;
        fn DestroyMenu(h_menu: HMENU) -> BOOL;
        fn CreateIconIndirect(piconinfo: *const ICONINFO) -> HICON;
        fn DestroyIcon(h_icon: HICON) -> BOOL;
        fn SetTimer(
            hwnd: HWND,
            n_id_event: usize,
            u_elapse: UINT,
            lp_timer_func: *const c_void,
        ) -> usize;
        fn KillTimer(hwnd: HWND, u_id_event: usize) -> BOOL;
        fn SetWindowLongPtrW(hwnd: HWND, n_index: i32, dw_new_long: isize) -> isize;
        fn GetWindowLongPtrW(hwnd: HWND, n_index: i32) -> isize;
        fn MessageBoxW(
            hwnd: HWND,
            lp_text: *const u16,
            lp_caption: *const u16,
            u_type: UINT,
        ) -> i32;
    }

    const MB_OK: UINT = 0x0000_0000;
    const MB_YESNO: UINT = 0x0000_0004;
    const MB_ICONINFORMATION: UINT = 0x0000_0040;
    const MB_ICONWARNING: UINT = 0x0000_0030;
    const IDYES: i32 = 6;

    #[link(name = "gdi32")]
    extern "system" {
        fn CreateBitmap(
            n_width: i32,
            n_height: i32,
            n_planes: UINT,
            n_bit_count: UINT,
            lp_bits: *const c_void,
        ) -> HBITMAP;
        fn DeleteObject(ho: isize) -> BOOL;
    }

    #[link(name = "shell32")]
    extern "system" {
        fn Shell_NotifyIconW(dw_message: DWORD, lp_data: *const NOTIFYICONDATAW) -> BOOL;
    }

    const GWLP_USERDATA: i32 = -21;

    struct TrayContext {
        manager: Arc<DualSenseManager>,
        api_addr: SocketAddr,
        config_path: PathBuf,
        shutdown_tx: watch::Sender<bool>,
        runtime: tokio::runtime::Runtime,
        icon: HICON,
    }

    fn to_wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn fill_tip(buf: &mut [u16; 128], text: &str) {
        buf.fill(0);
        for (i, unit) in text.encode_utf16().take(127).enumerate() {
            buf[i] = unit;
        }
    }

    pub fn hide_console_window() {
        unsafe {
            let hwnd = GetConsoleWindow();
            if hwnd != 0 {
                ShowWindow(hwnd, SW_HIDE);
            }
        }
    }

    pub fn post_close(hwnd: HWND) {
        unsafe {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
    }

    fn create_dualsense_hicon() -> HICON {
        let bgra = build_tray_icon_bgra();
        let mask = vec![0u8; 32 * 32 / 8];
        unsafe {
            let hbm_color = CreateBitmap(32, 32, 1, 32, bgra.as_ptr() as *const c_void);
            let hbm_mask = CreateBitmap(32, 32, 1, 1, mask.as_ptr() as *const c_void);
            let info = ICONINFO {
                f_icon: 1,
                x_hotspot: 0,
                y_hotspot: 0,
                hbm_mask,
                hbm_color,
            };
            let icon = CreateIconIndirect(&info);
            if hbm_color != 0 {
                DeleteObject(hbm_color);
            }
            if hbm_mask != 0 {
                DeleteObject(hbm_mask);
            }
            icon
        }
    }

    fn set_windows_autostart(enabled: bool) {
        let exe = match std::env::current_exe() {
            Ok(p) => p,
            Err(_) => return,
        };
        let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
        if enabled {
            let cmd = format!("\"{}\" --tray", exe.display());
            let _ = std::process::Command::new("reg")
                .args([
                    "add",
                    key,
                    "/v",
                    "DualSenseStudio",
                    "/t",
                    "REG_SZ",
                    "/d",
                    &cmd,
                    "/f",
                ])
                .output();
        } else {
            let _ = std::process::Command::new("reg")
                .args(["delete", key, "/v", "DualSenseStudio", "/f"])
                .output();
        }
    }

    fn persist_config(ctx: &TrayContext) {
        let cfg = ctx.runtime.block_on(ctx.manager.export_config());
        if let Err(e) = cfg.save_to_path(&ctx.config_path) {
            warn!("Tray failed to persist config: {}", e);
        }
    }

    fn show_context_menu(hwnd: HWND, ctx: &TrayContext) {
        let status = ctx.runtime.block_on(ctx.manager.get_status());
        let status_label = if status.connected {
            format!("Controller: {}", status.name)
        } else {
            "Controller: Disconnected".to_string()
        };

        unsafe {
            let menu = CreatePopupMenu();
            if menu == 0 {
                return;
            }

            let open_txt = to_wide(&format!("Open DualSense Studio (http://{})", ctx.api_addr));
            let status_txt = to_wide(&status_label);
            let p_classic = to_wide("Profile: PlayStation Classic");
            let p_fps = to_wide("Profile: FPS Tactical Shooter");
            let p_racing = to_wide("Profile: Racing (ABS)");
            let p_flight = to_wide("Profile: Flight Simulator");
            let mic_txt = to_wide("Mute DualSense Microphone");
            let udp_txt = to_wide("DualSenseX UDP Link Server");
            let cemu_txt = to_wide("Cemuhook Motion Server (DSU)");
            let auto_txt = to_wide("Start with Windows (Tray)");
            let update_txt = to_wide("Check for Updates...");
            let quit_txt = to_wide("Quit DualSense Studio");

            AppendMenuW(menu, MF_STRING, ID_OPEN_UI, open_txt.as_ptr());
            AppendMenuW(menu, MF_STRING | MF_GRAYED, ID_STATUS, status_txt.as_ptr());
            AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());

            AppendMenuW(menu, MF_STRING, ID_PROFILE_CLASSIC, p_classic.as_ptr());
            AppendMenuW(menu, MF_STRING, ID_PROFILE_FPS, p_fps.as_ptr());
            AppendMenuW(menu, MF_STRING, ID_PROFILE_RACING, p_racing.as_ptr());
            AppendMenuW(menu, MF_STRING, ID_PROFILE_FLIGHT, p_flight.as_ptr());
            AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());

            let mic_flags = if status.mic_muted {
                MF_STRING | MF_CHECKED
            } else {
                MF_STRING
            };
            let udp_flags = if status.udp_stats.running {
                MF_STRING | MF_CHECKED
            } else {
                MF_STRING
            };
            let cemu_flags = if status.cemuhook_stats.running {
                MF_STRING | MF_CHECKED
            } else {
                MF_STRING
            };
            let auto_flags = if status.autostart_on_launch {
                MF_STRING | MF_CHECKED
            } else {
                MF_STRING
            };

            AppendMenuW(menu, mic_flags, ID_TOGGLE_MIC, mic_txt.as_ptr());
            AppendMenuW(menu, udp_flags, ID_TOGGLE_UDP, udp_txt.as_ptr());
            AppendMenuW(menu, cemu_flags, ID_TOGGLE_CEMUHOOK, cemu_txt.as_ptr());
            AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
            AppendMenuW(menu, auto_flags, ID_TOGGLE_AUTOSTART, auto_txt.as_ptr());
            AppendMenuW(menu, MF_STRING, ID_CHECK_UPDATES, update_txt.as_ptr());
            AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
            AppendMenuW(menu, MF_STRING, ID_QUIT, quit_txt.as_ptr());

            let mut pt = POINT { x: 0, y: 0 };
            GetCursorPos(&mut pt);
            SetForegroundWindow(hwnd);
            TrackPopupMenu(
                menu,
                TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_RIGHTBUTTON,
                pt.x,
                pt.y,
                0,
                hwnd,
                std::ptr::null(),
            );
            DestroyMenu(menu);
        }
    }

    fn handle_command(hwnd: HWND, cmd_id: usize, ctx: &TrayContext) {
        match cmd_id {
            ID_OPEN_UI => open_browser(&format!("http://{}", ctx.api_addr)),
            ID_PROFILE_CLASSIC => {
                ctx.manager.apply_profile("ps_classic");
                persist_config(ctx);
            }
            ID_PROFILE_FPS => {
                ctx.manager.apply_profile("fps_tactical");
                persist_config(ctx);
            }
            ID_PROFILE_RACING => {
                ctx.manager.apply_profile("racing_abs");
                persist_config(ctx);
            }
            ID_PROFILE_FLIGHT => {
                ctx.manager.apply_profile("flight_sim");
                persist_config(ctx);
            }
            ID_TOGGLE_MIC => {
                ctx.manager.toggle_microphone();
            }
            ID_TOGGLE_UDP => {
                let running = ctx
                    .runtime
                    .block_on(ctx.manager.get_status())
                    .udp_stats
                    .running;
                if running {
                    ctx.runtime.block_on(ctx.manager.stop_udp());
                } else {
                    let _ = ctx.runtime.block_on(ctx.manager.start_udp());
                }
                persist_config(ctx);
            }
            ID_TOGGLE_CEMUHOOK => {
                let running = ctx
                    .runtime
                    .block_on(ctx.manager.get_status())
                    .cemuhook_stats
                    .running;
                if running {
                    ctx.runtime.block_on(ctx.manager.stop_cemuhook());
                } else {
                    let _ = ctx.runtime.block_on(ctx.manager.start_cemuhook());
                }
                persist_config(ctx);
            }
            ID_TOGGLE_AUTOSTART => {
                let next = !ctx.manager.is_autostart_enabled();
                ctx.manager.set_autostart_enabled(next);
                set_windows_autostart(next);
                persist_config(ctx);
            }
            ID_CHECK_UPDATES => {
                let title = to_wide("DualSense Studio - Updates");
                match ctx.runtime.block_on(crate::updater::check_for_updates()) {
                    Ok(info) => {
                        if info.update_available {
                            let msg = to_wide(&format!(
                                "New version {} is available (current: {}).\n\nDownload and install automatically now?",
                                info.latest_version, info.current_version
                            ));
                            let choice = unsafe {
                                MessageBoxW(
                                    hwnd,
                                    msg.as_ptr(),
                                    title.as_ptr(),
                                    MB_YESNO | MB_ICONINFORMATION,
                                )
                            };
                            if choice == IDYES {
                                if let Err(e) = ctx.runtime.block_on(crate::updater::apply_update(
                                    &info,
                                    Some(&ctx.shutdown_tx),
                                )) {
                                    let err_msg = to_wide(&format!("Update failed: {e}"));
                                    unsafe {
                                        MessageBoxW(
                                            hwnd,
                                            err_msg.as_ptr(),
                                            title.as_ptr(),
                                            MB_OK | MB_ICONWARNING,
                                        );
                                    }
                                }
                            }
                        } else {
                            let msg = to_wide(&format!(
                                "DualSense Studio (v{}) is up to date.",
                                info.current_version
                            ));
                            unsafe {
                                MessageBoxW(
                                    hwnd,
                                    msg.as_ptr(),
                                    title.as_ptr(),
                                    MB_OK | MB_ICONINFORMATION,
                                );
                            }
                        }
                    }
                    Err(e) => {
                        let msg = to_wide(&format!("Could not check for updates: {e}"));
                        unsafe {
                            MessageBoxW(hwnd, msg.as_ptr(), title.as_ptr(), MB_OK | MB_ICONWARNING);
                        }
                    }
                }
            }
            ID_QUIT => {
                let _ = ctx.shutdown_tx.send(true);
                unsafe {
                    PostMessageW(hwnd, WM_CLOSE, 0, 0);
                }
            }
            _ => {}
        }
    }

    fn update_tooltip(hwnd: HWND, ctx: &TrayContext) {
        let tip_str = format_tray_tooltip(
            ctx.manager.is_connected(),
            &ctx.manager.controller_name(),
            ctx.api_addr,
        );
        let mut nid = NOTIFYICONDATAW {
            cb_size: std::mem::size_of::<NOTIFYICONDATAW>() as DWORD,
            h_wnd: hwnd,
            u_id: 1,
            u_flags: NIF_TIP,
            u_callback_message: WM_TRAYICON,
            h_icon: ctx.icon,
            sz_tip: [0; 128],
        };
        fill_tip(&mut nid.sz_tip, &tip_str);
        unsafe {
            Shell_NotifyIconW(NIM_MODIFY, &nid);
        }
    }

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: UINT,
        w_param: WPARAM,
        l_param: LPARAM,
    ) -> LRESULT {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const TrayContext;
        match msg {
            WM_TRAYICON => {
                let event = (l_param as u32) & 0xFFFF;
                if !ptr.is_null() {
                    let ctx = &*ptr;
                    if event == WM_LBUTTONDBLCLK || event == WM_LBUTTONUP {
                        open_browser(&format!("http://{}", ctx.api_addr));
                    } else if event == WM_RBUTTONUP {
                        show_context_menu(hwnd, ctx);
                    }
                }
                0
            }
            WM_COMMAND => {
                if !ptr.is_null() {
                    let ctx = &*ptr;
                    handle_command(hwnd, w_param & 0xFFFF, ctx);
                }
                0
            }
            WM_TIMER => {
                if !ptr.is_null() && w_param == TIMER_REFRESH_ID {
                    update_tooltip(hwnd, &*ptr);
                }
                0
            }
            WM_CLOSE => {
                DestroyWindow(hwnd);
                0
            }
            WM_DESTROY => {
                KillTimer(hwnd, TIMER_REFRESH_ID);
                let nid = NOTIFYICONDATAW {
                    cb_size: std::mem::size_of::<NOTIFYICONDATAW>() as DWORD,
                    h_wnd: hwnd,
                    u_id: 1,
                    u_flags: 0,
                    u_callback_message: 0,
                    h_icon: 0,
                    sz_tip: [0; 128],
                };
                Shell_NotifyIconW(NIM_DELETE, &nid);
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, w_param, l_param),
        }
    }

    pub fn run_tray_loop(
        manager: Arc<DualSenseManager>,
        api_addr: SocketAddr,
        config_path: PathBuf,
        shutdown_tx: watch::Sender<bool>,
        hwnd_out: Arc<AtomicIsize>,
    ) {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                warn!("Failed to initialize tray runtime: {}", e);
                return;
            }
        };

        let icon = create_dualsense_hicon();
        let ctx = Box::new(TrayContext {
            manager,
            api_addr,
            config_path,
            shutdown_tx,
            runtime,
            icon,
        });
        let ctx_ptr = Box::into_raw(ctx);

        static REGISTERED: OnceLock<bool> = OnceLock::new();
        let class_name = to_wide("DualSenseStudioTrayClass");
        let window_name = to_wide("DualSense Studio Tray");

        unsafe {
            let h_instance = GetModuleHandleW(std::ptr::null());
            REGISTERED.get_or_init(|| {
                let wc = WNDCLASSW {
                    style: 0,
                    lpfn_wnd_proc: wnd_proc,
                    cb_cls_extra: 0,
                    cb_wnd_extra: 0,
                    h_instance,
                    h_icon: 0,
                    h_cursor: 0,
                    hbr_background: 0,
                    lpsz_menu_name: std::ptr::null(),
                    lpsz_class_name: class_name.as_ptr(),
                };
                RegisterClassW(&wc) != 0
            });

            let hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                window_name.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                h_instance,
                std::ptr::null(),
            );

            if hwnd == 0 {
                warn!("Could not create Win32 system tray window");
                if icon != 0 {
                    DestroyIcon(icon);
                }
                drop(Box::from_raw(ctx_ptr));
                return;
            }

            SetWindowLongPtrW(hwnd, GWLP_USERDATA, ctx_ptr as isize);
            hwnd_out.store(hwnd, Ordering::SeqCst);

            let tip_str = format_tray_tooltip(false, "Disconnected", api_addr);
            let mut nid = NOTIFYICONDATAW {
                cb_size: std::mem::size_of::<NOTIFYICONDATAW>() as DWORD,
                h_wnd: hwnd,
                u_id: 1,
                u_flags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
                u_callback_message: WM_TRAYICON,
                h_icon: icon,
                sz_tip: [0; 128],
            };
            fill_tip(&mut nid.sz_tip, &tip_str);
            if Shell_NotifyIconW(NIM_ADD, &nid) == 0 {
                warn!("Shell_NotifyIconW failed to add system tray icon");
            } else {
                info!("Windows system tray icon active for http://{}", api_addr);
            }

            SetTimer(hwnd, TIMER_REFRESH_ID, 2000, std::ptr::null());

            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, 0, 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            hwnd_out.store(0, Ordering::SeqCst);
            if icon != 0 {
                DestroyIcon(icon);
            }
            drop(Box::from_raw(ctx_ptr));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tray_icon_bgra_dimensions_and_non_empty() {
        let icon = build_tray_icon_bgra();
        assert_eq!(icon.len(), 32 * 32 * 4);
        let opaque_pixels = icon
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|px| px[3] > 0)
            .count();
        assert!(opaque_pixels > 400);
    }

    #[test]
    fn test_tray_tooltip_formatting_and_truncation() {
        let addr: SocketAddr = "127.0.0.1:48080".parse().unwrap();
        let short = format_tray_tooltip(true, "Sony DualSense", addr);
        assert!(short.contains("Sony DualSense"));
        assert!(short.contains("http://127.0.0.1:48080"));
        assert!(short.len() <= 120);

        let long_name = "A".repeat(200);
        let truncated = format_tray_tooltip(true, &long_name, addr);
        assert_eq!(truncated.chars().count(), 120);
        assert!(truncated.ends_with("..."));
    }
}
