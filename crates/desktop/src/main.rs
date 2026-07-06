#![allow(
    clippy::cargo,
    clippy::nursery,
    clippy::pedantic,
    reason = "Tauri desktop shell; strict pedantic/nursery lint not enforced on thin IPC glue"
)]

fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("orchestration=info,desktop=info"),
    )
    .init();
    desktop::run();
}
