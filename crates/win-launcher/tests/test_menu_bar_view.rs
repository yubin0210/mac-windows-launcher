use win_launcher::menu_bar_view::{MenuBarAction, MenuBarView};

#[test]
fn test_menu_bar_view_active_app_and_dropdown() {
    let mut view = MenuBarView::new();
    view.update_active_app("Visual Studio Code");
    assert_eq!(view.active_app_title(), "Visual Studio Code");

    // Open Apple menu by clicking around Apple logo (x: 18.0, y: 14.0)
    view.on_mouse_click(18.0, 14.0);
    assert!(view.is_dropdown_open());

    let dropdown = view.active_dropdown().expect("Dropdown should be active");
    assert_eq!(dropdown.menu_id, "apple");
    assert!(!dropdown.items.is_empty());

    // Click outside dropdown bounds (e.g., at x: 800, y: 300) closes dropdown
    let action = view.on_mouse_click(800.0, 300.0);
    assert_eq!(action, None);
    assert!(!view.is_dropdown_open());
}

#[test]
fn test_menu_bar_view_dropdown_item_selection() {
    let mut view = MenuBarView::new();
    view.open_dropdown("apple");
    assert!(view.is_dropdown_open());

    let dropdown = view.active_dropdown().unwrap();
    let item_count = dropdown.items.len();
    assert!(item_count > 3);

    // Hover over first item (About This Mac)
    let origin_x = dropdown.origin_x;
    let origin_y = dropdown.origin_y;
    view.on_mouse_move(origin_x + 20.0, origin_y + 12.0);

    let dropdown_hovered = view.active_dropdown().unwrap();
    assert_eq!(dropdown_hovered.hovered_index, Some(0));

    // Click first item executes AboutThisMac and closes dropdown
    let action = view.on_mouse_click(origin_x + 20.0, origin_y + 12.0);
    assert_eq!(action, Some(MenuBarAction::AboutThisMac));
    assert!(!view.is_dropdown_open());
}

#[test]
fn test_menu_bar_view_switching_menus_on_hover() {
    let mut view = MenuBarView::new();
    view.update_active_app("Finder");

    // Open Apple menu
    view.on_mouse_click(18.0, 14.0);
    assert!(view.is_dropdown_open());
    assert_eq!(view.active_dropdown().unwrap().menu_id, "apple");

    // Moving mouse to another menu bar item (e.g. app title at x: 50.0, y: 14.0) while dropdown is open
    // switches active dropdown without needing another click (standard macOS menu bar behavior)
    view.on_mouse_move(55.0, 14.0);
    assert!(view.is_dropdown_open());
    assert_eq!(view.active_dropdown().unwrap().menu_id, "app_root");
}

#[test]
fn test_menu_bar_view_update_active_app_closes_dropdown() {
    let mut view = MenuBarView::new();
    view.open_dropdown("apple");
    assert!(view.is_dropdown_open());

    view.update_active_app("Slack");
    assert!(!view.is_dropdown_open());
    assert_eq!(view.active_app_title(), "Slack");
}
