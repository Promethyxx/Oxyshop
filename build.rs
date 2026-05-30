fn main() {
    slint_build::compile("ui/main.slint").unwrap();
    windows_resources();
}

#[cfg(target_os = "windows")]
fn windows_resources() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    println!("cargo:rustc-link-arg-bins=/SUBSYSTEM:WINDOWS");
    println!("cargo:rustc-link-arg-bins=/ENTRY:mainCRTStartup");
    let icon = std::path::Path::new("assets/Oxyshop_icon.ico");
    if icon.exists() {
        let mut res = winresource::WindowsResource::new();
        res.set_icon(icon.to_str().unwrap());
        res.compile().expect("Failed to compile Windows resources");
    }
}

#[cfg(not(target_os = "windows"))]
fn windows_resources() {}
