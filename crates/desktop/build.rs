fn main() {
    // tauri-build watches tauri.conf.json but not bundle icons; dev embeds icon.icns at compile time.
    println!("cargo:rerun-if-changed=icons/icon.icns");
    println!("cargo:rerun-if-changed=icons/icon.png");
    tauri_build::build();
}
