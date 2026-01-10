pub fn clamp_chars(s: String, max: usize) -> String {
    assert!(max > 3, "max must be larger than 3");

    match s.char_indices().nth(max) {
        Some((i, _)) => format!("{}...", &s[..i - 3]),
        None => s,
    }
}
