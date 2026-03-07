use std::collections::{HashMap, VecDeque};

use chrono::{DateTime, Utc};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
};

use crate::tui::Frame;

const MAX_LOG_ENTRIES: usize = 500;

/// Color palette for different event type categories.
fn event_color(event_type: &str) -> Color {
    match event_type {
        t if t.contains("order_created") => Color::Green,
        t if t.contains("order_ready") => Color::Yellow,
        t if t.contains("order_picked_up") => Color::Cyan,
        t if t.contains("order_delivered") => Color::Magenta,
        t if t.contains("order_line") => Color::Blue,
        t if t.contains("check_in") => Color::LightGreen,
        t if t.contains("check_out") => Color::LightRed,
        _ => Color::White,
    }
}

/// Strip the common `caspers.universe.` prefix for compact display.
fn short_event_type(event_type: &str) -> &str {
    event_type
        .strip_prefix("caspers.universe.")
        .unwrap_or(event_type)
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
}

impl App {
    pub fn new(total_steps: usize) -> Self {
        Self {
            current_step: 0,
            total_steps,
            sim_time: None,
            log_entries: VecDeque::with_capacity(MAX_LOG_ENTRIES),
            cumulative_counts: HashMap::new(),
            completed: false,
            should_quit: false,
            scroll_offset: 0,
        }
    }

    pub fn push_step(&mut self, step: usize, sim_time: DateTime<Utc>, counts: HashMap<String, u64>) {
        self.current_step = step;
        self.sim_time = Some(sim_time);

        for (event_type, count) in &counts {
            *self.cumulative_counts.entry(event_type.clone()).or_default() += count;

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

    pub fn render(&self, frame: &mut Frame<'_>) {
        let area = frame.area();
        let layout = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(5),
            Constraint::Min(10),
        ])
        .split(area);

        self.render_progress(frame, layout[0]);
        self.render_summary(frame, layout[1]);
        self.render_log(frame, layout[2]);
    }

    fn render_progress(&self, frame: &mut Frame<'_>, area: Rect) {
        let ratio = if self.total_steps > 0 {
            self.current_step as f64 / self.total_steps as f64
        } else {
            0.0
        };

        let time_label = self
            .sim_time
            .map(|t| t.format("%H:%M").to_string())
            .unwrap_or_default();

        let label = if self.completed {
            format!("Completed {}/{} steps  [sim: {}]", self.current_step, self.total_steps, time_label)
        } else {
            format!("Step {}/{}  [sim: {}]", self.current_step, self.total_steps, time_label)
        };

        let gauge = Gauge::default()
            .block(
                Block::default()
                    .title(" Simulation Progress ")
                    .borders(Borders::ALL),
            )
            .gauge_style(
                Style::default()
                    .fg(if self.completed { Color::Green } else { Color::Cyan })
                    .add_modifier(Modifier::BOLD),
            )
            .ratio(ratio)
            .label(label);

        frame.render_widget(gauge, area);
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
            .block(
                Block::default()
                    .title(" Totals ")
                    .borders(Borders::ALL),
            )
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
            " Event Log [q to quit] "
        } else {
            " Event Log [q to abort] "
        };

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .title(footer)
                    .borders(Borders::ALL),
            )
            .scroll((scroll, 0));

        frame.render_widget(paragraph, area);
    }
}
