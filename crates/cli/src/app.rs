use std::collections::{HashMap, VecDeque};

use chrono::{DateTime, Utc};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Wrap},
};
use sim_core::{ControlCommand, TimeMode};
use uuid::Uuid;

use crate::tui::Frame;

const MAX_LOG_ENTRIES: usize = 500;

// ─── Screens ──────────────────────────────────────────────────────────────────

/// Top-level screen the TUI is currently showing.
pub enum Screen {
    /// Pre-sim launch screen: list existing simulations or create a new one.
    Launch,
    /// Main simulation view (current single-screen implementation).
    Simulation,
    /// Snapshot browser for a running simulation.
    SnapshotBrowser,
}

// ─── Simulation list entry (CLI-side) ────────────────────────────────────────

/// Summary of one simulation shown on the Launch screen.
pub struct SimListEntry {
    pub id: Uuid,
    pub label: Option<String>,
    pub created_at: DateTime<Utc>,
    pub snapshot_count: usize,
}

// ─── Snapshot summary (CLI-side) ─────────────────────────────────────────────

/// Snapshot row shown in the SnapshotBrowser screen.
pub struct SnapshotSummary {
    pub id: Uuid,
    pub label: Option<String>,
    pub sim_time: DateTime<Utc>,
    #[allow(dead_code)]
    pub created_at: DateTime<Utc>,
    pub step: usize,
    pub agent_count: usize,
}

/// Color palette for different event type categories.
fn event_color(event_type: &str) -> Color {
    match event_type {
        "io.kitchen.order.created" => Color::Green,
        "io.kitchen.order.assigned" => Color::Yellow,
        "io.kitchen.order.completed" => Color::Cyan,
        "io.kitchen.customer.eating" => Color::Magenta,
        "io.kitchen.customer.done" => Color::Blue,
        _ => {
            // Hash-based color for any domain's event types
            let hash = event_type
                .bytes()
                .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
            let palette = [
                Color::Red,
                Color::LightGreen,
                Color::LightBlue,
                Color::LightYellow,
                Color::LightMagenta,
                Color::LightCyan,
                Color::LightRed,
                Color::White,
            ];
            palette[(hash as usize) % palette.len()]
        }
    }
}

/// Strip the `io.<domain>.` prefix for compact display.
pub fn short_event_type(event_type: &str) -> &str {
    let s = event_type.strip_prefix("io.").unwrap_or(event_type);
    if let Some(dot) = s.find('.') {
        &s[dot + 1..]
    } else {
        s
    }
}

/// Modal dialog state
pub enum Dialog {
    None,
    ModeSelect,
    RangeInput {
        input: String,
        error: Option<String>,
    },
    AdvanceBy {
        input: String,
        error: Option<String>,
    },
    /// `n` on Launch screen — collect a name for a new simulation
    NewSimName {
        input: String,
    },
    /// `t` in SnapshotBrowser — collect a label for a new snapshot
    TakeSnapshot {
        input: String,
    },
    /// `Enter` on a snapshot row — confirm before restoring
    ConfirmRestore {
        snapshot_id: Uuid,
        label: Option<String>,
    },
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
    /// Time at the very first tick — used as range-progress baseline.
    pub sim_start_time: Option<DateTime<Utc>>,
    pub log_entries: VecDeque<LogEntry>,
    pub cumulative_counts: HashMap<String, u64>,
    pub completed: bool,
    pub should_quit: bool,
    pub scroll_offset: u16,
    pub error: Option<String>,
    pub thread_panicked: bool,
    pub paused: bool,
    pub time_mode: String,
    pub dialog: Dialog,
    pub live_speed: f32,
    pub range_end_time: Option<DateTime<Utc>>,
    pub time_step_secs: i64,
    // ─── Multi-screen state ───────────────────────────────────────────────────
    pub screen: Screen,
    pub sim_list: Vec<SimListEntry>,
    pub sim_list_cursor: usize,
    pub snapshot_list: Vec<SnapshotSummary>,
    pub snapshot_list_cursor: usize,
    pub active_simulation_id: Option<Uuid>,
    /// Transient notification shown as overlay; auto-clears after 3 seconds.
    pub notification: Option<(String, std::time::Instant)>,
    /// Rolling window of per-step event totals (last 10) for throughput calculation.
    recent_step_totals: VecDeque<u64>,
}

impl App {
    #[allow(dead_code)]
    pub fn new(
        total_steps: usize,
        time_mode: String,
        range_end_time: Option<DateTime<Utc>>,
        time_step_secs: i64,
    ) -> Self {
        Self {
            current_step: 0,
            total_steps,
            sim_time: None,
            sim_start_time: None,
            log_entries: VecDeque::with_capacity(MAX_LOG_ENTRIES),
            cumulative_counts: HashMap::new(),
            completed: false,
            should_quit: false,
            scroll_offset: 0,
            error: None,
            thread_panicked: false,
            paused: false,
            time_mode,
            dialog: Dialog::None,
            live_speed: 1.0,
            range_end_time,
            time_step_secs,
            screen: Screen::Simulation,
            sim_list: Vec::new(),
            sim_list_cursor: 0,
            snapshot_list: Vec::new(),
            snapshot_list_cursor: 0,
            active_simulation_id: None,
            notification: None,
            recent_step_totals: VecDeque::with_capacity(10),
        }
    }

    /// Create an App starting on the Launch screen (pre-sim simulation selector).
    pub fn new_launch(sim_list: Vec<SimListEntry>) -> Self {
        Self {
            current_step: 0,
            total_steps: 0,
            sim_time: None,
            sim_start_time: None,
            log_entries: VecDeque::with_capacity(MAX_LOG_ENTRIES),
            cumulative_counts: HashMap::new(),
            completed: false,
            should_quit: false,
            scroll_offset: 0,
            error: None,
            thread_panicked: false,
            paused: false,
            time_mode: "Range".to_string(),
            dialog: Dialog::None,
            live_speed: 1.0,
            range_end_time: None,
            time_step_secs: 60,
            screen: Screen::Launch,
            sim_list,
            sim_list_cursor: 0,
            snapshot_list: Vec::new(),
            snapshot_list_cursor: 0,
            active_simulation_id: None,
            notification: None,
            recent_step_totals: VecDeque::with_capacity(10),
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
        if self.sim_start_time.is_none() {
            self.sim_start_time = Some(sim_time);
        }

        let mut step_total = 0u64;
        for (event_type, count) in &counts {
            *self
                .cumulative_counts
                .entry(event_type.clone())
                .or_default() += count;
            step_total += count;

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

        if self.recent_step_totals.len() >= 10 {
            self.recent_step_totals.pop_front();
        }
        self.recent_step_totals.push_back(step_total);
    }

    /// Rolling average events per step over the last 10 steps.
    pub fn throughput(&self) -> f64 {
        if self.recent_step_totals.is_empty() {
            return 0.0;
        }
        self.recent_step_totals.iter().sum::<u64>() as f64 / self.recent_step_totals.len() as f64
    }

    pub fn mark_completed(&mut self) {
        self.completed = true;
    }

    /// Handle keystrokes when a dialog is open.
    ///
    /// Returns `true` if the caller should skip global key dispatch.
    pub fn handle_dialog_key(
        &mut self,
        key: crossterm::event::KeyCode,
        controller: &sim_core::SimulationController,
    ) -> crate::error::Result<()> {
        use crossterm::event::KeyCode;
        match &mut self.dialog {
            Dialog::None => {}

            Dialog::ModeSelect => match key {
                KeyCode::Char('1') => {
                    self.dialog = Dialog::RangeInput {
                        input: String::new(),
                        error: None,
                    };
                }
                KeyCode::Char('2') => {
                    let _ = controller.send(ControlCommand::SetMode(TimeMode::Live));
                    self.time_mode = "Live".to_string();
                    self.dialog = Dialog::None;
                }
                KeyCode::Char('3') => {
                    let _ = controller.send(ControlCommand::SetMode(TimeMode::Catchup));
                    self.time_mode = "Catchup".to_string();
                    self.dialog = Dialog::None;
                }
                KeyCode::Esc => {
                    self.dialog = Dialog::None;
                }
                _ => {}
            },

            Dialog::RangeInput { input, error } => match key {
                KeyCode::Char(c) => {
                    input.push(c);
                    *error = None;
                }
                KeyCode::Backspace => {
                    input.pop();
                    *error = None;
                }
                KeyCode::Enter => {
                    let raw = input.clone();
                    match self.parse_range_input(&raw) {
                        Ok(end_time) => {
                            let _ = controller.send(ControlCommand::SetMode(TimeMode::Range {
                                end_time: Some(end_time),
                            }));
                            self.range_end_time = Some(end_time);
                            self.time_mode = "Range".to_string();
                            self.dialog = Dialog::None;
                        }
                        Err(e) => {
                            if let Dialog::RangeInput { error, .. } = &mut self.dialog {
                                *error = Some(e);
                            }
                        }
                    }
                }
                KeyCode::Esc => {
                    self.dialog = Dialog::None;
                }
                _ => {}
            },

            Dialog::AdvanceBy { input, error } => match key {
                KeyCode::Char(c) => {
                    input.push(c);
                    *error = None;
                }
                KeyCode::Backspace => {
                    input.pop();
                    *error = None;
                }
                KeyCode::Enter => {
                    let raw = input.clone();
                    let base = self
                        .range_end_time
                        .or(self.sim_time)
                        .unwrap_or_else(Utc::now);
                    match self.parse_advance_input(&raw, base) {
                        Ok(new_end) => {
                            let _ = controller.send(ControlCommand::SetMode(TimeMode::Range {
                                end_time: Some(new_end),
                            }));
                            // Resume if paused
                            if self.paused {
                                let _ = controller.send(ControlCommand::Resume);
                                self.paused = false;
                            }
                            self.range_end_time = Some(new_end);
                            self.time_mode = "Range".to_string();
                            self.dialog = Dialog::None;
                        }
                        Err(e) => {
                            if let Dialog::AdvanceBy { error, .. } = &mut self.dialog {
                                *error = Some(e);
                            }
                        }
                    }
                }
                KeyCode::Esc => {
                    self.dialog = Dialog::None;
                }
                _ => {}
            },

            // NewSimName: handled by Launch-screen logic in run.rs via a callback;
            // here we just update the input buffer and dismiss on Esc.
            Dialog::NewSimName { input } => match key {
                KeyCode::Char(c) => {
                    input.push(c);
                }
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Esc => {
                    self.dialog = Dialog::None;
                }
                _ => {}
            },

            Dialog::TakeSnapshot { input } => match key {
                KeyCode::Char(c) => {
                    input.push(c);
                }
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Enter => {
                    let label = if let Dialog::TakeSnapshot { input } = &self.dialog {
                        input.clone()
                    } else {
                        String::new()
                    };
                    let _ = controller.send(ControlCommand::Snapshot(label));
                    self.dialog = Dialog::None;
                }
                KeyCode::Esc => {
                    self.dialog = Dialog::None;
                }
                _ => {}
            },

            Dialog::ConfirmRestore {
                snapshot_id,
                label: _,
            } => match key {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    let id = *snapshot_id;
                    let _ = controller.send(ControlCommand::RestoreSnapshot(id));
                    self.dialog = Dialog::None;
                }
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.dialog = Dialog::None;
                }
                _ => {}
            },
        }
        Ok(())
    }

    /// Parse a range input: `parse_sim_time()` formats OR `+N steps`.
    fn parse_range_input(&self, s: &str) -> Result<DateTime<Utc>, String> {
        let s = s.trim();
        if let Some(rest) = s.strip_prefix('+')
            && let Some(steps_str) = rest
                .strip_suffix(" steps")
                .or_else(|| rest.strip_suffix(" step"))
        {
            let n: i64 = steps_str
                .trim()
                .parse()
                .map_err(|_| format!("invalid step count: '{}'", steps_str))?;
            let base = self.sim_time.unwrap_or_else(Utc::now);
            return Ok(base + chrono::Duration::seconds(n * self.time_step_secs));
        }
        crate::time_parse::parse_sim_time(s)
    }

    /// Parse an advance-by input: relative offset or step count.
    fn parse_advance_input(&self, s: &str, base: DateTime<Utc>) -> Result<DateTime<Utc>, String> {
        let s = s.trim();
        // "+N steps"
        if let Some(rest) = s.strip_prefix('+') {
            if let Some(steps_str) = rest
                .strip_suffix(" steps")
                .or_else(|| rest.strip_suffix(" step"))
            {
                let n: i64 = steps_str
                    .trim()
                    .parse()
                    .map_err(|_| format!("invalid step count: '{}'", steps_str))?;
                return Ok(base + chrono::Duration::seconds(n * self.time_step_secs));
            }
            // "+1h" style
            let dur = humantime::parse_duration(rest)
                .map_err(|e| format!("invalid duration '{}': {}", rest, e))?;
            let nanos = dur.as_nanos() as i64;
            return Ok(base + chrono::Duration::nanoseconds(nanos));
        }
        Err(format!(
            "advance-by input must be a relative offset like +1h, +30m, or +100 steps; got '{}'",
            s
        ))
    }

    pub fn render(&mut self, frame: &mut Frame<'_>) {
        // Auto-clear expired notifications
        if let Some((_, instant)) = &self.notification
            && instant.elapsed() > std::time::Duration::from_secs(3)
        {
            self.notification = None;
        }

        match self.screen {
            Screen::Launch => self.render_launch(frame),
            Screen::Simulation => self.render_simulation(frame),
            Screen::SnapshotBrowser => self.render_snapshot_browser(frame),
        }

        // Notification overlay (shown on top of any screen)
        if self.notification.is_some() {
            let area = frame.area();
            self.render_notification(frame, area);
        }
    }

    fn render_simulation(&self, frame: &mut Frame<'_>) {
        let area = frame.area();

        // Determine layout
        let has_error = self.error.is_some() || self.thread_panicked;

        let constraints: Vec<Constraint> = if has_error {
            vec![
                Constraint::Length(1), // header
                Constraint::Length(3), // gauge
                Constraint::Length(5), // error
                Constraint::Min(10),   // body
                Constraint::Length(1), // help bar
            ]
        } else {
            vec![
                Constraint::Length(1), // header
                Constraint::Length(3), // gauge
                Constraint::Min(10),   // body
                Constraint::Length(1), // help bar
            ]
        };

        let layout = Layout::vertical(constraints).split(area);

        if has_error {
            self.render_header(frame, layout[0]);
            self.render_gauge(frame, layout[1]);
            self.render_error(frame, layout[2]);
            self.render_body(frame, layout[3]);
            self.render_help_bar(frame, layout[4]);
        } else {
            self.render_header(frame, layout[0]);
            self.render_gauge(frame, layout[1]);
            self.render_body(frame, layout[2]);
            self.render_help_bar(frame, layout[3]);
        }

        // Render modal dialog on top if active
        if !matches!(self.dialog, Dialog::None) {
            self.render_dialog(frame, area);
        }
    }

    fn render_launch(&self, frame: &mut Frame<'_>) {
        let area = frame.area();
        let layout = Layout::vertical([
            Constraint::Length(1), // header
            Constraint::Min(0),    // list
            Constraint::Length(1), // help bar
        ])
        .split(area);

        // Header
        let header = Line::from(vec![
            Span::styled(
                " Chef Casper's Kitchen Sim",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  — Select or create a simulation",
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        frame.render_widget(Paragraph::new(header), layout[0]);

        // Simulation list
        if self.sim_list.is_empty() {
            let msg = Paragraph::new("  No simulations yet. Press [n] to create one.")
                .style(Style::default().fg(Color::DarkGray))
                .block(Block::default().borders(Borders::ALL));
            frame.render_widget(msg, layout[1]);
        } else {
            let items: Vec<ListItem<'_>> = self
                .sim_list
                .iter()
                .enumerate()
                .map(|(i, entry)| {
                    let label = entry.label.as_deref().unwrap_or("<unnamed>");
                    let date = entry.created_at.format("%d %b %Y %H:%M").to_string();
                    let n = entry.snapshot_count;
                    let snap_str = if n == 1 {
                        "1 snapshot".to_string()
                    } else {
                        format!("{n} snapshots")
                    };
                    let line = Line::from(vec![
                        Span::styled(
                            format!("  {label:<30}"),
                            Style::default()
                                .fg(if i == self.sim_list_cursor {
                                    Color::Cyan
                                } else {
                                    Color::White
                                })
                                .add_modifier(if i == self.sim_list_cursor {
                                    Modifier::BOLD
                                } else {
                                    Modifier::empty()
                                }),
                        ),
                        Span::styled(
                            format!("created {date}  {snap_str}"),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]);
                    if i == self.sim_list_cursor {
                        ListItem::new(line).style(Style::default().bg(Color::DarkGray))
                    } else {
                        ListItem::new(line)
                    }
                })
                .collect();

            let list = List::new(items).block(
                Block::default()
                    .title(" Simulations ")
                    .borders(Borders::ALL),
            );
            frame.render_widget(list, layout[1]);
        }

        // Help bar
        let help = Paragraph::new(" [n] New  [Enter] Load  [↑/↓] Navigate  [q] Quit")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(help, layout[2]);

        // Dialog overlay
        if !matches!(self.dialog, Dialog::None) {
            self.render_dialog(frame, area);
        }
    }

    fn render_snapshot_browser(&self, frame: &mut Frame<'_>) {
        let area = frame.area();
        let layout = Layout::vertical([
            Constraint::Length(1), // header
            Constraint::Min(0),    // list
            Constraint::Length(1), // help bar
        ])
        .split(area);

        // Header
        let sim_label = self
            .active_simulation_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let header = Line::from(vec![
            Span::styled(
                " Snapshots",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  — {sim_label}"),
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        frame.render_widget(Paragraph::new(header), layout[0]);

        // Snapshot list
        if self.snapshot_list.is_empty() {
            let msg = Paragraph::new("  No snapshots yet. Press [t] to take one.")
                .style(Style::default().fg(Color::DarkGray))
                .block(Block::default().borders(Borders::ALL));
            frame.render_widget(msg, layout[1]);
        } else {
            let items: Vec<ListItem<'_>> = self
                .snapshot_list
                .iter()
                .enumerate()
                .map(|(i, snap)| {
                    let label = snap.label.as_deref().unwrap_or("<no label>");
                    let sim_time = snap.sim_time.format("%H:%M:%S").to_string();
                    let line = Line::from(vec![
                        Span::styled(
                            format!("  {label:<30}"),
                            Style::default().fg(if i == self.snapshot_list_cursor {
                                Color::Cyan
                            } else {
                                Color::White
                            }),
                        ),
                        Span::styled(
                            format!(
                                "sim {}  {} agents  step {}",
                                sim_time, snap.agent_count, snap.step
                            ),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]);
                    if i == self.snapshot_list_cursor {
                        ListItem::new(line).style(Style::default().bg(Color::DarkGray))
                    } else {
                        ListItem::new(line)
                    }
                })
                .collect();

            let list =
                List::new(items).block(Block::default().title(" Snapshots ").borders(Borders::ALL));
            frame.render_widget(list, layout[1]);
        }

        // Help bar
        let help =
            Paragraph::new(" [t] Take snapshot  [Enter] Restore  [↑/↓] Navigate  [Esc] Back")
                .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(help, layout[2]);

        // Dialog overlay
        if !matches!(self.dialog, Dialog::None) {
            self.render_dialog(frame, area);
        }
    }

    fn render_notification(&self, frame: &mut Frame<'_>, area: Rect) {
        let Some((msg, _)) = &self.notification else {
            return;
        };
        let width = (msg.len() as u16 + 4).min(area.width.saturating_sub(2));
        let x = area.x + area.width.saturating_sub(width + 1);
        let popup = Rect {
            x,
            y: area.y + 1,
            width,
            height: 3,
        };
        frame.render_widget(Clear, popup);
        let para = Paragraph::new(msg.as_str())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Green)),
            )
            .alignment(Alignment::Center);
        frame.render_widget(para, popup);
    }

    fn render_header(&self, frame: &mut Frame<'_>, area: Rect) {
        let (mode_label, mode_color) = match self.time_mode.as_str() {
            "Live" => ("LIVE", Color::Green),
            "Catchup" => ("CATCHUP", Color::Yellow),
            _ => ("RANGE", Color::Blue),
        };

        let time_str = self
            .sim_time
            .map(|t| t.format("%Y-%m-%d %H:%M:%S UTC").to_string())
            .unwrap_or_else(|| "waiting…".to_string());

        let status = if self.paused {
            " [PAUSED]"
        } else if self.completed {
            " [DONE]"
        } else {
            ""
        };

        let line = Line::from(vec![
            Span::styled(
                " Chef Casper's Kitchen Sim  ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("[{}]", mode_label),
                Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {}  step {}{}", time_str, self.current_step, status),
                Style::default().fg(Color::DarkGray),
            ),
        ]);

        frame.render_widget(Paragraph::new(line), area);
    }

    fn render_gauge(&self, frame: &mut Frame<'_>, area: Rect) {
        // Error state — always show a red gauge at whatever progress we reached.
        if self.error.is_some() || self.thread_panicked {
            let ratio = if self.total_steps > 0 {
                (self.current_step as f64 / self.total_steps as f64).min(1.0)
            } else {
                0.0
            };
            let label = format!("FAILED at step {}", self.current_step);
            let gauge = Gauge::default()
                .block(Block::default().borders(Borders::ALL))
                .gauge_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                .ratio(ratio)
                .label(label);
            frame.render_widget(gauge, area);
            return;
        }

        let is_live = self.time_mode == "Live" || self.time_mode == "Catchup";

        if is_live {
            // Live / Catchup: show the sim clock as a friendly marquee rather than a bar.
            self.render_live_clock(frame, area);
        } else {
            // Range mode: use time-based progress if we have start+end, fall back to steps.
            self.render_range_gauge(frame, area);
        }
    }

    /// Range-mode progress gauge — time-based when start+end are known, step-based otherwise.
    fn render_range_gauge(&self, frame: &mut Frame<'_>, area: Rect) {
        let (ratio, label, color) = match (self.sim_start_time, self.range_end_time, self.sim_time)
        {
            // Time-based: we know where we started and where we're going.
            (Some(start), Some(end), Some(now)) => {
                let total_secs = (end - start).num_seconds().max(1) as f64;
                let elapsed_secs = (now - start).num_seconds().max(0) as f64;
                let ratio = (elapsed_secs / total_secs).min(1.0);

                let fmt_time = |t: DateTime<Utc>| t.format("%d %b %H:%M").to_string();
                let status = if self.completed {
                    "  DONE"
                } else if self.paused {
                    "  PAUSED"
                } else {
                    ""
                };
                let label = format!(
                    "{} → {}  (step {}{})",
                    fmt_time(start),
                    fmt_time(end),
                    self.current_step,
                    status,
                );
                let color = if self.completed {
                    Color::Green
                } else if self.paused {
                    Color::Yellow
                } else {
                    Color::Cyan
                };
                (ratio, label, color)
            }
            // Step-based fallback: no end time set yet.
            _ => {
                let ratio = if self.total_steps > 0 {
                    (self.current_step as f64 / self.total_steps as f64).min(1.0)
                } else {
                    0.0
                };
                let status = if self.completed {
                    "  DONE"
                } else if self.paused {
                    "  PAUSED"
                } else {
                    ""
                };
                let label = format!(
                    "Step {} / {}{}",
                    self.current_step, self.total_steps, status
                );
                let color = if self.completed {
                    Color::Green
                } else if self.paused {
                    Color::Yellow
                } else {
                    Color::Cyan
                };
                (ratio, label, color)
            }
        };

        let gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL))
            .gauge_style(Style::default().fg(color).add_modifier(Modifier::BOLD))
            .ratio(ratio)
            .label(label);

        frame.render_widget(gauge, area);
    }

    /// Live/Catchup mode: a clock panel instead of a progress bar.
    fn render_live_clock(&self, frame: &mut Frame<'_>, area: Rect) {
        let sim_time = self.sim_time.unwrap_or_else(Utc::now);

        // Friendly date + time
        let date_str = sim_time.format("%a %d %b %Y").to_string();
        let time_str = sim_time.format("%H:%M:%S").to_string();

        // Elapsed since sim start
        let elapsed_str = if let Some(start) = self.sim_start_time {
            let secs = (sim_time - start).num_seconds().max(0);
            if secs < 3600 {
                format!("{}m {:02}s elapsed", secs / 60, secs % 60)
            } else {
                format!("{}h {:02}m elapsed", secs / 3600, (secs % 3600) / 60)
            }
        } else {
            String::new()
        };

        let speed_str = if self.live_speed != 1.0 {
            format!("  ×{:.3}", self.live_speed)
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string()
                + "x"
        } else {
            String::new()
        };

        let catchup_note = if self.time_mode == "Catchup" {
            "  catching up…"
        } else {
            ""
        };

        let line = Line::from(vec![
            Span::styled(
                format!("  {date_str}  "),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                time_str,
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    "  {elapsed_str}{speed_str}{catchup_note}  step {}",
                    self.current_step
                ),
                Style::default().fg(Color::DarkGray),
            ),
        ]);

        let paragraph = Paragraph::new(line)
            .block(Block::default().borders(Borders::ALL))
            .alignment(Alignment::Left);

        frame.render_widget(paragraph, area);
    }

    fn render_error(&self, frame: &mut Frame<'_>, area: Rect) {
        let error_text = if self.thread_panicked {
            "SIMULATION THREAD PANICKED\n\nThe simulation thread crashed unexpectedly.".to_string()
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
                    .title(" Error ")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Red)),
            )
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);
    }

    fn render_body(&self, frame: &mut Frame<'_>, area: Rect) {
        let [log_area, stats_area] =
            Layout::horizontal([Constraint::Percentage(65), Constraint::Percentage(35)])
                .areas::<2>(area);
        self.render_log(frame, log_area);
        self.render_stats_panel(frame, stats_area);
    }

    fn render_stats_panel(&self, frame: &mut Frame<'_>, area: Rect) {
        let throughput = self.throughput();
        let total_events: u64 = self.cumulative_counts.values().sum();

        // Compute sanity ratio: look for completed/created pairs in any domain
        let ratio_line: Option<String> = {
            let created = self
                .cumulative_counts
                .iter()
                .filter(|(k, _)| k.ends_with(".created"))
                .map(|(_, v)| v)
                .sum::<u64>();
            let completed = self
                .cumulative_counts
                .iter()
                .filter(|(k, _)| k.ends_with(".completed") || k.ends_with(".delivered"))
                .map(|(_, v)| v)
                .sum::<u64>();
            if created > 0 {
                Some(format!(
                    "{completed}/{created} = {:.1}%",
                    completed as f64 / created as f64 * 100.0
                ))
            } else {
                None
            }
        };

        let mut lines: Vec<Line<'_>> = vec![
            Line::from(vec![
                Span::styled("Step  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{} / {}", self.current_step, self.total_steps),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("Time  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    self.sim_time
                        .map(|t| t.format("%m-%d %H:%M").to_string())
                        .unwrap_or_else(|| "—".to_string()),
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                Span::styled("Tput  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{throughput:.1} ev/step"),
                    Style::default().fg(Color::Cyan),
                ),
            ]),
            Line::from(vec![
                Span::styled("Total ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{total_events} events"),
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "── By Type ──────────────",
                Style::default().fg(Color::DarkGray),
            )),
        ];

        let mut entries: Vec<_> = self.cumulative_counts.iter().collect();
        entries.sort_by(|a, b| b.1.cmp(a.1));
        let max_count = entries.first().map(|(_, v)| **v).unwrap_or(1).max(1);

        for (event_type, count) in &entries {
            let color = event_color(event_type);
            let bar_len = ((**count as f64 / max_count as f64) * 8.0).round() as usize;
            let bar: String = "█".repeat(bar_len);
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{:<16}", short_event_type(event_type)),
                    Style::default().fg(color),
                ),
                Span::styled(
                    format!("{:>5} {}", count, bar),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }

        if let Some(ratio) = ratio_line {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "── Completion ───────────",
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(Span::styled(
                ratio,
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )));
        }

        let paragraph =
            Paragraph::new(lines).block(Block::default().title(" Stats ").borders(Borders::ALL));
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

        let paragraph = Paragraph::new(lines)
            .block(Block::default().title(" Event Log ").borders(Borders::ALL))
            .scroll((scroll, 0));

        frame.render_widget(paragraph, area);
    }

    fn render_help_bar(&self, frame: &mut Frame<'_>, area: Rect) {
        let is_live = self.time_mode == "Live";
        let is_range = self.time_mode == "Range";

        let hints = if self.completed || self.error.is_some() || self.thread_panicked {
            " [q] Quit".to_string()
        } else if self.paused {
            if is_range {
                " [q] Quit  [Space] Resume  [s] +1  [f] +10  [m] Mode  [a] Advance  [S] Snapshots"
                    .to_string()
            } else {
                " [q] Quit  [Space] Resume  [s] +1  [f] +10  [m] Mode  [S] Snapshots".to_string()
            }
        } else {
            let base = " [q] Quit  [Space] Pause  [m] Mode  [S] Snapshots".to_string();
            if is_live {
                format!("{}  [+/-] Speed {:.3}x", base, self.live_speed)
            } else if is_range {
                format!("{}  [a] Advance", base)
            } else {
                base
            }
        };

        let paragraph = Paragraph::new(hints)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Left);

        frame.render_widget(paragraph, area);
    }

    fn render_dialog(&self, frame: &mut Frame<'_>, area: Rect) {
        match &self.dialog {
            Dialog::None => {}

            Dialog::ModeSelect => {
                let current = &self.time_mode;
                let popup = centered_rect(60, 7, area);
                frame.render_widget(Clear, popup);

                let lines = vec![
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(
                            "  [1] ",
                            Style::default()
                                .fg(Color::Blue)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw("Range   "),
                        Span::styled(
                            "[2] ",
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw("Live   "),
                        Span::styled(
                            "[3] ",
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw("Catchup"),
                    ]),
                    Line::from(""),
                    Line::from(Span::styled(
                        format!("  current: {}", current),
                        Style::default().fg(Color::DarkGray),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  [Esc] Cancel",
                        Style::default().fg(Color::DarkGray),
                    )),
                ];

                let block = Block::default()
                    .title(" Select Time Mode ")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::White));

                frame.render_widget(Paragraph::new(lines).block(block), popup);
            }

            Dialog::RangeInput { input, error } => {
                let popup = centered_rect(70, 8, area);
                frame.render_widget(Clear, popup);

                let mut lines = vec![
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("  End time: ", Style::default().fg(Color::White)),
                        Span::styled(
                            format!("{}_", input),
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Examples: now+1h  now+30m  2026-06-01  +100 steps",
                        Style::default().fg(Color::DarkGray),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  [Enter] Confirm  [Esc] Cancel",
                        Style::default().fg(Color::DarkGray),
                    )),
                ];

                if let Some(err) = error {
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        format!("  Error: {}", err),
                        Style::default().fg(Color::Red),
                    )));
                }

                let block = Block::default()
                    .title(" Set Range End Time ")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::White));

                frame.render_widget(Paragraph::new(lines).block(block), popup);
            }

            Dialog::AdvanceBy { input, error } => {
                let popup = centered_rect(70, 8, area);
                frame.render_widget(Clear, popup);

                let mut lines = vec![
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("  Duration: ", Style::default().fg(Color::White)),
                        Span::styled(
                            format!("{}_", input),
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Examples: +1h  +30m  +100 steps",
                        Style::default().fg(Color::DarkGray),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  [Enter] Confirm  [Esc] Cancel",
                        Style::default().fg(Color::DarkGray),
                    )),
                ];

                if let Some(err) = error {
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        format!("  Error: {}", err),
                        Style::default().fg(Color::Red),
                    )));
                }

                let block = Block::default()
                    .title(" Advance Simulation By ")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::White));

                frame.render_widget(Paragraph::new(lines).block(block), popup);
            }

            Dialog::NewSimName { input } => {
                let popup = centered_rect(60, 7, area);
                frame.render_widget(Clear, popup);

                let lines = vec![
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("  Name: ", Style::default().fg(Color::White)),
                        Span::styled(
                            format!("{input}_"),
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  [Enter] Create  [Esc] Cancel",
                        Style::default().fg(Color::DarkGray),
                    )),
                ];

                let block = Block::default()
                    .title(" New Simulation ")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::White));

                frame.render_widget(Paragraph::new(lines).block(block), popup);
            }

            Dialog::TakeSnapshot { input } => {
                let popup = centered_rect(60, 7, area);
                frame.render_widget(Clear, popup);

                let lines = vec![
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("  Label: ", Style::default().fg(Color::White)),
                        Span::styled(
                            format!("{input}_"),
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  [Enter] Take snapshot  [Esc] Cancel",
                        Style::default().fg(Color::DarkGray),
                    )),
                ];

                let block = Block::default()
                    .title(" Take Snapshot ")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::White));

                frame.render_widget(Paragraph::new(lines).block(block), popup);
            }

            Dialog::ConfirmRestore { label, .. } => {
                let popup = centered_rect(60, 7, area);
                frame.render_widget(Clear, popup);

                let label_str = label.as_deref().unwrap_or("<no label>");
                let lines = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        format!("  Restore snapshot: {label_str}"),
                        Style::default().fg(Color::White),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Simulation will pause at the restored state.",
                        Style::default().fg(Color::DarkGray),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  [y] Confirm  [n / Esc] Cancel",
                        Style::default().fg(Color::DarkGray),
                    )),
                ];

                let block = Block::default()
                    .title(" Restore Snapshot? ")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Yellow));

                frame.render_widget(Paragraph::new(lines).block(block), popup);
            }
        }
    }
}

/// Compute a centered rect with a given percentage width and fixed height.
fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let popup_width = r.width * percent_x / 100;
    let popup_x = r.x + (r.width.saturating_sub(popup_width)) / 2;
    let popup_y = r.y + r.height.saturating_sub(height) / 2;
    Rect {
        x: popup_x,
        y: popup_y,
        width: popup_width,
        height: height.min(r.height),
    }
}
