use std::io::{self, Stdout};

use crossterm::{
    event::EventStream,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures::StreamExt;
use ratatui::{Terminal, prelude::CrosstermBackend};

pub type Frame<'a> = ratatui::Frame<'a>;

pub struct Tui {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    events: EventStream,
}

impl Tui {
    pub fn new() -> io::Result<Self> {
        let terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
        let events = EventStream::new();
        Ok(Self { terminal, events })
    }

    pub fn enter(&mut self) -> io::Result<()> {
        enable_raw_mode()?;
        crossterm::execute!(io::stdout(), EnterAlternateScreen)?;
        self.terminal.clear()?;
        Ok(())
    }

    pub fn exit(&mut self) -> io::Result<()> {
        disable_raw_mode()?;
        crossterm::execute!(io::stdout(), LeaveAlternateScreen)?;
        Ok(())
    }

    pub fn draw(&mut self, f: impl FnOnce(&mut Frame<'_>)) -> io::Result<()> {
        self.terminal.draw(f)?;
        Ok(())
    }

    /// Return the next terminal event without any internal filtering loop.
    ///
    /// By returning every event individually we avoid starving the other
    /// branches of a `tokio::select!` when crossterm produces a burst of
    /// non-key events (resize, focus, mouse).
    pub async fn next_event(&mut self) -> Option<crossterm::event::Event> {
        self.events.next().await?.ok()
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        let _ = self.exit();
    }
}
