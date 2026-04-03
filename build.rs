fn main() {
    slint_build::compile("ui/main.slint").unwrap();

    #[cfg(windows)]
    {
        let icon = std::path::Path::new("assets/Oxyshop_icon.ico");
        if icon.exists() {
            let mut res = winresource::WindowsResource::new();
            res.set_icon(icon.to_str().unwrap());
            res.compile().expect("Failed to compile Windows resources");
        }
    }
}
