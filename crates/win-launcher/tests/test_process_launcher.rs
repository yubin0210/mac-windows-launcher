use win_launcher::app_tracker::AppTracker;
use win_launcher::dock_view::{DockItem, DockView};
use win_launcher::process_launcher::{
    format_arguments, LaunchResult, LaunchTarget, ProcessLauncher,
};

#[test]
fn test_process_launcher_activates_running_app_first() {
    let mut tracker = AppTracker::new();
    tracker.register_mock_running_app("notepad.exe", 5001, false);

    let target = LaunchTarget::AppExecutable {
        process_name: "notepad.exe".to_string(),
        command: "notepad.exe".to_string(),
        args: Vec::new(),
    };

    let result = ProcessLauncher::launch_or_activate(&target, &tracker);
    match result {
        LaunchResult::Activated { process_name, hwnd } => {
            assert_eq!(process_name, "notepad.exe");
            assert_eq!(hwnd, 5001);
        }
        _ => panic!("Expected running app to be activated instead of launched, got {:?}", result),
    }
}

#[test]
fn test_process_launcher_launches_non_running_app() {
    let tracker = AppTracker::new();

    let target = LaunchTarget::AppExecutable {
        process_name: "calc.exe".to_string(),
        command: "calc.exe".to_string(),
        args: Vec::new(),
    };

    let result = ProcessLauncher::launch_or_activate(&target, &tracker);
    match result {
        LaunchResult::Launched { process_name, .. } => {
            assert_eq!(process_name, "calc.exe");
        }
        _ => panic!("Expected non-running app to be launched, got {:?}", result),
    }
}

#[test]
fn test_process_launcher_protocol_uri() {
    let tracker = AppTracker::new();
    let target = LaunchTarget::ProtocolUri("ms-settings:".to_string());

    let result = ProcessLauncher::launch_or_activate(&target, &tracker);
    match result {
        LaunchResult::Launched { process_name, .. } => {
            assert_eq!(process_name, "ms-settings:");
        }
        _ => panic!("Expected protocol URI to be launched, got {:?}", result),
    }
}

#[test]
fn test_process_launcher_shell_action() {
    let tracker = AppTracker::new();
    let target = LaunchTarget::ShellAction("powershell".to_string());

    let result = ProcessLauncher::launch_or_activate(&target, &tracker);
    match result {
        LaunchResult::Launched { process_name, .. } => {
            assert_eq!(process_name, "powershell");
        }
        _ => panic!("Expected shell action to be launched, got {:?}", result),
    }
}

#[test]
fn test_process_launcher_empty_target_fails() {
    let tracker = AppTracker::new();
    let target = LaunchTarget::AppExecutable {
        process_name: "".to_string(),
        command: "".to_string(),
        args: Vec::new(),
    };

    let result = ProcessLauncher::launch_or_activate(&target, &tracker);
    match result {
        LaunchResult::Failed { error, .. } => {
            assert_eq!(error, "Executable command path cannot be empty");
        }
        _ => panic!("Expected empty target to fail, got {:?}", result),
    }
}

#[test]
fn test_process_launcher_empty_command_fails() {
    let tracker = AppTracker::new();
    let target = LaunchTarget::AppExecutable {
        process_name: "notepad.exe".to_string(),
        command: "   ".to_string(),
        args: Vec::new(),
    };

    let result = ProcessLauncher::launch_or_activate(&target, &tracker);
    match result {
        LaunchResult::Failed { error, .. } => {
            assert_eq!(error, "Executable command path cannot be empty");
        }
        _ => panic!("Expected empty command to fail, got {:?}", result),
    }
}

#[test]
fn test_format_arguments_quoting_whitespace() {
    let empty_args: Vec<String> = Vec::new();
    assert_eq!(format_arguments(&empty_args), "");

    let simple_args = vec!["--flag".to_string(), "file.txt".to_string()];
    assert_eq!(format_arguments(&simple_args), "--flag file.txt");

    let space_args = vec![
        "--output".to_string(),
        "C:\\Program Files\\App\\data.bin".to_string(),
        "normal".to_string(),
    ];
    assert_eq!(
        format_arguments(&space_args),
        "--output \"C:\\Program Files\\App\\data.bin\" normal"
    );

    let tab_args = vec!["arg\twith\ttabs".to_string()];
    assert_eq!(format_arguments(&tab_args), "\"arg\twith\ttabs\"");

    let already_quoted = vec!["\"C:\\My Path\"".to_string()];
    assert_eq!(format_arguments(&already_quoted), "\"C:\\My Path\"");
}

#[test]
fn test_process_launcher_names_match_tolerance() {
    let mut tracker = AppTracker::new();
    tracker.register_mock_running_app("C:\\Windows\\System32\\notepad.exe", 7001, false);

    // Target specified without .exe and lowercase
    let target = LaunchTarget::AppExecutable {
        process_name: "notepad".to_string(),
        command: "notepad.exe".to_string(),
        args: Vec::new(),
    };

    let result = ProcessLauncher::launch_or_activate(&target, &tracker);
    match result {
        LaunchResult::Activated { process_name, hwnd } => {
            assert_eq!(process_name, "notepad");
            assert_eq!(hwnd, 7001);
        }
        _ => panic!("Expected case/path-insensitive match to activate, got {:?}", result),
    }
}

#[test]
fn test_dock_item_launch_target_wiring() {
    let item = DockItem::new("test", "Test App", false).with_launch_target(LaunchTarget::AppExecutable {
        process_name: "test.exe".to_string(),
        command: "test.exe".to_string(),
        args: vec!["--arg".to_string()],
    });

    assert!(item.launch_target.is_some());
}

#[test]
fn test_dock_view_default_launch_targets() {
    let dock_view = DockView::new();

    // Check Finder -> explorer.exe
    let finder_target = dock_view.get_launch_target(0);
    assert!(finder_target.is_some());
    match finder_target.unwrap() {
        LaunchTarget::AppExecutable { process_name, .. } => {
            assert_eq!(process_name, "explorer.exe");
        }
        _ => panic!("Expected Finder to have explorer.exe AppExecutable target"),
    }

    // Check Safari -> msedge.exe
    let safari_target = dock_view.get_launch_target(2);
    assert!(safari_target.is_some());
    match safari_target.unwrap() {
        LaunchTarget::AppExecutable { process_name, .. } => {
            assert_eq!(process_name, "msedge.exe");
        }
        _ => panic!("Expected Safari to have msedge.exe AppExecutable target"),
    }

    // Check Terminal -> wt.exe
    let terminal_target = dock_view.get_launch_target(8);
    assert!(terminal_target.is_some());
    match terminal_target.unwrap() {
        LaunchTarget::AppExecutable { process_name, .. } => {
            assert_eq!(process_name, "wt.exe");
        }
        _ => panic!("Expected Terminal to have wt.exe AppExecutable target"),
    }

    // Check Settings -> systemsettings.exe
    let settings_target = dock_view.get_launch_target(9);
    assert!(settings_target.is_some());
    match settings_target.unwrap() {
        LaunchTarget::AppExecutable { process_name, .. } => {
            assert_eq!(process_name, "systemsettings.exe");
        }
        _ => panic!("Expected Settings to have systemsettings.exe AppExecutable target"),
    }

    // Out of bounds index
    assert!(dock_view.get_launch_target(999).is_none());
}

#[test]
fn test_launch_target_convenience_constructors() {
    let app = LaunchTarget::app("notepad.exe", "notepad.exe");
    assert_eq!(app.name(), "notepad.exe");

    let app_args = LaunchTarget::app_with_args("code", "code.cmd", vec![".".to_string()]);
    assert_eq!(app_args.name(), "code");

    let proto = LaunchTarget::protocol("https://github.com");
    assert_eq!(proto.name(), "https://github.com");

    let shell = LaunchTarget::shell("dir");
    assert_eq!(shell.name(), "dir");
}

#[test]
fn test_process_launcher_explicit_hwnd_activation() {
    let mut tracker = AppTracker::new();
    tracker.register_mock_running_app_with_hwnd("notepad.exe", 5001, false, 0x12345678);

    let target = LaunchTarget::app("notepad.exe", "notepad.exe");
    let result = ProcessLauncher::launch_or_activate(&target, &tracker);

    match result {
        LaunchResult::Activated { process_name, hwnd } => {
            assert_eq!(process_name, "notepad.exe");
            assert_eq!(hwnd, 0x12345678);
        }
        _ => panic!("Expected activation with explicit hwnd"),
    }
}

#[test]
fn test_process_launcher_empty_protocol_and_shell_fail() {
    let tracker = AppTracker::new();

    let empty_proto = LaunchTarget::ProtocolUri("   ".to_string());
    match ProcessLauncher::launch_or_activate(&empty_proto, &tracker) {
        LaunchResult::Failed { error, .. } => {
            assert!(error.contains("empty"));
        }
        other => panic!("Expected failure for empty protocol URI, got {other:?}"),
    }

    let empty_shell = LaunchTarget::ShellAction("".to_string());
    match ProcessLauncher::launch_or_activate(&empty_shell, &tracker) {
        LaunchResult::Failed { error, .. } => {
            assert!(error.contains("empty"));
        }
        other => panic!("Expected failure for empty shell action, got {other:?}"),
    }
}
