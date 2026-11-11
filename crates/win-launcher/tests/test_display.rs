use win_launcher::display::{DockPlacement, WorkArea};
#[test]
fn layout_scales_and_stays_inside_negative_monitor_work_area() {
    let area = WorkArea { left: -1920, top: -200, right: 0, bottom: 840 };
    let layout = DockPlacement::fit(area, 144).unwrap();
    assert!(layout.x >= area.left);
    assert!(layout.y >= area.top);
    assert!(layout.x + layout.width <= area.right);
    assert!(layout.y + layout.height <= area.bottom);
    assert_eq!(layout.view_config().magnification.base_size, 72.0);
    assert!(layout.capacity > 0);
    let small = DockPlacement::fit(WorkArea { left: 0, top: 0, right: 800, bottom: 560 }, 96).unwrap();
    assert!(small.capacity < layout.capacity);
    assert!(DockPlacement::fit(WorkArea { left: 0, top: 0, right: 0, bottom: 0 }, 96).is_err());
}

#[test]
fn maximum_page_magnification_fits_the_surface_at_each_dpi() {
    use win_launcher::{DockItem, DockView};
    for dpi in [96, 144, 192] {
        for width in [800, 1920, 2560] {
          for size in [32, 48, 64] {
            let layout = DockPlacement::fit_with_size(WorkArea { left: 0, top: 0, right: width, bottom: 1080 }, dpi, size).unwrap();
            let mut dock = DockView::with_config((0..layout.capacity).map(|i| DockItem::new(i.to_string(), "app", false)).collect(), layout.view_config());
            let base = dock.get_render_items();
            for icon in base {
                dock.on_mouse_move(icon.center_x, icon.center_y);
                for enlarged in dock.get_render_items() {
                    assert!(enlarged.center_x - enlarged.current_size/2.0 >= 0.0, "left clipped at {dpi} DPI/{width}px");
                    assert!(enlarged.center_x + enlarged.current_size/2.0 <= layout.width as f32, "right clipped at {dpi} DPI/{width}px");
                }
            }
          }
        }
    }
}
