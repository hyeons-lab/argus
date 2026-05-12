use std::sync::Arc;

use wezterm_term::color::ColorPalette;
use wezterm_term::{Terminal, TerminalConfiguration, TerminalSize};

#[derive(Debug)]
struct SpikeConfig;

impl TerminalConfiguration for SpikeConfig {
    fn color_palette(&self) -> ColorPalette {
        ColorPalette::default()
    }
}

#[test]
fn wezterm_term_accepts_chunked_pty_bytes() {
    let mut terminal = Terminal::new(
        TerminalSize {
            rows: 24,
            cols: 80,
            pixel_width: 640,
            pixel_height: 384,
            dpi: 96,
        },
        Arc::new(SpikeConfig),
        "Argus",
        env!("CARGO_PKG_VERSION"),
        Box::new(Vec::<u8>::new()),
    );

    let initial_seqno = terminal.current_seqno();

    terminal.advance_bytes(b"one\r\n");
    terminal.advance_bytes(b"\x1b[31mtwo");
    terminal.advance_bytes(b"\x1b[0m\r\nthree");

    assert_eq!(terminal.get_size().rows, 24);
    assert_eq!(terminal.get_size().cols, 80);
    assert_ne!(terminal.current_seqno(), initial_seqno);
    assert!(terminal.screen().scrollback_rows() >= 24);
}
