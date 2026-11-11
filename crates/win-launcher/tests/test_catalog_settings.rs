use win_launcher::app_catalog::{AppCatalog, AppRecord};
use win_launcher::settings::{Settings, SettingsStore};

fn app(path: &str, title: &str) -> AppRecord {
    AppRecord::new(path, title, None)
}

#[test]
fn catalog_uses_identity_not_title_and_retains_pin_order() {
    let a = app(r"C:\Apps\One.lnk", "편집기");
    let b = app(r"C:\Apps\Two.lnk", "편집기");
    let catalog = AppCatalog::new(vec![a.clone(), b.clone(), app(r"c:\apps\ONE.lnk", "duplicate")]);
    assert_eq!(catalog.apps().len(), 2);
    let selected = catalog.pinned(&[b.id.clone(), "missing-app".into(), a.id.clone()]);
    assert_eq!(selected.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), vec![b.id.as_str(), a.id.as_str()]);
    let results = catalog.spotlight_items();
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|item| item.subtitle != "Application"), "duplicate names need a visible source distinction");
}

#[test]
fn pin_changes_keep_unknown_ids_and_allow_an_empty_dock() {
    let mut settings = Settings::default();
    settings.pinned = Some(vec!["missing".into(), "one".into(), "two".into()]);
    settings.move_pin("two", -1);
    assert_eq!(settings.pinned.as_ref().unwrap(), &["missing", "two", "one"]);
    settings.pin("two");
    assert_eq!(settings.pinned.as_ref().unwrap().len(), 3);
    settings.unpin("missing"); settings.unpin("two"); settings.unpin("one");
    assert_eq!(settings.pinned, Some(vec![]));
}

fn directory(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("launcher-{name}-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()))
}

#[test]
fn settings_round_trip_preserves_unicode_order_and_explicit_empty() {
    let dir = directory("settings");
    let store = SettingsStore::new(dir.join("settings.json"));
    assert_eq!(store.load().unwrap().settings.pinned, None);
    let mut settings = Settings::default();
    settings.pinned = Some(vec!["한글 경로".into(), "second".into()]);
    settings.monitor = Some("\\\\.\\DISPLAY2".into());
    settings.icon_size = 64;
    store.save(&settings).unwrap();
    assert_eq!(store.load().unwrap().settings, settings);
    settings.pinned = Some(vec![]);
    store.save(&settings).unwrap();
    assert_eq!(store.load().unwrap().settings.pinned, Some(vec![]));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn corrupt_settings_are_backed_up_and_future_versions_are_not_overwritten() {
    let dir = directory("corrupt");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("settings.json");
    let original = b"{broken settings";
    std::fs::write(&file, original).unwrap();
    let store = SettingsStore::new(file.clone());
    let recovered = store.load().unwrap();
    let backup = recovered.backup.unwrap();
    assert_eq!(std::fs::read(backup).unwrap(), original);
    assert_eq!(std::fs::read(&file).unwrap(), original);
    std::fs::write(&file, br#"{"version":999,"pinned":["one"]}"#).unwrap();
    assert!(store.load().is_err());
    assert!(std::fs::read_to_string(&file).unwrap().contains("999"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn full_executable_identity_does_not_match_another_installation() {
    let mut tracker = win_launcher::AppTracker::new();
    tracker.register_mock_running_app(r"C:\Apps\One\editor.exe", 10, true);
    assert!(!tracker.is_app_running(r"D:\Apps\Two\editor.exe"));
    assert!(tracker.is_app_running(r"c:\apps\one\EDITOR.exe"));
    assert!(tracker.is_app_running("editor.exe")); // Legacy basename queries still work.
}

#[test]
fn running_apps_can_be_pinned_and_survive_restart_without_duplicates() {
    use win_launcher::dock_session::DockSession;
    let pinned = app(r"C:\Menu\Editor.lnk", "Editor");
    let mut pinned = pinned;
    pinned.executable = Some(r"C:\Apps\editor.exe".into());
    let catalog = AppCatalog::new(vec![pinned.clone()]);
    let mut settings = Settings::default();
    settings.pinned = Some(vec![pinned.id.clone()]);
    let mut session = DockSession::new(catalog, settings);
    let mut tracker = win_launcher::AppTracker::new();
    tracker.register_mock_running_app(r"C:\Apps\editor.exe", 1, true);
    tracker.register_mock_running_app(r"C:\Apps\한글.exe", 2, false);
    session.refresh(&tracker);
    let items = session.visible(10);
    assert_eq!(items.len(), 2);
    let running_id = items[1].id.clone();
    let saved = session.pin_settings(&running_id).unwrap();
    let saved = serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
    let restarted = DockSession::new(AppCatalog::default(), saved);
    assert!(restarted.visible(10).iter().any(|app| app.id == running_id));
    assert_eq!(session.visible(1).len(), 1);
    session.page_by(1, 1);
    assert_eq!(session.visible(1)[0].id, running_id);
}

#[test]
fn first_run_can_pin_packaged_apps_and_explicit_empty_stays_empty() {
    use win_launcher::dock_session::DockSession;
    let catalog = AppCatalog::new(vec![app("shell:AppsFolder\\Package!App", "메모장")]);
    assert_eq!(DockSession::new(catalog.clone(), Settings::default()).visible(10).len(), 1);
    let mut settings = Settings::default(); settings.pinned = Some(vec![]);
    assert!(DockSession::new(catalog, settings).visible(10).is_empty());
}

#[test]
fn unsupported_save_does_not_replace_existing_settings() {
    let dir = directory("save");
    let path = dir.join("settings.json");
    let store = SettingsStore::new(path.clone());
    store.save(&Settings::default()).unwrap();
    let original = std::fs::read(&path).unwrap();
    let mut future = Settings::default(); future.version = 99;
    assert!(store.save(&future).is_err());
    assert_eq!(std::fs::read(path).unwrap(), original);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn packaged_app_identity_connects_pin_to_running_window() {
    use win_launcher::dock_session::DockSession;
    let pinned = app("shell:AppsFolder\\Package!Calculator", "계산기").with_app_id("Package!Calculator".into());
    let mut settings = Settings::default(); settings.pinned = Some(vec![pinned.id.clone()]);
    let mut session = DockSession::new(AppCatalog::new(vec![pinned.clone()]), settings);
    let mut tracker = win_launcher::AppTracker::new();
    tracker.register_mock_running_app_with_app_id(r"C:\WindowsApps\calc.exe", 40, true, "Package!Calculator");
    assert!(pinned.matches(&tracker.all_apps()[0]));
    session.refresh(&tracker);
    assert_eq!(session.visible(10).len(), 1);
}

#[test]
fn different_packaged_apps_sharing_a_host_remain_separate() {
    use win_launcher::dock_session::DockSession;
    let mut settings = Settings::default(); settings.pinned = Some(vec![]);
    let mut session = DockSession::new(AppCatalog::default(), settings);
    let mut tracker = win_launcher::AppTracker::new();
    tracker.register_mock_running_app_with_app_id(r"C:\Windows\host.exe", 1, true, "One!App");
    tracker.register_mock_running_app_with_app_id(r"C:\Windows\host.exe", 2, false, "Two!App");
    session.refresh(&tracker);
    assert_eq!(session.visible(10).len(), 2);
}

#[test]
fn unregistered_desktop_app_id_does_not_replace_executable_launch_target() {
    use win_launcher::dock_session::DockSession;
    let mut settings = Settings::default(); settings.pinned = Some(vec![]);
    let mut session = DockSession::new(AppCatalog::default(), settings);
    let mut tracker = win_launcher::AppTracker::new();
    tracker.register_mock_running_app_with_app_id(r"C:\Portable\editor.exe", 3, true, "Custom.Desktop.Editor");
    session.refresh(&tracker);
    let record = session.visible(10).remove(0);
    assert_eq!(record.target, r"C:\Portable\editor.exe");
    assert_eq!(record.app_id.as_deref(), Some("Custom.Desktop.Editor"));
}
