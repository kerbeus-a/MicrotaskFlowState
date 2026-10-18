#[path = "src/icon.rs"]
#[allow(dead_code)]
mod icon;

fn main() {
    println!("cargo:rerun-if-changed=src/icon.rs");
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let ico_path = out_dir.join("flowstate.ico");
    std::fs::write(&ico_path, icon::render_ico(&[16, 20, 24, 32, 40, 48, 64, 128, 256]))
        .expect("Failed to write app icon");

    // Embed the icon in the .exe so Explorer, the taskbar and shortcuts show it.
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winres::WindowsResource::new();
        res.set_icon(ico_path.to_str().unwrap());
        res.compile().expect("Failed to compile Windows resources");
    }
}
