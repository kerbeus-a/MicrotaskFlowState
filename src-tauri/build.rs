fn main() {
    #[cfg(feature = "tauri-ui")]
    tauri_build::build();

    // For native-ui builds on Windows, embed the icon in the executable
    #[cfg(all(feature = "native-ui", target_os = "windows"))]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("icons/record.ico");
        res.compile().expect("Failed to compile Windows resources");
    }
}
