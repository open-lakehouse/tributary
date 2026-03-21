use std::collections::{HashMap, VecDeque};

use chrono::{DateTime, Utc};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
};
use sim_core::TimeMode;

use crate::tui::Frame;

const MAX_LOG_ENTRIES: usize = 500;

/// Color palette for different event type categories.
fn event_color(event_type: &str) -> Color {
    match event_type {
        "io.kitchen.order.created" => Color::Green,
        "io.kitchen.order.assigned" => Color::Yellow,
        "io.kitchen.order.completed" => Color::Cyan,
        "io.kitchen.customer.eating" => Color::Magenta,
        "io.kitchen.customer.done" => Color::Blue,
        _ => Color::White,
    }
}

/// Strip the common `io.kitchen.` prefix for compact display.
fn short_event_type(event_type: &str) -> &str {
    event_type.strip_prefix("io.kitchen.").unwrap_or(event_type)
}

pub struct LogEntry {
    pub step: usize,
    pub sim_time: DateTime<Utc>,
    pub event_type: String,
    pub count: u64,
}

pub struct App {
    pub current_step: usize,
    pub total_steps: usize,
    pub sim_time: Option<DateTime<Utc>>,
    pub log_entries: VecDeque<LogEntry>,
    pub cumulative_counts: HashMap<String, u64>,
    pub completed: bool,
    pub should_quit: bool,
    pub scroll_offset: u16,
    pub error: Option<String>,
    pub thread_panicked: bool,
    pub paused: bool,
    pub time_mode: String,
}

impl App {
    pub fn new(total_steps: usize, time_mode: String) -> Self {
        Self {
            current_step: 0,
            total_steps,
            sim_time: None,
            log_entries: VecDeque::with_capacity(MAX_LOG_ENTRIES),
            cumulative_counts: HashMap::new(),
            completed: false,
            should_quit: false,
            scroll_offset: 0,
            error: None,
            thread_panicked: false,
            paused: false,
            time_mode,
        }
    }

    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
    }

    #[allow(dead_code)]
    pub fn set_thread_panicked(&mut self) {
        self.thread_panicked = true;
    }

    pub fn push_step(
        &mut self,
        step: usize,
        sim_time: DateTime<Utc>,
        counts: HashMap<String, u64>,
    ) {
        self.current_step = step;
        self.sim_time = Some(sim_time);

        for (event_type, count) in &counts {
            *self
                .cumulative_counts
                .entry(event_type.clone())
                .or_default() += count;

            if self.log_entries.len() >= MAX_LOG_ENTRIES {
                self.log_entries.pop_front();
            }
            self.log_entries.push_back(LogEntry {
                step,
                sim_time,
                event_type: event_type.clone(),
                count: *count,
            });
        }
    }

    pub fn mark_completed(&mut self) {
        self.completed = true;
    }

    /// Cycle through Range → Live → Catchup → Range, return the `TimeMode` to send.
    pub fn cycle_mode(&mut self) -> TimeMode {
        let next = match self.time_mode.as_str() {
            "Range" => "Live",
            "Live" => "Catchup",
            _ => "Range",
        };
        self.time_mode = next.to_string();
        match next {
            "Live" => TimeMode::Live,
            "Catchup" => TimeMode::Catchup,
            _ => TimeMode::Range { end_time: None },
        }
    }

    pub fn render(&self, frame: &mut Frame<'_>) {
        let area = frame.area();

        if self.error.is_some() || self.thread_panicked {
            let layout = Layout::vertical([
                Constraint::Length(3),
                Constraint::Length(7),
                Constraint::Length(5),
                Constraint::Min(10),
                Constraint::Length(3),
            ])
            .split(area);

            self.render_progress(frame, layout[0]);
            self.render_error(frame, layout[1]);
            self.render_summary(frame, layout[2]);
            self.render_log(frame, layout[3]);
            self.render_status_bar(frame, layout[4]);
        } else {
            let layout = Layout::vertical([
                Constraint::Length(3),
                Constraint::Length(5),
                Constraint::Min(10),
                Constraint::Length(3),
            ])
            .split(area);

            self.render_progress(frame, layout[0]);
            self.render_summary(frame, layout[1]);
            self.render_log(frame, layout[2]);
            self.render_status_bar(frame, layout[3]);
        }
    }

    fn render_progress(&self, frame: &mut Frame<'_>, area: Rect) {
        let ratio = if self.total_steps > 0 {
            (self.current_step as f64 / self.total_steps as f64).min(1.0)
        } else {
            0.0
        };

        let time_label = self
            .sim_time
            .map(|t| t.format("%H:%M").to_string())
            .unwrap_or_default();

        let (label, color) = if self.error.is_some() || self.thread_panicked {
            (
                format!(
                    "FAILED at step {}/{}  [sim: {}]",
                    self.current_step, self.total_steps, time_label
                ),
                Color::Red,
            )
        } else if self.completed {
            (
                format!(
                    "Completed {}/{} steps  [sim: {}]",
                    self.current_step, self.total_steps, time_label
                ),
                Color::Green,
            )
        } else if self.paused {
            (
                format!(
                    "Step {}/{}  [sim: {}]  ■ PAUSED",
                    self.current_step, self.total_steps, time_label
                ),
                Color::Yellow,
            )
        } else {
            (
                format!(
                    "Step {}/{}  [sim: {}]",
                    self.current_step, self.total_steps, time_label
                ),
                Color::Cyan,
            )
        };

        let mode_badge = format!("[{}]", self.time_mode);
        let paused_badge = if self.paused { " [■ PAUSED]" } else { "" };
        let title = format!(" Simulation Progress {mode_badge}{paused_badge} ");

        let gauge = Gauge::default()
            .block(Block::default().title(title).borders(Borders::ALL))
            .gauge_style(Style::default().fg(color).add_modifier(Modifier::BOLD))
            .ratio(ratio)
            .label(label);

        frame.render_widget(gauge, area);
    }

    fn render_error(&self, frame: &mut Frame<'_>, area: Rect) {
        let error_text = if self.thread_panicked {
            "SIMULATION THREAD PANICKED\n\nThe simulation thread crashed unexpectedly. Check logs for details.".to_string()
        } else if let Some(err) = &self.error {
            format!("SIMULATION ERROR\n\n{}", err)
        } else {
            String::new()
        };

        let lines: Vec<Line<'_>> = error_text
            .lines()
            .map(|line| {
                if line.starts_with("SIMULATION") {
                    Line::from(Span::styled(
                        line,
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ))
                } else {
                    Line::from(Span::styled(line, Style::default().fg(Color::White)))
                }
            })
            .collect();

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" ⚠ Error Details ")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Red)),
            )
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);
    }

    fn render_summary(&self, frame: &mut Frame<'_>, area: Rect) {
        let mut entries: Vec<_> = self.cumulative_counts.iter().collect();
        entries.sort_by_key(|(k, _)| (*k).clone());

        let spans: Vec<Span<'_>> = entries
            .iter()
            .flat_map(|(event_type, count)| {
                let color = event_color(event_type);
                vec![
                    Span::styled(
                        format!(" {} ", short_event_type(event_type)),
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(format!("{count}  "), Style::default().fg(Color::DarkGray)),
                ]
            })
            .collect();

        let paragraph = Paragraph::new(Line::from(spans))
            .block(Block::default().title(" Totals ").borders(Borders::ALL))
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);
    }

    fn render_log(&self, frame: &mut Frame<'_>, area: Rect) {
        let inner_height = area.height.saturating_sub(2) as usize;

        let lines: Vec<Line<'_>> = self
            .log_entries
            .iter()
            .map(|entry| {
                let time_str = entry.sim_time.format("%H:%M:%S").to_string();
                Line::from(vec![
                    Span::styled(
                        format!("[{time_str}] "),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        format!("step {:>4} ", entry.step),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        format!("{:<28}", short_event_type(&entry.event_type)),
                        Style::default().fg(event_color(&entry.event_type)),
                    ),
                    Span::styled(
                        format!(" x{}", entry.count),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                ])
            })
            .collect();

        let total_lines = lines.len();
        let auto_scroll = total_lines.saturating_sub(inner_height) as u16;
        let scroll = if self.scroll_offset == 0 {
            auto_scroll
        } else {
            self.scroll_offset.min(auto_scroll)
        };

        let footer = if self.completed {
            " Event Log "
        } else {
            " Event Log "
        };

        let paragraph = Paragraph::new(lines)
            .block(Block::default().title(footer).borders(Borders::ALL))
            .scroll((scroll, 0));

        frame.render_widget(paragraph, area);
    }

    fn render_status_bar(&self, frame: &mut Frame<'_>, area: Rect) {
        let hints = if self.completed || self.error.is_some() || self.thread_panicked {
            " [q] Quit".to_string()
        } else if self.paused {
            " [q] Quit  [Space] Resume  [s] Step 1  [f] Step 10  [m] Mode".to_string()
        } else {
            " [q] Quit  [Space] Pause  [m] Mode".to_string()
        };

        let paragraph = Paragraph::new(hints)
            .block(Block::default().title(" Controls ").borders(Borders::ALL))
            .style(Style::default().fg(Color::DarkGray));

        frame.render_widget(paragraph, area);
    }
}
