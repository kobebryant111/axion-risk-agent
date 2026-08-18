use std::sync::Mutex;

pub struct AppState {
    pub conn: Mutex<rusqlite::Connection>,
}
