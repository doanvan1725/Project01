// ============================================================================
// app.rs - Giao dien nguoi dung (egui/eframe) va dieu phoi trang thai
// ============================================================================

use crate::config::{Config, ScheduleFrequency, Weekday};
use crate::copier::run_backup;
use crate::model::{human_bytes, human_duration, LogEntry, LogLevel, ProgressMsg, Stats};
use crate::schedule;
use crate::theme;
use eframe::egui;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::Instant;

const MAX_LOG_ENTRIES: usize = 800;

#[derive(PartialEq, Clone, Copy)]
enum RunState {
    Idle,
    Scanning,
    Copying,
    Cancelling,
    Done,
}

pub struct BackupApp {
    config: Config,
    config_path: PathBuf,

    project_enabled: Vec<(String, bool)>,
    new_project_name: String,

    destination: Option<PathBuf>,
    thread_count: usize,
    mirror_delete: bool,
    dark_mode: bool,

    schedule_enabled: bool,
    schedule_frequency: ScheduleFrequency,
    schedule_hour: u32,
    schedule_minute: u32,
    schedule_weekday: Weekday,
    schedule_day_of_month: u32,

    state: RunState,
    stats: Stats,
    log: VecDeque<LogEntry>,
    current_project: String,
    last_cancelled: bool,

    rx: Option<Receiver<ProgressMsg>>,
    cancel_flag: Option<Arc<AtomicBool>>,

    run_start: Option<Instant>,
    run_finished_secs: Option<f64>,
    speed_window: (Instant, u64), // (moc thoi gian, so byte da copy tai moc do) de tinh toc do
    current_speed_bps: f64,

    status_message: String,
}

impl BackupApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Luon tim/ tao config.toml canh file .exe thuc su, KHONG phu thuoc vao
        // thu muc lam viec (CWD) luc khoi chay - vi CWD co the khac nhau tuy cach
        // mo tool (double-click trong Explorer, tao shortcut, hay chay tu `cargo run`),
        // dan den nham lan tao ra nhieu ban config.toml khac nhau o nhieu noi.
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()));
        let config_path = exe_dir
            .clone()
            .map(|d| d.join("config.toml"))
            .unwrap_or_else(|| PathBuf::from("config.toml"));
        let config = Config::load_or_create(&config_path).unwrap_or_default();

        let project_enabled: Vec<(String, bool)> =
            config.projects.iter().cloned().map(|p| (p, true)).collect();

        let destination = if config.default_destination.trim().is_empty() {
            None
        } else {
            Some(PathBuf::from(&config.default_destination))
        };

        let thread_count = config.effective_thread_count();
        let mirror_delete = config.enable_mirror_delete;
        let dark_mode = config.ui_dark_mode;
        theme::apply(&cc.egui_ctx, dark_mode);

        let schedule_enabled = config.schedule.enabled;
        let schedule_frequency = config.schedule.frequency;
        let schedule_hour = config.schedule.hour;
        let schedule_minute = config.schedule.minute;
        let schedule_weekday = config.schedule.weekday;
        let schedule_day_of_month = config.schedule.day_of_month;

        let last_auto_summary = exe_dir.as_deref().and_then(load_last_auto_status);

        let mut app = Self {
            config,
            config_path,
            project_enabled,
            new_project_name: String::new(),
            destination,
            thread_count,
            mirror_delete,
            dark_mode,
            schedule_enabled,
            schedule_frequency,
            schedule_hour,
            schedule_minute,
            schedule_weekday,
            schedule_day_of_month,
            state: RunState::Idle,
            stats: Stats::default(),
            log: VecDeque::new(),
            current_project: String::new(),
            last_cancelled: false,
            rx: None,
            cancel_flag: None,
            run_start: None,
            run_finished_secs: None,
            speed_window: (Instant::now(), 0),
            current_speed_bps: 0.0,
            status_message: "San sang.".to_string(),
        };

        if let Some((ok, summary)) = last_auto_summary {
            app.push_log(
                if ok { LogLevel::Success } else { LogLevel::Warn },
                summary,
            );
        }

        app
    }

    fn push_log(&mut self, level: LogLevel, message: String) {
        let time = chrono::Local::now().format("%H:%M:%S").to_string();
        self.log.push_back(LogEntry { time, level, message });
        while self.log.len() > MAX_LOG_ENTRIES {
            self.log.pop_front();
        }
    }

    fn selected_projects(&self) -> Vec<String> {
        self.project_enabled
            .iter()
            .filter(|(_, on)| *on)
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Mo hop thoai duyet thu muc (co the chon nhieu cung luc), bat dau tu source_root.
    /// Chi chap nhan thu muc nam TRUC TIEP trong source_root (dung yeu cau: backup theo
    /// danh sach du an la thu muc con cua \\Vci-bim-nas\projects); thu muc ngoai pham
    /// vi nay se bi bo qua kem canh bao trong nhat ky.
    fn browse_projects_from_nas(&mut self) {
        let start_dir = self.config.source_root_path();
        let picked = rfd::FileDialog::new()
            .set_directory(&start_dir)
            .pick_folders();

        let Some(folders) = picked else {
            return;
        };

        let mut added = 0usize;
        for folder in folders {
            match folder.strip_prefix(&start_dir) {
                Ok(rel) if !rel.as_os_str().is_empty() => {
                    let name = rel.to_string_lossy().replace('\\', "/");
                    if self.project_enabled.iter().any(|(n, _)| n == &name) {
                        continue;
                    }
                    self.project_enabled.push((name, true));
                    added += 1;
                }
                _ => {
                    self.push_log(
                        LogLevel::Warn,
                        format!(
                            "Bo qua '{}': khong nam trong thu muc nguon {}",
                            folder.display(),
                            self.config.source_root
                        ),
                    );
                }
            }
        }
        if added > 0 {
            self.push_log(LogLevel::Info, format!("Da them {added} du an tu NAS."));
        }
    }

    fn can_start(&self) -> bool {
        matches!(self.state, RunState::Idle | RunState::Done)
            && self.destination.is_some()
            && !self.selected_projects().is_empty()
    }

    fn start_backup(&mut self) {
        let Some(destination) = self.destination.clone() else {
            return;
        };
        let projects = self.selected_projects();
        if projects.is_empty() {
            self.push_log(LogLevel::Warn, "Chua chon du an nao de backup.".to_string());
            return;
        }

        self.stats = Stats::default();
        self.log.clear();
        self.state = RunState::Scanning;
        self.current_project.clear();
        self.last_cancelled = false;
        self.run_start = Some(Instant::now());
        self.run_finished_secs = None;
        self.speed_window = (Instant::now(), 0);
        self.current_speed_bps = 0.0;
        self.status_message = "Dang quet du lieu...".to_string();

        self.push_log(
            LogLevel::Info,
            format!(
                "Bat dau backup {} du an sang {} (so luong: {}, xoa file thua: {}).",
                projects.len(),
                destination.display(),
                self.thread_count,
                if self.mirror_delete { "CO" } else { "khong" }
            ),
        );

        let (tx, rx) = channel::<ProgressMsg>();
        let cancel_flag = Arc::new(AtomicBool::new(false));

        self.rx = Some(rx);
        self.cancel_flag = Some(cancel_flag.clone());

        let config = self.config.clone();
        let thread_count = self.thread_count;
        let mirror_delete = self.mirror_delete;

        std::thread::spawn(move || {
            run_backup(
                config,
                projects,
                destination,
                thread_count,
                mirror_delete,
                tx,
                cancel_flag,
            );
        });
    }

    fn cancel_backup(&mut self) {
        if let Some(flag) = &self.cancel_flag {
            flag.store(true, Ordering::Relaxed);
            self.state = RunState::Cancelling;
            self.status_message = "Dang huy... cho cac file dang copy hoan tat.".to_string();
            self.push_log(LogLevel::Warn, "Da yeu cau huy backup.".to_string());
        }
    }

    fn sync_config_from_ui(&mut self) {
        self.config.projects = self.project_enabled.iter().map(|(n, _)| n.clone()).collect();
        self.config.default_destination = self
            .destination
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        self.config.thread_count = self.thread_count;
        self.config.enable_mirror_delete = self.mirror_delete;
        self.config.ui_dark_mode = self.dark_mode;

        self.config.schedule.enabled = self.schedule_enabled;
        self.config.schedule.frequency = self.schedule_frequency;
        self.config.schedule.hour = self.schedule_hour;
        self.config.schedule.minute = self.schedule_minute;
        self.config.schedule.weekday = self.schedule_weekday;
        self.config.schedule.day_of_month = self.schedule_day_of_month;
    }

    fn save_config(&mut self) {
        self.sync_config_from_ui();
        match self.config.save(&self.config_path) {
            Ok(_) => self.push_log(LogLevel::Success, "Da luu cau hinh vao config.toml.".to_string()),
            Err(e) => self.push_log(LogLevel::Error, format!("Loi luu cau hinh: {e}")),
        }
    }

    /// Luu cau hinh VA dang ky/go bo tac vu trong Windows Task Scheduler cho
    /// dung voi trang thai "Bat lich tu dong" hien tai tren giao dien.
    fn save_schedule(&mut self) {
        self.sync_config_from_ui();
        if let Err(e) = self.config.save(&self.config_path) {
            self.push_log(LogLevel::Error, format!("Loi luu cau hinh: {e}"));
            return;
        }

        let exe_path = match std::env::current_exe() {
            Ok(p) => p,
            Err(e) => {
                self.push_log(LogLevel::Error, format!("Khong xac dinh duoc duong dan .exe: {e}"));
                return;
            }
        };

        if self.schedule_enabled {
            if self.destination.is_none() {
                self.push_log(
                    LogLevel::Warn,
                    "Chua chon o dich - lich tu dong se khong chay duoc gi cho den khi ban chon o dich va bam Luu lich lai.".to_string(),
                );
            }
            match schedule::install(&exe_path, &self.config.schedule) {
                Ok(_) => self.push_log(
                    LogLevel::Success,
                    format!(
                        "Da BAT lich tu dong: {} luc {:02}:{:02}{}.",
                        self.schedule_frequency.display_vi(),
                        self.schedule_hour,
                        self.schedule_minute,
                        match self.schedule_frequency {
                            ScheduleFrequency::Weekly =>
                                format!(" ({})", self.schedule_weekday.display_vi()),
                            ScheduleFrequency::Monthly =>
                                format!(" (ngay {})", self.schedule_day_of_month),
                            ScheduleFrequency::Daily => String::new(),
                        }
                    ),
                ),
                Err(e) => self.push_log(LogLevel::Error, format!("Khong bat duoc lich tu dong: {e}")),
            }
        } else {
            match schedule::uninstall() {
                Ok(_) => self.push_log(LogLevel::Info, "Da TAT lich tu dong.".to_string()),
                Err(e) => self.push_log(LogLevel::Error, format!("Khong tat duoc lich tu dong: {e}")),
            }
        }
    }

    fn poll_messages(&mut self) {
        let mut messages = Vec::new();
        if let Some(rx) = &self.rx {
            for msg in rx.try_iter().take(2000) {
                messages.push(msg);
            }
        }
        for msg in messages {
            self.handle_message(msg);
        }
    }

    fn handle_message(&mut self, msg: ProgressMsg) {
        match msg {
            ProgressMsg::ScanningProject(name) => {
                self.current_project = name.clone();
                self.status_message = format!("Dang quet du an: {name}");
            }
            ProgressMsg::ScanTick { scanned } => {
                self.stats.scanned_so_far = scanned;
            }
            ProgressMsg::ScanComplete {
                total_files_to_copy,
                total_bytes_to_copy,
                already_up_to_date,
                total_delete_candidates,
            } => {
                self.stats.total_files_to_copy = total_files_to_copy;
                self.stats.total_bytes_to_copy = total_bytes_to_copy;
                self.stats.already_up_to_date = already_up_to_date;
                self.stats.total_delete_candidates = total_delete_candidates;
                self.state = RunState::Copying;
                self.status_message = format!(
                    "Quet xong: {} file can copy ({}), {} file da la moi nhat.",
                    total_files_to_copy,
                    human_bytes(total_bytes_to_copy),
                    already_up_to_date
                );
                self.push_log(LogLevel::Info, self.status_message.clone());
                self.speed_window = (Instant::now(), 0);
            }
            ProgressMsg::FileCopied { rel_path, bytes } => {
                self.stats.files_copied += 1;
                self.stats.bytes_copied += bytes;
                self.push_log(LogLevel::Success, format!("Da chep: {rel_path} ({})", human_bytes(bytes)));
            }
            ProgressMsg::FileError { rel_path, message } => {
                self.stats.error_count += 1;
                let label = if rel_path.is_empty() {
                    message.clone()
                } else {
                    format!("{rel_path}: {message}")
                };
                self.push_log(LogLevel::Error, label);
            }
            ProgressMsg::FileDeleted { rel_path } => {
                self.stats.files_deleted += 1;
                self.push_log(LogLevel::Warn, format!("Da xoa o dich: {rel_path}"));
            }
            ProgressMsg::AllDone { cancelled, elapsed_secs } => {
                self.state = RunState::Done;
                self.last_cancelled = cancelled;
                self.run_finished_secs = Some(elapsed_secs);
                self.current_project.clear();
                let summary = if cancelled {
                    format!(
                        "Da HUY sau {}. Da chep {}/{} file.",
                        human_duration(elapsed_secs),
                        self.stats.files_copied,
                        self.stats.total_files_to_copy
                    )
                } else {
                    format!(
                        "Hoan tat trong {}. Da chep {} file ({}), xoa {} muc, {} loi.",
                        human_duration(elapsed_secs),
                        self.stats.files_copied,
                        human_bytes(self.stats.bytes_copied),
                        self.stats.files_deleted,
                        self.stats.error_count
                    )
                };
                self.status_message = summary.clone();
                self.push_log(
                    if cancelled { LogLevel::Warn } else { LogLevel::Success },
                    summary,
                );
            }
            ProgressMsg::Info(text) => {
                self.push_log(LogLevel::Info, text);
            }
        }
    }

    fn update_speed(&mut self) {
        if self.state != RunState::Copying {
            return;
        }
        let now = Instant::now();
        let (last_t, last_bytes) = self.speed_window;
        let dt = now.duration_since(last_t).as_secs_f64();
        if dt >= 0.5 {
            let delta_bytes = self.stats.bytes_copied.saturating_sub(last_bytes);
            self.current_speed_bps = delta_bytes as f64 / dt;
            self.speed_window = (now, self.stats.bytes_copied);
        }
    }

    fn eta_secs(&self) -> Option<f64> {
        if self.current_speed_bps <= 1.0 {
            return None;
        }
        let remaining = self
            .stats
            .total_bytes_to_copy
            .saturating_sub(self.stats.bytes_copied) as f64;
        Some(remaining / self.current_speed_bps)
    }
}

impl eframe::App for BackupApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_messages();
        self.update_speed();
        theme::apply(ctx, self.dark_mode);

        let is_running = matches!(self.state, RunState::Scanning | RunState::Copying | RunState::Cancelling);
        if is_running {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }

        let dark_before = self.dark_mode;
        draw_header(ctx, &self.status_message, self.state, self.dark_mode, &mut self.dark_mode);
        if self.dark_mode != dark_before {
            // Luu ngay so thich giao dien (Sang/Toi) de lan mo sau giu nguyen, khong can bam "Luu cau hinh"
            self.sync_config_from_ui();
            let _ = self.config.save(&self.config_path);
        }
        self.draw_side_panel(ctx);
        self.draw_central_panel(ctx);
    }
}

fn draw_header(ctx: &egui::Context, status: &str, state: RunState, dark: bool, dark_mode: &mut bool) {
    egui::TopBottomPanel::top("header").show(ctx, |ui| {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("VCI BIM Backup")
                    .heading()
                    .strong()
                    .color(theme::accent(dark)),
            );
            ui.label(egui::RichText::new("  •  \\\\Vci-bim-nas\\projects  →  Elements").color(theme::muted(dark)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (dot, text) = match state {
                    RunState::Idle => (theme::muted(dark), "San sang"),
                    RunState::Scanning => (theme::accent(dark), "Dang quet"),
                    RunState::Copying => (theme::accent_strong(dark), "Dang copy"),
                    RunState::Cancelling => (theme::warning(dark), "Dang huy"),
                    RunState::Done => (theme::success(dark), "Hoan tat"),
                };
                ui.colored_label(dot, "●");
                ui.label(text);

                ui.add_space(10.0);
                let toggle_label = if dark { "☀  Sang" } else { "🌙  Toi" };
                if ui.button(toggle_label).clicked() {
                    *dark_mode = !*dark_mode;
                }
            });
        });
        ui.add_space(4.0);
        ui.label(egui::RichText::new(status).color(theme::muted(dark)).small());
        ui.add_space(6.0);
    });
}

impl BackupApp {
    fn draw_side_panel(&mut self, ctx: &egui::Context) {
        let is_running = matches!(self.state, RunState::Scanning | RunState::Copying | RunState::Cancelling);

        egui::SidePanel::left("config_panel")
            .resizable(false)
            .exact_width(340.0)
            .show(ctx, |ui| {
              egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                ui.add_space(4.0);
                ui.label(egui::RichText::new("NGUON").strong().color(theme::muted(self.dark_mode)));
                ui.label(&self.config.source_root);
                ui.add_space(10.0);

                ui.label(egui::RichText::new("DICH (o Elements)").strong().color(theme::muted(self.dark_mode)));
                ui.horizontal(|ui| {
                    let text = self
                        .destination
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "(chua chon)".to_string());
                    ui.add(egui::Label::new(text).truncate());
                });
                if ui
                    .add_enabled(!is_running, egui::Button::new("📁  Chon o dich..."))
                    .clicked()
                {
                    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        self.destination = Some(folder);
                    }
                }
                ui.add_space(10.0);

                ui.separator();
                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("DU AN CAN BACKUP").strong().color(theme::muted(self.dark_mode)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Bo chon het").clicked() {
                            for (_, on) in self.project_enabled.iter_mut() {
                                *on = false;
                            }
                        }
                        if ui.small_button("Chon het").clicked() {
                            for (_, on) in self.project_enabled.iter_mut() {
                                *on = true;
                            }
                        }
                    });
                });

                egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                    let mut remove_idx: Option<usize> = None;
                    for (idx, (name, enabled)) in self.project_enabled.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.add_enabled(!is_running, egui::Checkbox::new(enabled, name.as_str()));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.add_enabled(!is_running, egui::Button::new("✕").small()).clicked() {
                                    remove_idx = Some(idx);
                                }
                            });
                        });
                    }
                    if let Some(idx) = remove_idx {
                        self.project_enabled.remove(idx);
                    }
                });

                if ui
                    .add_enabled(
                        !is_running,
                        egui::Button::new("📂  Duyet chon du an tu NAS...")
                            .min_size(egui::vec2(ui.available_width(), 0.0)),
                    )
                    .on_hover_text(format!(
                        "Mo cua so duyet thu muc, bat dau tu {}. Co the chon nhieu thu muc cung luc.",
                        self.config.source_root
                    ))
                    .clicked()
                {
                    self.browse_projects_from_nas();
                }

                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Hoac tu go ten thu muc project (neu biet chinh xac):")
                        .color(theme::muted(self.dark_mode))
                        .small(),
                );
                ui.horizontal(|ui| {
                    ui.add_enabled(
                        !is_running,
                        egui::TextEdit::singleline(&mut self.new_project_name)
                            .hint_text("Ten thu muc du an moi..."),
                    );
                    if ui.add_enabled(!is_running, egui::Button::new("+ Them")).clicked() {
                        let name = self.new_project_name.trim().to_string();
                        if !name.is_empty() {
                            self.project_enabled.push((name, true));
                            self.new_project_name.clear();
                        }
                    }
                });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(6.0);

                ui.label(egui::RichText::new("HIEU NANG").strong().color(theme::muted(self.dark_mode)));
                ui.add(
                    egui::Slider::new(&mut self.thread_count, 1..=32)
                        .text("So luong copy song song"),
                )
                .on_hover_text(
                    "Backup qua mang la tac vu I/O-bound. Qua nhieu luong co the lam nghen \
                     bang thong mang thay vi nhanh hon. Mac dinh de xuat: 4-8.",
                );

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(6.0);

                ui.label(egui::RichText::new("AN TOAN").strong().color(theme::muted(self.dark_mode)));
                ui.add_enabled(
                    !is_running,
                    egui::Checkbox::new(&mut self.mirror_delete, "Xoa file thua o dich (Mirror that)"),
                );
                if self.mirror_delete {
                    ui.label(
                        egui::RichText::new(
                            "⚠ Nhung file/thu muc khong con ton tai o nguon se bi XOA VINH VIEN o o Elements.",
                        )
                        .color(theme::warning(self.dark_mode))
                        .small(),
                    );
                } else {
                    ui.label(
                        egui::RichText::new("Chi them/cap nhat file, khong xoa gi o dich.")
                            .color(theme::muted(self.dark_mode))
                            .small(),
                    );
                }

                ui.add_space(14.0);
                ui.separator();
                ui.add_space(6.0);

                ui.label(egui::RichText::new("LICH TU DONG").strong().color(theme::muted(self.dark_mode)));
                ui.checkbox(&mut self.schedule_enabled, "Bat backup tu dong theo lich");

                if self.schedule_enabled {
                    egui::ComboBox::from_id_salt("schedule_freq")
                        .selected_text(self.schedule_frequency.display_vi())
                        .show_ui(ui, |ui| {
                            for f in ScheduleFrequency::ALL {
                                ui.selectable_value(&mut self.schedule_frequency, f, f.display_vi());
                            }
                        });

                    ui.horizontal(|ui| {
                        ui.label("Gio chay:");
                        ui.add(egui::DragValue::new(&mut self.schedule_hour).range(0..=23).suffix(" h"));
                        ui.add(egui::DragValue::new(&mut self.schedule_minute).range(0..=59).suffix(" m"));
                    });

                    match self.schedule_frequency {
                        ScheduleFrequency::Weekly => {
                            egui::ComboBox::from_id_salt("schedule_weekday")
                                .selected_text(self.schedule_weekday.display_vi())
                                .show_ui(ui, |ui| {
                                    for d in Weekday::ALL {
                                        ui.selectable_value(&mut self.schedule_weekday, d, d.display_vi());
                                    }
                                });
                        }
                        ScheduleFrequency::Monthly => {
                            ui.horizontal(|ui| {
                                ui.label("Ngay trong thang:");
                                ui.add(
                                    egui::DragValue::new(&mut self.schedule_day_of_month)
                                        .range(1..=28),
                                );
                            })
                            .response
                            .on_hover_text(
                                "Gioi han 1-28 de luon chay du moi thang (thang 2 khong co ngay 29-31).",
                            );
                        }
                        ScheduleFrequency::Daily => {}
                    }

                    ui.label(
                        egui::RichText::new(
                            "⚠ May tinh phai dang BAT va o Elements phai dang CAM vao dung gio hen thi lan backup do moi thuc su chay.",
                        )
                        .color(theme::warning(self.dark_mode))
                        .small(),
                    );
                }

                if ui.button("⏰  Luu lich tu dong").clicked() {
                    self.save_schedule();
                }

                ui.add_space(14.0);
                ui.separator();
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    if ui.button("💾  Luu cau hinh").clicked() {
                        self.save_config();
                    }
                });

                ui.add_space(10.0);

                let start_btn = egui::Button::new(
                    egui::RichText::new("▶  BAT DAU BACKUP").strong().size(15.0),
                )
                .min_size(egui::vec2(ui.available_width(), 38.0))
                .fill(theme::accent_strong(self.dark_mode));

                match self.state {
                    RunState::Idle | RunState::Done => {
                        if ui.add_enabled(self.can_start(), start_btn).clicked() {
                            self.start_backup();
                        }
                        if self.destination.is_none() {
                            ui.label(egui::RichText::new("Hay chon o dich truoc.").color(theme::warning(self.dark_mode)).small());
                        } else if self.selected_projects().is_empty() {
                            ui.label(egui::RichText::new("Hay chon it nhat 1 du an.").color(theme::warning(self.dark_mode)).small());
                        }
                    }
                    RunState::Scanning | RunState::Copying => {
                        let cancel_btn = egui::Button::new(
                            egui::RichText::new("⏹  HUY BACKUP").strong().size(15.0),
                        )
                        .min_size(egui::vec2(ui.available_width(), 38.0))
                        .fill(theme::danger(self.dark_mode));
                        if ui.add(cancel_btn).clicked() {
                            self.cancel_backup();
                        }
                    }
                    RunState::Cancelling => {
                        ui.add_enabled(false, egui::Button::new("Dang huy..."));
                    }
                }
              });
            });
    }

    fn draw_central_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(4.0);

            // --- Thanh tien do tong the ---
            let ratio = self.stats.progress_ratio();
            let pct = (ratio * 100.0).round();
            let bar_text = match self.state {
                RunState::Idle => "Chua bat dau".to_string(),
                RunState::Scanning => format!("Dang quet... ({} muc)", self.stats.scanned_so_far),
                RunState::Copying | RunState::Cancelling => format!(
                    "{pct:.0}%  •  {}/{} file  •  {} / {}",
                    self.stats.files_copied,
                    self.stats.total_files_to_copy,
                    human_bytes(self.stats.bytes_copied),
                    human_bytes(self.stats.total_bytes_to_copy)
                ),
                RunState::Done => {
                    if self.last_cancelled {
                        "Da huy".to_string()
                    } else {
                        "100%  •  Hoan tat".to_string()
                    }
                }
            };

            let progress = egui::ProgressBar::new(ratio)
                .desired_height(26.0)
                .text(bar_text)
                .fill(if self.last_cancelled && self.state == RunState::Done {
                    theme::warning(self.dark_mode)
                } else {
                    theme::accent_strong(self.dark_mode)
                });
            ui.add(progress);

            if !self.current_project.is_empty() {
                ui.label(
                    egui::RichText::new(format!("Du an hien tai: {}", self.current_project))
                        .color(theme::muted(self.dark_mode))
                        .small(),
                );
            }

            ui.add_space(10.0);

            // --- Cac the thong ke ---
            ui.horizontal_wrapped(|ui| {
                stat_card(ui, "Da chep", &format!("{}", self.stats.files_copied), theme::success(self.dark_mode), self.dark_mode);
                stat_card(
                    ui,
                    "Dung luong da chep",
                    &human_bytes(self.stats.bytes_copied),
                    theme::accent(self.dark_mode),
                    self.dark_mode,
                );
                let speed_text = if self.state == RunState::Copying {
                    format!("{}/s", human_bytes(self.current_speed_bps as u64))
                } else {
                    "-".to_string()
                };
                stat_card(ui, "Toc do", &speed_text, theme::accent(self.dark_mode), self.dark_mode);

                let eta_text = match (self.state, self.eta_secs()) {
                    (RunState::Copying, Some(s)) => human_duration(s),
                    (RunState::Done, _) => "-".to_string(),
                    _ => "-".to_string(),
                };
                stat_card(ui, "Con lai (uoc tinh)", &eta_text, theme::muted(self.dark_mode), self.dark_mode);

                let elapsed = match (self.run_start, self.run_finished_secs) {
                    (_, Some(secs)) => human_duration(secs),
                    (Some(t), None) => human_duration(t.elapsed().as_secs_f64()),
                    _ => "00:00".to_string(),
                };
                stat_card(ui, "Thoi gian chay", &elapsed, theme::muted(self.dark_mode), self.dark_mode);

                stat_card(
                    ui,
                    "Da la moi nhat",
                    &format!("{}", self.stats.already_up_to_date),
                    theme::muted(self.dark_mode),
                    self.dark_mode,
                );
                stat_card(ui, "Da xoa (mirror)", &format!("{}", self.stats.files_deleted), theme::warning(self.dark_mode), self.dark_mode);
                stat_card(ui, "Loi", &format!("{}", self.stats.error_count), theme::danger(self.dark_mode), self.dark_mode);
            });

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(6.0);

            // --- Bieu do tron: co cau ket qua backup ---
            ui.label(egui::RichText::new("CO CAU KET QUA (BIEU DO TRON)").strong().color(theme::muted(self.dark_mode)));
            egui::Frame::group(ui.style())
                .fill(theme::bg_panel(self.dark_mode))
                .rounding(egui::Rounding::same(8.0))
                .inner_margin(egui::Margin::same(12.0))
                .show(ui, |ui| {
                    let slices = [
                        ("Da chep", self.stats.files_copied as f64, theme::success(self.dark_mode)),
                        ("Da la moi nhat", self.stats.already_up_to_date as f64, theme::accent(self.dark_mode)),
                        ("Da xoa (mirror)", self.stats.files_deleted as f64, theme::warning(self.dark_mode)),
                        ("Loi", self.stats.error_count as f64, theme::danger(self.dark_mode)),
                    ];
                    let total: f64 = slices.iter().map(|(_, v, _)| v).sum();

                    ui.horizontal(|ui| {
                        draw_donut_chart(
                            ui,
                            140.0,
                            &slices,
                            theme::bg_panel(self.dark_mode),
                            theme::muted(self.dark_mode),
                            theme::text(self.dark_mode),
                        );
                        ui.add_space(16.0);
                        ui.vertical(|ui| {
                            for (name, value, color) in slices {
                                let pct = if total > 0.0 { value / total * 100.0 } else { 0.0 };
                                ui.horizontal(|ui| {
                                    let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                                    ui.painter().rect_filled(rect, egui::Rounding::same(2.0), color);
                                    ui.label(
                                        egui::RichText::new(format!("{name}: {value:.0} ({pct:.0}%)"))
                                            .color(theme::text(self.dark_mode))
                                            .small(),
                                    );
                                });
                            }
                        });
                    });
                });

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(6.0);

            ui.label(egui::RichText::new("NHAT KY").strong().color(theme::muted(self.dark_mode)));
            egui::Frame::group(ui.style())
                .fill(theme::bg_panel(self.dark_mode))
                .inner_margin(egui::Margin::same(8.0))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .stick_to_bottom(true)
                        .max_height(ui.available_height())
                        .show(ui, |ui| {
                            for entry in &self.log {
                                let color = match entry.level {
                                    LogLevel::Info => theme::muted(self.dark_mode),
                                    LogLevel::Success => theme::success(self.dark_mode),
                                    LogLevel::Warn => theme::warning(self.dark_mode),
                                    LogLevel::Error => theme::danger(self.dark_mode),
                                };
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(&entry.time).color(theme::muted(self.dark_mode)).monospace().small());
                                    ui.label(egui::RichText::new(&entry.message).color(color).small());
                                });
                            }
                            if self.log.is_empty() {
                                ui.label(
                                    egui::RichText::new("Chua co hoat dong nao. Bam \"Bat dau backup\" de bat dau.")
                                        .color(theme::muted(self.dark_mode))
                                        .italics(),
                                );
                            }
                        });
                });
        });
    }
}

fn stat_card(ui: &mut egui::Ui, label: &str, value: &str, color: egui::Color32, dark: bool) {
    egui::Frame::group(ui.style())
        .fill(theme::bg_card(dark))
        .rounding(egui::Rounding::same(8.0))
        .inner_margin(egui::Margin::symmetric(14.0, 10.0))
        .show(ui, |ui| {
            ui.set_min_width(150.0);
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(label).color(theme::muted(dark)).small());
                ui.label(egui::RichText::new(value).color(color).strong().size(19.0));
            });
        });
}

/// Ve 1 bieu do tron (dang donut) the hien ty le cac `slices`.
/// Ve dang "pie" (quat tam giac tu tam ra vien - luon dung hinh hoc voi moi
/// goc), sau do phu 1 hinh tron mau nen len giua de tao lo rong (hieu ung
/// donut) va hien tong so o chinh giua.
fn draw_donut_chart(
    ui: &mut egui::Ui,
    size: f32,
    slices: &[(&str, f64, egui::Color32)],
    hole_color: egui::Color32,
    muted_color: egui::Color32,
    text_color: egui::Color32,
) {
    let total: f64 = slices.iter().map(|(_, v, _)| v).sum();
    let (rect, _response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let center = rect.center();
    let radius = size * 0.5 - 2.0;

    if total <= 0.0 {
        painter.circle_stroke(center, radius, egui::Stroke::new(2.0, muted_color));
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            "Chua co\ndu lieu",
            egui::FontId::proportional(12.0),
            muted_color,
        );
        return;
    }

    let mut start_angle = -std::f32::consts::FRAC_PI_2; // bat dau tu vi tri 12 gio
    for (_, value, color) in slices {
        if *value <= 0.0 {
            continue;
        }
        let sweep = (*value / total) as f32 * std::f32::consts::TAU;
        let steps = ((sweep.abs() / (std::f32::consts::TAU / 64.0)).ceil() as usize).max(1);
        let mut points = Vec::with_capacity(steps + 2);
        points.push(center);
        for i in 0..=steps {
            let t = start_angle + sweep * (i as f32 / steps as f32);
            points.push(center + egui::vec2(t.cos(), t.sin()) * radius);
        }
        painter.add(egui::Shape::convex_polygon(points, *color, egui::Stroke::NONE));
        start_angle += sweep;
    }

    // "Khoet lo" chinh giua bang 1 hinh tron mau nen -> tao hieu ung donut
    let hole_radius = radius * 0.58;
    painter.circle_filled(center, hole_radius, hole_color);

    painter.text(
        center + egui::vec2(0.0, -6.0),
        egui::Align2::CENTER_CENTER,
        format!("{total:.0}"),
        egui::FontId::proportional(20.0),
        text_color,
    );
    painter.text(
        center + egui::vec2(0.0, 12.0),
        egui::Align2::CENTER_CENTER,
        "tong file",
        egui::FontId::proportional(10.0),
        muted_color,
    );
}

/// Doc file `last_auto_backup.txt` (do lan chay `--auto-backup` gan nhat ghi lai,
/// xem main.rs) va tra ve (thanh_cong, dong_tom_tat_de_hien_thi).
fn load_last_auto_status(exe_dir: &std::path::Path) -> Option<(bool, String)> {
    let path = exe_dir.join("last_auto_backup.txt");
    let text = std::fs::read_to_string(path).ok()?;

    let mut timestamp = String::new();
    let mut message = String::new();
    let mut success = false;
    for line in text.lines() {
        if let Some((key, value)) = line.split_once('=') {
            match key {
                "timestamp" => timestamp = value.to_string(),
                "message" => message = value.to_string(),
                "success" => success = value == "true",
                _ => {}
            }
        }
    }

    if timestamp.is_empty() && message.is_empty() {
        return None;
    }

    let summary = format!(
        "[Lan backup tu dong luc {timestamp}] {message}{}",
        if success { "" } else { " (co van de, xem thu muc logs/)" }
    );
    Some((success, summary))
}
