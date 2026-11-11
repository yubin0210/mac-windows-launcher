use win_launcher::diagnostics::LogFile;

#[test]
fn log_appends_and_rotates_without_losing_previous_session() {
    let dir = std::env::temp_dir().join(format!("launcher-log-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut log = LogFile::open(&dir, 50).unwrap();
    log.write("first session").unwrap();
    drop(log);
    let mut log = LogFile::open(&dir, 50).unwrap();
    log.write("second session").unwrap();
    drop(log);
    let mut log = LogFile::open(&dir, 50).unwrap();
    log.write("third session").unwrap();
    assert!(std::fs::read_to_string(dir.join("launcher.previous.log")).unwrap().contains("first session"));
    assert!(std::fs::read_to_string(dir.join("launcher.log")).unwrap().contains("third session"));
    drop(log);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn invalid_log_directory_is_reported() {
    let file = std::env::temp_dir().join(format!("launcher-log-file-{}", std::process::id()));
    std::fs::write(&file, "not a directory").unwrap();
    assert!(LogFile::open(&file, 1024).is_err());
    std::fs::remove_file(file).unwrap();
}
