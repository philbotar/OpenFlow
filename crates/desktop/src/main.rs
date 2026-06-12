#![allow(
    clippy::cargo,
    clippy::nursery,
    clippy::pedantic,
    reason = "Tauri desktop shell; strict pedantic/nursery lint not enforced on thin IPC glue"
)]

fn main() {
    desktop::run();
}
