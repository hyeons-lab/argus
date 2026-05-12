//! Snapshot helpers for renderer-style golden tests.

/// Build a deterministic multi-line frame for golden snapshot assertions.
pub fn frame<'line>(lines: impl IntoIterator<Item = &'line str>) -> String {
    normalize_text(&lines.into_iter().collect::<Vec<_>>().join("\n"))
}

/// Normalize platform-specific and terminal-originated newlines.
pub fn normalize_text(input: &str) -> String {
    input
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim_end_matches('\n')
        .to_string()
}
