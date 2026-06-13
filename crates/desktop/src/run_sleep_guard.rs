//! Prevent system sleep while a workflow run is active.

use std::sync::Mutex;

use keepawake::Builder;
use tauri::{AppHandle, Manager};

pub struct RunSleepGuard(Mutex<Option<keepawake::KeepAwake>>);

impl RunSleepGuard {
    #[must_use]
    pub fn new() -> Self {
        Self(Mutex::new(None))
    }

    pub fn start(&self, app_name: &str) {
        let mut slot = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if slot.is_some() {
            return;
        }
        match Builder::default()
            .idle(true)
            .sleep(true)
            .display(false)
            .app_name(app_name)
            .reason("Workflow run in progress")
            .create()
        {
            Ok(handle) => *slot = Some(handle),
            Err(error) => eprintln!("run sleep guard: failed to prevent sleep: {error}"),
        }
    }

    pub fn stop(&self) {
        let mut slot = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        slot.take();
    }
}

pub fn start_for_app(app: &AppHandle) {
    let name = app.package_info().name.clone();
    app.state::<RunSleepGuard>().start(&name);
}

pub fn stop_for_app(app: &AppHandle) {
    app.state::<RunSleepGuard>().stop();
}
