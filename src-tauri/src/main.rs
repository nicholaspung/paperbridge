// No console window on Windows release builds (harmless elsewhere).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    linux_webkit_workaround();
    paperbridge_lib::run()
}

/// WebKitGTK's DMA-BUF renderer crashes with "Error 71 (Protocol error)" on
/// Wayland with the NVIDIA driver (seen on Hyprland, RTX A6000). Turn that
/// renderer off there, unless the user has set the variable themselves.
fn linux_webkit_workaround() {
    #[cfg(target_os = "linux")]
    {
        const VAR: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";
        let nvidia = std::path::Path::new("/sys/module/nvidia").exists();
        let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
        if nvidia && wayland && std::env::var_os(VAR).is_none() {
            // Single-threaded at this point: nothing else has started yet.
            std::env::set_var(VAR, "1");
        }
    }
}
