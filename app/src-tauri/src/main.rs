// Prevents an additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // elevated copy started by the "repair" button: hosts file + CA, then exit (Windows only)
    #[cfg(windows)]
    {
        let args: Vec<String> = std::env::args().collect();
        if args.len() == 4 && args[1] == "--admin-setup" {
            std::process::exit(aeternum_launcher_lib::admin_setup(&args[2], &args[3]));
        }
        // self-update: the new exe replaces the old one (backup + rollback), restarts it and exits; no window
        if args.len() == 4 && args[1] == "--apply-update" {
            std::process::exit(aeternum_launcher_lib::apply_update(&args[2], &args[3]));
        }
    }
    // one launcher per PC (fixed-name mutex, whatever the exe name / copy); a launcher restarted by the self-update
    // waits for the old one to release it
    let restart = std::env::args().skip(1).any(|a| a.starts_with("--updated=") || a.starts_with("--update-error="));
    if !aeternum_launcher_lib::single_instance(restart) {
        return;
    }
    // WebKitGTK: the DMA-BUF renderer gives a blank window on some GPU/Wayland setups (NVIDIA...)
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    aeternum_launcher_lib::run()
}
