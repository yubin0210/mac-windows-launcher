//! test_taskbar_guard.rs
//! Unit tests for Windows Taskbar Auto-Hide & Fail-Safe Drop Restoration Guard.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use win_launcher::taskbar_guard::TaskbarGuard;

#[test]
fn test_taskbar_guard_lifecycle() {
    let guard = TaskbarGuard::new_mock(true);
    assert!(guard.is_hidden());
    let tracker = guard
        .restoration_tracker()
        .expect("Mock guard should provide a restoration tracker");
    assert!(!tracker.load(Ordering::SeqCst));

    drop(guard);
    // Verified via mock callback/tracker that taskbar was restored on drop
    assert!(tracker.load(Ordering::SeqCst));
}

#[test]
fn test_taskbar_guard_new_mock_false() {
    let guard = TaskbarGuard::new_mock(false);
    assert!(!guard.is_hidden());
    let tracker = guard
        .restoration_tracker()
        .expect("Mock guard should provide a restoration tracker");
    assert!(!tracker.load(Ordering::SeqCst));

    drop(guard);
    // Because it was never hidden, restoration tracker is not triggered or reflects not hidden
    assert!(!tracker.load(Ordering::SeqCst));
}

#[test]
fn test_taskbar_guard_manual_restore_idempotent() {
    let mut guard = TaskbarGuard::new_mock(true);
    assert!(guard.is_hidden());
    let tracker = guard.restoration_tracker().unwrap();
    assert!(!tracker.load(Ordering::SeqCst));

    // Manual restore
    guard.restore();
    assert!(!guard.is_hidden());
    assert!(tracker.load(Ordering::SeqCst));

    // Subsequent restore is idempotent
    guard.restore();
    assert!(!guard.is_hidden());
    assert!(tracker.load(Ordering::SeqCst));

    // Dropping already restored guard does not re-restore or panic
    drop(guard);
    assert!(tracker.load(Ordering::SeqCst));
}

#[test]
fn test_taskbar_guard_with_callback() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let count_clone = call_count.clone();

    let guard = TaskbarGuard::new_mock_with_callback(true, move || {
        count_clone.fetch_add(1, Ordering::SeqCst);
    });
    assert!(guard.is_hidden());
    assert_eq!(call_count.load(Ordering::SeqCst), 0);

    drop(guard);
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

#[test]
fn test_taskbar_guard_callback_on_manual_restore() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let count_clone = call_count.clone();

    let mut guard = TaskbarGuard::new_mock_with_callback(true, move || {
        count_clone.fetch_add(1, Ordering::SeqCst);
    });
    assert!(guard.is_hidden());
    assert_eq!(call_count.load(Ordering::SeqCst), 0);

    // Calling restore explicitly should invoke the callback
    guard.restore();
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
    assert!(!guard.is_hidden());

    // Calling restore again should not invoke callback again
    guard.restore();
    assert_eq!(call_count.load(Ordering::SeqCst), 1);

    // Dropping should not invoke callback again
    drop(guard);
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

#[test]
fn test_taskbar_guard_set_restore_callback() {
    let invoked = Arc::new(AtomicBool::new(false));
    let invoked_clone = invoked.clone();

    let mut guard = TaskbarGuard::new_mock(true);
    guard.set_restore_callback(move || {
        invoked_clone.store(true, Ordering::SeqCst);
    });

    assert!(!invoked.load(Ordering::SeqCst));
    drop(guard);
    assert!(invoked.load(Ordering::SeqCst));
}

#[test]
fn test_taskbar_guard_default_new() {
    // new(false) should not hide
    let guard_false = TaskbarGuard::new(false);
    assert!(!guard_false.is_hidden());
    drop(guard_false);

    // new(true) on non-windows behaves safely as a guard
    let guard_true = TaskbarGuard::new(true);
    assert!(guard_true.is_hidden());
    drop(guard_true);
}
