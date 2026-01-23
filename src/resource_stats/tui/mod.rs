//! Terminal UI module using ratatui

use std::io;
use std::sync::Arc;
use std::time::Duration;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Row, Table},
    Frame, Terminal,
};

use crate::resource_stats::UiState;

/// TUI Application state
pub struct App {
    #[allow(dead_code)]
    ui_state: Arc<UiState>,
    selected_tab: usize,
    should_quit: bool,
}

impl App {
    pub fn new(ui_state: Arc<UiState>) -> Self {
        Self {
            ui_state,
            selected_tab: 0,
            should_quit: false,
        }
    }

    /// Run the TUI application
    pub async fn run(&mut self) -> io::Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Main loop
        loop {
            terminal.draw(|f| self.ui(f))?;

            // Handle input with timeout
            if event::poll(Duration::from_millis(250))? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') => self.should_quit = true,
                        KeyCode::Tab => self.selected_tab = (self.selected_tab + 1) % 3,
                        KeyCode::Left => {
                            self.selected_tab = self.selected_tab.saturating_sub(1);
                        }
                        KeyCode::Right => {
                            self.selected_tab = (self.selected_tab + 1).min(2);
                        }
                        _ => {}
                    }
                }
            }

            if self.should_quit {
                break;
            }
        }

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        Ok(())
    }

    /// Render the UI
    fn ui(&self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Length(3), // Tabs
                Constraint::Min(10),   // Content
                Constraint::Length(3), // Footer
            ])
            .split(f.area());

        // Title
        let title = Paragraph::new("Resource Stats Operator")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(title, chunks[0]);

        // Tabs
        let tabs = vec!["Overview", "Nodes", "Costs"];
        let tab_titles: Vec<Line> = tabs
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let style = if i == self.selected_tab {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                };
                Line::from(Span::styled(format!(" {} ", t), style))
            })
            .collect();
        let tabs_widget = Paragraph::new(tab_titles.into_iter().map(|l| l).collect::<Vec<_>>())
            .block(Block::default().borders(Borders::ALL).title("Tabs"));
        f.render_widget(tabs_widget, chunks[1]);

        // Content based on selected tab
        match self.selected_tab {
            0 => self.render_overview(f, chunks[2]),
            1 => self.render_nodes(f, chunks[2]),
            2 => self.render_costs(f, chunks[2]),
            _ => {}
        }

        // Footer
        let footer = Paragraph::new("Press 'q' to quit, Tab/Arrows to navigate")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(footer, chunks[3]);
    }

    /// Render overview tab
    fn render_overview(&self, f: &mut Frame, area: Rect) {
        let _block = Block::default().borders(Borders::ALL).title("Cluster Overview");

        // Create a simple overview with gauges
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        // CPU gauge (placeholder)
        let cpu_gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL).title("CPU Usage"))
            .gauge_style(Style::default().fg(Color::Blue))
            .percent(0)
            .label("0%");
        f.render_widget(cpu_gauge, chunks[0]);

        // Memory gauge (placeholder)
        let mem_gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL).title("Memory Usage"))
            .gauge_style(Style::default().fg(Color::Green))
            .percent(0)
            .label("0%");
        f.render_widget(mem_gauge, chunks[1]);
    }

    /// Render nodes tab
    fn render_nodes(&self, f: &mut Frame, area: Rect) {
        let header = Row::new(vec!["Node", "CPU", "Memory", "Cost/hr"])
            .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

        // Placeholder rows
        let rows: Vec<Row> = vec![Row::new(vec!["No data", "-", "-", "-"])];

        let table = Table::new(rows, [
            Constraint::Percentage(40),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
        ])
        .header(header)
        .block(Block::default().borders(Borders::ALL).title("Nodes"));

        f.render_widget(table, area);
    }

    /// Render costs tab
    fn render_costs(&self, f: &mut Frame, area: Rect) {
        let cost_info = vec![
            Line::from(Span::styled("Cost Summary", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
            Line::from(""),
            Line::from(vec![
                Span::raw("CPU Cost/hr:     "),
                Span::styled("$0.00", Style::default().fg(Color::Green)),
            ]),
            Line::from(vec![
                Span::raw("Memory Cost/hr:  "),
                Span::styled("$0.00", Style::default().fg(Color::Green)),
            ]),
            Line::from(vec![
                Span::raw("GPU Cost/hr:     "),
                Span::styled("$0.00", Style::default().fg(Color::Green)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::raw("Total/hr:        "),
                Span::styled("$0.00", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::raw("Projected/mo:    "),
                Span::styled("$0.00", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
        ];

        let costs = Paragraph::new(cost_info)
            .block(Block::default().borders(Borders::ALL).title("Cost Breakdown"));
        f.render_widget(costs, area);
    }
}
