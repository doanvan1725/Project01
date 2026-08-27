// ============================================================================
// main.rs - Diem khoi dong ung dung "VCI BIM Backup Tool"
// ============================================================================
// Backup du lieu tu \\Vci-bim-nas\projects sang o cung roi (Elements) theo
// che do dong bo mot chieu (mirror), da luong, toi uu RAM va toc do.
//
// Build ban phat hanh:   cargo build --release
// File .exe tao ra tai:  target\release\vci_backup.exe
// (Nho dat file config.toml canh file .exe khi mang di su dung)
//
// Che do chay ngam theo lich: khi khoi chay voi tham so `--auto-backup`
// (do Windows Task Scheduler tu goi, xem src/schedule.rs), tool se KHONG mo
// giao dien - chi doc config.toml, backup toan bo `projects` da cau hinh sang
// `default_destination`, roi ghi ket qua vao `last_auto_backup.txt` va thu
// muc `logs/` canh file .exe, sau do thoat.

// Tren Windows, an console den khi chay ban release (chi giu console luc dev/debug)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod copier;
mod model;
mod patterns;
mod scanner;
mod schedule;
mod theme;

use app::BackupApp;
use config::Config;
use model::{human_bytes, ProgressMsg};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::channel;
use std::sync::Arc;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().skip(1).any(|a| a == "--auto-backup") {
        run_auto_backup();
        return Ok(());
    }

    let viewport = egui::ViewportBuilder::default()
        .with_title("VCI BIM Backup Tool")
        .with_inner_size([1080.0, 720.0])
        .with_min_inner_size([860.0, 560.0]);

    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "VCI BIM Backup Tool",
        native_options,
        Box::new(|cc| Ok(Box::new(BackupApp::new(cc)))),
    )
}

/// Chay 1 lan backup khong giao dien (duoc Windows Task Scheduler goi), roi thoat.
fn run_auto_backup() {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    let config_path = exe_dir.join("config.toml");

    let config = match Config::load_or_create(&config_path) {
        Ok(c) => c,
        Err(e) => {
            write_status(&exe_dir, false, &format!("Loi doc config.toml: {e}"), None);
            return;
        }
    };

    if config.default_destination.trim().is_empty() {
        write_status(
            &exe_dir,
            false,
            "Chua cau hinh thu muc dich (default_destination) trong config.toml.",
            None,
        );
        return;
    }
    let destination = PathBuf::from(&config.default_destination);
    if !destination.exists() {
        write_status(
            &exe_dir,
            false,
            "O dich khong ket noi luc lich chay (co the o Elements chua duoc cam vao). Da bo qua lan backup nay.",
            None,
        );
        return;
    }
    if config.projects.is_empty() {
        write_status(
            &exe_dir,
            false,
            "Danh sach 'projects' trong config.toml dang trong, khong co gi de backup.",
            None,
        );
        return;
    }

    let (tx, rx) = channel::<ProgressMsg>();
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let thread_count = config.effective_thread_count();
    let mirror_delete = config.enable_mirror_delete;
    let projects = config.projects.clone();

    // Chay dong bo ngay tren luong chinh: khong co giao dien nao can cho, va
    // run_backup() tu quan ly da luong copy ben trong roi moi tra ve khi xong.
    copier::run_backup(config, projects, destination, thread_count, mirror_delete, tx, cancel_flag);

    let mut files_copied = 0u64;
    let mut bytes_copied = 0u64;
    let mut files_deleted = 0u64;
    let mut errors = 0u64;
    let mut cancelled = false;
    for msg in rx.try_iter() {
        match msg {
            ProgressMsg::FileCopied { bytes, .. } => {
                files_copied += 1;
                bytes_copied += bytes;
            }
            ProgressMsg::FileDeleted { .. } => files_deleted += 1,
            ProgressMsg::FileError { .. } => errors += 1,
            ProgressMsg::AllDone { cancelled: c, .. } => cancelled = c,
            _ => {}
        }
    }

    let message = format!(
        "Da chep {files_copied} file ({}), xoa {files_deleted} muc, {errors} loi.",
        human_bytes(bytes_copied)
    );
    write_status(
        &exe_dir,
        errors == 0 && !cancelled,
        &message,
        Some((files_copied, bytes_copied, files_deleted, errors)),
    );
}

fn write_status(dir: &Path, success: bool, message: &str, stats: Option<(u64, u64, u64, u64)>) {
    let now = chrono::Local::now();

    let mut content = String::new();
    content.push_str(&format!("timestamp={}\n", now.format("%Y-%m-%d %H:%M:%S")));
    content.push_str(&format!("success={success}\n"));
    content.push_str(&format!("message={message}\n"));
    if let Some((copied, bytes, deleted, errors)) = stats {
        content.push_str(&format!("files_copied={copied}\n"));
        content.push_str(&format!("bytes_copied={bytes}\n"));
        content.push_str(&format!("files_deleted={deleted}\n"));
        content.push_str(&format!("errors={errors}\n"));
    }
    let _ = std::fs::write(dir.join("last_auto_backup.txt"), content);

    // Them 1 file log rieng co dau thoi gian de giu lai lich su cac lan chay tu dong
    let logs_dir = dir.join("logs");
    if std::fs::create_dir_all(&logs_dir).is_ok() {
        let log_name = format!("auto_backup_{}.log", now.format("%Y%m%d_%H%M%S"));
        let _ = std::fs::write(
            logs_dir.join(log_name),
            format!("{}\n{}\n", now.format("%Y-%m-%d %H:%M:%S"), message),
        );
    }
}
