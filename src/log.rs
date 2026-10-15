//! Log em %APPDATA%\GameRec\gamerec.log, para diagnosticar falhas em outras maquinas.
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

const MAX_SIZE: u64 = 1024 * 1024;

static FILE: Mutex<Option<File>> = Mutex::new(None);

pub fn path() -> PathBuf {
    crate::config::config_dir().join("gamerec.log")
}

fn open() -> Option<File> {
    let path = path();
    let _ = std::fs::create_dir_all(path.parent()?);
    // mantem o anterior como .old quando passa do limite
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_SIZE) {
        let _ = std::fs::rename(&path, path.with_extension("old.log"));
    }
    OpenOptions::new().create(true).append(true).open(path).ok()
}

pub fn write(msg: &str) {
    let mut f = FILE.lock().unwrap_or_else(|e| e.into_inner());
    if f.is_none() {
        *f = open();
    }
    if let Some(f) = f.as_mut() {
        let _ = writeln!(f, "[{}] {msg}", crate::win::local_time());
    }
}

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => { $crate::log::write(&format!($($arg)*)) };
}
