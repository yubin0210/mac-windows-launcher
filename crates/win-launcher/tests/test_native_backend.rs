//! Exercises the actual Windows composition backend, independently of mocks.
#![cfg(windows)]
use win_launcher::{Direct2DRenderer, DirectCompositionPipeline, DockView, DockWindow, DockWindowConfig};

#[test]
#[ignore = "requires a Windows desktop and graphics device; run explicitly in native smoke CI"]
fn native_dock_surface_draws_and_commits() {
    let mut window = DockWindow::create(DockWindowConfig::default()).expect("native HWND");
    // DComp owns per-pixel alpha. An uninitialized layered host can remain invisible
    // even when surface draw/Commit return success.
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongW, GWL_EXSTYLE, WS_EX_LAYERED, WS_EX_NOREDIRECTIONBITMAP};
    let style = unsafe { GetWindowLongW(window.hwnd(), GWL_EXSTYLE) } as u32;
    assert_eq!(style & WS_EX_LAYERED.0, 0, "DComp host must not require layered-window initialization");
    assert_ne!(style & WS_EX_NOREDIRECTIONBITMAP.0, 0);
    {
        let pipeline = DirectCompositionPipeline::new(window.hwnd()).expect("native composition device");
        let mut renderer = Direct2DRenderer::with_pipeline(&pipeline, 1200, 180).expect("native surface");
        let dock = DockView::new();
        let pixels = win_launcher::icon_cache::IconPixels::new(1, 1, vec![20, 40, 60, 128]).unwrap();
        renderer.set_icon(&dock.items()[0].id, &pixels).expect("GPU bitmap upload");
        for _ in 0..3 {
            renderer.render_dock(&dock.get_render_items(), dock.get_tray_geometry(), &dock.get_tray_squircle_contour()).expect("BeginDraw/draw/EndDraw");
            pipeline.commit().expect("native commit");
        }
    }
    window.close();
    assert!(!window.is_open());
}

#[test]
#[ignore = "requires Explorer notification area; injects the documented Shell restart notification"]
fn native_tray_can_be_recreated_after_shell_restart_notification() {
    use windows::{core::w, Win32::{Foundation::{WPARAM, LPARAM}, UI::{Shell::*, WindowsAndMessaging::*}}};
    let mut window = DockWindow::create(DockWindowConfig::default()).unwrap();
    let tray = win_launcher::native_tray::TrayIcon::new(window.hwnd()).unwrap();
    // Explorer loses its icon table on restart. Remove this test's entry first.
    let data = NOTIFYICONDATAW { cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: window.hwnd(), uID: 1, ..Default::default() };
    assert!(unsafe { Shell_NotifyIconW(NIM_DELETE, &data) }.as_bool());
    let message = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };
    assert_ne!(message, 0);
    unsafe { SendMessageW(window.hwnd(), message, WPARAM(0), LPARAM(0)); }
    assert!(window.poll_events().contains(&win_launcher::WindowEvent::ShellRestarted));
    tray.readd().expect("the retained icon must register again after Shell restart");
    drop(tray);
    window.close();
}

#[test]
#[ignore = "requires native Windows Shell and an interactive desktop"]
fn native_catalog_and_icon_worker_use_real_shell_data() {
    use win_launcher::{app_catalog::AppCatalog, icon_cache::native::IconLoader};
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
    struct Apartment;
    impl Drop for Apartment { fn drop(&mut self) { unsafe { CoUninitialize(); } } }
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.ok().expect("Shell COM apartment");
    let _apartment = Apartment;
    let catalog = AppCatalog::discover();
    assert!(!catalog.apps().is_empty(), "Windows app catalog should include real Shell apps");
    let source = std::env::current_exe().unwrap().to_string_lossy().to_string();
    let mut icons = IconLoader::new();
    icons.request("smoke-test", &source);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        if let Some(reply) = icons.drain().into_iter().next() {
            let pixels = reply.pixels.expect("Shell icon pixels");
            assert_eq!(pixels.bgra.len(), pixels.width as usize * pixels.height as usize * 4);
            assert!(pixels.bgra.chunks_exact(4).any(|pixel| pixel[3] > 0), "Shell icon alpha is not empty");
            break;
        }
        assert!(std::time::Instant::now() < deadline, "Icon worker timed out");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[test]
#[ignore = "requires native Windows window-state transitions"]
fn activating_a_maximized_window_preserves_its_bounds() {
    use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, IsZoomed, IsIconic, SW_MAXIMIZE, SW_MINIMIZE};
    let mut window = DockWindow::create(DockWindowConfig::default()).expect("test HWND");
    unsafe { let _ = ShowWindow(window.hwnd(), SW_MAXIMIZE); }
    assert!(unsafe { IsZoomed(window.hwnd()) }.as_bool());
    // Foreground permission can vary in CI; the window-state rule is independent.
    let _ = win_launcher::ProcessLauncher::activate_window(window.hwnd().0 as usize, std::process::id());
    assert!(unsafe { IsZoomed(window.hwnd()) }.as_bool(), "activation must not unmaximize the window");
    unsafe { let _ = ShowWindow(window.hwnd(), SW_MINIMIZE); }
    let _ = win_launcher::ProcessLauncher::activate_window(window.hwnd().0 as usize, std::process::id());
    assert!(!unsafe { IsIconic(window.hwnd()) }.as_bool());
    window.close();
}

#[test]
#[ignore = "requires native Windows EDIT, DirectWrite and composition"]
fn native_spotlight_input_and_results_share_the_composition_device() {
    use win_launcher::{spotlight_native::NativeSpotlightWindow, spotlight_window::SpotlightWindowConfig,
        spotlight_render::Direct2DSpotlightRenderer, AppIndexer};
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::SetWindowTextW;
    let mut dock = DockWindow::create(DockWindowConfig::default()).unwrap();
    let shared = DirectCompositionPipeline::new(dock.hwnd()).unwrap();
    {
        let mut search = NativeSpotlightWindow::new(SpotlightWindowConfig::default()).unwrap();
        let pipeline = shared.for_window(search.hwnd(), false).expect("composition behind native EDIT");
        let query = "메모 😀";
        let units: Vec<u16> = query.encode_utf16().chain(Some(0)).collect();
        unsafe { SetWindowTextW(search.edit_hwnd(), PCWSTR(units.as_ptr())) }.unwrap();
        assert_eq!(search.query(), query, "native UTF-16 input must preserve Korean and surrogate pairs");
        let mut indexer = AppIndexer::new();
        indexer.register_mock_item(format!("메모 😀 {}", "긴 앱 이름 ".repeat(80)), "Long application name for native layout", mac_physics_core::spotlight::SpotlightCategory::Application, "notepad.exe");
        let results = indexer.search(query, 5);
        search.set_results(results.len(), true).unwrap();
        let layout = search.geometry();
        let mut renderer = Direct2DSpotlightRenderer::with_pipeline(&pipeline, search.width(), search.height()).unwrap();
        renderer.set_icon(&results[0].id, &win_launcher::icon_cache::IconPixels::new(1, 1, vec![20, 40, 60, 128]).unwrap()).unwrap();
        renderer.draw(query, &results, 0, layout).expect("native DirectWrite text and icon draw");
        pipeline.commit().unwrap();
        renderer.draw("absent", &[], 0, layout).expect("native empty result message");
        pipeline.commit().unwrap();
        // Explicit order: surface and target before EDIT/parent HWND.
        drop(renderer); drop(pipeline); drop(search);
    }
    drop(shared);
    dock.close();
}

#[test]
#[ignore = "requires available Windows global shortcuts; runs serially in native smoke CI"]
fn native_shortcut_conflict_preserves_the_registered_shortcut() {
    use win_launcher::{spotlight_native::NativeSpotlightWindow, spotlight_window::SpotlightWindowConfig, settings::SpotlightHotkey};
    let mut first = NativeSpotlightWindow::new(SpotlightWindowConfig::default()).unwrap();
    let mut second = NativeSpotlightWindow::new(SpotlightWindowConfig::default()).unwrap();
    first.register_hotkey(SpotlightHotkey::CtrlAltSpace).unwrap();
    second.register_hotkey(SpotlightHotkey::CtrlShiftSpace).unwrap();
    assert!(first.register_hotkey(SpotlightHotkey::CtrlShiftSpace).is_err());
    assert_eq!(first.hotkey(), Some(SpotlightHotkey::CtrlAltSpace));
    first.clear_hotkey();
    second.register_hotkey(SpotlightHotkey::CtrlAltSpace).unwrap();
}

#[test]
#[ignore = "requires Explorer desktop; temporarily reserves and restores the top work area"]
fn native_menu_bar_reserves_and_releases_only_its_strip() {
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
    struct Apartment;
    impl Drop for Apartment { fn drop(&mut self) { unsafe { CoUninitialize(); } } }
    unsafe { CoInitializeEx(None,COINIT_APARTMENTTHREADED) }.ok().unwrap();
    let _apartment=Apartment;
    let mut dock=DockWindow::create(DockWindowConfig::default()).unwrap();
    let monitor=unsafe { MonitorFromWindow(dock.hwnd(),MONITOR_DEFAULTTOPRIMARY) };
    let work=|| {
        let mut info=MONITORINFO { cbSize:std::mem::size_of::<MONITORINFO>() as u32,..Default::default() };
        assert!(unsafe { GetMonitorInfoW(monitor,&mut info) }.as_bool()); info.rcWork
    };
    let before=work();
    let pipeline=DirectCompositionPipeline::new(dock.hwnd()).unwrap();
    {
        let mut bar=win_launcher::menu_bar_native::NativeMenuBar::new(&pipeline,dock.hwnd()).unwrap();
        bar.frame(dock.hwnd(),&win_launcher::app_catalog::AppCatalog::default(),&win_launcher::AppTracker::new()).unwrap();
        assert!(work().top>before.top,"native AppBar must reserve its own top strip");
    }
    let deadline=std::time::Instant::now()+std::time::Duration::from_secs(1);
    while work()!=before && std::time::Instant::now()<deadline { std::thread::sleep(std::time::Duration::from_millis(10)); }
    assert_eq!(work(),before,"Drop must release the menu reservation");
    drop(pipeline); dock.close();
}

#[test]
#[ignore = "requires native DWM and desktop; checks API wiring, not visual thumbnail visibility"]
fn native_stage_collapses_input_and_registers_real_dwm_preview(){
 use windows::Win32::{Foundation::*,Graphics::Dwm::*,UI::WindowsAndMessaging::*};
 use windows::Win32::System::Com::{CoInitializeEx,CoUninitialize,COINIT_APARTMENTTHREADED};
 struct Apartment;impl Drop for Apartment{fn drop(&mut self){unsafe{CoUninitialize();}}}
 unsafe{CoInitializeEx(None,COINIT_APARTMENTTHREADED)}.ok().unwrap();let _apartment=Apartment;
 let mut source=DockWindow::create(DockWindowConfig::default()).unwrap();
 let shared=DirectCompositionPipeline::new(source.hwnd()).unwrap();
 let mut shelf=win_launcher::stage_native::NativeStage::new(&shared).unwrap();
 shelf.frame(source.hwnd(),&win_launcher::app_catalog::AppCatalog::default(),&win_launcher::AppTracker::new(),0.016).unwrap();
 let mut rect=RECT::default();unsafe{GetWindowRect(shelf.hwnd(),&mut rect)}.unwrap();
 assert!(rect.right-rect.left<=48 && rect.bottom-rect.top<=384,"collapsed shelf must not occupy a full-screen overlay");
 struct Thumbnail(isize);impl Drop for Thumbnail{fn drop(&mut self){unsafe{let _=DwmUnregisterThumbnail(self.0);}}}
 let thumbnail=Thumbnail(unsafe{DwmRegisterThumbnail(shelf.hwnd(),source.hwnd())}.unwrap());
 let size=unsafe{DwmQueryThumbnailSourceSize(thumbnail.0)}.unwrap();assert!(size.cx>0&&size.cy>0);
 let properties=DWM_THUMBNAIL_PROPERTIES{dwFlags:DWM_TNP_RECTDESTINATION|DWM_TNP_VISIBLE,rcDestination:RECT{left:8,top:60,right:180,bottom:140},fVisible:true.into(),..Default::default()};
 unsafe{DwmUpdateThumbnailProperties(thumbnail.0,&properties)}.unwrap();
 drop(thumbnail);drop(shelf);drop(shared);source.close();
}
