#![forbid(unsafe_code)]
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use eframe::egui::{self, Color32, RichText};
use std::{
    future::Future,
    io::Read,
    path::PathBuf,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll, Wake},
    time::{Duration, Instant, SystemTime},
};
use vault_import::Entry;
use zeroize::Zeroizing;

mod import_task;
use import_task::{ImportWorker, ReadDeadline};

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

type PickerFuture = Pin<Box<dyn Future<Output = Option<Vec<rfd::FileHandle>>> + Send>>;
enum ImportFlow {
    Idle,
    // Clear private content for two complete UI frames before even creating a
    // native dialog. Some native backends can fall back to a blocking dialog.
    Masking { started: u64 },
    Picking(PickerFuture),
    Selected(Vec<PathBuf>),
    Reading,
}
struct PickerWake(egui::Context);
impl Wake for PickerWake {
    fn wake(self: Arc<Self>) {
        self.0.request_repaint();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.request_repaint();
    }
}

#[derive(Default)]
struct HardwareBackend;
impl HardwareBackend {
    fn unlock(&mut self) -> Result<Access, &'static str> {
        // The R04 authenticated sensor/device channel is not integrated yet.
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
    import_flow: ImportFlow,
    import_worker: ImportWorker,
    generation: Option<u64>,
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
            import_flow: ImportFlow::Idle,
            import_worker: ImportWorker::default(),
            generation: Some(0),
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
        self.generation = self.generation.and_then(|value| value.checked_add(1));
        self.import_worker.cancel();
        self.import_flow = ImportFlow::Idle;
        self.access = None;
        self.pending.clear();
        self.query = Zeroizing::new(String::new());
        self.message.clear();
        self.tab = Tab::Overview;
    }
    fn privacy_tick(&mut self, now: Instant, wall: SystemTime, lost_focus: bool) {
        if self.access.as_ref().is_some_and(|access| {
            !access.live_at(now, wall)
                || lost_focus
                || interrupted_frame(now, self.last_frame, wall, self.last_wall_frame)
        }) {
            self.lock();
        }
        self.last_frame = now;
        self.last_wall_frame = wall;
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
    fn request_unlock(&mut self, ctx: &egui::Context) {
        if self.import_worker.busy()
            || !matches!(self.import_flow, ImportFlow::Idle | ImportFlow::Selected(_))
        {
            return;
        }
        // Retain only paths across a fresh device verification. No file is
        // opened, parsed or previewed while the native picker is active.
        let paths = match std::mem::replace(&mut self.import_flow, ImportFlow::Idle) {
            ImportFlow::Selected(paths) => Some(paths),
            _ => None,
        };
        self.lock();
        if self.generation.is_none() {
            self.message = "Authorization generation exhausted. Restart the application.".into();
            return;
        }
        match self.backend.unlock() {
            Ok(access) if access.live() => {
                self.access = Some(access);
                self.tab = Tab::Import;
                if let Some(paths) = paths {
                    self.begin_read(paths, ctx);
                }
            }
            Ok(_) => {
                self.lock();
                self.message = "Device authorization already expired.".into();
            }
            Err(message) => {
                if let Some(paths) = paths {
                    self.import_flow = ImportFlow::Selected(paths);
                }
                self.message = message.into();
            }
        }
    }
    fn begin_read(&mut self, paths: Vec<PathBuf>, ctx: &egui::Context) {
        let (Some(access), Some(generation)) = (&self.access, self.generation) else {
            self.lock();
            return;
        };
        if !access.live() {
            self.lock();
            return;
        }
        let deadline = ReadDeadline {
            expires: access.expires,
            issued_wall: access.issued_wall,
            expires_wall: access.expires_wall,
        };
        match self
            .import_worker
            .start(paths, deadline, generation, ctx.clone())
        {
            Ok(()) => {
                self.import_flow = ImportFlow::Reading;
                self.message = "Reading selected files. No data has been written.".into();
            }
            Err(error) => {
                self.lock();
                self.message = error.message().into();
            }
        }
    }
    fn import_files(&mut self, frame: u64) {
        if !self.unlocked() {
            self.lock();
            return;
        }
        if self.import_worker.busy() || !matches!(self.import_flow, ImportFlow::Idle) {
            return;
        }
        self.lock();
        self.import_flow = ImportFlow::Masking { started: frame };
    }
    fn accept_import(&mut self, generation: u64, result: Result<Vec<Entry>, import_task::Error>) {
        if self.generation != Some(generation)
            || !self.unlocked()
            || !matches!(self.import_flow, ImportFlow::Reading)
        {
            // In particular, never let an old completion clear or populate a
            // newer authorization. Dropping result zeroizes Entry strings.
            return;
        }
        self.import_flow = ImportFlow::Idle;
        match result {
            Ok(entries) => {
                self.pending = entries
                    .into_iter()
                    .map(|entry| Pending {
                        entry,
                        selected: false,
                    })
                    .collect();
                self.message = "Prepared in memory only. Confirm the exact import on the device before writing.".into();
            }
            Err(error) => {
                self.lock();
                self.message = error.message().into();
            }
        }
    }
    fn advance_import(&mut self, ctx: &egui::Context) {
        if let Some((generation, result)) = self.import_worker.poll() {
            self.accept_import(generation, result);
        }
        if let ImportFlow::Masking { started } = self.import_flow {
            if ctx.cumulative_frame_nr().saturating_sub(started) >= 2 {
                self.import_flow = ImportFlow::Picking(Box::pin(
                    rfd::AsyncFileDialog::new()
                        .add_filter("Notes / recovery import", &["json"])
                        .pick_files(),
                ));
            }
            return;
        }
        if let ImportFlow::Picking(future) = &mut self.import_flow {
            let waker = Arc::new(PickerWake(ctx.clone())).into();
            if let Poll::Ready(files) = future.as_mut().poll(&mut Context::from_waker(&waker)) {
                self.import_flow = match files {
                    Some(files) if !files.is_empty() && files.len() <= import_task::MAX_FILES => {
                        self.message = "Files selected. Verify your device fingerprint again before files are read.".into();
                        ImportFlow::Selected(files.into_iter().map(Into::into).collect())
                    }
                    Some(_) => {
                        self.message = import_task::Error::Files.message().into();
                        ImportFlow::Idle
                    }
                    None => ImportFlow::Idle,
                };
            }
        }
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
                    let can_request = !self.import_worker.busy()
                        && matches!(self.import_flow, ImportFlow::Idle | ImportFlow::Selected(_));
                    if ui.add_enabled(can_request, egui::Button::new("Request fingerprint verification")).clicked(){self.request_unlock(ui.ctx());}
                    if matches!(self.import_flow, ImportFlow::Masking { .. } | ImportFlow::Selected(_))
                        && ui.button("Cancel import selection").clicked() {self.lock();}
                });
                if matches!(self.import_flow, ImportFlow::Masking { .. } | ImportFlow::Picking(_)) {
                    ui.label("Private content cleared. File selection does not read file contents.");
                }
                if self.import_worker.busy() {
                    ui.label("Waiting for the previous file operation to stop.");
                }
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
            let mut choose = false;
            ui.horizontal(|ui| {
                choose = ui
                    .add_enabled(
                        !self.import_worker.busy() && matches!(self.import_flow, ImportFlow::Idle),
                        egui::Button::new("Choose files"),
                    )
                    .clicked();
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
            if choose {
                self.import_files(ui.ctx().cumulative_frame_nr());
                ui.ctx().request_repaint();
                // Do not paint the old import title/filter rows in this frame.
                return;
            }
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
                && self.unlocked()
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
        self.privacy_tick(now, wall, lost_focus);
        if let Some(scan) = &self.scan
            && let Ok(result) = scan.try_recv()
        {
            self.devices = result.unwrap_or_default();
            self.scan = None;
        }
        self.advance_import(&ctx);
        self.sidebar(root);
        egui::CentralPanel::default().show(root, |ui| {
            self.central(ui);
        });
        if self.scan.is_some()
            || self.access.is_some()
            || self.import_worker.busy()
            || !matches!(self.import_flow, ImportFlow::Idle)
        {
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

    fn fixture_access(app: &mut AuthDesk) {
        // Only tests can manufacture a lease; production has no host unlock
        // constructor, environment switch or mock hardware backend.
        let now = Instant::now();
        let wall = SystemTime::now();
        app.access = Some(Access {
            expires: now + Duration::from_secs(15),
            issued_wall: wall,
            expires_wall: wall + Duration::from_secs(15),
        });
        app.last_frame = now;
        app.last_wall_frame = wall;
        app.tab = Tab::Import;
    }
    fn fixture_entries() -> Vec<Entry> {
        vault_import::parse_import(br#"{"version":2,"entries":[{"type":"note","title":"SENSITIVE-TITLE-9217","body":"SENSITIVE-BODY-8426"}]}"#).unwrap()
    }
    fn stage_fixture(app: &mut AuthDesk) {
        app.pending = fixture_entries()
            .into_iter()
            .map(|entry| Pending {
                entry,
                selected: true,
            })
            .collect();
        app.query = Zeroizing::new("private-filter".into());
    }
    #[test]
    fn choosing_files_revokes_and_clears_before_a_dialog_can_exist() {
        let mut app = AuthDesk::default();
        fixture_access(&mut app);
        stage_fixture(&mut app);
        let old = app.generation;
        app.import_files(27);
        assert!(app.access.is_none());
        assert!(app.pending.is_empty());
        assert!(app.query.is_empty());
        assert_ne!(app.generation, old);
        assert!(matches!(
            app.import_flow,
            ImportFlow::Masking { started: 27 }
        ));
        assert!(!app.import_worker.busy());
        // The same/earlier frame must not construct a native dialog.
        app.advance_import(&egui::Context::default());
        assert!(matches!(app.import_flow, ImportFlow::Masking { .. }));
    }
    #[test]
    fn a_picker_result_contains_only_paths_and_cannot_unlock_or_read_files() {
        let mut app = AuthDesk::default();
        let path = PathBuf::from("this-file-does-not-exist-and-must-not-be-opened.json");
        app.import_flow = ImportFlow::Picking(Box::pin(std::future::ready(Some(vec![
            rfd::FileHandle::from(path.clone()),
        ]))));
        let ctx = egui::Context::default();
        app.advance_import(&ctx);
        assert!(
            matches!(&app.import_flow, ImportFlow::Selected(paths) if paths == std::slice::from_ref(&path))
        );
        assert!(app.pending.is_empty());
        assert!(app.access.is_none());
        assert!(!app.import_worker.busy());
        app.request_unlock(&ctx);
        assert!(!app.unlocked());
        assert!(!app.import_worker.busy());
        assert!(matches!(&app.import_flow, ImportFlow::Selected(paths) if paths == &[path]));
        app.lock();
        assert!(matches!(app.import_flow, ImportFlow::Idle));
    }
    #[test]
    fn old_or_expired_import_results_cannot_enter_a_new_authorization() {
        let mut app = AuthDesk::default();
        fixture_access(&mut app);
        app.import_flow = ImportFlow::Reading;
        let old = app.generation.unwrap();
        app.lock();
        fixture_access(&mut app);
        app.import_flow = ImportFlow::Reading;
        app.accept_import(old, Ok(fixture_entries()));
        assert!(app.pending.is_empty());
        assert!(app.unlocked());
        assert!(matches!(app.import_flow, ImportFlow::Reading));
        let current = app.generation.unwrap();
        app.access.as_mut().unwrap().expires = Instant::now();
        app.accept_import(current, Ok(fixture_entries()));
        assert!(app.pending.is_empty());
        app.lock();
        fixture_access(&mut app);
        app.import_flow = ImportFlow::Reading;
        app.accept_import(app.generation.unwrap(), Ok(fixture_entries()));
        assert_eq!(app.pending.len(), 1);
        assert!(!app.pending[0].selected);
        app.lock();
        assert!(app.pending.is_empty());
    }
    #[test]
    fn focus_sleep_rollback_and_expiry_revoke_before_private_ui_or_result_polling() {
        for case in 0..4 {
            let mut app = AuthDesk::default();
            fixture_access(&mut app);
            stage_fixture(&mut app);
            app.import_flow = ImportFlow::Reading;
            let now = app.last_frame;
            let wall = app.last_wall_frame;
            let generation = app.generation.unwrap();
            match case {
                0 => app.privacy_tick(now, wall, true),
                1 => app.privacy_tick(now, wall + Duration::from_secs(6), false),
                2 => app.privacy_tick(now, wall - Duration::from_secs(1), false),
                3 => app.privacy_tick(now + Duration::from_secs(15), wall, false),
                _ => unreachable!(),
            }
            assert!(app.access.is_none());
            assert!(app.pending.is_empty());
            assert!(app.query.is_empty());
            app.accept_import(generation, Ok(fixture_entries()));
            assert!(app.pending.is_empty());
        }
    }
    #[test]
    fn generation_exhaustion_is_permanent_until_process_restart() {
        let mut app = AuthDesk {
            generation: Some(u64::MAX),
            ..Default::default()
        };
        app.request_unlock(&egui::Context::default());
        assert!(app.generation.is_none());
        assert!(!app.unlocked());
        assert!(app.message.contains("generation exhausted"));
        app.lock();
        app.request_unlock(&egui::Context::default());
        assert!(app.generation.is_none());
        assert!(!app.unlocked());
    }
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
        app.request_unlock(&egui::Context::default());
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
