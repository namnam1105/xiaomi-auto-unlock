use chrono::Local;
use once_cell::sync::OnceCell;
use slint::{ComponentHandle, Weak};
use std::sync::Mutex;

use crate::MainWindow;

#[allow(dead_code, unused_variables, unused_imports)]

// Global storage for our window reference
static WINDOW: OnceCell<Mutex<Option<Weak<MainWindow>>>> = OnceCell::new();

pub fn init(window: &MainWindow) {
    let window_weak = window.as_weak();
    WINDOW.get_or_init(|| Mutex::new(Some(window_weak)));
}

// Log a message to the window
pub fn log<T: std::fmt::Display>(message: T) {
    if let Some(window_lock) = WINDOW.get() {
        if let Ok(window_ref) = window_lock.lock() {
            if let Some(window_weak) = window_ref.clone() {
                let message_str = format!("{}", message);
                let now = Local::now();
                let timestamp = now.format("%H:%M:%S").to_string();
                let formatted_log = format!("[{}] {}\n", timestamp, message_str);

                slint::invoke_from_event_loop(move || {
                    if let Some(window) = window_weak.upgrade() {
                        let current_logs = window.get_logs();
                        let updated_logs = format!("{}{}", current_logs, formatted_log);
                        window.set_logs(updated_logs.into());
                    }
                })
                .unwrap();
            }
        }
    }
}

pub fn update_status(ready: bool, message: &str) {
    if let Some(window_lock) = WINDOW.get() {
        if let Ok(window_ref) = window_lock.lock() {
            if let Some(window_weak) = &*window_ref {
                if let Some(window) = window_weak.upgrade() {
                    window.set_status_text(message.into());
                    window.set_ready(ready);
                }
            }
        }
    }
}

// Clear logs
pub fn clear() {
    if let Some(window_lock) = WINDOW.get() {
        if let Ok(window_ref) = window_lock.lock() {
            if let Some(window_weak) = &*window_ref {
                if let Some(window) = window_weak.upgrade() {
                    window.set_logs("".into());
                }
            }
        }
    }
}
