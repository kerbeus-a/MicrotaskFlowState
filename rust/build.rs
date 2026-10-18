fn main() {
    #[cfg(all(feature = "native-ui", target_os = "windows"))]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("icons/record.ico");
        res.compile().expect("Failed to compile Windows resources");
    }
}
