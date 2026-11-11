#[test]
fn version_is_machine_readable_and_does_not_start_the_runtime(){
 let output=std::process::Command::new(env!("CARGO_BIN_EXE_win-launcher")).arg("--version").output().unwrap();
 assert!(output.status.success());
 assert_eq!(String::from_utf8(output.stdout).unwrap().trim(),format!("MacLauncher {}",win_launcher::version()));
}
#[test]
#[cfg(not(windows))]
fn quit_does_not_run_the_non_windows_demo(){
 let output=std::process::Command::new(env!("CARGO_BIN_EXE_win-launcher")).arg("--quit").output().unwrap();
 assert!(!output.status.success());assert!(output.stdout.is_empty());
}
