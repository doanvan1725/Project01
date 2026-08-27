// ============================================================================
// config.rs - Doc/ghi file cau hinh config.toml
// ============================================================================
// File config.toml nam canh file .exe. Nguoi dung chinh sua danh sach project
// can backup, thu muc dich mac dinh, so luong, v.v. tai day (hoac qua giao dien).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Duong dan goc chua toan bo project tren NAS
    #[serde(default = "default_source_root")]
    pub source_root: String,

    /// Danh sach ten thu muc project (con cua source_root) can backup
    #[serde(default)]
    pub projects: Vec<String>,

    /// Thu muc dich mac dinh goi y khi mo tool (co the de trong)
    #[serde(default)]
    pub default_destination: String,

    /// So luong copy song song. 0 = tu dong chon theo CPU
    #[serde(default)]
    pub thread_count: usize,

    /// Cho phep xoa file/thu muc thua o dich (mirror that). false = an toan, chi them/cap nhat
    #[serde(default)]
    pub enable_mirror_delete: bool,

    /// Cac mau ten file/thu muc can bo qua khi backup (ho tro ky tu dai dien *)
    #[serde(default = "default_excludes")]
    pub exclude_patterns: Vec<String>,

    /// Cau hinh lich backup tu dong (dung Windows Task Scheduler)
    #[serde(default)]
    pub schedule: ScheduleConfig,

    /// Giao dien: true = che do Toi (mac dinh), false = che do Sang
    #[serde(default = "default_true")]
    pub ui_dark_mode: bool,
}

fn default_true() -> bool {
    true
}

/// Tan suat backup tu dong
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScheduleFrequency {
    Daily,
    Weekly,
    Monthly,
}

impl Default for ScheduleFrequency {
    fn default() -> Self {
        ScheduleFrequency::Daily
    }
}

impl ScheduleFrequency {
    pub fn as_toml_str(&self) -> &'static str {
        match self {
            ScheduleFrequency::Daily => "daily",
            ScheduleFrequency::Weekly => "weekly",
            ScheduleFrequency::Monthly => "monthly",
        }
    }

    pub fn display_vi(&self) -> &'static str {
        match self {
            ScheduleFrequency::Daily => "Hang ngay",
            ScheduleFrequency::Weekly => "Hang tuan",
            ScheduleFrequency::Monthly => "Hang thang",
        }
    }

    pub const ALL: [ScheduleFrequency; 3] = [
        ScheduleFrequency::Daily,
        ScheduleFrequency::Weekly,
        ScheduleFrequency::Monthly,
    ];
}

/// Thu trong tuan (dung cho tan suat "weekly")
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Weekday {
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
    Sun,
}

impl Default for Weekday {
    fn default() -> Self {
        Weekday::Mon
    }
}

impl Weekday {
    pub fn as_toml_str(&self) -> &'static str {
        match self {
            Weekday::Mon => "mon",
            Weekday::Tue => "tue",
            Weekday::Wed => "wed",
            Weekday::Thu => "thu",
            Weekday::Fri => "fri",
            Weekday::Sat => "sat",
            Weekday::Sun => "sun",
        }
    }

    /// Ma thu ma lenh `schtasks` (Windows Task Scheduler) hieu duoc
    pub fn schtasks_code(&self) -> &'static str {
        match self {
            Weekday::Mon => "MON",
            Weekday::Tue => "TUE",
            Weekday::Wed => "WED",
            Weekday::Thu => "THU",
            Weekday::Fri => "FRI",
            Weekday::Sat => "SAT",
            Weekday::Sun => "SUN",
        }
    }

    pub fn display_vi(&self) -> &'static str {
        match self {
            Weekday::Mon => "Thu Hai",
            Weekday::Tue => "Thu Ba",
            Weekday::Wed => "Thu Tu",
            Weekday::Thu => "Thu Nam",
            Weekday::Fri => "Thu Sau",
            Weekday::Sat => "Thu Bay",
            Weekday::Sun => "Chu Nhat",
        }
    }

    pub const ALL: [Weekday; 7] = [
        Weekday::Mon,
        Weekday::Tue,
        Weekday::Wed,
        Weekday::Thu,
        Weekday::Fri,
        Weekday::Sat,
        Weekday::Sun,
    ];
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleConfig {
    /// Bat/tat backup tu dong chay ngam theo lich
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub frequency: ScheduleFrequency,
    /// Gio chay, 0-23
    #[serde(default = "default_schedule_hour")]
    pub hour: u32,
    /// Phut chay, 0-59
    #[serde(default)]
    pub minute: u32,
    /// Chi dung khi frequency = Weekly
    #[serde(default)]
    pub weekday: Weekday,
    /// Chi dung khi frequency = Monthly. Gioi han 1-28 de luon hop le moi thang
    /// (thang 2 khong co ngay 29-31, Task Scheduler se bo qua thang do neu chon > 28).
    #[serde(default = "default_day_of_month")]
    pub day_of_month: u32,
}

fn default_schedule_hour() -> u32 {
    22
}

fn default_day_of_month() -> u32 {
    1
}

impl Default for ScheduleConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            frequency: ScheduleFrequency::Daily,
            hour: default_schedule_hour(),
            minute: 0,
            weekday: Weekday::Mon,
            day_of_month: default_day_of_month(),
        }
    }
}

fn default_source_root() -> String {
    r"\\Vci-bim-nas\projects".to_string()
}

fn default_excludes() -> Vec<String> {
    vec![
        "*.bak".to_string(),
        "~$*".to_string(),
        "*.tmp".to_string(),
        ".dropbox".to_string(),
        "Thumbs.db".to_string(),
        "desktop.ini".to_string(),
    ]
}

impl Default for Config {
    fn default() -> Self {
        Self {
            source_root: default_source_root(),
            projects: vec![
                "Project_A".to_string(),
                "Project_B".to_string(),
            ],
            default_destination: String::new(),
            thread_count: 0,
            enable_mirror_delete: false,
            exclude_patterns: default_excludes(),
            schedule: ScheduleConfig::default(),
            ui_dark_mode: true,
        }
    }
}

impl Config {
    /// Doc config tu duong dan; neu chua co thi tao file mau va tra ve config mac dinh
    pub fn load_or_create(path: &Path) -> anyhow::Result<Config> {
        if path.exists() {
            let text = std::fs::read_to_string(path)?;
            let cfg: Config = toml::from_str(&text)
                .map_err(|e| anyhow::anyhow!("Loi doc config.toml: {e}"))?;
            Ok(cfg)
        } else {
            let cfg = Config::default();
            cfg.save(path)?;
            Ok(cfg)
        }
    }

    /// Ghi config.toml bang dinh dang co chu thich (khong dung toml::to_string_pretty
    /// truc tiep vi no se lam mat toan bo comment giai thich moi lan luu tu giao dien).
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        std::fs::write(path, self.to_pretty_toml())?;
        Ok(())
    }

    fn to_pretty_toml(&self) -> String {
        let mut s = String::new();
        s.push_str("# ============================================================\n");
        s.push_str("# CAU HINH - VCI BIM Backup Tool\n");
        s.push_str("# ============================================================\n");
        s.push_str("# File nay co the chinh sua truc tiep bang Notepad, hoac chinh qua\n");
        s.push_str("# giao dien roi bam \"Luu cau hinh\" (comment se duoc giu lai).\n\n");

        s.push_str("# Duong dan goc chua toan bo project tren NAS\n");
        s.push_str(&format!("source_root = {:?}\n\n", self.source_root));

        s.push_str("# Danh sach cac project (thu muc con cua source_root) can backup.\n");
        s.push_str("# Chi nhung project co trong danh sach nay moi duoc backup.\n");
        s.push_str("projects = [\n");
        for p in &self.projects {
            s.push_str(&format!("    {:?},\n", p));
        }
        s.push_str("]\n\n");

        s.push_str("# Thu muc dich mac dinh goi y khi mo tool (co the de trong \"\").\n");
        s.push_str("# Van co the bam \"Chon o dich...\" trong giao dien de doi o khac.\n");
        s.push_str(&format!("default_destination = {:?}\n\n", self.default_destination));

        s.push_str("# So luong copy song song. 0 = tool tu dong chon theo so nhan CPU.\n");
        s.push_str(&format!("thread_count = {}\n\n", self.thread_count));

        s.push_str("# true = XOA cac file/thu muc thua o dich khong con o nguon (mirror that).\n");
        s.push_str("# false = chi them/cap nhat, khong xoa gi (an toan hon).\n");
        s.push_str(&format!("enable_mirror_delete = {}\n\n", self.enable_mirror_delete));

        s.push_str("# Cac mau ten file/thu muc can bo qua khi backup (ho tro ky tu dai dien *)\n");
        s.push_str("exclude_patterns = [\n");
        for p in &self.exclude_patterns {
            s.push_str(&format!("    {:?},\n", p));
        }
        s.push_str("]\n\n");

        s.push_str("# Giao dien: true = che do Toi, false = che do Sang.\n");
        s.push_str("# Cach de xuat: bam nut Sang/Toi tren giao dien, tool tu luu lai o day.\n");
        s.push_str(&format!("ui_dark_mode = {}\n", self.ui_dark_mode));

        s.push_str("\n[schedule]\n");
        s.push_str("# Bat/tat backup TU DONG chay ngam theo lich (dung Windows Task Scheduler).\n");
        s.push_str("# Luu y: may tinh phai dang BAT va o Elements phai dang CAM dung luc lich\n");
        s.push_str("# chay thi lan backup tu dong do moi thuc su dien ra.\n");
        s.push_str(&format!("enabled = {}\n", self.schedule.enabled));
        s.push_str("# \"daily\" | \"weekly\" | \"monthly\"\n");
        s.push_str(&format!("frequency = {:?}\n", self.schedule.frequency.as_toml_str()));
        s.push_str("# Gio chay (0-23) va phut (0-59)\n");
        s.push_str(&format!("hour = {}\n", self.schedule.hour));
        s.push_str(&format!("minute = {}\n", self.schedule.minute));
        s.push_str("# Chi dung khi frequency = \"weekly\": mon/tue/wed/thu/fri/sat/sun\n");
        s.push_str(&format!("weekday = {:?}\n", self.schedule.weekday.as_toml_str()));
        s.push_str("# Chi dung khi frequency = \"monthly\": ngay trong thang (1-28)\n");
        s.push_str(&format!("day_of_month = {}\n", self.schedule.day_of_month));

        s
    }

    /// So luong luong copy hieu qua: neu config = 0 thi tu dong chon theo so nhan CPU.
    /// Backup qua mang (SMB/NAS) la tac vu I/O-bound nen khong can qua nhieu luong,
    /// qua nhieu se gay tranh chap bang thong mang va o dia dich thay vi nhanh hon.
    pub fn effective_thread_count(&self) -> usize {
        if self.thread_count > 0 {
            self.thread_count
        } else {
            let cpu = num_cpus::get();
            cpu.clamp(4, 8)
        }
    }

    pub fn source_root_path(&self) -> PathBuf {
        PathBuf::from(&self.source_root)
    }
}

#[cfg(test)]
mod shipped_config_test {
    use super::*;

    #[test]
    fn test_shipped_config_toml_parses_and_roundtrips() {
        let text = std::fs::read_to_string(
            concat!(env!("CARGO_MANIFEST_DIR"), "/config.toml"),
        )
        .expect("khong doc duoc config.toml mau di kem");
        let cfg: Config = toml::from_str(&text).expect("config.toml mau di kem bi loi cu phap");
        assert_eq!(cfg.source_root, r"\\Vci-bim-nas\projects");
        assert_eq!(cfg.projects, vec!["Project_A".to_string(), "Project_B".to_string()]);
        assert_eq!(cfg.thread_count, 0);
        assert!(!cfg.enable_mirror_delete);
        assert!(cfg.exclude_patterns.contains(&"*.bak".to_string()));
        // config.toml mau co [schedule] nhung mac dinh la TAT
        assert!(!cfg.schedule.enabled);
        assert_eq!(cfg.schedule.frequency, ScheduleFrequency::Daily);
        assert!(cfg.ui_dark_mode);
    }

    #[test]
    fn test_ui_dark_mode_roundtrip() {
        let mut cfg = Config::default();
        cfg.ui_dark_mode = false;
        let text = cfg.to_pretty_toml();
        let parsed: Config = toml::from_str(&text).unwrap();
        assert!(!parsed.ui_dark_mode);
        // Cac truong sau ui_dark_mode (vd [schedule]) van phai doc dung, tranh
        // loi thu tu bang TOML (bare key sau 1 header [table] se bi gan nham
        // vao table do thay vi o cap root).
        assert_eq!(parsed.schedule.frequency, ScheduleFrequency::Daily);
    }

    #[test]
    fn test_missing_ui_dark_mode_defaults_true() {
        // Mo phong 1 config.toml cu chua co dong ui_dark_mode -> phai fallback true
        let text = "source_root = \"\\\\\\\\srv\\\\share\"\n";
        let cfg: Config = toml::from_str(text).unwrap();
        assert!(cfg.ui_dark_mode);
    }

    #[test]
    fn test_schedule_roundtrip_through_pretty_toml() {
        let mut cfg = Config::default();
        cfg.schedule.enabled = true;
        cfg.schedule.frequency = ScheduleFrequency::Weekly;
        cfg.schedule.hour = 23;
        cfg.schedule.minute = 30;
        cfg.schedule.weekday = Weekday::Fri;
        cfg.schedule.day_of_month = 15;

        let text = cfg.to_pretty_toml();
        let parsed: Config = toml::from_str(&text).expect("loi cu phap sau khi ghi lai config");

        assert!(parsed.schedule.enabled);
        assert_eq!(parsed.schedule.frequency, ScheduleFrequency::Weekly);
        assert_eq!(parsed.schedule.hour, 23);
        assert_eq!(parsed.schedule.minute, 30);
        assert_eq!(parsed.schedule.weekday, Weekday::Fri);
        assert_eq!(parsed.schedule.day_of_month, 15);
    }

    #[test]
    fn test_schedule_monthly_roundtrip() {
        let mut cfg = Config::default();
        cfg.schedule.enabled = true;
        cfg.schedule.frequency = ScheduleFrequency::Monthly;
        cfg.schedule.day_of_month = 5;

        let text = cfg.to_pretty_toml();
        let parsed: Config = toml::from_str(&text).unwrap();
        assert_eq!(parsed.schedule.frequency, ScheduleFrequency::Monthly);
        assert_eq!(parsed.schedule.day_of_month, 5);
    }
}
