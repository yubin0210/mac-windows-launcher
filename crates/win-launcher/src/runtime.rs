//! Production startup and native resource ownership. No mock fallback.
use crate::{
    app_catalog::{AppCatalog, AppRecord}, diagnostics, display, dock_menu::{self, DockAction},
    dock_session::DockSession, icon_cache::native::IconLoader, settings::SettingsStore,
    AppInfo, AppTracker, Direct2DRenderer, DirectCompositionPipeline, DockView,
    DockWindow, DockWindowConfig, LaunchResult, ProcessLauncher, WindowEvent,
};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, HWND, ERROR_ALREADY_EXISTS};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::WindowsAndMessaging::*;

struct ComApartment;
impl Drop for ComApartment {
    fn drop(&mut self) { unsafe { CoUninitialize(); } }
}
struct Instance(HANDLE);
impl Drop for Instance {
    fn drop(&mut self) { unsafe { let _ = CloseHandle(self.0); } }
}
fn notify(hwnd: Option<HWND>, text: &str) {
    diagnostics::write(text);
    let text: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    if std::env::args().any(|arg| arg == "--smoke-test") { return; }
    unsafe { MessageBoxW(hwnd.unwrap_or_default(), PCWSTR(text.as_ptr()), w!("MacLauncher"), MB_OK | MB_ICONERROR); }
}
pub fn entry() {
    if let Err(error) = run() {
        notify(None, &format!("MacLauncher could not continue:\n{error}"));
        std::process::exit(1);
    }
}

pub fn run() -> Result<(), String> {
    let handle = unsafe { CreateMutexW(None, false, w!("Local\\MacLauncher.Runtime")) }.map_err(|e| e.to_string())?;
    let already_running = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    let _instance = Instance(handle);
    if already_running { return Ok(()); }
    let local = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is unavailable")?;
    let data_directory = PathBuf::from(local).join("MacLauncher");
    let smoke = std::env::args().any(|arg| arg == "--smoke-test");
    diagnostics::initialize(&data_directory.join("logs")).map_err(|e| format!("Cannot open log in {}: {e}", data_directory.display()))?;
    diagnostics::write(&format!("Starting MacLauncher {}", crate::version()));
    if std::env::args().any(|arg| arg == "--hide-taskbar") {
        return Err("--hide-taskbar is unavailable during the rebuild. Windows taskbar remains visible.".into());
    }
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) }.map_err(|e| format!("Cannot set DPI awareness: {e}"))?;
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.ok().map_err(|e| format!("COM initialization: {e}"))?;
    let _com = ComApartment;
    let store = SettingsStore::new(data_directory.join("settings.json"));
    let loaded = store.load().map_err(|e| format!("Cannot load settings: {e}"))?;
    if let Some(backup) = loaded.backup {
        notify(None, &format!("Settings were damaged. A copy was preserved at {}. Default pins will be used.", backup.display()));
    }
    let first_run = loaded.settings.pinned.is_none();
    let mut session = DockSession::new(AppCatalog::discover(), loaded.settings);
    if first_run { store.save(&session.settings).map_err(|e| format!("Cannot initialize settings: {e}"))?; }
    diagnostics::write(&format!("Catalog: {} applications", session.catalog.apps().len()));
    let smoke_menu = session.settings.menu_bar_enabled;
    let smoke_stage = session.settings.stage_enabled;
    let mut tracker = AppTracker::new();
    tracker.refresh_running_apps();
    session.refresh(&tracker);
    let mut window = DockWindow::create(DockWindowConfig::default())?;
    let mut placement = display::configured(window.hwnd(), &session.settings)?;
    window.place(&placement)?;
    // Composition and icon resources drop before their destination HWND and COM apartment.
    let pipeline = DirectCompositionPipeline::new(window.hwnd()).map_err(|e| format!("Composition initialization: {e}"))?;
    let mut renderer = Direct2DRenderer::with_pipeline(&pipeline, placement.width as u32, placement.height as u32).map_err(|e| e.to_string())?;
    let tray = crate::native_tray::TrayIcon::new(window.hwnd())?;
    let mut spotlight = crate::spotlight_controller::SpotlightController::new(&pipeline, &session.catalog)?;
    if let Err(error) = spotlight.set_hotkey(session.settings.spotlight_hotkey) { notify(Some(window.hwnd()), &error); }
    let mut stage: Option<crate::stage_native::NativeStage> = None;
    let mut menu_bar: Option<crate::menu_bar_native::NativeMenuBar> = None;
    let mut icons = IconLoader::new();
    let mut visible = session.visible(placement.capacity);
    let mut dock = DockView::with_config(visible.iter().map(AppRecord::dock_item).collect(), placement.view_config());
    update_status(&mut dock, &visible, &tracker);
    let mut last_poll = Instant::now();
    let mut last_frame = Instant::now();
    let mut previous = Vec::new();
    let smoke_start = Instant::now();
    let mut smoke_draws = 0u64;
    while window.is_open() {
        if smoke && !std::env::args().any(|arg| arg == "--wait-for-quit") && smoke_start.elapsed() >= Duration::from_secs(5) { break; }
        let now = Instant::now();
        let dt = now.duration_since(last_frame).as_secs_f32().min(0.033);
        last_frame = now;
        let mut force = false;
        let mut rebuild = false;
        for event in window.poll_events() {
            let result = match event {
                WindowEvent::MouseMove { x, y } => { dock.on_mouse_move(x, y); Ok(()) }
                WindowEvent::MouseLeave => { dock.on_mouse_leave(); Ok(()) }
                WindowEvent::MouseClick { x, y } => {
                    if let Some(index) = dock.on_mouse_click(x, y) {
                        if let Some(app) = visible.get(index) { launch(window.hwnd(), app, &tracker) } else { Ok(()) }
                    } else { Ok(()) }
                }
                WindowEvent::ContextMenu { x, y } => {
                    let id = hit(&dock, x, y);
                    let windows = id.as_deref().and_then(|id| session.catalog.get(id)).map(|app| matching_windows(app, &tracker)).unwrap_or_default();
                    match dock_menu::context(window.hwnd(), id.as_deref(), &session.catalog, &session.settings, &windows) {
                        Ok(Some(action)) => {
                            rebuild = true;
                            apply(action, window.hwnd(), &mut session, &store, &tracker, placement.capacity, &mut spotlight)
                        }
                        Ok(None) => Ok(()), Err(error) => Err(error),
                    }
                }
                WindowEvent::Scroll(delta) => {
                    if delta != 0 { session.page_by(if delta < 0 { 1 } else { -1 }, placement.capacity); rebuild = true; }
                    Ok(())
                }
                WindowEvent::DisplayChanged => {
                    rebuild = true;
                    force = true;
                    Ok(())
                }
                WindowEvent::OpenSpotlight => { spotlight.show(); Ok(()) }
                WindowEvent::ShellRestarted => tray.readd(),
                WindowEvent::Paint => { force = true; Ok(()) }
                WindowEvent::Destroy => { window.close(); Ok(()) }
            };
            if let Err(error) = result { notify(Some(window.hwnd()), &error); }
        }
        if !window.is_open() { break; }
        if last_poll.elapsed() >= Duration::from_millis(500) || rebuild {
            if rebuild {
                let next_placement = display::configured(window.hwnd(), &session.settings)?;
                if next_placement != placement {
                    placement = next_placement;
                    window.place(&placement)?;
                    renderer.resize(&pipeline, placement.width as u32, placement.height as u32).map_err(|e| e.to_string())?;
                }
            }
            tracker.refresh_running_apps();
            session.refresh(&tracker);
            last_poll = now;
            let next = session.visible(placement.capacity);
            if next != visible || rebuild {
                visible = next;
                dock = DockView::with_config(visible.iter().map(AppRecord::dock_item).collect(), placement.view_config());
                renderer.retain_icons(&visible.iter().map(|app| app.id.clone()).collect::<Vec<_>>());
                icons.retain(&visible.iter().map(|app| (app.id.clone(), app.target.clone())).collect::<Vec<_>>());
                force = true;
            }
            update_status(&mut dock, &visible, &tracker);
        }
        if session.settings.menu_bar_enabled && menu_bar.is_none() {
            match crate::menu_bar_native::NativeMenuBar::new(&pipeline, window.hwnd()) {
                Ok(bar) => menu_bar = Some(bar),
                Err(error) => { session.settings.menu_bar_enabled = false; notify(Some(window.hwnd()), &error); }
            }
        } else if !session.settings.menu_bar_enabled { menu_bar = None; }
        if let Some(bar) = &mut menu_bar {
            match bar.frame(window.hwnd(), &session.catalog, &tracker) {
                Ok(commands) => for command in commands {
                    use crate::menu_bar_native::MenuCommand;
                    match command {
                        MenuCommand::Search => spotlight.show(),
                        MenuCommand::Hide => {
                            let mut settings = session.settings.clone(); settings.menu_bar_enabled = false;
                            match store.save(&settings) { Ok(()) => session.settings = settings, Err(error) => notify(Some(window.hwnd()), &format!("Cannot save menu bar setting: {error}")) }
                        }
                        MenuCommand::Exit => window.close(),
                        command => {
                            let target = match command {
                                MenuCommand::Settings => "ms-settings:", MenuCommand::Network => "ms-settings:network-status",
                                MenuCommand::Sound => "ms-settings:sound", MenuCommand::DateTime => "ms-settings:dateandtime",
                                MenuCommand::Power => "ms-settings:powersleep", MenuCommand::About => "ms-settings:about",
                                MenuCommand::TaskManager => "taskmgr.exe", MenuCommand::Logs => "", _ => unreachable!(),
                            };
                            let target = if target.is_empty() { data_directory.join("logs").to_string_lossy().to_string() } else { target.into() };
                            if let Err(error) = launch(window.hwnd(), &AppRecord::new(&target, &target, None), &tracker) { notify(Some(window.hwnd()), &error); }
                        }
                    }
                },
                Err(error) => { menu_bar = None; session.settings.menu_bar_enabled = false; notify(Some(window.hwnd()), &format!("Menu bar disabled: {error}")); }
            }
        }
        if !window.is_open() { break; }
        if session.settings.stage_enabled && stage.is_none() {
            match crate::stage_native::NativeStage::new(&pipeline) {
                Ok(shelf) => stage = Some(shelf),
                Err(error) => { session.settings.stage_enabled = false; notify(Some(window.hwnd()), &error); }
            }
        } else if !session.settings.stage_enabled { stage = None; }
        if let Some(shelf) = &mut stage {
            match shelf.frame(window.hwnd(), &session.catalog, &tracker, dt) {
                Ok(()) => if let Some(error) = shelf.take_error() { notify(Some(window.hwnd()), &error); },
                Err(error) => { stage = None; session.settings.stage_enabled = false; notify(Some(window.hwnd()), &format!("Stage Manager disabled: {error}")); }
            }
        }
        match spotlight.frame(&session.catalog) {
            Ok(Some(item)) => {
                let result = if item.category == mac_physics_core::spotlight::SpotlightCategory::Calculation {
                    crate::clipboard::copy(spotlight.hwnd(), &item.action_target)
                } else {
                    let record = session.catalog.get(&item.id).cloned().unwrap_or_else(|| AppRecord::new(&item.action_target, &item.title, None));
                    launch(spotlight.hwnd(), &record, &tracker)
                };
                match result {
                    Ok(()) => spotlight.dismiss(),
                    Err(error) => notify(Some(spotlight.hwnd()), &error),
                }
            }
            Ok(None) => {}
            Err(error) => { spotlight.hide(); notify(Some(window.hwnd()), &format!("Spotlight: {error}")); }
        }
        for app in &visible { icons.request(&app.id, &app.target); }
        for reply in icons.drain() {
            if !visible.iter().any(|app| app.id == reply.id && app.target == reply.source) { continue; }
            match reply.pixels {
                Ok(pixels) => {
                    renderer.set_icon(&reply.id, &pixels).map_err(|e| e.to_string())?;
                    force = true;
                }
                Err(error) => diagnostics::write(&format!("Icon unavailable for {}: {error}", reply.source)),
            }
        }
        dock.update(dt);
        let items = dock.get_render_items();
        let changed = items != previous;
        if changed || force {
            let tray = dock.get_tray_geometry();
            window.set_hit_regions(tray, &items);
            renderer.render_dock(&items, tray, &dock.get_tray_squircle_contour()).map_err(|e| format!("Dock draw: {e}"))?;
            pipeline.commit().map_err(|e| format!("Composition commit: {e}"))?;
            previous = items;
            smoke_draws += 1;
            if smoke && smoke_draws == 1 {
                let ready = serde_json::json!({"pid":std::process::id(),"version":crate::version()});
                std::fs::write(data_directory.join("startup-ready.json"),serde_json::to_vec(&ready).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
            }
        }
        unsafe { MsgWaitForMultipleObjectsEx(None, if changed || stage.as_ref().is_some_and(|shelf| shelf.animating()) { 8 } else { 250 }, QS_ALLINPUT, MWMO_INPUTAVAILABLE); }
    }
    if smoke {
        if smoke_draws == 0 || (smoke_menu && menu_bar.is_none()) || (smoke_stage && stage.is_none()) {
            return Err("Startup smoke did not start all requested native components".into());
        }
        let report = serde_json::json!({"version": crate::version(), "mode":"startup-smoke", "dock_draws":smoke_draws,
            "menu_bar_started":menu_bar.is_some(), "stage_started":stage.is_some(), "visual_ime_verified":false});
        std::fs::write(data_directory.join("startup-smoke.json"), serde_json::to_vec_pretty(&report).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    }
    diagnostics::write("Shutdown complete");
    Ok(())
}
fn hit(dock: &DockView, x: f32, y: f32) -> Option<String> {
    dock.get_render_items().into_iter().find(|item| {
        let half = item.current_size / 2.0;
        x >= item.center_x - half && x <= item.center_x + half && y >= item.center_y - half && y <= item.center_y + half
    }).map(|item| item.id)
}
fn matching_windows(app: &AppRecord, tracker: &AppTracker) -> Vec<AppInfo> {
    tracker.all_apps().into_iter().filter(|window| window.hwnd != 0 && app.matches(window)).collect()
}
fn launch(hwnd: HWND, app: &AppRecord, tracker: &AppTracker) -> Result<(), String> {
    let windows = matching_windows(app, tracker);
    if windows.len() > 1 {
        if let Some(DockAction::Activate(window, pid)) = dock_menu::windows(hwnd, &windows)? { ProcessLauncher::activate_window(window, pid)?; }
        return Ok(());
    }
    if let Some(window) = windows.first() { return ProcessLauncher::activate_window(window.hwnd, window.pid); }
    let item = app.dock_item();
    if let Some(target) = item.launch_target {
        // No known matching window remains. Use Shell launch and preserve shortcut semantics.
        let result = ProcessLauncher::launch_or_activate(&target, &AppTracker::new());
        diagnostics::write(&format!("Launch: {result:?}"));
        if let LaunchResult::Failed { error, .. } = result { return Err(format!("Could not open {}: {error}", app.title)); }
    }
    Ok(())
}
fn apply(action: DockAction, hwnd: HWND, session: &mut DockSession, store: &SettingsStore, tracker: &AppTracker, capacity: usize, spotlight: &mut crate::spotlight_controller::SpotlightController) -> Result<(), String> {
    let mut settings = session.settings.clone();
    let previous_hotkey = spotlight.hotkey();
    match action {
        DockAction::Activate(window, pid) => return ProcessLauncher::activate_window(window, pid),
        DockAction::Launch(id) => return session.catalog.get(&id).map(|app| launch(hwnd, app, tracker)).unwrap_or(Ok(())),
        DockAction::Page(delta) => { session.page_by(delta, capacity); return Ok(()); }
        DockAction::Pin(id) => { settings = session.pin_settings(&id).ok_or("This app is no longer available")?; }
        DockAction::Unpin(id) => {
            settings.unpin(&id);
            settings.custom_apps.retain(|app| app.id != id);
        }
        DockAction::Move(id, delta) => settings.move_pin(&id, delta),
        DockAction::Monitor(id) => settings.monitor = Some(id),
        DockAction::IconSize(size) => settings.icon_size = size,
        DockAction::ToggleStage => settings.stage_enabled = !settings.stage_enabled,
        DockAction::ToggleMenuBar => settings.menu_bar_enabled = !settings.menu_bar_enabled,
        DockAction::SpotlightHotkey(key) => {
            spotlight.set_hotkey(key)?;
            settings.spotlight_hotkey = key;
        }
    }
    if let Err(error) = store.save(&settings) {
        if settings.spotlight_hotkey != session.settings.spotlight_hotkey {
            if let Some(previous) = previous_hotkey {
                if let Err(rollback) = spotlight.set_hotkey(previous) {
                    spotlight.clear_hotkey();
                    return Err(format!("Could not save settings: {error}. Could not restore previous shortcut: {rollback}. Open Spotlight from the tray."));
                }
            } else { spotlight.clear_hotkey(); }
        }
        return Err(format!("Could not save Dock settings: {error}"));
    }
    session.settings = settings; // Commit UI state only after durable save succeeds.
    Ok(())
}

fn update_status(dock: &mut DockView, visible: &[AppRecord], tracker: &AppTracker) {
    let windows = tracker.all_apps();
    for (index, app) in visible.iter().enumerate() {
        let matches: Vec<_> = windows.iter().filter(|window| app.matches(window)).collect();
        dock.set_app_running(index, !matches.is_empty(), matches.iter().any(|window| window.is_focused));
    }
}
