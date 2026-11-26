//! Gives the Windows exe its icon. Does nothing when building anywhere else.

fn main() {
    println!("cargo:rerun-if-changed=assets/mo4.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/mo4.ico");
        if let Err(e) = resource.compile() {
            println!("cargo:warning=the exe has no icon: {e}");
        }
    }
}
