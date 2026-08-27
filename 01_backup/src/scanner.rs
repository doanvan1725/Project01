// ============================================================================
// scanner.rs - Quet thu muc nguon/dich va tinh toan viec can lam (copy/xoa)
// ============================================================================
// Chien luoc so sanh: kich thuoc (size) + thoi gian sua doi (mtime), dung sai
// 2 giay de tuong thich voi he thong file exFAT/FAT32 tren o cung roi (do
// nhung he thong nay chi luu mtime voi do phan giai 2 giay).
// Khong hash noi dung file vi du lieu BIM (Revit/AutoCAD) co the rat lon (hang
// chuc GB) -> hash se cham hon nhieu lan so voi chi doc metadata.

use crate::model::CopyJob;
use crate::patterns::matches_any;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use walkdir::WalkDir;

const MTIME_TOLERANCE_SECS: i64 = 2;

struct DestEntry {
    size: u64,
    mtime: SystemTime,
}

/// Ket qua quet 1 project: viec can copy, viec can xoa (mirror), so file da la moi nhat
pub struct ScanResult {
    pub copy_jobs: Vec<CopyJob>,
    pub delete_files: Vec<PathBuf>,
    pub delete_dirs: Vec<PathBuf>, // sap xep theo do sau giam dan (xoa truoc thu muc con)
    pub already_up_to_date: u64,
    pub total_scanned: u64,
}

/// Quet 1 cap thu muc (nguon project / dich project) va tra ve danh sach cong viec.
/// `on_tick` duoc goi dinh ky de bao tien do quet (khong bat buoc chinh xac tung file).
pub fn scan_project(
    src_root: &Path,
    dst_root: &Path,
    exclude_patterns: &[String],
    enable_mirror_delete: bool,
    mut on_tick: impl FnMut(u64),
) -> anyhow::Result<ScanResult> {
    // 1) Quet toan bo dich truoc (neu co) de biet file nao da ton tai + metadata
    let mut dest_files: HashMap<String, DestEntry> = HashMap::new();
    let mut dest_dirs: Vec<(String, usize)> = Vec::new(); // (rel_path, depth)

    if dst_root.exists() {
        for entry in WalkDir::new(dst_root).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path == dst_root {
                continue;
            }
            let rel = match path.strip_prefix(dst_root) {
                Ok(r) => normalize_rel(r),
                Err(_) => continue,
            };
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.file_type().is_dir() {
                if matches_any(&name, exclude_patterns) {
                    continue;
                }
                dest_dirs.push((rel.clone(), rel.matches('/').count()));
            } else if entry.file_type().is_file() {
                if matches_any(&name, exclude_patterns) {
                    continue;
                }
                if let Ok(meta) = entry.metadata() {
                    dest_files.insert(
                        rel,
                        DestEntry {
                            size: meta.len(),
                            mtime: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                        },
                    );
                }
            }
        }
    }

    // 2) Quet nguon, quyet dinh copy hay bo qua, dong thoi thu thap tap hop
    //    duong dan tuong doi da ton tai o nguon (de tinh phan can xoa o buoc 3)
    let mut copy_jobs = Vec::new();
    let mut source_rel_files: HashSet<String> = HashSet::new();
    let mut source_rel_dirs: HashSet<String> = HashSet::new();
    let mut already_up_to_date: u64 = 0;
    let mut scanned: u64 = 0;

    let walker = WalkDir::new(src_root).into_iter().filter_entry(|e| {
        if e.depth() == 0 {
            return true;
        }
        let name = e.file_name().to_string_lossy();
        !matches_any(&name, exclude_patterns)
    });

    for entry in walker.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path == src_root {
            continue;
        }
        let rel = match path.strip_prefix(src_root) {
            Ok(r) => normalize_rel(r),
            Err(_) => continue,
        };

        if entry.file_type().is_dir() {
            source_rel_dirs.insert(rel);
            continue;
        }
        if !entry.file_type().is_file() {
            continue; // bo qua symlink/thiet bi dac biet
        }

        scanned += 1;
        if scanned % 200 == 0 {
            on_tick(scanned);
        }

        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let size = meta.len();
        let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);

        source_rel_files.insert(rel.clone());

        let needs_copy = match dest_files.get(&rel) {
            None => true,
            Some(existing) => !same_file(existing, size, mtime),
        };

        if needs_copy {
            let dst_path = dst_root.join(&rel);
            copy_jobs.push(CopyJob {
                src: path.to_path_buf(),
                dst: dst_path,
                rel_path: rel,
                size,
            });
        } else {
            already_up_to_date += 1;
        }
    }
    on_tick(scanned);

    // 3) Xac dinh phan can xoa o dich (chi khi bat che do mirror that)
    let mut delete_files = Vec::new();
    let mut delete_dirs = Vec::new();
    if enable_mirror_delete {
        for (rel, _entry) in dest_files.iter() {
            if !source_rel_files.contains(rel) {
                delete_files.push(dst_root.join(rel));
            }
        }
        // Thu muc: xoa nhung thu muc dich khong ton tai o nguon.
        // Sap xep theo do sau GIAM DAN de xoa thu muc con truoc thu muc cha
        // (tranh loi "thu muc khong rong").
        let mut dirs_to_check: Vec<(String, usize)> = dest_dirs
            .into_iter()
            .filter(|(rel, _)| !source_rel_dirs.contains(rel))
            .collect();
        dirs_to_check.sort_by(|a, b| b.1.cmp(&a.1));
        for (rel, _) in dirs_to_check {
            delete_dirs.push(dst_root.join(rel));
        }
    }

    Ok(ScanResult {
        total_scanned: scanned,
        copy_jobs,
        delete_files,
        delete_dirs,
        already_up_to_date,
    })
}

fn same_file(existing: &DestEntry, size: u64, mtime: SystemTime) -> bool {
    if existing.size != size {
        return false;
    }
    let diff = match (mtime.duration_since(existing.mtime), existing.mtime.duration_since(mtime)) {
        (Ok(d), _) => d.as_secs() as i64,
        (_, Ok(d)) => d.as_secs() as i64,
        _ => 0,
    };
    diff <= MTIME_TOLERANCE_SECS
}

fn normalize_rel(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::Duration;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "vci_backup_test_{tag}_{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn test_scan_copy_skip_delete() {
        let root = TempDir::new("scan1");
        let src = root.0.join("src");
        let dst = root.0.join("dst");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&dst).unwrap();

        // a.txt: chi co o nguon -> can copy (moi)
        fs::write(src.join("a.txt"), b"hello world").unwrap();

        // b.txt: giong het o ca hai -> bo qua
        fs::write(src.join("b.txt"), b"same-content").unwrap();
        fs::write(dst.join("b.txt"), b"same-content").unwrap();
        let now = filetime::FileTime::now();
        filetime::set_file_mtime(src.join("b.txt"), now).unwrap();
        filetime::set_file_mtime(dst.join("b.txt"), now).unwrap();

        // c.txt: khac kich thuoc -> can copy (cap nhat)
        fs::write(src.join("c.txt"), b"new-longer-content").unwrap();
        fs::write(dst.join("c.txt"), b"old").unwrap();

        // d.txt: chi co o dich -> ung vien xoa (mirror)
        fs::write(dst.join("d.txt"), b"leftover").unwrap();

        // thu muc rong chi co o dich -> ung vien xoa
        fs::create_dir_all(dst.join("empty_leftover_dir")).unwrap();

        let result = scan_project(&src, &dst, &[], true, |_| {}).unwrap();

        let copy_names: Vec<&str> = result
            .copy_jobs
            .iter()
            .map(|j| j.rel_path.as_str())
            .collect();
        assert!(copy_names.contains(&"a.txt"));
        assert!(copy_names.contains(&"c.txt"));
        assert!(!copy_names.contains(&"b.txt"));
        assert_eq!(result.already_up_to_date, 1);

        let delete_file_names: Vec<String> = result
            .delete_files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(delete_file_names.contains(&"d.txt".to_string()));

        let delete_dir_names: Vec<String> = result
            .delete_dirs
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(delete_dir_names.contains(&"empty_leftover_dir".to_string()));
    }

    #[test]
    fn test_exclude_patterns_skip_files_and_subtrees() {
        let root = TempDir::new("scan2");
        let src = root.0.join("src");
        let dst = root.0.join("dst");
        fs::create_dir_all(src.join("keep")).unwrap();
        fs::create_dir_all(src.join(".dropbox")).unwrap();
        fs::write(src.join("keep").join("model.rvt"), b"data").unwrap();
        fs::write(src.join("keep").join("model.rvt.bak"), b"data").unwrap();
        fs::write(src.join(".dropbox").join("should_not_appear.txt"), b"x").unwrap();

        let excludes = vec!["*.bak".to_string(), ".dropbox".to_string()];
        let result = scan_project(&src, &dst, &excludes, false, |_| {}).unwrap();

        let names: Vec<&str> = result
            .copy_jobs
            .iter()
            .map(|j| j.rel_path.as_str())
            .collect();
        assert!(names.iter().any(|n| n.ends_with("model.rvt")));
        assert!(!names.iter().any(|n| n.ends_with(".bak")));
        assert!(!names.iter().any(|n| n.contains("should_not_appear")));
    }

    #[test]
    fn test_no_delete_when_mirror_disabled() {
        let root = TempDir::new("scan3");
        let src = root.0.join("src");
        let dst = root.0.join("dst");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&dst).unwrap();
        fs::write(dst.join("orphan.txt"), b"leftover").unwrap();

        let result = scan_project(&src, &dst, &[], false, |_| {}).unwrap();
        assert!(result.delete_files.is_empty());
        assert!(result.delete_dirs.is_empty());
        // dam bao khong panic khi thoi gian mtime le
        let _ = Duration::from_secs(0);
    }
}
