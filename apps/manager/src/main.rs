#![forbid(unsafe_code)]
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use eframe::egui::{self, Color32, RichText};
use std::{
    io::Read,
    time::{Duration, Instant, SystemTime},
};
use vault_import::{Entry, MAX_FILE_BYTES, parse_import};
use zeroize::Zeroizing;

/// A capability must originate from a verified device channel. The integration
/// build intentionally has no constructor that accepts a host Boolean or PIN.
struct Access {
    expires: Instant,
    issued_wall: SystemTime,
    expires_wall: SystemTime,
}
impl Access {
    fn live(&self) -> bool {
        self.live_at(Instant::now(), SystemTime::now())
    }
    fn live_at(&self, monotonic: Instant, wall: SystemTime) -> bool {
        // Some platform monotonic clocks pause during sleep. This independent
        // display/privacy guard also denies wall-clock rollback. Device-side
        // authorization and USB suspend revocation remain authoritative.
        monotonic < self.expires && wall >= self.issued_wall && wall < self.expires_wall
    }
}

fn interrupted_frame(now: Instant, last: Instant, wall: SystemTime, last_wall: SystemTime) -> bool {
    now.checked_duration_since(last)
        .is_none_or(|gap| gap > Duration::from_secs(5))
        || wall
            .duration_since(last_wall)
            .map_or(true, |gap| gap > Duration::from_secs(5))
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Overview,
    Import,
    Notes,
    Recovery,
}

struct Pending {
    entry: Entry,
    selected: bool,
}

#[derive(Default)]
struct HardwareBackend;
impl HardwareBackend {
    fn unlock(&mut self) -> Result<Access, &'static str> {
        // No authenticated sensor/channel driver exists for a selected PCB yet.
        // Fail closed: discovery, a click, or an OS biometric prompt is not UV.
        Err(
            "Device authorization is not integrated. This build cannot unlock a vault or accept real secrets.",
        )
    }
    fn commit(&mut self, _entries: &[&Entry], _access: &Access) -> Result<(), &'static str> {
        Err("Device storage and encrypted transport are not integrated. No data was written.")
    }
}

struct AuthDesk {
    backend: HardwareBackend,
    access: Option<Access>,
    tab: Tab,
    pending: Vec<Pending>,
    query: Zeroizing<String>,
    message: String,
    devices: Vec<String>,
    scan: Option<std::sync::mpsc::Receiver<Result<Vec<String>, ()>>>,
    last_frame: Instant,
    last_wall_frame: SystemTime,
}

impl Default for AuthDesk {
    fn default() -> Self {
        Self {
            backend: HardwareBackend,
            access: None,
            tab: Tab::Overview,
            pending: Vec::new(),
            query: Zeroizing::new(String::new()),
            message: String::new(),
            devices: Vec::new(),
            scan: None,
            last_frame: Instant::now(),
            last_wall_frame: SystemTime::now(),
        }
    }
}

impl AuthDesk {
    fn unlocked(&self) -> bool {
        self.access.as_ref().is_some_and(Access::live)
    }
    fn lock(&mut self) {
        self.access = None;
        self.pending.clear();
        self.query = Zeroizing::new(String::new());
        self.message.clear();
        self.tab = Tab::Overview;
    }
    fn start_scan(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.scan = Some(rx);
        std::thread::spawn(move || {
            let result = nusb::list_devices()
                .map(|devices| {
                    devices
                        .filter(|d| matches!(d.vendor_id(), 0x1209 | 0x20a0 | 0x0483))
                        .take(32)
                        .map(|d| {
                            format!(
                                "USB {:04x}:{:04x}  /  identity unverified",
                                d.vendor_id(),
                                d.product_id()
                            )
                        })
                        .collect()
                })
                .map_err(|_| ());
            let _ = tx.send(result);
        });
    }
    fn request_unlock(&mut self) {
        self.lock();
        match self.backend.unlock() {
            Ok(access) => {
                self.access = Some(access);
                self.tab = Tab::Import;
            }
            Err(message) => self.message = message.into(),
        }
    }
    fn import_files(&mut self) {
        if !self.unlocked() {
            self.lock();
            return;
        }
        let Some(paths) = rfd::FileDialog::new()
            .add_filter("Notes / recovery import", &["json"])
            .pick_files()
        else {
            return;
        };
        // Native dialogs may remain open beyond the authorization deadline.
        if !self.unlocked() {
            self.lock();
            return;
        }
        let mut batch = Vec::new();
        for path in paths.iter().take(16) {
            let result = (|| {
                let file = std::fs::File::open(path).map_err(|_| ())?;
                if file.metadata().map_err(|_| ())?.len() > MAX_FILE_BYTES as u64 {
                    return Err(());
                }
                let mut data = Zeroizing::new(Vec::new());
                file.take((MAX_FILE_BYTES + 1) as u64)
                    .read_to_end(&mut data)
                    .map_err(|_| ())?;
                parse_import(&data).map_err(|_| ())
            })();
            match result {
                Ok(entries) => batch.extend(entries.into_iter().map(|entry| Pending {
                    entry,
                    selected: false,
                })),
                Err(()) => {
                    self.message =
                        "Import rejected. Check the format and limits. No data was written.".into();
                    return;
                }
            }
            if batch.len() > vault_import::MAX_ENTRIES {
                self.message = "Import batch exceeds 256 entries.".into();
                return;
            }
        }
        if paths.len() > 16 {
            self.message = "Select at most 16 files per batch.".into();
            return;
        }
        if !self.unlocked() {
            self.lock();
            return;
        }
        self.pending = batch;
        self.message =
            "Prepared in memory only. Select entries, then confirm the exact import on the device."
                .into();
    }
    fn sidebar(&mut self, root: &mut egui::Ui) {
        egui::Panel::left("navigation")
            .exact_size(208.0)
            .show(root, |ui| {
                ui.add_space(20.0);
                ui.heading(RichText::new("AuthDesk").size(26.0));
                ui.label(
                    RichText::new("HARDWARE VAULT")
                        .size(10.0)
                        .color(Color32::from_rgb(140, 158, 186)),
                );
                ui.add_space(28.0);
                for (tab, label) in [
                    (Tab::Overview, "Device"),
                    (Tab::Import, "Import"),
                    (Tab::Notes, "Private notes"),
                    (Tab::Recovery, "Recovery codes"),
                ] {
                    if ui
                        .add_enabled(
                            tab == Tab::Overview || self.unlocked(),
                            egui::Button::new(label)
                                .selected(self.tab == tab)
                                .min_size(egui::vec2(175.0, 38.0)),
                        )
                        .clicked()
                    {
                        self.tab = tab;
                    }
                    ui.add_space(6.0);
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.add_space(16.0);
                    ui.label(
                        RichText::new("DEVELOPMENT BUILD")
                            .size(10.0)
                            .color(Color32::from_rgb(234, 183, 100)),
                    );
                    ui.label("Local only / no cloud");
                    if ui
                        .add_enabled(self.unlocked(), egui::Button::new("Lock now"))
                        .clicked()
                    {
                        self.lock();
                    }
                });
            });
    }
    fn central(&mut self, ui: &mut egui::Ui) {
        ui.add_space(22.0);
        ui.horizontal(|ui| {
            ui.heading(
                RichText::new(match self.tab {
                    Tab::Overview => "Your device",
                    Tab::Import => "Import to device",
                    Tab::Notes => "Private notes",
                    Tab::Recovery => "Recovery codes",
                })
                .size(28.0),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(if self.unlocked() {
                        "AUTHORIZED"
                    } else {
                        "LOCKED"
                    })
                    .color(if self.unlocked() {
                        Color32::LIGHT_GREEN
                    } else {
                        Color32::from_rgb(234, 183, 100)
                    }),
                );
            });
        });
        ui.add_space(24.0);
        if !self.unlocked() {
            egui::Frame::group(ui.style()).inner_margin(24.0).show(ui,|ui| {
                ui.heading("Private by default"); ui.add_space(10.0);
                ui.label("Verify your fingerprint on the hardware key to view private records.");
                ui.label("Fingerprint is mandatory. Sensitive management also requires the device PIN, entered with its four buttons.");
                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    if ui.button("Request fingerprint verification").clicked(){self.request_unlock();}
                });
            });
            ui.add_space(24.0);
            ui.horizontal(|ui| {
                ui.heading("USB discovery");
                if ui
                    .add_enabled(self.scan.is_none(), egui::Button::new("Scan"))
                    .clicked()
                {
                    self.start_scan();
                }
            });
            ui.label("Discovery does not authenticate a device or unlock data.");
            if self.devices.is_empty() {
                ui.label("No target device discovered.");
            }
            for device in &self.devices {
                ui.monospace(device);
            }
            ui.add_space(24.0);
            ui.colored_label(Color32::from_rgb(234,183,100),"Hardware integration is pending. Do not use real credentials with this development build.");
        } else if self.tab == Tab::Import {
            ui.label("Supported: version 2 JSON with private notes and recovery codes.");
            ui.horizontal(|ui| {
                if ui.button("Choose files").clicked() {
                    self.import_files();
                }
                if ui.button("Select all").clicked() {
                    for e in &mut self.pending {
                        e.selected = true;
                    }
                }
                if ui.button("Clear selection").clicked() {
                    for e in &mut self.pending {
                        e.selected = false;
                    }
                }
            });
            ui.add_space(12.0);
            ui.add(egui::TextEdit::singleline(&mut *self.query).hint_text("Filter by title"));
            egui::ScrollArea::vertical()
                .max_height(320.0)
                .show(ui, |ui| {
                    for pending in &mut self.pending {
                        if pending.entry.title().contains(self.query.as_str()) {
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut pending.selected, "");
                                ui.label(pending.entry.kind());
                                ui.label(pending.entry.title());
                            });
                        }
                    }
                });
            ui.label("Note bodies and recovery codes stay masked in previews.");
            let selected = self.pending.iter().filter(|x| x.selected).count();
            if ui
                .add_enabled(
                    selected > 0,
                    egui::Button::new(format!(
                        "Request device confirmation for {selected} selected entries"
                    )),
                )
                .clicked()
                && let Some(access) = &self.access
            {
                let selected: Vec<_> = self
                    .pending
                    .iter()
                    .filter(|x| x.selected)
                    .map(|x| &x.entry)
                    .collect();
                self.message = match self.backend.commit(&selected, access) {
                    Ok(()) => "Device confirmed import.".into(),
                    Err(message) => message.into(),
                };
            }
        } else {
            ui.label("Device-private records are retrieved only through an authenticated, authorized hardware connection.");
            ui.label("Each operation requires a fresh device fingerprint and confirmation. Deletion also requires the device PIN.");
        }
        if !self.message.is_empty() {
            ui.add_space(20.0);
            ui.colored_label(Color32::from_rgb(234, 183, 100), &self.message);
        }
    }
}

impl eframe::App for AuthDesk {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        let now = Instant::now();
        let wall = SystemTime::now();
        let lost_focus = ctx.input(|i| i.viewport().focused == Some(false));
        if self.access.is_some()
            && (!self.unlocked()
                || lost_focus
                || interrupted_frame(now, self.last_frame, wall, self.last_wall_frame))
        {
            self.lock();
        }
        self.last_frame = now;
        self.last_wall_frame = wall;
        if let Some(scan) = &self.scan
            && let Ok(result) = scan.try_recv()
        {
            self.devices = result.unwrap_or_default();
            self.scan = None;
        }
        self.sidebar(root);
        egui::CentralPanel::default().show(root, |ui| {
            self.central(ui);
        });
        if self.scan.is_some() || self.access.is_some() {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }
}

fn install_system_cjk_font(ctx: &egui::Context) {
    // Use installed fonts; do not redistribute proprietary OS font files.
    for path in [
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/System/Library/Fonts/PingFang.ttc",
        r"C:\Windows\Fonts\msyh.ttc",
    ] {
        let Ok(file) = std::fs::File::open(path) else {
            continue;
        };
        let Ok(meta) = file.metadata() else { continue };
        if meta.len() > 32 * 1024 * 1024 {
            continue;
        }
        let mut data = Vec::new();
        if file
            .take(32 * 1024 * 1024 + 1)
            .read_to_end(&mut data)
            .is_err()
            || data.len() > 32 * 1024 * 1024
        {
            continue;
        }
        let mut fonts = egui::FontDefinitions::default();
        fonts
            .font_data
            .insert("system-cjk".into(), egui::FontData::from_owned(data).into());
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts
                .families
                .entry(family)
                .or_default()
                .push("system-cjk".into());
        }
        ctx.set_fonts(fonts);
        break;
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 680.0])
            .with_min_inner_size([820.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "AuthDesk — development build",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            install_system_cjk_font(&cc.egui_ctx);
            Ok(Box::<AuthDesk>::default())
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use vault_import::SecretText;
    #[test]
    fn paused_monotonic_clock_and_wall_rollback_cannot_extend_access() {
        let now = Instant::now();
        let wall = SystemTime::now();
        let access = Access {
            expires: now + Duration::from_secs(15),
            issued_wall: wall,
            expires_wall: wall + Duration::from_secs(15),
        };
        assert!(access.live_at(now, wall));
        assert!(!access.live_at(now, wall + Duration::from_secs(15)));
        assert!(!access.live_at(now, wall - Duration::from_secs(1)));
        assert!(interrupted_frame(
            now,
            now,
            wall + Duration::from_secs(6),
            wall
        ));
        assert!(interrupted_frame(
            now,
            now,
            wall - Duration::from_secs(1),
            wall
        ));
    }
    #[test]
    fn ui_cannot_fake_hardware_authorization() {
        let mut app = AuthDesk::default();
        app.request_unlock();
        assert!(!app.unlocked());
    }
    #[test]
    fn lock_clears_staged_secrets_and_filter() {
        let mut app = AuthDesk::default();
        app.pending.push(Pending {
            entry: Entry::Note(vault_import::Note {
                title: SecretText::new("private".into()),
                body: SecretText::new("recovery-value".into()),
            }),
            selected: true,
        });
        app.query = Zeroizing::new("private".into());
        app.lock();
        assert!(app.pending.is_empty());
        assert!(app.query.is_empty());
        assert!(!app.unlocked());
    }
    #[test]
    fn locked_frame_never_paints_private_metadata() {
        let mut app = AuthDesk::default();
        app.pending.push(Pending {
            entry: Entry::Note(vault_import::Note {
                title: SecretText::new("SENSITIVE-TITLE-9217".into()),
                body: SecretText::new("SENSITIVE-BODY-8426".into()),
            }),
            selected: true,
        });
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |root| {
            egui::CentralPanel::default().show(root, |ui| app.central(ui));
        });
        fn text(shape: &egui::epaint::Shape, out: &mut String) {
            match shape {
                egui::epaint::Shape::Text(t) => out.push_str(t.galley.text()),
                egui::epaint::Shape::Vec(shapes) => {
                    for s in shapes {
                        text(s, out);
                    }
                }
                _ => {}
            }
        }
        let mut painted = String::new();
        for shape in output.shapes {
            text(&shape.shape, &mut painted);
        }
        // Headless test has no renderer to consume font texture uploads.
        output.textures_delta.clear();
        assert!(!painted.contains("SENSITIVE-TITLE-9217"));
        assert!(!painted.contains("SENSITIVE-BODY-8426"));
    }
}
