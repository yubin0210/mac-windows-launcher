//! Bounded session logs, including errors from windowless release builds.
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

pub struct LogFile { file: File }
impl LogFile {
    pub fn open(directory: &Path, max_bytes: u64) -> io::Result<Self> {
        fs::create_dir_all(directory)?;
        let path = directory.join("launcher.log");
        if fs::metadata(&path).map(|m| m.len() >= max_bytes).unwrap_or(false) {
            let previous = directory.join("launcher.previous.log");
            if previous.exists() { fs::remove_file(&previous)?; }
            fs::rename(&path, previous)?;
        }
        Ok(Self { file: OpenOptions::new().create(true).append(true).open(path)? })
    }
    pub fn write(&mut self, message: &str) -> io::Result<()> {
        let seconds = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
        writeln!(self.file, "[{seconds}] {message}")?;
        self.file.flush()
    }
}
static LOG: OnceLock<Mutex<LogFile>> = OnceLock::new();
pub fn initialize(directory: &Path) -> io::Result<PathBuf> {
    let log = LogFile::open(directory, 2 * 1024 * 1024)?;
    LOG.set(Mutex::new(log)).map_err(|_| io::Error::new(io::ErrorKind::AlreadyExists, "logger already initialized"))?;
    std::panic::set_hook(Box::new(|info| write(&format!("PANIC: {info}"))));
    Ok(directory.join("launcher.log"))
}
pub fn write(message: &str) {
    if let Some(log) = LOG.get() {
        if let Ok(mut log) = log.lock() { let _ = log.write(message); }
    }
}
