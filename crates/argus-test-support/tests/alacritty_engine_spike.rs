use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi;

struct SpikeSize {
    columns: usize,
    screen_lines: usize,
}

impl Dimensions for SpikeSize {
    fn total_lines(&self) -> usize {
        self.screen_lines
    }

    fn screen_lines(&self) -> usize {
        self.screen_lines
    }

    fn columns(&self) -> usize {
        self.columns
    }
}

#[test]
fn alacritty_terminal_accepts_chunked_pty_bytes() {
    let size = SpikeSize {
        columns: 80,
        screen_lines: 24,
    };
    let mut terminal = Term::new(Config::default(), &size, VoidListener);
    let mut parser = ansi::Processor::<ansi::StdSyncHandler>::new();

    parser.advance(&mut terminal, b"one\r\n");
    parser.advance(&mut terminal, b"\x1b[31mtwo");
    parser.advance(&mut terminal, b"\x1b[0m\r\nthree");

    assert_eq!(terminal.screen_lines(), 24);
    assert_eq!(terminal.columns(), 80);
    assert_eq!(terminal.grid().display_offset(), 0);
}
