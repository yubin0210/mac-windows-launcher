use win_launcher::menu_bar_model::{BarAction, BarModel, BarStatus};
#[test]
fn responsive_items_do_not_overlap_and_unknown_status_is_not_invented() {
    for scale in [1.0, 1.5, 2.0] {
        for width in [640.0, 1920.0] {
            let model = BarModel { app_title: "긴 앱 이름 ".repeat(20), status: BarStatus { clock: "Wed 11:30".into(), battery: None, volume: None, muted: None } };
            let items = model.layout(width, scale);
            for item in &items {
                assert!(item.x >= 0.0 && item.x + item.width <= width + 0.1);
                assert_eq!(model.hit(width, scale, item.x + item.width/2.0, 10.0*scale), item.action);
                assert!(!item.label.contains("85%") && !item.label.contains("65%"));
            }
            for pair in items.windows(2) { assert!(pair[0].x + pair[0].width <= pair[1].x + 0.1); }
            assert!(items.iter().any(|item| item.action == Some(BarAction::Search)));
            assert!(items.iter().any(|item| item.action == Some(BarAction::DateTime)));
        }
    }
}
#[test]
fn battery_and_volume_only_use_observed_values() {
    let model = BarModel { app_title: "Editor".into(), status: BarStatus { clock: "12:30".into(), battery: Some((43, true)), volume: Some(27), muted: Some(false) } };
    let items = model.layout(1920.0, 1.0);
    assert!(items.iter().any(|item| item.label.contains("43%")));
    assert!(items.iter().any(|item| item.label.contains("27%")));
    assert_eq!(model.hit(1920.0, 1.0, 1.0, -1.0), None);
    assert!(!win_launcher::settings::Settings::default().menu_bar_enabled);
}
