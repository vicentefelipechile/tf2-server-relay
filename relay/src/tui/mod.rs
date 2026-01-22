//! TF2 Server Relay - Terminal User Interface
//!
//! Provides a real-time monitoring interface using ratatui.

mod panels;

use std::io::{self, Stdout};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};
use tokio::sync::mpsc;

use crate::config::Config;
use crate::connection::ServerInfo;
use crate::relay::{RecordedEvent, Relay, RelayStats};

// Panel components in panels.rs are reserved for future use

/// TUI application state.
pub struct App {
    /// Configuration.
    config: Arc<Config>,
    /// Relay instance.
    relay: Arc<Relay>,
    /// Whether the app should quit.
    should_quit: bool,
    /// Whether the event feed is paused.
    paused: bool,
    /// Currently focused panel.
    focused_panel: Panel,
    /// Event history for display.
    events: Vec<RecordedEvent>,
    /// Scroll offset for events panel.
    events_scroll: usize,
    /// Selected server (1-4, 0 for all).
    selected_server: u8,
    /// Show help overlay.
    show_help: bool,
    /// Show settings panel.
    show_settings: bool,
    /// Event receiver.
    event_rx: Option<mpsc::Receiver<RecordedEvent>>,
    /// Cached server list (updated each tick).
    cached_servers: Vec<ServerInfo>,
    /// Cached stats (updated each tick).
    cached_stats: RelayStats,
    /// Start time for uptime calculation.
    start_time: Instant,
}

/// Panel identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Servers,
    Events,
    Stats,
}

impl App {
    /// Create a new TUI application.
    pub fn new(config: Arc<Config>, relay: Arc<Relay>) -> Self {
        Self {
            config,
            relay,
            should_quit: false,
            paused: false,
            focused_panel: Panel::Events,
            events: Vec::new(),
            events_scroll: 0,
            selected_server: 0,
            show_help: false,
            show_settings: false,
            event_rx: None,
            cached_servers: Vec::new(),
            cached_stats: RelayStats::default(),
            start_time: Instant::now(),
        }
    }

    /// Run the TUI application.
    pub async fn run(&mut self) -> io::Result<()> {
        // Take event receiver from relay
        self.event_rx = self.relay.take_event_receiver().await;

        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Run main loop
        let result = self.run_loop(&mut terminal).await;

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    /// Main event loop.
    async fn run_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> io::Result<()> {
        let tick_rate = Duration::from_millis(self.config.tui.refresh_rate_ms);

        loop {
            // Update cached data from relay (async-safe)
            self.cached_servers = self.relay.get_servers().await;
            self.cached_stats = self.relay.get_stats().await;

            // Check for new relay events
            if let Some(ref mut rx) = self.event_rx {
                while let Ok(event) = rx.try_recv() {
                    if !self.paused {
                        self.events.push(event);
                    }
                }

                // Trim events if needed
                let max_events = self.config.tui.max_event_history;
                if self.events.len() > max_events {
                    let drain_count = self.events.len() - max_events;
                    self.events.drain(0..drain_count);
                }
            }

            // Draw UI (sync)
            terminal.draw(|f| self.draw(f))?;

            // Poll for keyboard events with timeout
            if crossterm::event::poll(tick_rate)? {
                if let Event::Key(key) = crossterm::event::read()? {
                    // Only handle key press, not release
                    if key.kind == KeyEventKind::Press {
                        self.handle_key(key.code, key.modifiers);
                    }
                }
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    /// Handle keyboard input.
    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        // Handle help overlay
        if self.show_help {
            self.show_help = false;
            return;
        }

        // Handle settings panel
        if self.show_settings {
            match code {
                KeyCode::Esc | KeyCode::F(2) => self.show_settings = false,
                _ => {}
            }
            return;
        }

        match code {
            // Quit
            KeyCode::Char('q') | KeyCode::Char('Q') => self.should_quit = true,
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true
            }
            KeyCode::Esc => self.should_quit = true,

            // Pause/unpause
            KeyCode::Char('p') | KeyCode::Char('P') => self.paused = !self.paused,

            // Clear events
            KeyCode::Char('c') | KeyCode::Char('C') => {
                self.events.clear();
                self.events_scroll = 0;
            }

            // Panel navigation
            KeyCode::Tab => {
                self.focused_panel = match self.focused_panel {
                    Panel::Servers => Panel::Events,
                    Panel::Events => Panel::Stats,
                    Panel::Stats => Panel::Servers,
                };
            }

            // Server selection
            KeyCode::Char('1') => self.selected_server = 1,
            KeyCode::Char('2') => self.selected_server = 2,
            KeyCode::Char('3') => self.selected_server = 3,
            KeyCode::Char('4') => self.selected_server = 4,
            KeyCode::Char('0') | KeyCode::Char('a') | KeyCode::Char('A') => {
                self.selected_server = 0
            } // All servers

            // Scroll
            KeyCode::Up => {
                if self.events_scroll < self.events.len().saturating_sub(1) {
                    self.events_scroll += 1;
                }
            }
            KeyCode::Down => {
                if self.events_scroll > 0 {
                    self.events_scroll -= 1;
                }
            }
            KeyCode::PageUp => {
                self.events_scroll = self
                    .events_scroll
                    .saturating_add(10)
                    .min(self.events.len().saturating_sub(1));
            }
            KeyCode::PageDown => {
                self.events_scroll = self.events_scroll.saturating_sub(10);
            }
            KeyCode::Home => {
                self.events_scroll = self.events.len().saturating_sub(1);
            }
            KeyCode::End => {
                self.events_scroll = 0;
            }

            // Help
            KeyCode::Char('?') | KeyCode::F(1) => self.show_help = true,

            // Settings
            KeyCode::F(2) => self.show_settings = true,

            _ => {}
        }
    }

    /// Draw the UI.
    fn draw(&self, f: &mut Frame) {
        let size = f.area();

        // Create main layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(10),   // Main content
                Constraint::Length(1), // Footer
            ])
            .split(size);

        // Draw header
        self.draw_header(f, chunks[0]);

        // Draw main content
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25), // Servers
                Constraint::Percentage(50), // Events
                Constraint::Percentage(25), // Stats
            ])
            .split(chunks[1]);

        self.draw_servers_panel(f, main_chunks[0]);
        self.draw_events_panel(f, main_chunks[1]);
        self.draw_stats_panel(f, main_chunks[2]);

        // Draw footer
        self.draw_footer(f, chunks[2]);

        // Draw overlays
        if self.show_help {
            self.draw_help_overlay(f, size);
        }

        if self.show_settings {
            self.draw_settings_panel(f, size);
        }
    }

    /// Draw the header panel.
    fn draw_header(&self, f: &mut Frame, area: Rect) {
        let uptime = self.start_time.elapsed().as_secs();
        let hours = uptime / 3600;
        let minutes = (uptime % 3600) / 60;
        let seconds = uptime % 60;

        let now = chrono::Local::now().format("%H:%M:%S").to_string();

        let title = format!(
            " TF2 Server Relay v1.0.0 │ Uptime: {:02}:{:02}:{:02} │ {} ",
            hours, minutes, seconds, now
        );

        let status = if self.paused { " ⏸ PAUSED " } else { "" };

        let header = Paragraph::new(Line::from(vec![
            Span::styled(
                title,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                status,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );

        f.render_widget(header, area);
    }

    /// Draw the footer panel.
    fn draw_footer(&self, f: &mut Frame, area: Rect) {
        let keybindings =
            " Q/Esc Quit │ P Pause │ C Clear │ 1-4/0 Server │ ↑↓ Scroll │ ? Help │ F2 Settings ";

        let footer = Paragraph::new(Span::styled(
            keybindings,
            Style::default().fg(Color::DarkGray),
        ));

        f.render_widget(footer, area);
    }

    /// Draw the servers panel.
    fn draw_servers_panel(&self, f: &mut Frame, area: Rect) {
        let servers = &self.cached_servers;

        let mut lines: Vec<Line> = Vec::new();

        if servers.is_empty() {
            lines.push(Line::from(Span::styled(
                "No servers connected",
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Waiting for connections",
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(Span::styled(
                "on port 27050...",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            for server in servers {
                let status_color = Color::Green;
                let selected = self.selected_server == server.server_id;

                let prefix = if selected { "▶ " } else { "  " };

                lines.push(Line::from(vec![
                    Span::styled(prefix, Style::default().fg(Color::Cyan)),
                    Span::styled(
                        format!("Server {} ", server.server_id),
                        Style::default()
                            .fg(if selected { Color::Cyan } else { Color::White })
                            .add_modifier(if selected {
                                Modifier::BOLD
                            } else {
                                Modifier::empty()
                            }),
                    ),
                    Span::styled("●", Style::default().fg(status_color)),
                ]));

                lines.push(Line::from(Span::styled(
                    format!("  {}", server.server_name),
                    Style::default().fg(Color::Gray),
                )));

                lines.push(Line::from(Span::styled(
                    format!("  {}", server.map_name),
                    Style::default().fg(Color::DarkGray),
                )));

                lines.push(Line::from(Span::styled(
                    format!(
                        "  Players: {}/{}",
                        server.current_players, server.max_players
                    ),
                    Style::default().fg(Color::DarkGray),
                )));

                lines.push(Line::from(""));
            }
        }

        let border_color = if self.focused_panel == Panel::Servers {
            Color::Cyan
        } else {
            Color::Gray
        };

        let panel = Paragraph::new(lines).block(
            Block::default()
                .title(" Servers ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        );

        f.render_widget(panel, area);
    }

    /// Draw the events panel.
    fn draw_events_panel(&self, f: &mut Frame, area: Rect) {
        let inner_height = area.height.saturating_sub(2) as usize;

        // Filter events by selected server
        let filtered_events: Vec<&RecordedEvent> = self
            .events
            .iter()
            .filter(|e| self.selected_server == 0 || e.server_id == self.selected_server)
            .collect();

        // Calculate visible range
        let total_events = filtered_events.len();
        let start_idx = if total_events > inner_height {
            total_events
                - inner_height
                - self
                    .events_scroll
                    .min(total_events.saturating_sub(inner_height))
        } else {
            0
        };
        let end_idx = (start_idx + inner_height).min(total_events);

        let mut lines: Vec<Line> = Vec::new();

        for event in &filtered_events[start_idx..end_idx] {
            let timestamp = if self.config.tui.show_timestamps {
                let elapsed = event.timestamp.elapsed().as_secs();
                if elapsed < 60 {
                    format!("{:2}s ", elapsed)
                } else {
                    format!("{:2}m ", elapsed / 60)
                }
            } else {
                String::new()
            };

            let server_color = match event.server_id {
                1 => Color::Red,
                2 => Color::Blue,
                3 => Color::Green,
                4 => Color::Yellow,
                _ => Color::Gray,
            };

            let description = event.event.description();

            lines.push(Line::from(vec![
                Span::styled(timestamp, Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("[S{}] ", event.server_id),
                    Style::default().fg(server_color),
                ),
                Span::styled(description, Style::default().fg(Color::White)),
            ]));
        }

        if lines.is_empty() {
            lines.push(Line::from(Span::styled(
                "No events yet",
                Style::default().fg(Color::DarkGray),
            )));
        }

        let border_color = if self.focused_panel == Panel::Events {
            Color::Cyan
        } else {
            Color::Gray
        };

        let title = if self.selected_server > 0 {
            format!(" Events (Server {}) ", self.selected_server)
        } else {
            " Events (All) ".to_string()
        };

        let panel = Paragraph::new(lines).block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        );

        f.render_widget(panel, area);
    }

    /// Draw the stats panel.
    fn draw_stats_panel(&self, f: &mut Frame, area: Rect) {
        let stats = &self.cached_stats;
        let servers = &self.cached_servers;

        let mut lines = vec![
            Line::from(vec![
                Span::styled("Connected: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("{}", servers.len()),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Events: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("{}", stats.total_events),
                    Style::default().fg(Color::Green),
                ),
            ]),
            Line::from(vec![
                Span::styled("Events/sec: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("{:.1}", stats.events_per_second),
                    Style::default().fg(Color::Green),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Bytes TX: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format_bytes(stats.bytes_transferred),
                    Style::default().fg(Color::Yellow),
                ),
            ]),
        ];

        // Add player count
        let total_players: u8 = servers.iter().map(|s| s.current_players).sum();
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Players: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}", total_players),
                Style::default().fg(Color::Magenta),
            ),
        ]));

        let border_color = if self.focused_panel == Panel::Stats {
            Color::Cyan
        } else {
            Color::Gray
        };

        let panel = Paragraph::new(lines).block(
            Block::default()
                .title(" Stats ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        );

        f.render_widget(panel, area);
    }

    /// Draw help overlay.
    fn draw_help_overlay(&self, f: &mut Frame, area: Rect) {
        let help_text = vec![
            Line::from(Span::styled(
                "Keyboard Shortcuts",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("  Q/Esc      Quit"),
            Line::from("  P          Pause/Resume event feed"),
            Line::from("  C          Clear event history"),
            Line::from("  Tab        Cycle focus between panels"),
            Line::from("  1-4        Filter by server 1-4"),
            Line::from("  0/A        Show all servers"),
            Line::from("  ↑/↓        Scroll events"),
            Line::from("  PgUp/PgDn  Scroll events (fast)"),
            Line::from("  Home/End   Jump to start/end"),
            Line::from("  F2         Open settings"),
            Line::from("  ?/F1       Show this help"),
            Line::from(""),
            Line::from(Span::styled(
                "Press any key to close",
                Style::default().fg(Color::DarkGray),
            )),
        ];

        let overlay_width = 44;
        let overlay_height = help_text.len() as u16 + 2;

        let overlay_area = Rect {
            x: area.width.saturating_sub(overlay_width) / 2,
            y: area.height.saturating_sub(overlay_height) / 2,
            width: overlay_width.min(area.width),
            height: overlay_height.min(area.height),
        };

        let help = Paragraph::new(help_text).block(
            Block::default()
                .title(" Help ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );

        // Clear area behind overlay
        f.render_widget(ratatui::widgets::Clear, overlay_area);
        f.render_widget(help, overlay_area);
    }

    /// Draw settings panel.
    fn draw_settings_panel(&self, f: &mut Frame, area: Rect) {
        let settings_text = vec![
            Line::from(Span::styled(
                "Settings (Read-Only)",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(format!(
                "  Bind Address: {}",
                self.config.server.bind_address
            )),
            Line::from(format!("  Max Servers: {}", self.config.server.max_servers)),
            Line::from(format!(
                "  Heartbeat: {}ms",
                self.config.server.heartbeat_interval_ms
            )),
            Line::from(""),
            Line::from(format!(
                "  Broadcast Chat: {}",
                if self.config.relay.broadcast_chat {
                    "Yes"
                } else {
                    "No"
                }
            )),
            Line::from(format!(
                "  Broadcast Deaths: {}",
                if self.config.relay.broadcast_deaths {
                    "Yes"
                } else {
                    "No"
                }
            )),
            Line::from(format!(
                "  Cross Healing: {}",
                if self.config.relay.enable_cross_healing {
                    "Yes"
                } else {
                    "No"
                }
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Press Esc or F2 to close",
                Style::default().fg(Color::DarkGray),
            )),
        ];

        let overlay_width = 50;
        let overlay_height = settings_text.len() as u16 + 2;

        let overlay_area = Rect {
            x: area.width.saturating_sub(overlay_width) / 2,
            y: area.height.saturating_sub(overlay_height) / 2,
            width: overlay_width.min(area.width),
            height: overlay_height.min(area.height),
        };

        let settings = Paragraph::new(settings_text).block(
            Block::default()
                .title(" Settings ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        );

        f.render_widget(ratatui::widgets::Clear, overlay_area);
        f.render_widget(settings, overlay_area);
    }
}

/// Format bytes to human-readable string.
fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
