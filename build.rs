//! Windows: embed the icon in the executable as resource 1, the one gpui's
//! Windows platform loads for the window (`LoadImageW(module, 1, ...)`)
//! and the one Explorer shows. The other platforms take the icon from the
//! bundle (macOS) or the desktop entry (Linux), so nothing happens here.
//!
//! The resource compiler comes with the Windows SDK; without it the build
//! still succeeds, only without the icon, so `cargo install` is never
//! stopped by a missing `rc.exe`.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/icon/smep.ico");
    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/icon/smep.ico");
        if let Err(err) = resource.compile() {
            println!("cargo:warning=smep builds without its icon: {err}");
        }
    }
}
