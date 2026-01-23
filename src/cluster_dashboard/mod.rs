//! Cluster Dashboard - Local TUI for cluster management
//!
//! A terminal-based dashboard that shows:
//! - Cluster information and health
//! - Dependencies and their status
//! - Applications and their state
//! - Security vulnerabilities and best practices
//! - FinOps cost analysis
//! - Port forwards for local development

pub mod app_definition;
pub mod views;
pub mod data;
pub mod ui;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

pub use app_definition::AppDefinition;
pub use data::DashboardData;
pub use views::View;

/// Dashboard application state
pub struct Dashboard {
    /// Current view
    pub current_view: View,
    /// Dashboard data
    pub data: Arc<RwLock<DashboardData>>,
    /// Should quit
    pub should_quit: bool,
    /// Status message
    pub status_message: Option<String>,
    /// Search query
    pub search_query: String,
    /// Is searching
    pub is_searching: bool,
    /// Selected index in current view
    pub selected_index: usize,
    /// Scroll offset
    pub scroll_offset: usize,
}

impl Dashboard {
    /// Create new dashboard
    pub async fn new() -> anyhow::Result<Self> {
        let data = DashboardData::load().await?;

        Ok(Self {
            current_view: View::Overview,
            data: Arc::new(RwLock::new(data)),
            should_quit: false,
            status_message: None,
            search_query: String::new(),
            is_searching: false,
            selected_index: 0,
            scroll_offset: 0,
        })
    }

    /// Run the dashboard
    pub async fn run(&mut self) -> anyhow::Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Start background data refresh
        let data = self.data.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                if let Ok(new_data) = DashboardData::load().await {
                    let mut guard = data.write().await;
                    *guard = new_data;
                }
            }
        });

        // Main loop
        let result = self.main_loop(&mut terminal).await;

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

    async fn main_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> anyhow::Result<()> {
        loop {
            // Draw UI
            let data = self.data.read().await;
            terminal.draw(|f| {
                ui::render(f, self, &data);
            })?;
            drop(data);

            // Handle events
            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if self.is_searching {
                        match key.code {
                            KeyCode::Esc => {
                                self.is_searching = false;
                                self.search_query.clear();
                            }
                            KeyCode::Enter => {
                                self.is_searching = false;
                            }
                            KeyCode::Backspace => {
                                self.search_query.pop();
                            }
                            KeyCode::Char(c) => {
                                self.search_query.push(c);
                            }
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Char('q') => {
                                self.should_quit = true;
                            }
                            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.should_quit = true;
                            }
                            KeyCode::Char('/') => {
                                self.is_searching = true;
                            }
                            KeyCode::Char('?') => {
                                self.current_view = View::Help;
                            }
                            // Navigation
                            KeyCode::Char('1') => {
                                self.current_view = View::Overview;
                                self.selected_index = 0;
                            }
                            KeyCode::Char('2') => {
                                self.current_view = View::Nodes;
                                self.selected_index = 0;
                            }
                            KeyCode::Char('3') => {
                                self.current_view = View::Applications;
                                self.selected_index = 0;
                            }
                            KeyCode::Char('4') => {
                                self.current_view = View::Dependencies;
                                self.selected_index = 0;
                            }
                            KeyCode::Char('5') => {
                                self.current_view = View::Security;
                                self.selected_index = 0;
                            }
                            KeyCode::Char('6') => {
                                self.current_view = View::FinOps;
                                self.selected_index = 0;
                            }
                            KeyCode::Char('7') => {
                                self.current_view = View::PortForwards;
                                self.selected_index = 0;
                            }
                            KeyCode::Char('8') => {
                                self.current_view = View::Providers;
                                self.selected_index = 0;
                            }
                            // List navigation
                            KeyCode::Up | KeyCode::Char('k') => {
                                if self.selected_index > 0 {
                                    self.selected_index -= 1;
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                self.selected_index += 1;
                            }
                            KeyCode::PageUp => {
                                self.selected_index = self.selected_index.saturating_sub(10);
                            }
                            KeyCode::PageDown => {
                                self.selected_index += 10;
                            }
                            KeyCode::Home => {
                                self.selected_index = 0;
                            }
                            // Actions
                            KeyCode::Char('r') => {
                                self.status_message = Some("Refreshing...".to_string());
                                if let Ok(new_data) = DashboardData::load().await {
                                    let mut guard = self.data.write().await;
                                    *guard = new_data;
                                    self.status_message = Some("Refreshed!".to_string());
                                }
                            }
                            KeyCode::Enter => {
                                // Action on selected item
                                self.handle_enter().await;
                            }
                            KeyCode::Esc => {
                                self.current_view = View::Overview;
                            }
                            _ => {}
                        }
                    }
                }
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    async fn handle_enter(&mut self) {
        match self.current_view {
            View::PortForwards => {
                // Start/stop port forward
                self.status_message = Some("Port forward toggled".to_string());
            }
            View::Security => {
                // Show vulnerability details
                self.current_view = View::VulnerabilityDetail;
            }
            View::Applications => {
                // Show app details
                self.current_view = View::AppDetail;
            }
            View::Providers => {
                // Show provider details
                self.current_view = View::ProviderDetail;
            }
            View::ProviderDetail => {
                // Return to providers list
                self.current_view = View::Providers;
            }
            _ => {}
        }
    }
}

