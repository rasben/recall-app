use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, Arc, Mutex};

pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    pub db_path: PathBuf,
    pub ical_syncing: Arc<AtomicBool>,
}
