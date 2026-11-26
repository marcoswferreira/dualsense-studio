fn main() {
    #[cfg(target_os = "windows")]
    {
        let ico_path = std::path::Path::new("../../packaging/windows/dualsense-studio.ico");
        println!("cargo:rerun-if-changed={}", ico_path.display());
        if ico_path.exists() {
            let mut res = winresource::WindowsResource::new();
            res.set_icon(ico_path.to_str().unwrap_or_default());
            if let Err(e) = res.compile() {
                println!("cargo:warning=Failed to embed Windows icon resource: {e}");
            }
        }
    }
}
