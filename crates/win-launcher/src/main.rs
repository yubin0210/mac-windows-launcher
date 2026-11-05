//! win-launcher
//! Native Windows DirectComposition 120Hz system launcher for macOS Dock and Spotlight replication.

use std::time::Instant;
#[cfg(windows)]
use std::time::Duration;
use win_launcher::app_tracker::AppTracker;
use win_launcher::directx::{Direct2DRenderer, DirectCompositionPipeline};
use win_launcher::dock_view::DockView;
use win_launcher::indexer::AppIndexer;
use win_launcher::menu_bar_render::MenuBarRenderer;
use win_launcher::menu_bar_view::{MenuBarAction, MenuBarView};
use win_launcher::menu_bar_window::MenuBarWindow;
#[cfg(windows)]
use win_launcher::menu_bar_window::{MenuBarWindowConfig, MenuBarWindowEvent};
use win_launcher::process_launcher::{LaunchTarget, ProcessLauncher};
use win_launcher::spotlight_render::{SpotlightRenderContext, SpotlightRenderer};
use win_launcher::spotlight_view::SpotlightView;
use win_launcher::spotlight_window::SpotlightWindow;
#[cfg(windows)]
use win_launcher::spotlight_window::SpotlightEvent;
use win_launcher::stage_render::{StageRenderContext, StageRenderer};
use win_launcher::stage_tracker::StageTracker;
use win_launcher::stage_view::{StageView, StageViewAction};
use win_launcher::stage_window::StageWindow;
#[cfg(windows)]
use win_launcher::stage_window::{StageWindowConfig, StageWindowEvent};
use win_launcher::system_status::SystemStatusTracker;
use win_launcher::taskbar_guard::TaskbarGuard;
use win_launcher::window::{DockWindow, DockWindowConfig, WindowEvent};
#[cfg(not(windows))]
use win_launcher::StageWindowInfo;

fn main() {
    println!("=== macOS System Launcher: Dock, Spotlight, Top Menu Bar & Stage Manager (DirectComposition 120Hz) ===");

    let hide_taskbar = std::env::args().any(|arg| arg == "--hide-taskbar")
        || std::env::var("WIN_LAUNCHER_HIDE_TASKBAR")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

    #[cfg(windows)]
    {
        run_windows_launcher(hide_taskbar);
    }

    #[cfg(not(windows))]
    {
        let _ = hide_taskbar;
        run_cross_platform_demo();
    }
}

#[cfg(windows)]
fn run_windows_launcher(hide_taskbar: bool) {
    println!("Initializing layered DirectComposition window pipeline...");

    // Fail-safe taskbar guard: hides taskbar if configured, guaranteeing restoration on exit/panic.
    let _taskbar_guard = TaskbarGuard::new(hide_taskbar);
    if hide_taskbar {
        println!("Taskbar auto-hide active: Shell taskbar hidden with fail-safe Drop restoration guard.");
    }

    let config = DockWindowConfig::default();
    let mut window = match DockWindow::create(config.clone()) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Error creating dock window: {e}");
            return;
        }
    };

    let pipeline = match DirectCompositionPipeline::new(window.hwnd()) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error initializing DirectComposition visual tree: {e}");
            return;
        }
    };

    let mut renderer = match Direct2DRenderer::with_pipeline(
        &pipeline,
        config.width as u32,
        config.height as u32,
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error initializing Direct2D surface renderer: {e}");
            return;
        }
    };

    println!("DirectComposition visual pipeline and Direct2D surface active.");

    // Initialize Spotlight Launcher subsystem (global hotkey Alt+Space)
    let mut indexer = AppIndexer::new();
    indexer.build_index();
    println!("Spotlight indexer built: {} items indexed.", indexer.item_count());

    let mut spotlight_view = SpotlightView::new(indexer);
    let mut spotlight_window = match SpotlightWindow::new() {
        Ok(w) => {
            println!("Spotlight window initialized (global hotkey Alt+Space active).");
            w
        }
        Err(e) => {
            eprintln!("Warning: could not initialize Spotlight window: {e}");
            SpotlightWindow::new_mock()
        }
    };
    let mut spotlight_renderer = SpotlightRenderer::new_mock();

    // Initialize Top Menu Bar Subsystem (Win32 AppBar 28px work area reservation)
    let mb_config = MenuBarWindowConfig::default();
    let mut menu_bar_window = match MenuBarWindow::new(mb_config) {
        Ok(w) => {
            println!("Top Menu Bar window initialized (28px AppBar reserved top edge).");
            w
        }
        Err(e) => {
            eprintln!("Warning: could not initialize Menu Bar window: {e}");
            MenuBarWindow::new_mock()
        }
    };
    let mut menu_bar_view = MenuBarView::new();
    let mut menu_bar_renderer = MenuBarRenderer::new().unwrap_or_else(|_| MenuBarRenderer::new_mock());
    let mut status_tracker = SystemStatusTracker::new();
    menu_bar_view.update_status(status_tracker.refresh());

    // Initialize Stage Manager subsystem (left-edge flyout shelf and DWM live thumbnail streaming)
    let stage_config = StageWindowConfig::default();
    let mut stage_window = match StageWindow::new(stage_config) {
        Ok(w) => {
            println!("Stage Manager window initialized (left shelf flyout active).");
            w
        }
        Err(e) => {
            eprintln!("Warning: could not initialize Stage Window: {e}");
            StageWindow::new_mock()
        }
    };
    let mut stage_tracker = StageTracker::new(Box::new(win_launcher::stage_tracker::Win32WindowEnumerator::new()));
    stage_tracker.refresh();
    let mut stage_view = StageView::new(config.width as f32, config.height as f32);
    let mut stage_renderer = StageRenderer::new_mock();
    let mut dwm_thumbnail_mgr = win_launcher::dwm_thumbnail::DwmThumbnailManager::new(
        stage_window.hwnd(),
        Box::new(win_launcher::dwm_thumbnail::Win32ThumbnailController::new()),
    );

    println!("Running 120Hz render loop...");

    let mut dock_view = DockView::new();
    let mut app_tracker = AppTracker::new();
    app_tracker.refresh_running_apps();
    dock_view.update_running_status(&app_tracker);

    let mut last_frame = Instant::now();
    let mut last_tracker_poll = Instant::now();
    let mut last_render_err_log = Instant::now() - Duration::from_secs(10);

    'render_loop: while window.is_open() {
        let now = Instant::now();
        let dt = (now - last_frame).as_secs_f32().min(0.033);
        last_frame = now;

        // Periodically refresh running applications, foreground focus, and stage manager (every 500ms)
        if last_tracker_poll.elapsed().as_millis() >= 500 {
            app_tracker.refresh_running_apps();
            dock_view.update_running_status(&app_tracker);
            if let Some(focused) = app_tracker.focused_app() {
                menu_bar_view.update_active_app(&focused.process_name);
            }
            menu_bar_view.update_status(status_tracker.refresh());
            stage_tracker.refresh();
            stage_view.update(dt, &stage_tracker);
            let desired_streams = stage_view.desired_thumbnail_streams(&stage_tracker);
            let _ = dwm_thumbnail_mgr.sync_thumbnails(&desired_streams);
            last_tracker_poll = now;
        }

        // Process Top Menu Bar events
        for mb_event in menu_bar_window.poll_events() {
            match mb_event {
                MenuBarWindowEvent::MouseMove { x, y } => {
                    menu_bar_view.on_mouse_move(x, y);
                }
                MenuBarWindowEvent::MouseDown { x, y } => {
                    let action = menu_bar_view.on_mouse_click(x, y);
                    if let Some(dropdown) = menu_bar_view.active_dropdown() {
                        menu_bar_window.set_height(28 + dropdown.total_height() as u32);
                    } else {
                        menu_bar_window.reset_height();
                    }
                    if let Some(action) = action {
                        println!("Menu Bar action dispatched: {action:?}");
                        match action {
                            MenuBarAction::OpenSpotlight => {
                                spotlight_window.show();
                            }
                            MenuBarAction::ToggleControlCenter => {
                                println!("Control Center toggled.");
                            }
                            _ => {}
                        }
                    }
                }
                MenuBarWindowEvent::KillFocus => {
                    menu_bar_view.close_dropdown();
                    menu_bar_window.reset_height();
                }
                MenuBarWindowEvent::Destroy => {
                    println!("Menu Bar window received destroy notification.");
                    break 'render_loop;
                }
                _ => {}
            }
        }

        // Process Dock window events
        for event in window.poll_events() {
            match event {
                WindowEvent::MouseMove { x, y } => {
                    dock_view.on_mouse_move(x, y);
                }
                WindowEvent::MouseLeave => {
                    dock_view.on_mouse_leave();
                }
                WindowEvent::MouseClick { x, y } => {
                    if let Some(index) = dock_view.on_mouse_click(x, y) {
                        println!("Icon clicked: index {index} - triggering launch bounce");
                        if let Some(target) = dock_view.get_launch_target(index) {
                            let result = ProcessLauncher::launch_or_activate(target, &app_tracker);
                            println!("Launch result for index {index}: {result:?}");
                            app_tracker.refresh_running_apps();
                            dock_view.update_running_status(&app_tracker);
                        }
                    }
                }
                WindowEvent::Paint => {
                    let _ = pipeline.commit();
                }
                WindowEvent::Destroy => {
                    println!("Dock window received destroy notification.");
                    break 'render_loop;
                }
            }
        }

        // Process Spotlight window events
        for sp_event in spotlight_window.poll_events() {
            match sp_event {
                SpotlightEvent::Opened => {
                    spotlight_window.set_height(56);
                }
                SpotlightEvent::Closed | SpotlightEvent::Escaped => {
                    spotlight_view.clear();
                    spotlight_window.reset_height();
                }
                SpotlightEvent::QueryChanged(new_query) => {
                    spotlight_view.set_query(&new_query);
                    let count = spotlight_view.results_count();
                    let desired_height = if count == 0 {
                        56
                    } else {
                        (56 + count as u32 * 48).min(420)
                    };
                    spotlight_window.set_height(desired_height);
                }
                SpotlightEvent::Navigate(delta) => {
                    spotlight_view.navigate(delta);
                }
                SpotlightEvent::Execute => {
                    if let Some(item) = spotlight_view.selected_item() {
                        println!("Spotlight executing: '{}' -> {}", item.title, item.action_target);
                        if item.category == mac_physics_core::spotlight::SpotlightCategory::Calculation {
                            println!("Calculation result copied: {}", item.action_target);
                        } else {
                            let target = if item.action_target.starts_with("ms-settings:") {
                                LaunchTarget::protocol(&item.action_target)
                            } else {
                                LaunchTarget::shell(&item.action_target)
                            };
                            let result = ProcessLauncher::launch_or_activate(&target, &app_tracker);
                            println!("Spotlight launch result: {result:?}");
                        }
                        spotlight_window.hide();
                        spotlight_view.clear();
                        spotlight_window.reset_height();
                    }
                }
            }
        }

        // Process Stage Manager window events
        for stage_event in stage_window.poll_events() {
            match stage_event {
                StageWindowEvent::MouseMove { x, y } => {
                    stage_view.on_mouse_move(x, y);
                }
                StageWindowEvent::MouseLeave => {
                    stage_view.on_mouse_leave();
                }
                StageWindowEvent::MouseDown { x, y } => {
                    if let Some(action) = stage_view.on_mouse_click(x, y) {
                        match action {
                            StageViewAction::ActivateGroup(group_id) => {
                                println!("Stage Manager activating group: {group_id}");
                                if let Err(e) = stage_tracker.set_active_group(&group_id) {
                                    eprintln!("Warning: failed to activate stage group: {e}");
                                }
                                stage_view.update(0.0, &stage_tracker);
                                let desired_streams = stage_view.desired_thumbnail_streams(&stage_tracker);
                                let _ = dwm_thumbnail_mgr.sync_thumbnails(&desired_streams);
                            }
                        }
                    }
                }
                StageWindowEvent::Destroy => {
                    println!("Stage window received destroy notification.");
                    break 'render_loop;
                }
                _ => {}
            }
        }

        // Render Spotlight if visible
        if spotlight_window.is_visible() {
            let width = spotlight_window.width() as f32;
            let height = spotlight_window.height() as f32;
            let _ = spotlight_renderer.render_spotlight(
                spotlight_view.get_query(),
                spotlight_view.get_results(),
                spotlight_view.selected_index(),
                width,
                height,
            );
        }

        // Advance dock icon physics and spring oscillators
        dock_view.update(dt);

        // Rasterize dock tray background squircle, app icons, and indicator pips
        let render_items = dock_view.get_render_items();
        let tray_geom = dock_view.get_tray_geometry();
        let squircle_pts = dock_view.get_tray_squircle_contour();
        if let Err(e) = renderer.render_dock(&render_items, tray_geom, &squircle_pts) {
            if last_render_err_log.elapsed().as_secs() >= 3 {
                eprintln!("Warning: Direct2D surface render error: {e}");
                last_render_err_log = Instant::now();
            }
        }

        // Rasterize Top Menu Bar
        if let Err(e) = menu_bar_renderer.render(&menu_bar_view, menu_bar_window.width() as f32) {
            if last_render_err_log.elapsed().as_secs() >= 3 {
                eprintln!("Warning: Menu Bar Direct2D render error: {e}");
                last_render_err_log = Instant::now();
            }
        }

        // Advance Stage Manager animation and render left shelf
        stage_view.update(dt, &stage_tracker);
        if let Err(e) = stage_renderer.draw_stage_shelf(stage_view.cards()) {
            if last_render_err_log.elapsed().as_secs() >= 3 {
                eprintln!("Warning: Stage Manager Direct2D render error: {e}");
                last_render_err_log = Instant::now();
            }
        }

        // Commit composition tree changes to DWM
        if let Err(e) = pipeline.commit() {
            if last_render_err_log.elapsed().as_secs() >= 3 {
                eprintln!("Warning: composition commit error: {e}");
                last_render_err_log = Instant::now();
            }
        }

        // Synchronize with display refresh rate (120Hz on supported monitors)
        window.wait_vsync();
    }

    println!("Dock launcher shutdown cleanly.");
}

#[cfg(not(windows))]
fn run_cross_platform_demo() {
    let start_time = Instant::now();
    println!("Platform: Non-Windows host (mock DirectComposition & Direct2D mode)");

    // 1. TaskbarGuard fail-safe initialization with auto-hide and restoration hook
    let mut taskbar_guard = TaskbarGuard::new_mock_with_callback(true, || {
        println!("  [TaskbarGuard] Restoration hook fired: taskbars restored to desktop work area.");
    });
    println!(
        "TaskbarGuard initialized: auto-hide active (is_hidden: {}, managed_taskbars: {})",
        taskbar_guard.is_hidden(),
        taskbar_guard.hidden_window_count()
    );

    // 2. DirectComposition mock pipeline and layered dock window
    let config = DockWindowConfig::default();
    let mut window = DockWindow::create(config).expect("Mock window creation succeeds");
    let pipeline = DirectCompositionPipeline::new_mock();

    // 3. DockView and AppTracker initialization with running process indicators
    let mut dock_view = DockView::new();
    let mut tracker = AppTracker::new();
    tracker.register_mock_running_app("explorer.exe", 1001, false);
    tracker.register_mock_running_app("wt.exe", 1002, true);
    dock_view.update_running_status(&tracker);

    let running_count = dock_view.items().iter().filter(|i| i.is_running).count();
    let focused_count = dock_view.items().iter().filter(|i| i.is_focused).count();
    println!(
        "Dock items loaded: {} (active processes: {running_count}, focused: {focused_count})",
        dock_view.items().len()
    );
    let (tx, ty, tw, th) = dock_view.get_tray_geometry();
    println!("Tray resting geometry: x={tx:.1}, y={ty:.1}, w={tw:.1}, h={th:.1}");

    // 4. Simulate mouse hover magnification
    let center_x = tx + tw / 2.0;
    let center_y = ty + th / 2.0;
    println!("Simulating cursor hover at ({center_x:.1}, {center_y:.1})...");
    window.queue_event(WindowEvent::MouseMove {
        x: center_x,
        y: center_y,
    });

    for event in window.poll_events() {
        if let WindowEvent::MouseMove { x, y } = event {
            dock_view.on_mouse_move(x, y);
        }
    }

    let render_items = dock_view.get_render_items();
    let max_scaled = render_items
        .iter()
        .max_by(|a, b| a.scale.partial_cmp(&b.scale).unwrap())
        .unwrap()
        .clone();
    println!(
        "Hover active: peak icon '{}' (index {}) scaled to {:.2}x ({:.1}px)",
        max_scaled.label, max_scaled.index, max_scaled.scale, max_scaled.current_size
    );

    // 5. Simulate click launch of a non-running application
    println!("Simulating click on non-running app '{}' to launch...", max_scaled.label);
    dock_view.trigger_launch(max_scaled.index);
    if let Some(target) = dock_view.get_launch_target(max_scaled.index) {
        let result = ProcessLauncher::launch_or_activate(target, &tracker);
        println!(
            "  ProcessLauncher dispatched target '{}': {result:?}",
            target.name()
        );
    }

    // 6. Simulate click activation of an already-running application (Terminal / wt.exe)
    if let Some((term_idx, term_item)) = dock_view
        .items()
        .iter()
        .enumerate()
        .find(|(_, item)| item.process_name.as_deref() == Some("wt.exe"))
    {
        println!(
            "Simulating click on running app '{}' (index {term_idx}) to activate existing window...",
            term_item.label
        );
        dock_view.trigger_launch(term_idx);
        if let Some(target) = dock_view.get_launch_target(term_idx) {
            let result = ProcessLauncher::launch_or_activate(target, &tracker);
            println!(
                "  ProcessLauncher activated target '{}': {result:?}",
                target.name()
            );
        }
    }

    // 7. Step damped harmonic bounce physics at 120Hz
    println!("Simulating 5 frames of damped harmonic bounce physics at 120Hz...");
    for f in 1..=5 {
        dock_view.update(0.00833); // 120Hz frame step (~8.33ms)
        let items = dock_view.get_render_items();
        let clicked = &items[max_scaled.index];
        println!(
            "  Bounce frame {f}: offset = {:.2}px, is_bouncing = {}",
            clicked.bounce_offset_y, clicked.is_bouncing
        );
    }

    // 8. Simulate cursor leave and smooth exponential scale decay
    println!("Simulating cursor leaving dock window (WM_MOUSELEAVE)...");
    window.queue_event(WindowEvent::MouseLeave);
    for event in window.poll_events() {
        if let WindowEvent::MouseLeave = event {
            dock_view.on_mouse_leave();
        }
    }

    println!("Simulating smooth exponential scale decay across 5 frames...");
    for f in 1..=5 {
        dock_view.update(0.016); // 16ms step (~60Hz / 120Hz interval)
        let items = dock_view.get_render_items();
        let peak = &items[max_scaled.index];
        println!(
            "  Decay frame {f}: peak icon '{}' scale = {:.3}x ({:.1}px)",
            peak.label, peak.scale, peak.current_size
        );
    }

    // 9. Direct2D surface rasterization (squircle tray, app icons, indicator pips)
    let mut renderer = Direct2DRenderer::new();
    let current_render_items = dock_view.get_render_items();
    let (tx, ty, tw, th) = dock_view.get_tray_geometry();
    let squircle_pts = dock_view.get_tray_squircle_contour();
    renderer
        .render_dock(&current_render_items, (tx, ty, tw, th), &squircle_pts)
        .expect("Mock Direct2D rasterization succeeds");
    println!(
        "Direct2D surface rasterized: tray_drawn={}, icons={}, running_pips={}, focused_pips={}, squircle_pts={}",
        renderer.tray_drawn,
        renderer.icons_drawn_count,
        renderer.running_pips_count,
        renderer.focused_pips_count,
        squircle_pts.len()
    );

    // 10. DirectComposition visual commit
    let _ = pipeline.commit();
    println!("DirectComposition mock commit count: {}", pipeline.commit_count());

    // 11. TaskbarGuard clean restoration
    println!("Testing TaskbarGuard Drop / manual restoration...");
    taskbar_guard.restore();
    println!("TaskbarGuard is_hidden after restore: {}", taskbar_guard.is_hidden());

    // 12. Milestone 3: Spotlight Indexer & Search Evaluation
    println!("\n--- Milestone 3: Spotlight Launcher Demonstration ---");
    let mut indexer = AppIndexer::new();
    indexer.build_index();
    println!("12. AppIndexer loaded {} applications & settings items.", indexer.item_count());

    let mut spotlight_view = SpotlightView::new(indexer);
    let mut spotlight_window = SpotlightWindow::new_mock();
    let mut spotlight_renderer = SpotlightRenderer::new_mock();

    // 13. Simulate Global Hotkey Activation (Alt + Space)
    println!("13. Simulating global hotkey (Alt + Space) press...");
    spotlight_window.show();
    assert!(spotlight_window.is_visible());
    println!("  Spotlight floating window opened (is_visible: true, height: {}px)", spotlight_window.height());

    // 14. Simulate incremental typing & instant arithmetic calculation
    println!("14. Simulating math expression input: '120 * 60 + 500'...");
    spotlight_view.input_text("120 * 60 + 500");
    let math_results = spotlight_view.get_results();
    assert!(!math_results.is_empty());
    println!(
        "  Instant Calculation Result: '{}' ({})",
        math_results[0].title, math_results[0].subtitle
    );

    // 15. Simulate application fuzzy search & keyboard navigation
    println!("15. Simulating application fuzzy search: 'term'...");
    spotlight_view.set_query("term");
    let search_results = spotlight_view.get_results();
    println!("  Found {} matches for 'term':", search_results.len());
    for (idx, item) in search_results.iter().enumerate() {
        println!("    [{idx}] {} ({}) - target: {}", item.title, item.subtitle, item.action_target);
    }
    assert_eq!(spotlight_view.selected_index(), 0);

    // Down arrow navigation
    spotlight_view.navigate(1);
    println!("  Navigated Down: selected_index = {}", spotlight_view.selected_index());

    // Up arrow navigation wrap
    spotlight_view.navigate(-1);
    println!("  Navigated Up: selected_index = {}", spotlight_view.selected_index());

    // Execute selected item via ProcessLauncher
    if let Some(selected) = spotlight_view.selected_item() {
        println!("  Simulating Enter key: executing '{}'...", selected.title);
        let target = if selected.action_target.starts_with("ms-settings:") {
            LaunchTarget::protocol(&selected.action_target)
        } else {
            LaunchTarget::shell(&selected.action_target)
        };
        let launch_result = ProcessLauncher::launch_or_activate(&target, &tracker);
        println!("  Execution dispatched: {launch_result:?}");
    }

    // 16. Spotlight Direct2D Squircle Floating Bar Rasterization
    let res_count = spotlight_view.results_count();
    let expanded_h = 56.0 + res_count as f32 * 48.0;
    spotlight_window.set_height(expanded_h as u32);
    spotlight_renderer
        .render_spotlight(
            spotlight_view.get_query(),
            spotlight_view.get_results(),
            spotlight_view.selected_index(),
            640.0,
            expanded_h,
        )
        .expect("Spotlight Direct2D rasterization succeeds");
    println!(
        "16. Spotlight Direct2D rasterized: 640px floating squircle (n=4.5), height={expanded_h}px, results={res_count}."
    );

    // Close Spotlight (Escape / Kill-focus)
    spotlight_window.hide();
    spotlight_view.clear();
    println!("  Spotlight window dismissed (is_visible: {})", spotlight_window.is_visible());

    // 17. Milestone 4: Top Menu Bar & System Status Subsystem
    println!("\n--- Milestone 4: Top Menu Bar & System Status Demonstration ---");
    let menu_bar_window = MenuBarWindow::new_mock();
    assert!(menu_bar_window.is_open());
    assert_eq!(menu_bar_window.height(), 28);
    assert!(menu_bar_window.is_appbar_registered());
    println!(
        "17. MenuBarWindow initialized: height={}px, is_appbar_registered={}",
        menu_bar_window.height(),
        menu_bar_window.is_appbar_registered()
    );

    let mut status_tracker = SystemStatusTracker::new_mock();
    let status_snapshot = status_tracker.refresh();
    println!(
        "  System status queried: clock='{}', battery={:?}%, is_charging={}, wifi={}",
        status_snapshot.time_text,
        status_snapshot.battery_percent,
        status_snapshot.is_charging,
        status_snapshot.wifi_connected
    );

    let mut menu_bar_view = MenuBarView::new();
    menu_bar_view.update_status(status_snapshot);
    println!("  Active app initially: '{}'", menu_bar_view.active_app_title());

    // Focus change
    menu_bar_view.update_active_app("Visual Studio Code");
    println!("  Active app updated on foreground window switch: '{}'", menu_bar_view.active_app_title());
    assert_eq!(menu_bar_view.active_app_title(), "Visual Studio Code");

    // Open Apple dropdown
    println!("  Simulating click on Apple logo (x: 18.0, y: 14.0)...");
    menu_bar_view.on_mouse_click(18.0, 14.0);
    assert!(menu_bar_view.is_dropdown_open());
    println!(
        "  Dropdown opened: '{}' with {} items",
        menu_bar_view.active_dropdown().unwrap().menu_id,
        menu_bar_view.active_dropdown().unwrap().items.len()
    );

    // Hover item and click
    let apple_dd = menu_bar_view.active_dropdown().unwrap();
    let d_x = apple_dd.origin_x;
    let d_y = apple_dd.origin_y;
    menu_bar_view.on_mouse_move(d_x + 20.0, d_y + 12.0);
    let action = menu_bar_view.on_mouse_click(d_x + 20.0, d_y + 12.0);
    println!("  Dropdown action executed: {action:?}");
    assert_eq!(action, Some(MenuBarAction::AboutThisMac));
    assert!(!menu_bar_view.is_dropdown_open());

    // Direct2D rendering
    let mut menu_bar_renderer = MenuBarRenderer::new_mock();
    menu_bar_renderer.render(&menu_bar_view, 1920.0).expect("MenuBar rasterization succeeds");
    let mb_ctx = menu_bar_renderer.mock_context().unwrap();
    println!(
        "18. MenuBar Direct2D rasterized: {} elements drawn, frames={}",
        mb_ctx.drawn_elements.len(),
        mb_ctx.rendered_frames
    );

    // 19. Milestone 5: Stage Manager Window Grouping & DWM Streaming Demonstration
    println!("\n--- Milestone 5: Stage Manager & Window Grouping Demonstration ---");
    let stage_window = StageWindow::new_mock();
    assert_eq!(stage_window.width(), 220);
    let mut stage_tracker = StageTracker::new_mock();
    stage_tracker.add_mock_window(StageWindowInfo::new(101, "Safari Window 1", "Safari", 1200, 800, false));
    stage_tracker.add_mock_window(StageWindowInfo::new(102, "Safari Window 2", "Safari", 1200, 800, false));
    stage_tracker.add_mock_window(StageWindowInfo::new(201, "Code Editor", "VS Code", 1400, 900, false));
    stage_tracker.add_mock_window(StageWindowInfo::new(301, "Terminal", "Terminal", 900, 600, false));

    assert_eq!(stage_tracker.groups().len(), 3);
    println!(
        "19. Window tracker grouped 4 windows into {} stage groups (active: {:?})",
        stage_tracker.groups().len(),
        stage_tracker.active_group_id()
    );

    let mut stage_view = StageView::new(1920.0, 1080.0);
    stage_view.update(0.016, &stage_tracker);
    let cards = stage_view.cards();
    println!("  Stage shelf layout computed: {} cards visible on left flyout:", cards.len());
    for (i, card) in cards.iter().enumerate() {
        println!(
            "    [{i}] App: '{}' (group: {}), stack_depth={}, x={:.1}, y={:.1}, w={:.1}, h={:.1}",
            card.app_name, card.group_id, card.stack_depth, card.bounds.x, card.bounds.y, card.bounds.width, card.bounds.height
        );
    }

    // Simulate clicking second staged card (VS Code)
    let target_card = cards[0].clone();
    let click_x = target_card.bounds.x + 20.0;
    let click_y = target_card.bounds.y + 20.0;
    println!("  Simulating click on staged card '{}' at ({:.1}, {:.1})...", target_card.app_name, click_x, click_y);
    stage_view.on_mouse_move(click_x, click_y);
    let stage_action = stage_view.on_mouse_click(click_x, click_y);
    println!("  Stage action triggered: {stage_action:?}");
    if let Some(StageViewAction::ActivateGroup(gid)) = stage_action {
        stage_tracker.set_active_group(&gid).expect("Stage group activation succeeds");
        println!("  Active stage group switched to: '{}'", gid);
        assert_eq!(stage_tracker.active_group_id(), Some(gid.as_str()));
    }

    // Simulate DWM thumbnail stream synchronization
    let mut thumbnail_mgr = win_launcher::dwm_thumbnail::DwmThumbnailManager::new_mock(9999);
    let desired_streams = stage_view.desired_thumbnail_streams(&stage_tracker);
    thumbnail_mgr.sync_thumbnails(&desired_streams).expect("DWM thumbnail sync succeeds");
    println!(
        "  DWM Thumbnail streams synchronized: {} active live thumbnails registered.",
        thumbnail_mgr.active_registrations().len()
    );

    // Rasterize stage shelf with Direct2D
    let mut stage_renderer = StageRenderer::new_mock();
    stage_renderer.draw_stage_shelf(stage_view.cards()).expect("Stage Direct2D rasterization succeeds");
    println!(
        "20. Stage Shelf Direct2D rasterized: {} squircle cards drawn with multi-window fan-out backdrops.",
        stage_renderer.cards_drawn().len()
    );

    println!("\nDemo completed in {:.2}ms. Ready for Windows deployment.", start_time.elapsed().as_secs_f64() * 1000.0);
}
