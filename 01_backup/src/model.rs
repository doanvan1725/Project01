// ============================================================================
// model.rs - Cac kieu du lieu dung chung: thong diep tien trinh, thong ke, log
// ============================================================================

use std::path::PathBuf;

/// Thong diep worker/scanner gui ve luong giao dien (GUI) qua kenh mpsc.
/// GUI chi doc (try_recv) moi frame, khong bao gio bi block.
#[derive(Debug, Clone)]
pub enum ProgressMsg {
    /// Bat dau qua giai doan quet du lieu cho 1 project
    ScanningProject(String),
    /// Cap nhat so luong da quet (goi dinh ky, khong phai tung file, de tranh nghen kenh)
    ScanTick { scanned: u64 },
    /// Ket thuc quet toan bo, biet duoc tong khoi luong can copy
    ScanComplete {
        total_files_to_copy: u64,
        total_bytes_to_copy: u64,
        already_up_to_date: u64,
        total_delete_candidates: u64,
    },
    /// Mot file da copy xong
    FileCopied { rel_path: String, bytes: u64 },
    /// Mot file bi loi khi copy
    FileError { rel_path: String, message: String },
    /// Mot file/thu muc da bi xoa o dich (che do mirror)
    FileDeleted { rel_path: String },
    /// Toan bo qua trinh backup da hoan tat (hoac bi huy)
    AllDone { cancelled: bool, elapsed_secs: f64 },
    /// Thong bao chung hien thi trong nhat ky
    #[allow(dead_code)]
    Info(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Success,
    Warn,
    Error,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub time: String,
    pub level: LogLevel,
    pub message: String,
}

/// Thong ke tong hop hien thi tren giao dien, duoc cap nhat lien tuc tu ProgressMsg
#[derive(Debug, Default, Clone)]
pub struct Stats {
    pub scanned_so_far: u64,

    pub total_files_to_copy: u64,
    pub total_bytes_to_copy: u64,
    pub already_up_to_date: u64,
    pub total_delete_candidates: u64,

    pub files_copied: u64,
    pub bytes_copied: u64,
    pub files_deleted: u64,
    pub error_count: u64,
}

impl Stats {
    pub fn progress_ratio(&self) -> f32 {
        if self.total_bytes_to_copy == 0 {
            if self.total_files_to_copy == 0 {
                0.0
            } else {
                1.0
            }
        } else {
            (self.bytes_copied as f32 / self.total_bytes_to_copy as f32).clamp(0.0, 1.0)
        }
    }
}

/// Mot cong viec copy 1 file tu nguon sang dich
#[derive(Debug, Clone)]
pub struct CopyJob {
    pub src: PathBuf,
    pub dst: PathBuf,
    pub rel_path: String,
    pub size: u64,
}

pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut val = bytes as f64;
    let mut unit_idx = 0usize;
    while val >= 1024.0 && unit_idx < UNITS.len() - 1 {
        val /= 1024.0;
        unit_idx += 1;
    }
    if unit_idx == 0 {
        format!("{bytes} B")
    } else {
        format!("{:.2} {}", val, UNITS[unit_idx])
    }
}

pub fn human_duration(secs: f64) -> String {
    let secs = secs.max(0.0) as u64;
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h:02}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}
