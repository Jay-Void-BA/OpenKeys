#![forbid(unsafe_code)]
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() {
    gpui_platform::application()
        .with_quit_mode(gpui::QuitMode::LastWindowClosed)
        .run(openkeys_manager::ui::run);
}
