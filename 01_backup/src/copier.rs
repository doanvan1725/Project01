// ============================================================================
// copier.rs - Engine backup: dieu phoi quet + copy da luong + xoa (mirror)
// ============================================================================
// Chay tren 1 luong "dieu phoi" rieng (khong phai luong giao dien) de UI khong
// bao gio bi dung. Luong dieu phoi lai chia cong viec copy cho N luong worker.
//
// Toi uu RAM: moi worker chi cap phat DUY NHAT 1 buffer co dinh (mac dinh 4MB)
// va tai su dung cho toan bo cac file no xu ly, thay vi doc nguyen ca file vao
// bo nho (tranh OOM voi cac file Revit/AutoCAD nang hang chuc GB).

use crate::config::Config;
use crate::model::{CopyJob, ProgressMsg};
use crate::scanner::scan_project;
use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

/// 4 MB: du lon de giam so lan goi syscall doc/ghi qua mang (tang toc do),
/// du nho de RAM chi ton (so_luong_luong * 4MB) thay vi tai ca file vao bo nho.
const COPY_BUFFER_SIZE: usize = 4 * 1024 * 1024;

enum CopyOutcome {
    Cancelled,
    Error(String),
}

/// Ham chay chinh, goi tu 1 std::thread::spawn khi nguoi dung bam "Bat dau backup".
pub fn run_backup(
    config: Config,
    projects: Vec<String>,
    destination_root: PathBuf,
    thread_count: usize,
    mirror_delete: bool,
    tx: Sender<ProgressMsg>,
    cancel_flag: Arc<AtomicBool>,
) {
    let start = Instant::now();
    let source_root = config.source_root_path();

    if !source_root.exists() {
        let _ = tx.send(ProgressMsg::FileError {
            rel_path: String::new(),
            message: format!(
                "Khong truy cap duoc thu muc nguon: {}. Kiem tra ket noi mang toi NAS.",
                source_root.display()
            ),
        });
        let _ = tx.send(ProgressMsg::AllDone {
            cancelled: false,
            elapsed_secs: start.elapsed().as_secs_f64(),
        });
        return;
    }

    let _ = fs::create_dir_all(&destination_root);

    let mut all_jobs: Vec<CopyJob> = Vec::new();
    let mut all_delete_files: Vec<PathBuf> = Vec::new();
    let mut all_delete_dirs: Vec<PathBuf> = Vec::new();
    let mut total_up_to_date: u64 = 0;
    let mut running_scanned: u64 = 0;

    for project in &projects {
        if cancel_flag.load(Ordering::Relaxed) {
            break;
        }
        let _ = tx.send(ProgressMsg::ScanningProject(project.clone()));

        let src_project = source_root.join(project);
        let dst_project = destination_root.join(project);

        if !src_project.exists() {
            let _ = tx.send(ProgressMsg::FileError {
                rel_path: project.clone(),
                message: "Khong tim thay thu muc du an nay tren NAS, da bo qua.".to_string(),
            });
            continue;
        }

        let base = running_scanned;
        let tx_tick = tx.clone();
        let result = scan_project(
            &src_project,
            &dst_project,
            &config.exclude_patterns,
            mirror_delete,
            |scanned| {
                let _ = tx_tick.send(ProgressMsg::ScanTick {
                    scanned: base + scanned,
                });
            },
        );

        match result {
            Ok(r) => {
                running_scanned += r.total_scanned;
                total_up_to_date += r.already_up_to_date;
                all_jobs.extend(r.copy_jobs);
                all_delete_files.extend(r.delete_files);
                all_delete_dirs.extend(r.delete_dirs);
            }
            Err(e) => {
                let _ = tx.send(ProgressMsg::FileError {
                    rel_path: project.clone(),
                    message: format!("Loi khi quet du an: {e}"),
                });
            }
        }
    }

    let total_bytes: u64 = all_jobs.iter().map(|j| j.size).sum();
    let total_files: u64 = all_jobs.len() as u64;
    let total_delete_candidates = (all_delete_files.len() + all_delete_dirs.len()) as u64;

    let _ = tx.send(ProgressMsg::ScanComplete {
        total_files_to_copy: total_files,
        total_bytes_to_copy: total_bytes,
        already_up_to_date: total_up_to_date,
        total_delete_candidates,
    });

    if cancel_flag.load(Ordering::Relaxed) {
        let _ = tx.send(ProgressMsg::AllDone {
            cancelled: true,
            elapsed_secs: start.elapsed().as_secs_f64(),
        });
        return;
    }

    // --- Giai doan copy da luong ---
    if !all_jobs.is_empty() {
        let queue: Arc<Mutex<VecDeque<CopyJob>>> = Arc::new(Mutex::new(all_jobs.into()));
        let n_threads = thread_count.max(1);
        let mut handles = Vec::with_capacity(n_threads);

        for _ in 0..n_threads {
            let queue = Arc::clone(&queue);
            let tx = tx.clone();
            let cancel_flag = Arc::clone(&cancel_flag);

            handles.push(thread::spawn(move || {
                let mut buffer = vec![0u8; COPY_BUFFER_SIZE];
                loop {
                    if cancel_flag.load(Ordering::Relaxed) {
                        break;
                    }
                    let job = {
                        let mut q = queue.lock().unwrap();
                        q.pop_front()
                    };
                    let job = match job {
                        Some(j) => j,
                        None => break,
                    };

                    match copy_file(&job.src, &job.dst, &mut buffer, &cancel_flag) {
                        Ok(bytes) => {
                            let _ = tx.send(ProgressMsg::FileCopied {
                                rel_path: job.rel_path,
                                bytes,
                            });
                        }
                        Err(CopyOutcome::Cancelled) => break,
                        Err(CopyOutcome::Error(message)) => {
                            let _ = tx.send(ProgressMsg::FileError {
                                rel_path: job.rel_path,
                                message,
                            });
                        }
                    }
                }
            }));
        }

        for h in handles {
            let _ = h.join();
        }
    }

    // --- Giai doan xoa (mirror that) - chi chay neu KHONG bi huy giua chung ---
    if mirror_delete && !cancel_flag.load(Ordering::Relaxed) {
        for f in &all_delete_files {
            if cancel_flag.load(Ordering::Relaxed) {
                break;
            }
            match fs::remove_file(f) {
                Ok(_) => {
                    let _ = tx.send(ProgressMsg::FileDeleted {
                        rel_path: display_rel(f, &destination_root),
                    });
                }
                Err(e) => {
                    let _ = tx.send(ProgressMsg::FileError {
                        rel_path: display_rel(f, &destination_root),
                        message: format!("Khong xoa duoc: {e}"),
                    });
                }
            }
        }
        for d in &all_delete_dirs {
            if cancel_flag.load(Ordering::Relaxed) {
                break;
            }
            // remove_dir chi thanh cong neu thu muc rong -> an toan, khong xoa nham
            if fs::remove_dir(d).is_ok() {
                let _ = tx.send(ProgressMsg::FileDeleted {
                    rel_path: display_rel(d, &destination_root),
                });
            }
        }
    }

    let _ = tx.send(ProgressMsg::AllDone {
        cancelled: cancel_flag.load(Ordering::Relaxed),
        elapsed_secs: start.elapsed().as_secs_f64(),
    });
}

fn display_rel(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn copy_file(
    src: &Path,
    dst: &Path,
    buffer: &mut [u8],
    cancel_flag: &AtomicBool,
) -> Result<u64, CopyOutcome> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| CopyOutcome::Error(format!("Khong tao duoc thu muc dich: {e}")))?;
    }

    let mut src_file =
        File::open(src).map_err(|e| CopyOutcome::Error(format!("Khong mo duoc file nguon: {e}")))?;
    let src_mtime = src_file.metadata().ok().and_then(|m| m.modified().ok());

    let mut dst_file =
        File::create(dst).map_err(|e| CopyOutcome::Error(format!("Khong tao duoc file dich: {e}")))?;

    let mut total: u64 = 0;
    loop {
        if cancel_flag.load(Ordering::Relaxed) {
            drop(dst_file);
            let _ = fs::remove_file(dst);
            return Err(CopyOutcome::Cancelled);
        }

        let n = match src_file.read(buffer) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => return Err(CopyOutcome::Error(format!("Loi doc file: {e}"))),
        };

        if let Err(e) = dst_file.write_all(&buffer[..n]) {
            return Err(CopyOutcome::Error(format!("Loi ghi file (kiem tra dung luong o dich): {e}")));
        }
        total += n as u64;
    }

    if let Err(e) = dst_file.flush() {
        return Err(CopyOutcome::Error(format!("Loi ghi file: {e}")));
    }
    drop(dst_file);

    if let Some(mtime) = src_mtime {
        let ft = filetime::FileTime::from_system_time(mtime);
        let _ = filetime::set_file_mtime(dst, ft);
    }

    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_copy_file_preserves_content_and_mtime() {
        let dir = std::env::temp_dir().join(format!("vci_copier_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let src = dir.join("src.bin");
        let dst = dir.join("sub").join("dst.bin");

        // Du lieu lon hon buffer 4MB de kiem tra vong lap doc/ghi nhieu lan
        let payload = vec![7u8; COPY_BUFFER_SIZE * 2 + 123];
        fs::write(&src, &payload).unwrap();

        let cancel = AtomicBool::new(false);
        let mut buf = vec![0u8; COPY_BUFFER_SIZE];
        let bytes = copy_file(&src, &dst, &mut buf, &cancel).map_err(|e| match e {
            CopyOutcome::Cancelled => "cancelled".to_string(),
            CopyOutcome::Error(m) => m,
        }).unwrap();

        assert_eq!(bytes, payload.len() as u64);
        let copied = fs::read(&dst).unwrap();
        assert_eq!(copied.len(), payload.len());
        assert!(copied.iter().all(|&b| b == 7));

        let src_mtime = fs::metadata(&src).unwrap().modified().unwrap();
        let dst_mtime = fs::metadata(&dst).unwrap().modified().unwrap();
        let diff = src_mtime
            .duration_since(dst_mtime)
            .or_else(|_| dst_mtime.duration_since(src_mtime))
            .unwrap();
        assert!(diff.as_secs() <= 1, "mtime khong duoc bao toan dung");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_run_backup_end_to_end_mirror() {
        use crate::config::Config;
        use std::sync::mpsc::channel;

        let dir = std::env::temp_dir().join(format!("vci_run_backup_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let source_root = dir.join("nas");
        let dest_root = dir.join("elements");
        fs::create_dir_all(source_root.join("ProjectX")).unwrap();
        fs::create_dir_all(dest_root.join("ProjectX")).unwrap();

        // 2 file moi can copy
        fs::write(source_root.join("ProjectX").join("a.rvt"), vec![9u8; 5000]).unwrap();
        fs::write(source_root.join("ProjectX").join("b.rvt"), vec![9u8; 6000]).unwrap();
        // 1 file thua o dich can bi xoa (mirror)
        fs::write(dest_root.join("ProjectX").join("orphan.old"), b"x").unwrap();

        let mut config = Config::default();
        config.source_root = source_root.to_string_lossy().to_string();
        config.exclude_patterns = vec![];

        let (tx, rx) = channel::<ProgressMsg>();
        let cancel = Arc::new(AtomicBool::new(false));

        run_backup(
            config,
            vec!["ProjectX".to_string()],
            dest_root.clone(),
            2,
            true, // mirror_delete
            tx,
            cancel,
        );

        let mut copied = 0u64;
        let mut deleted = 0u64;
        let mut done = false;
        for msg in rx.try_iter() {
            match msg {
                ProgressMsg::FileCopied { .. } => copied += 1,
                ProgressMsg::FileDeleted { .. } => deleted += 1,
                ProgressMsg::AllDone { cancelled, .. } => {
                    done = true;
                    assert!(!cancelled);
                }
                _ => {}
            }
        }

        assert_eq!(copied, 2);
        assert_eq!(deleted, 1);
        assert!(done);
        assert!(dest_root.join("ProjectX").join("a.rvt").exists());
        assert!(dest_root.join("ProjectX").join("b.rvt").exists());
        assert!(!dest_root.join("ProjectX").join("orphan.old").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_copy_file_cancelled_removes_partial_file() {
        let dir = std::env::temp_dir().join(format!("vci_copier_test_cancel_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let src = dir.join("src.bin");
        let dst = dir.join("dst.bin");
        fs::write(&src, vec![1u8; 1024]).unwrap();

        let cancel = AtomicBool::new(true); // da huy tu truoc khi bat dau
        let mut buf = vec![0u8; COPY_BUFFER_SIZE];
        let result = copy_file(&src, &dst, &mut buf, &cancel);
        assert!(matches!(result, Err(CopyOutcome::Cancelled)));
        assert!(!dst.exists(), "file dich do dang phai duoc don dep sau khi huy");

        let _ = fs::remove_dir_all(&dir);
    }
}
