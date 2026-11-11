#![cfg_attr(windows, windows_subsystem = "windows")]
//! Platform entry point. The macOS demo does not verify the Windows runtime.
#[cfg(not(windows))]
mod demo;

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments == ["--version"] { println!("MacLauncher {}", win_launcher::version()); return; }
    if arguments == ["--quit"] {
        #[cfg(windows)]
        { if let Err(error) = win_launcher::shutdown::request() { win_launcher::diagnostics::write(&error); std::process::exit(1); } return; }
        #[cfg(not(windows))]
        { eprintln!("--quit is available only on Windows"); std::process::exit(2); }
    }
    if arguments == ["--smoke-test"] || arguments == ["--smoke-test", "--wait-for-quit"] {
        #[cfg(windows)]
        { win_launcher::runtime::entry(); return; }
        #[cfg(not(windows))]
        { eprintln!("--smoke-test is available only on Windows"); std::process::exit(2); }
    }
    if !arguments.is_empty() { eprintln!("Usage: win-launcher [--version | --quit | --smoke-test]"); std::process::exit(2); }

    #[cfg(windows)]
    win_launcher::runtime::entry();
    #[cfg(not(windows))]
    demo::run();
}
