// ============================================================================
// schedule.rs - Dang ky/go bo lich backup tu dong bang Windows Task Scheduler
// ============================================================================
// Tool khong tu chay ngam 24/7 (ton RAM khong can thiet). Thay vao do, khi
// nguoi dung bat lich, ta tao 1 tac vu trong Windows Task Scheduler goi lai
// chinh file .exe nay kem co `--auto-backup`; luc do main.rs se chay backup
// khong hien giao dien roi thoat, ghi ket qua ra file log canh .exe.
//
// Luu y quan trong: lich chi thuc su chay duoc neu MAY TINH DANG BAT va O
// DICH (Elements) DANG CAM VAO dung thoi diem da hen - day la gioi han tu
// nhien cua backup ra o cung roi, khong the khac phuc bang phan mem.

use crate::config::ScheduleConfig;
use std::path::Path;
use std::process::Command;

pub const TASK_NAME: &str = "VCI_BIM_Backup_Auto";

#[cfg(windows)]
fn base_command() -> Command {
    use std::os::windows::process::CommandExt;
    // Khong hien cua so console den khi goi schtasks tu giao dien
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut cmd = Command::new("schtasks");
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

#[cfg(not(windows))]
fn base_command() -> Command {
    Command::new("schtasks")
}

/// Tao (hoac cap nhat) tac vu lap lich trong Windows Task Scheduler.
pub fn install(exe_path: &Path, cfg: &ScheduleConfig) -> anyhow::Result<()> {
    if !cfg!(windows) {
        anyhow::bail!("Lap lich tu dong hien chi ho tro tren Windows (Task Scheduler).");
    }

    let time = format!("{:02}:{:02}", cfg.hour, cfg.minute);
    // Gia tri /TR phai tu chua dau ngoac kep quanh duong dan .exe (de schtasks
    // hieu dung ranh gioi ten file khi duong dan co khoang trang).
    let tr_value = format!("\"{}\" --auto-backup", exe_path.display());

    let mut cmd = base_command();
    cmd.args([
        "/Create",
        "/TN",
        TASK_NAME,
        "/TR",
        &tr_value,
        "/ST",
        &time,
        "/RL",
        "LIMITED",
        "/F",
    ]);

    use crate::config::ScheduleFrequency;
    match cfg.frequency {
        ScheduleFrequency::Daily => {
            cmd.args(["/SC", "DAILY"]);
        }
        ScheduleFrequency::Weekly => {
            cmd.args(["/SC", "WEEKLY", "/D", cfg.weekday.schtasks_code()]);
        }
        ScheduleFrequency::Monthly => {
            let day = cfg.day_of_month.clamp(1, 28).to_string();
            cmd.args(["/SC", "MONTHLY", "/D", &day]);
        }
    }

    let output = cmd
        .output()
        .map_err(|e| anyhow::anyhow!("Khong chay duoc schtasks.exe: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        anyhow::bail!(
            "schtasks bao loi: {}",
            if stderr.trim().is_empty() {
                stdout.trim().to_string()
            } else {
                stderr.trim().to_string()
            }
        );
    }
    Ok(())
}

/// Xoa tac vu lap lich (neu co). Khong bao loi neu tac vu chua ton tai.
pub fn uninstall() -> anyhow::Result<()> {
    let mut cmd = base_command();
    cmd.args(["/Delete", "/TN", TASK_NAME, "/F"]);

    let output = cmd
        .output()
        .map_err(|e| anyhow::anyhow!("Khong chay duoc schtasks.exe: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
        let not_found = stderr.contains("khong tim thay") || stderr.contains("cannot find");
        if not_found {
            return Ok(());
        }
        anyhow::bail!("schtasks bao loi: {}", stderr.trim());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ScheduleConfig, ScheduleFrequency, Weekday};
    use std::path::PathBuf;

    /// Chi kiem tra ham khong panic va tra ve Err co thong bao ro rang tren
    /// nen tang khong phai Windows (moi truong CI/dev cua repo nay la Linux).
    /// Hanh vi thuc te tren Windows (goi schtasks that) khong the kiem thu
    /// tu dong o day - can test thu cong tren may Windows that.
    #[test]
    fn test_install_fails_gracefully_off_windows() {
        if cfg!(windows) {
            return;
        }
        let cfg = ScheduleConfig {
            enabled: true,
            frequency: ScheduleFrequency::Daily,
            hour: 22,
            minute: 0,
            weekday: Weekday::Mon,
            day_of_month: 1,
        };
        let result = install(&PathBuf::from("/tmp/fake_vci_backup.exe"), &cfg);
        assert!(result.is_err());
    }
}
