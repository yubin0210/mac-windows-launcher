//! test_dwm_thumbnail
//! Unit and integration tests for DWM thumbnail stream manager with RAII cleanup.

use mac_physics_core::Rect;
use win_launcher::dwm_thumbnail::{
    DwmThumbnailManager, MockThumbnailController, ThumbnailController, ThumbnailRegistration,
};

#[test]
fn test_thumbnail_controller_trait_direct() {
    let mut controller = MockThumbnailController::new();
    assert!(!controller.is_registered(1));

    let thumb_id = controller.register(100, 200).unwrap();
    assert!(controller.is_registered(thumb_id));

    let rect = Rect::new(10.0, 10.0, 50.0, 50.0);
    controller.update(thumb_id, rect, 200, true).unwrap();

    let records = controller.active_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records.get(&thumb_id).unwrap().dest_rect, rect);

    controller.unregister(thumb_id).unwrap();
    assert!(!controller.is_registered(thumb_id));
    assert_eq!(controller.registered_count(), 0);

    // Register two and test clear
    let t1 = controller.register(100, 201).unwrap();
    let t2 = controller.register(100, 202).unwrap();
    assert_eq!(controller.registered_count(), 2);
    controller.clear();
    assert_eq!(controller.registered_count(), 0);
    assert!(!controller.is_registered(t1));
    assert!(!controller.is_registered(t2));
}

#[test]
fn test_registration_and_update_stream() {
    let mut manager = DwmThumbnailManager::new_mock(100);
    assert_eq!(manager.active_registrations().len(), 0);
    assert!(!manager.is_window_streamed(1001));

    let rect1 = Rect::new(10.0, 20.0, 200.0, 150.0);
    let thumb_id = manager
        .register_window(1001, rect1, 255)
        .expect("registration should succeed");
    assert!(thumb_id > 0);
    assert!(manager.is_window_streamed(1001));
    assert_eq!(manager.active_registrations().len(), 1);

    let reg: &ThumbnailRegistration = manager.active_registrations().get(&1001).unwrap();
    assert_eq!(reg.window_id, 1001);
    assert_eq!(reg.thumb_id, thumb_id);
    assert_eq!(reg.target_rect, rect1);
    assert_eq!(reg.opacity, 255);
    assert!(reg.is_visible);

    // Update thumbnail properties
    let rect2 = Rect::new(30.0, 40.0, 250.0, 180.0);
    manager
        .update_window(1001, rect2, 200, true)
        .expect("update should succeed");

    let reg = manager.active_registrations().get(&1001).unwrap();
    assert_eq!(reg.target_rect, rect2);
    assert_eq!(reg.opacity, 200);
    assert!(reg.is_visible);
}

#[test]
fn test_sync_thumbnails_updating_existing_adding_new_and_pruning_stale() {
    let mut manager = DwmThumbnailManager::new_mock(100);

    let rect1 = Rect::new(0.0, 0.0, 100.0, 100.0);
    let rect2 = Rect::new(120.0, 0.0, 100.0, 100.0);

    // Initial sync with 2 windows
    let desired_initial = vec![(101, rect1, 255), (102, rect2, 200)];
    manager
        .sync_thumbnails(&desired_initial)
        .expect("initial sync should succeed");

    assert_eq!(manager.active_registrations().len(), 2);
    assert!(manager.is_window_streamed(101));
    assert!(manager.is_window_streamed(102));

    let thumb1_orig = manager.active_registrations().get(&101).unwrap().thumb_id;
    let thumb2_orig = manager.active_registrations().get(&102).unwrap().thumb_id;

    // Second sync: modify 101, remove 102, add 103
    let rect1_mod = Rect::new(10.0, 10.0, 150.0, 150.0);
    let rect3 = Rect::new(240.0, 0.0, 100.0, 100.0);
    let desired_next = vec![(101, rect1_mod, 220), (103, rect3, 255)];

    manager
        .sync_thumbnails(&desired_next)
        .expect("second sync should succeed");

    assert_eq!(manager.active_registrations().len(), 2);
    assert!(manager.is_window_streamed(101));
    assert!(!manager.is_window_streamed(102));
    assert!(manager.is_window_streamed(103));

    // Window 101 should preserve the original thumbnail handle (reused)
    let reg1 = manager.active_registrations().get(&101).unwrap();
    assert_eq!(reg1.thumb_id, thumb1_orig);
    assert_eq!(reg1.target_rect, rect1_mod);
    assert_eq!(reg1.opacity, 220);

    // Window 103 should have a fresh thumbnail handle distinct from 101 & 102
    let reg3 = manager.active_registrations().get(&103).unwrap();
    assert_ne!(reg3.thumb_id, thumb1_orig);
    assert_ne!(reg3.thumb_id, thumb2_orig);
}

#[test]
fn test_sync_thumbnails_idempotency() {
    let mut manager = DwmThumbnailManager::new_mock(100);

    let rect1 = Rect::new(10.0, 10.0, 100.0, 80.0);
    let rect2 = Rect::new(120.0, 10.0, 100.0, 80.0);
    let desired = vec![(201, rect1, 240), (202, rect2, 240)];

    manager.sync_thumbnails(&desired).unwrap();
    let reg1_id = manager.active_registrations().get(&201).unwrap().thumb_id;
    let reg2_id = manager.active_registrations().get(&202).unwrap().thumb_id;

    // Sync with identical desired configuration
    manager.sync_thumbnails(&desired).unwrap();

    assert_eq!(manager.active_registrations().len(), 2);
    assert_eq!(
        manager.active_registrations().get(&201).unwrap().thumb_id,
        reg1_id
    );
    assert_eq!(
        manager.active_registrations().get(&202).unwrap().thumb_id,
        reg2_id
    );
}

#[test]
fn test_unregister_single_window_and_unregister_all() {
    let mut manager = DwmThumbnailManager::new_mock(100);

    let rect = Rect::new(0.0, 0.0, 50.0, 50.0);
    manager.register_window(301, rect, 255).unwrap();
    manager.register_window(302, rect, 255).unwrap();
    manager.register_window(303, rect, 255).unwrap();
    assert_eq!(manager.active_registrations().len(), 3);

    // Unregister single window 302
    manager.unregister_window(302).unwrap();
    assert_eq!(manager.active_registrations().len(), 2);
    assert!(manager.is_window_streamed(301));
    assert!(!manager.is_window_streamed(302));
    assert!(manager.is_window_streamed(303));

    // Unregister non-existent window should return an error or handle gracefully
    assert!(manager.unregister_window(9999).is_err());

    // Unregister all
    manager.unregister_all();
    assert_eq!(manager.active_registrations().len(), 0);
    assert!(!manager.is_window_streamed(301));
    assert!(!manager.is_window_streamed(303));
}

#[test]
fn test_raii_drop_unregisters_all_thumbnails() {
    let mock = MockThumbnailController::new();
    let mock_clone = mock.clone();

    {
        let mut manager = DwmThumbnailManager::new(100, Box::new(mock_clone));
        let rect = Rect::new(0.0, 0.0, 100.0, 100.0);
        manager.register_window(401, rect, 255).unwrap();
        manager.register_window(402, rect, 255).unwrap();

        assert_eq!(manager.active_registrations().len(), 2);
        assert_eq!(mock.registered_count(), 2);
        assert_eq!(mock.unregistrations_history().len(), 0);
    } // manager drops here

    // After RAII drop, mock should reflect all thumbnails unregistered
    assert_eq!(mock.registered_count(), 0);
    assert_eq!(mock.unregistrations_history().len(), 2);
}

#[test]
fn test_error_handling_invalid_hwnd() {
    // Dest HWND is 0
    let mut manager_invalid_dest = DwmThumbnailManager::new_mock(0);
    let rect = Rect::new(0.0, 0.0, 100.0, 100.0);
    let err = manager_invalid_dest.register_window(501, rect, 255);
    assert!(err.is_err());

    // Source HWND is 0
    let mut manager_valid_dest = DwmThumbnailManager::new_mock(100);
    let err = manager_valid_dest.register_window(0, rect, 255);
    assert!(err.is_err());
}
