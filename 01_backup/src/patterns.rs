// ============================================================================
// patterns.rs - So khop ten file/thu muc voi mau dang wildcard don gian (*)
// ============================================================================
// Ho tro cu phap kieu shell glob co ban: '*' khop 0 hoac nhieu ky tu bat ky.
// Vi du: "*.bak", "~$*", "Thumbs.db". Khong phan biet hoa/thuong.

pub fn matches_any(name: &str, patterns: &[String]) -> bool {
    let name_lower = name.to_lowercase();
    patterns
        .iter()
        .any(|p| matches_pattern(&name_lower, &p.to_lowercase()))
}

fn matches_pattern(name: &str, pattern: &str) -> bool {
    // Thuat toan so khop wildcard hai con tro chuan (khong can de quy, O(n*m) toi da)
    let name_b = name.as_bytes();
    let pat_b = pattern.as_bytes();

    let (mut ni, mut pi) = (0usize, 0usize);
    let (mut star_idx, mut match_idx) = (None::<usize>, 0usize);

    while ni < name_b.len() {
        if pi < pat_b.len() && (pat_b[pi] == b'*') {
            star_idx = Some(pi);
            match_idx = ni;
            pi += 1;
        } else if pi < pat_b.len() && pat_b[pi] == name_b[ni] {
            ni += 1;
            pi += 1;
        } else if let Some(si) = star_idx {
            pi = si + 1;
            match_idx += 1;
            ni = match_idx;
        } else {
            return false;
        }
    }
    while pi < pat_b.len() && pat_b[pi] == b'*' {
        pi += 1;
    }
    pi == pat_b.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        assert!(matches_any("file.bak", &["*.bak".to_string()]));
        assert!(matches_any("~$revit.rvt", &["~$*".to_string()]));
        assert!(!matches_any("model.rvt", &["*.bak".to_string(), "~$*".to_string()]));
        assert!(matches_any("Thumbs.db", &["Thumbs.db".to_string()]));
        assert!(matches_any("THUMBS.DB", &["thumbs.db".to_string()]));
    }
}
