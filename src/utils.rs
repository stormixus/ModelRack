use unicode_segmentation::UnicodeSegmentation;

/// Truncate a filename to `max_width` visual characters, inserting "..." in the middle.
/// CJK characters count as 2 units; ASCII as 1.
/// Preserves the file extension when possible.
pub fn truncate_filename(path: &str, max_width: usize) -> String {
    if visual_width(path) <= max_width {
        return path.to_string();
    }

    let (stem, ext) = split_ext(path);
    let ellipsis = "...";
    let ellipsis_width = 3;
    let ext_width = visual_width(ext);
    let available = max_width.saturating_sub(ellipsis_width + ext_width);

    if available < 4 {
        // Too narrow — just truncate the whole thing
        let mut result = String::new();
        let mut w = 0;
        for g in path.graphemes(true) {
            let cw = if is_cjk(g) { 2 } else { 1 };
            if w + cw + ellipsis_width > max_width {
                break;
            }
            result.push_str(g);
            w += cw;
        }
        result.push_str(ellipsis);
        return result;
    }

    // Take from start
    let mut start = String::new();
    let mut w = 0;
    for g in stem.graphemes(true) {
        let cw = if is_cjk(g) { 2 } else { 1 };
        if w + cw > available / 2 {
            break;
        }
        start.push_str(g);
        w += cw;
    }

    // Take from end
    let remaining = available - w;
    let mut end_parts: Vec<&str> = Vec::new();
    let mut ew = 0;
    for g in stem.graphemes(true).rev() {
        let cw = if is_cjk(g) { 2 } else { 1 };
        if ew + cw > remaining {
            break;
        }
        end_parts.push(g);
        ew += cw;
    }
    let end: String = end_parts.into_iter().rev().collect();

    format!("{}{}{}{}", start, ellipsis, end, ext)
}

fn visual_width(s: &str) -> usize {
    s.graphemes(true)
        .map(|g| if is_cjk(g) { 2 } else { 1 })
        .sum()
}

fn is_cjk(g: &str) -> bool {
    g.chars().any(|c| {
        ('\u{4E00}'..='\u{9FFF}').contains(&c)
            || ('\u{AC00}'..='\u{D7AF}').contains(&c)
            || ('\u{3040}'..='\u{309F}').contains(&c)
            || ('\u{30A0}'..='\u{30FF}').contains(&c)
            || ('\u{FF00}'..='\u{FFEF}').contains(&c)
    })
}

fn split_ext(path: &str) -> (&str, &str) {
    // Get just the filename from the full path
    let filename = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path);

    match filename.rfind('.') {
        Some(dot) if dot > 0 => (&filename[..dot], &filename[dot..]),
        _ => (filename, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_short_unchanged() {
        let result = truncate_filename("raspberry_pi_mount.stl", 40);
        assert_eq!(result, "raspberry_pi_mount.stl");
    }

    #[test]
    fn ascii_long_truncated() {
        let result = truncate_filename("very_long_filename_that_exceeds_limit.stl", 20);
        assert!(result.len() <= 20);
        assert!(result.contains("..."));
        assert!(result.ends_with(".stl"));
    }

    #[test]
    fn korean_within_limit() {
        let result = truncate_filename("라즈베리파이_마운트.stl", 40);
        assert!(visual_width(&result) <= 40);
    }

    #[test]
    fn korean_exceeds_limit() {
        let result = truncate_filename("매우_긴_한국어_파일_이름입니다_정말.stl", 20);
        assert!(visual_width(&result) <= 20);
        assert!(result.contains("..."));
        assert!(result.ends_with(".stl"));
    }

    #[test]
    fn mixed_cjk_ascii() {
        let result = truncate_filename("라즈베리Pi_5_마운트_v2.stl", 25);
        assert!(visual_width(&result) <= 25);
    }

    #[test]
    fn no_extension() {
        let result = truncate_filename("very_long_filename_without_extension", 15);
        assert!(visual_width(&result) <= 15);
        assert!(result.contains("..."));
    }
}
