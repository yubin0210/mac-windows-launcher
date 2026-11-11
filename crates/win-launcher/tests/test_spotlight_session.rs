use win_launcher::{app_catalog::{AppCatalog, AppRecord}, indexer::AppIndexer, spotlight_view::SpotlightView};
use win_launcher::display::WorkArea;
use win_launcher::spotlight_layout::SpotlightGeometry;

#[test]
fn search_uses_shared_app_identity_and_refresh_keeps_the_current_query() {
    let one = AppRecord::new(r"C:\앱\하나.lnk", "메모 편집기", None);
    let two = AppRecord::new(r"D:\앱\둘.lnk", "메모 편집기", None);
    let mut view = SpotlightView::new(AppIndexer::from_catalog(&AppCatalog::new(vec![one.clone(), two.clone()])));
    view.set_query("메모");
    assert_eq!(view.results_count(), 2);
    view.set_selected_index(1);
    let selected = view.selected_item().unwrap().id.clone();
    view.replace_index(AppIndexer::from_catalog(&AppCatalog::new(vec![two, one])));
    assert_eq!(view.get_query(), "메모");
    assert_eq!(view.selected_item().unwrap().id, selected);
    view.set_query("수학😀 없는 검색어");
    assert!(view.selected_item().is_none());
}

#[test]
fn popup_geometry_limits_rows_and_hit_testing_at_each_dpi() {
    for dpi in [96, 144, 192] {
        for width in [640, 1920] {
            let area = WorkArea { left: -width, top: -100, right: 0, bottom: 580 };
            let geometry = SpotlightGeometry::fit(area, dpi, 5, true).unwrap();
            assert!(geometry.x >= area.left && geometry.y >= area.top);
            assert!(geometry.x + geometry.width <= area.right);
            assert!(geometry.y + geometry.height <= area.bottom);
            assert!(geometry.capacity > 0 && geometry.visible_rows <= geometry.capacity);
            assert_eq!(geometry.row_at(geometry.header + geometry.row_height / 2.0), Some(0));
            assert_eq!(geometry.row_at(geometry.header - 1.0), None);
            assert_eq!(geometry.row_at(geometry.height as f32 - 1.0), None);
        }
    }
    let empty = SpotlightGeometry::fit(WorkArea { left: 0, top: 0, right: 1920, bottom: 1040 }, 96, 0, true).unwrap();
    assert_eq!(empty.row_at(empty.header + 10.0), None);
    assert!(empty.height as f32 > empty.header); // A no-results message remains visible.
}

#[test]
fn default_shortcut_and_unicode_clipboard_encoding_are_explicit() {
    use win_launcher::settings::{Settings, SpotlightHotkey};
    assert_eq!(Settings::default().spotlight_hotkey, SpotlightHotkey::AltSpace);
    let units = win_launcher::clipboard::unicode_text("결과 😀").unwrap();
    assert_eq!(units.last(), Some(&0));
    assert_eq!(String::from_utf16(&units[..units.len()-1]).unwrap(), "결과 😀");
    assert!(win_launcher::clipboard::unicode_text("bad\0text").is_err());
}

#[test]
fn a_click_from_old_painted_results_cannot_execute_a_new_row() {
    let one = AppRecord::new(r"C:\one.lnk", "First app", None);
    let two = AppRecord::new(r"C:\two.lnk", "Second app", None);
    let mut view = SpotlightView::new(AppIndexer::from_catalog(&AppCatalog::new(vec![one.clone(), two.clone()])));
    view.set_query("First");
    let painted_id = view.selected_item().unwrap().id.clone();
    view.set_query("Second");
    assert!(!view.select_id(&painted_id));
    assert_eq!(view.selected_item().unwrap().id, two.id);
}

#[test]
fn unicode_query_at_the_edit_limit_is_preserved_and_hotkey_migration_is_additive() {
    use win_launcher::settings::{Settings, SpotlightHotkey};
    let query = format!("{}{}", "가".repeat(2048), "😀".repeat(1024));
    assert_eq!(query.encode_utf16().count(), 4096);
    let mut view = SpotlightView::new(AppIndexer::new());
    view.set_query(&query);
    assert_eq!(view.get_query(), query);
    assert!(view.is_empty());
    let mut settings: Settings = serde_json::from_str(r#"{"version":1,"pinned":[]}"#).unwrap();
    assert_eq!(settings.spotlight_hotkey, SpotlightHotkey::AltSpace);
    settings.spotlight_hotkey = SpotlightHotkey::CtrlAltSpace;
    let restored: Settings = serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
    assert_eq!(restored.spotlight_hotkey, SpotlightHotkey::CtrlAltSpace);
    assert_eq!(restored.pinned, Some(vec![]));
}
