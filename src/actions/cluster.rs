use crate::commands::RunCommand;
use crate::context::OutputFormat;
use crate::traits::CommandContext;
use clap::Subcommand;
use k8s_openapi::api::core::v1::Node;
use kube::config::{KubeConfigOptions, Kubeconfig};
use kube::{Api, Client, Config};
use serde::Serialize;
use std::time::Duration;

#[cfg(feature = "tui")]
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
#[cfg(feature = "tui")]
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};

#[derive(Debug, Clone, Serialize)]
pub struct ContextCheck {
    pub context: String,
    pub nodes: Vec<String>,
    pub healthy: bool,
    pub error: Option<String>,
}

#[derive(Subcommand)]
pub enum ClusterAction {
    /// Check all kubectl contexts and their node status
    Check {
        /// Timeout in seconds per context
        #[arg(short, long, default_value = "10")]
        timeout: u64,

        /// Only show healthy clusters
        #[arg(long)]
        healthy_only: bool,

        /// Only show unhClusterActionealthy clusters
        #[arg(long)]
        unhealthy_only: bool,
    },
    /// Set the current kubectl context
    Set {
        /// Context name to switch to
        context: String,
    },
    /// List all available contexts
    List,
    /// Show current context
    Current,
    /// Delete a kubectl context
    Delete {
        /// Context name to delete
        context: String,

        /// Force delete without confirmation
        #[arg(short, long)]
        force: bool,
    },
    /// Interactive TUI for browsing and switching contexts
    #[cfg(feature = "tui")]
    Tui,
    /// Slim picker TUI for use inside a Zellij pane
    #[cfg(feature = "tui")]
    Picker,
}

impl ClusterAction {
    async fn check_context(context_name: &str, timeout_secs: u64) -> ContextCheck {
        let kubeconfig = match Kubeconfig::read() {
            Ok(kc) => kc,
            Err(e) => {
                return ContextCheck {
                    context: context_name.to_string(),
                    nodes: vec![],
                    healthy: false,
                    error: Some(format!("Failed to read kubeconfig: {}", e)),
                };
            }
        };

        let options = KubeConfigOptions {
            context: Some(context_name.to_string()),
            ..Default::default()
        };

        let config = match Config::from_custom_kubeconfig(kubeconfig, &options).await {
            Ok(mut c) => {
                c.connect_timeout = Some(Duration::from_secs(timeout_secs));
                c.read_timeout = Some(Duration::from_secs(timeout_secs));
                c
            }
            Err(e) => {
                return ContextCheck {
                    context: context_name.to_string(),
                    nodes: vec![],
                    healthy: false,
                    error: Some(format!("Failed to create config: {}", e)),
                };
            }
        };

        let client = match Client::try_from(config) {
            Ok(c) => c,
            Err(e) => {
                return ContextCheck {
                    context: context_name.to_string(),
                    nodes: vec![],
                    healthy: false,
                    error: Some(format!("Failed to create client: {}", e)),
                };
            }
        };

        let api: Api<Node> = Api::all(client);

        match tokio::time::timeout(
            Duration::from_secs(timeout_secs),
            api.list(&kube::api::ListParams::default()),
        )
        .await
        {
            Ok(Ok(nodes)) => {
                let node_names: Vec<String> = nodes
                    .items
                    .iter()
                    .filter_map(|n| n.metadata.name.clone())
                    .collect();

                if node_names.is_empty() {
                    ContextCheck {
                        context: context_name.to_string(),
                        nodes: vec![],
                        healthy: false,
                        error: Some("No nodes found".to_string()),
                    }
                } else {
                    ContextCheck {
                        context: context_name.to_string(),
                        nodes: node_names,
                        healthy: true,
                        error: None,
                    }
                }
            }
            Ok(Err(e)) => ContextCheck {
                context: context_name.to_string(),
                nodes: vec![],
                healthy: false,
                error: Some(format!("API error: {}", e)),
            },
            Err(_) => ContextCheck {
                context: context_name.to_string(),
                nodes: vec![],
                healthy: false,
                error: Some(format!("Timeout after {}s", timeout_secs)),
            },
        }
    }

    fn get_all_contexts() -> anyhow::Result<Vec<String>> {
        let kubeconfig = Kubeconfig::read()?;
        Ok(kubeconfig
            .contexts
            .into_iter()
            .map(|c| c.name)
            .collect())
    }

    fn get_current_context() -> anyhow::Result<String> {
        let kubeconfig = Kubeconfig::read()?;
        kubeconfig
            .current_context
            .ok_or_else(|| anyhow::anyhow!("No current context set"))
    }
}

#[async_trait::async_trait]
impl RunCommand for ClusterAction {
    async fn run(&self, ctx: &dyn CommandContext) -> anyhow::Result<()> {
        match self {
            ClusterAction::Check {
                timeout,
                healthy_only,
                unhealthy_only,
            } => {
                let contexts = Self::get_all_contexts()?;

                if contexts.is_empty() {
                    tracing::warn!("No kubectl contexts found");
                    return Ok(());
                }

                tracing::info!("Checking {} context(s)...", contexts.len());

                // Check all contexts concurrently
                let checks: Vec<_> = futures::future::join_all(
                    contexts
                        .iter()
                        .map(|c| Self::check_context(c, *timeout)),
                )
                .await;

                // Filter results
                let filtered: Vec<_> = checks
                    .into_iter()
                    .filter(|c| {
                        if *healthy_only {
                            c.healthy
                        } else if *unhealthy_only {
                            !c.healthy
                        } else {
                            true
                        }
                    })
                    .collect();

                match ctx.output_format() {
                    OutputFormat::Json => {
                        println!("{}", serde_json::to_string_pretty(&filtered)?);
                    }
                    OutputFormat::Human => {
                        println!();
                        println!("Cluster Health Check Results:");
                        println!("==============================");

                        for check in &filtered {
                            let status = if check.healthy { "✓" } else { "✗" };
                            let color = if check.healthy { "\x1b[32m" } else { "\x1b[31m" };
                            let reset = "\x1b[0m";

                            let info = if check.healthy {
                                format!("{} node(s)", check.nodes.len())
                            } else {
                                check.error.clone().unwrap_or_default()
                            };

                            println!("{}{}{} {}: {}", color, status, reset, check.context, info);
                        }

                        println!();

                        let healthy_count = filtered.iter().filter(|c| c.healthy).count();
                        let total = filtered.len();

                        if healthy_count == total {
                            tracing::info!("All {} clusters healthy", total);
                        } else {
                            tracing::warn!("{}/{} clusters healthy", healthy_count, total);
                        }
                    }
                    OutputFormat::Quiet => {}
                }

                Ok(())
            }
            ClusterAction::Set { context } => {
                let mut kubeconfig = Kubeconfig::read()?;
                let available: Vec<String> =
                    kubeconfig.contexts.iter().map(|c| c.name.clone()).collect();

                if !available.contains(context) {
                    anyhow::bail!(
                        "Context '{}' not found. Available: {}",
                        context,
                        available.join(", ")
                    );
                }

                kubeconfig.current_context = Some(context.clone());

                let kubeconfig_path = kube::config::KubeConfigOptions::default();
                let _ = kubeconfig_path; // read path from env or default
                let path = std::env::var("KUBECONFIG").unwrap_or_else(|_| {
                    let home = std::env::var("HOME").unwrap_or_default();
                    format!("{}/.kube/config", home)
                });

                if ctx.dry_run() {
                    match ctx.output_format() {
                        OutputFormat::Json => {
                            println!(
                                "{}",
                                serde_json::to_string(&serde_json::json!({
                                    "action": "set-context",
                                    "context": context,
                                    "dry_run": true
                                }))?
                            );
                        }
                        OutputFormat::Human => {
                            println!("Would switch to context: {}", context);
                        }
                        OutputFormat::Quiet => {}
                    }
                    return Ok(());
                }

                let yaml = serde_yaml::to_string(&kubeconfig)?;
                std::fs::write(&path, yaml)?;

                match ctx.output_format() {
                    OutputFormat::Json => {
                        println!(
                            "{}",
                            serde_json::to_string(&serde_json::json!({
                                "context": context,
                                "switched": true
                            }))?
                        );
                    }
                    OutputFormat::Human => {
                        println!("Switched to context: {}", context);
                    }
                    OutputFormat::Quiet => {}
                }

                Ok(())
            }
            ClusterAction::List => {
                let contexts = Self::get_all_contexts()?;
                let current = Self::get_current_context().ok();

                match ctx.output_format() {
                    OutputFormat::Json => {
                        println!("{}", serde_json::to_string_pretty(&contexts)?);
                    }
                    OutputFormat::Human => {
                        for c in contexts {
                            let marker = if Some(&c) == current.as_ref() {
                                "* "
                            } else {
                                "  "
                            };
                            println!("{}{}", marker, c);
                        }
                    }
                    OutputFormat::Quiet => {}
                }

                Ok(())
            }
            ClusterAction::Current => {
                let current = Self::get_current_context()?;

                match ctx.output_format() {
                    OutputFormat::Json => {
                        println!("{}", serde_json::to_string(&current)?);
                    }
                    OutputFormat::Human => {
                        println!("{}", current);
                    }
                    OutputFormat::Quiet => {}
                }

                Ok(())
            }
            ClusterAction::Delete { context, force } => {
                let contexts = Self::get_all_contexts()?;

                if !contexts.contains(&context) {
                    anyhow::bail!(
                        "Context '{}' not found. Available: {}",
                        context,
                        contexts.join(", ")
                    );
                }

                let current = Self::get_current_context().ok();
                if current.as_deref() == Some(context.as_str()) {
                    anyhow::bail!("Cannot delete the currently active context '{}'", context);
                }

                if ctx.dry_run() {
                    match ctx.output_format() {
                        OutputFormat::Json => {
                            println!(
                                "{}",
                                serde_json::to_string(&serde_json::json!({
                                    "action": "delete-context",
                                    "context": context,
                                    "dry_run": true
                                }))?
                            );
                        }
                        OutputFormat::Human => {
                            println!("Would delete context: {}", context);
                        }
                        OutputFormat::Quiet => {}
                    }
                    return Ok(());
                }

                if !*force {
                    print!("Delete context '{}'? [y/N] ", context);
                    use std::io::Write;
                    std::io::stdout().flush()?;
                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input)?;
                    if !input.trim().eq_ignore_ascii_case("y") {
                        println!("Aborted.");
                        return Ok(());
                    }
                }

                let mut kubeconfig = Kubeconfig::read()?;
                kubeconfig.contexts.retain(|c| c.name != *context);

                let path = std::env::var("KUBECONFIG").unwrap_or_else(|_| {
                    let home = std::env::var("HOME").unwrap_or_default();
                    format!("{}/.kube/config", home)
                });
                let yaml = serde_yaml::to_string(&kubeconfig)?;
                std::fs::write(&path, yaml)?;

                match ctx.output_format() {
                    OutputFormat::Json => {
                        println!(
                            "{}",
                            serde_json::to_string(&serde_json::json!({
                                "context": context,
                                "deleted": true
                            }))?
                        );
                    }
                    OutputFormat::Human => {
                        println!("Deleted context: {}", context);
                    }
                    OutputFormat::Quiet => {}
                }

                Ok(())
            }
            #[cfg(feature = "tui")]
            ClusterAction::Tui => {
                if std::env::var("ZELLIJ").is_ok() {
                    tracing::info!("Inside Zellij, running picker mode");
                    ContextPickerTui::run().await
                } else {
                    let status = std::process::Command::new("zellij")
                        .args(["--layout", "main"])
                        .status()?;
                    if !status.success() {
                        anyhow::bail!("zellij exited with non-zero status");
                    }
                    Ok(())
                }
            }
            #[cfg(feature = "tui")]
            ClusterAction::Picker => {
                ContextPickerTui::run().await
            }
        }
    }
}

// ── Context Picker TUI (slim, for Zellij pane) ─────────────────────

#[cfg(feature = "tui")]
struct ContextPickerTui {
    contexts: Vec<String>,
    current_context: String,
    list_state: ListState,
    namespaces: Vec<String>,
    current_namespace: String,
    ns_list_state: ListState,
    active_panel: PickerPanel,
    status_message: Option<StatusMessage>,
    should_quit: bool,
}

#[cfg(feature = "tui")]
#[derive(Clone, Copy, PartialEq)]
enum PickerPanel {
    Contexts,
    Namespaces,
}

#[cfg(feature = "tui")]
impl ContextPickerTui {
    async fn run() -> anyhow::Result<()> {
        let contexts = ClusterAction::get_all_contexts()?;
        let current = ClusterAction::get_current_context().unwrap_or_default();
        let current_idx = contexts.iter().position(|c| c == &current).unwrap_or(0);
        let mut list_state = ListState::default();
        list_state.select(Some(current_idx));

        let namespaces = fetch_namespaces();
        let current_ns = get_current_namespace();
        let ns_idx = namespaces.iter().position(|n| n == &current_ns).unwrap_or(0);
        let mut ns_list_state = ListState::default();
        ns_list_state.select(Some(ns_idx));

        let mut app = Self {
            contexts,
            current_context: current,
            list_state,
            namespaces,
            current_namespace: current_ns,
            ns_list_state,
            active_panel: PickerPanel::Contexts,
            status_message: None,
            should_quit: false,
        };

        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = app.picker_loop(&mut terminal);

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    fn picker_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> anyhow::Result<()> {
        loop {
            terminal.draw(|f| self.render_picker(f))?;

            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        self.should_quit = true;
                    } else {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
                            KeyCode::Tab | KeyCode::BackTab => {
                                self.active_panel = match self.active_panel {
                                    PickerPanel::Contexts => PickerPanel::Namespaces,
                                    PickerPanel::Namespaces => PickerPanel::Contexts,
                                };
                            }
                            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
                            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
                            KeyCode::Home | KeyCode::Char('g') => self.jump_to(0),
                            KeyCode::End | KeyCode::Char('G') => self.jump_to_end(),
                            KeyCode::Enter | KeyCode::Char('s') => self.select_current(),
                            KeyCode::Char('d') | KeyCode::Delete => self.delete_context(),
                            KeyCode::Char('r') => self.refresh(),
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

    fn move_selection(&mut self, delta: i32) {
        let (state, len) = match self.active_panel {
            PickerPanel::Contexts => (&mut self.list_state, self.contexts.len()),
            PickerPanel::Namespaces => (&mut self.ns_list_state, self.namespaces.len()),
        };
        if len == 0 { return; }
        let current = state.selected().unwrap_or(0) as i32;
        let max = len as i32 - 1;
        state.select(Some((current + delta).clamp(0, max) as usize));
    }

    fn jump_to(&mut self, idx: usize) {
        match self.active_panel {
            PickerPanel::Contexts => self.list_state.select(Some(idx)),
            PickerPanel::Namespaces => self.ns_list_state.select(Some(idx)),
        };
    }

    fn jump_to_end(&mut self) {
        match self.active_panel {
            PickerPanel::Contexts => {
                if !self.contexts.is_empty() {
                    self.list_state.select(Some(self.contexts.len() - 1));
                }
            }
            PickerPanel::Namespaces => {
                if !self.namespaces.is_empty() {
                    self.ns_list_state.select(Some(self.namespaces.len() - 1));
                }
            }
        }
    }

    fn select_current(&mut self) {
        match self.active_panel {
            PickerPanel::Contexts => {
                let Some(idx) = self.list_state.selected() else { return };
                let Some(name) = self.contexts.get(idx).cloned() else { return };
                if name == self.current_context {
                    self.status_message = Some(StatusMessage::new(
                        format!("Already on '{}'", name), false,
                    ));
                    return;
                }
                match write_context(&name) {
                    Ok(()) => {
                        self.current_context = name.clone();
                        self.namespaces = fetch_namespaces();
                        self.current_namespace = get_current_namespace();
                        // Tell k9s to switch context natively
                        Self::notify_k9s_context(&name);
                        self.status_message = Some(StatusMessage::new(
                            format!("Switched to '{}'", name), false,
                        ));
                    }
                    Err(e) => {
                        self.status_message = Some(StatusMessage::new(
                            format!("Error: {}", e), true,
                        ));
                    }
                }
            }
            PickerPanel::Namespaces => {
                let Some(idx) = self.ns_list_state.selected() else { return };
                let Some(name) = self.namespaces.get(idx).cloned() else { return };
                if name == self.current_namespace {
                    self.status_message = Some(StatusMessage::new(
                        format!("Already in '{}'", name), false,
                    ));
                    return;
                }
                match write_namespace(&name) {
                    Ok(()) => {
                        self.current_namespace = name.clone();
                        self.status_message = Some(StatusMessage::new(
                            format!("Namespace -> '{}'", name), false,
                        ));
                    }
                    Err(e) => {
                        self.status_message = Some(StatusMessage::new(
                            format!("Error: {}", e), true,
                        ));
                    }
                }
            }
        }
    }

    fn delete_context(&mut self) {
        if self.active_panel != PickerPanel::Contexts { return; }
        let Some(idx) = self.list_state.selected() else { return };
        let Some(name) = self.contexts.get(idx).cloned() else { return };
        if name == self.current_context {
            self.status_message = Some(StatusMessage::new(
                format!("Cannot delete active context '{}'", name), true,
            ));
            return;
        }
        match delete_context(&name) {
            Ok(()) => {
                self.contexts.retain(|c| c != &name);
                if idx >= self.contexts.len() && !self.contexts.is_empty() {
                    self.list_state.select(Some(self.contexts.len() - 1));
                }
                self.status_message = Some(StatusMessage::new(
                    format!("Deleted '{}'", name), false,
                ));
            }
            Err(e) => {
                self.status_message = Some(StatusMessage::new(
                    format!("Delete error: {}", e), true,
                ));
            }
        }
    }

    /// Tell k9s to switch context via Zellij's write-chars action.
    /// Finds the k9s pane by name, then sends `:ctx <context>\n` which k9s handles natively.
    fn notify_k9s_context(context: &str) {
        let Some(pane_id) = Self::find_zellij_pane_id("k9s") else {
            tracing::warn!("Could not find k9s pane in Zellij session");
            return;
        };
        let _ = std::process::Command::new("zellij")
            .args(["action", "write-chars", "--pane-id", &pane_id, &format!(":ctx {}\n", context)])
            .status();
    }

    /// Find a Zellij pane ID by its name using `zellij action list-panes --json`.
    fn find_zellij_pane_id(name: &str) -> Option<String> {
        let output = std::process::Command::new("zellij")
            .args(["action", "list-panes", "--json"])
            .output()
            .ok()?;
        if !output.status.success() { return None; }
        let panes: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
        let arr = panes.as_array()?;
        for pane in arr {
            if pane.get("name").and_then(|n| n.as_str()) == Some(name) {
                if let Some(id) = pane.get("id").and_then(|id| id.as_u64()) {
                    return Some(format!("terminal_{}", id));
                }
            }
        }
        None
    }

    fn refresh(&mut self) {
        self.contexts = ClusterAction::get_all_contexts().unwrap_or_default();
        self.current_context = ClusterAction::get_current_context().unwrap_or_default();
        self.namespaces = fetch_namespaces();
        self.current_namespace = get_current_namespace();
        self.status_message = Some(StatusMessage::new("Refreshed".to_string(), false));
    }

    fn render_picker(&mut self, f: &mut ratatui::Frame) {
        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // header
                Constraint::Min(5),   // contexts
                Constraint::Min(5),   // namespaces
                Constraint::Length(3), // footer
            ])
            .split(f.area());

        // Header: current context + namespace
        let header = Line::from(vec![
            Span::styled(" ctx: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                &self.current_context,
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ns: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                &self.current_namespace,
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
        ]);
        f.render_widget(
            Paragraph::new(header).block(Block::default().borders(Borders::ALL).title("Active")),
            outer[0],
        );

        // Contexts list
        let ctx_items: Vec<ListItem> = self.contexts.iter().map(|name| {
            let is_current = name == &self.current_context;
            let marker = if is_current { "* " } else { "  " };
            let style = if is_current {
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(Span::styled(format!("{}{}", marker, name), style)))
        }).collect();

        let ctx_border_style = if self.active_panel == PickerPanel::Contexts {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let ctx_list = List::new(ctx_items)
            .block(Block::default().borders(Borders::ALL)
                .title(format!("Contexts ({})", self.contexts.len()))
                .border_style(ctx_border_style))
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");
        f.render_stateful_widget(ctx_list, outer[1], &mut self.list_state);

        // Namespaces list
        let ns_items: Vec<ListItem> = self.namespaces.iter().map(|name| {
            let is_current = name == &self.current_namespace;
            let marker = if is_current { "* " } else { "  " };
            let style = if is_current {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(Span::styled(format!("{}{}", marker, name), style)))
        }).collect();

        let ns_border_style = if self.active_panel == PickerPanel::Namespaces {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let ns_list = List::new(ns_items)
            .block(Block::default().borders(Borders::ALL)
                .title(format!("Namespaces ({})", self.namespaces.len()))
                .border_style(ns_border_style))
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");
        f.render_stateful_widget(ns_list, outer[2], &mut self.ns_list_state);

        // Footer
        let footer_content = if let Some(ref msg) = self.status_message {
            let color = if msg.is_error { Color::Red } else { Color::Yellow };
            Line::from(vec![
                Span::styled(format!("[{}] ", msg.timestamp()), Style::default().fg(Color::DarkGray)),
                Span::styled(&msg.text, Style::default().fg(color)),
            ])
        } else {
            Line::from(vec![
                Span::styled(" j/k", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(":nav  ", Style::default().fg(Color::DarkGray)),
                Span::styled("Tab", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(":switch  ", Style::default().fg(Color::DarkGray)),
                Span::styled("Enter", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(":set  ", Style::default().fg(Color::DarkGray)),
                Span::styled("d", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::styled(":delete  ", Style::default().fg(Color::DarkGray)),
                Span::styled("r", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(":refresh", Style::default().fg(Color::DarkGray)),
            ])
        };
        f.render_widget(
            Paragraph::new(footer_content).block(Block::default().borders(Borders::ALL)),
            outer[3],
        );
    }
}

// ── kubectl utility functions ────────────────────────────────────────

#[cfg(feature = "tui")]
fn fetch_namespaces() -> Vec<String> {
    let output = std::process::Command::new("kubectl")
        .args(["get", "namespaces", "-o", "name", "--request-timeout=5s"])
        .output();
    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|l| l.trim_start_matches("namespace/").to_string())
            .collect(),
        _ => vec!["default".to_string()],
    }
}

#[cfg(feature = "tui")]
fn get_current_namespace() -> String {
    if let Ok(kc) = Kubeconfig::read() {
        if let Some(ref ctx_name) = kc.current_context {
            if let Some(ctx) = kc.contexts.iter().find(|c| &c.name == ctx_name) {
                if let Some(ref ctx_val) = ctx.context {
                    if let Some(ref ns) = ctx_val.namespace {
                        return ns.clone();
                    }
                }
            }
        }
    }
    "default".to_string()
}

#[cfg(feature = "tui")]
fn write_namespace(ns: &str) -> anyhow::Result<()> {
    let status = std::process::Command::new("kubectl")
        .args(["config", "set-context", "--current", "--namespace", ns])
        .status()?;
    if !status.success() {
        anyhow::bail!("kubectl set-context namespace failed");
    }
    Ok(())
}

#[cfg(feature = "tui")]
fn write_context(name: &str) -> anyhow::Result<()> {
    let mut kubeconfig = Kubeconfig::read()?;
    kubeconfig.current_context = Some(name.to_string());
    let path = std::env::var("KUBECONFIG").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_default();
        format!("{}/.kube/config", home)
    });
    let yaml = serde_yaml::to_string(&kubeconfig)?;
    std::fs::write(&path, yaml)?;
    Ok(())
}

#[cfg(feature = "tui")]
fn delete_context(name: &str) -> anyhow::Result<()> {
    let mut kubeconfig = Kubeconfig::read()?;
    kubeconfig.contexts.retain(|c| c.name != name);
    let path = std::env::var("KUBECONFIG").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_default();
        format!("{}/.kube/config", home)
    });
    let yaml = serde_yaml::to_string(&kubeconfig)?;
    std::fs::write(&path, yaml)?;
    Ok(())
}

// ── Status / event log ───────────────────────────────────────────────

#[cfg(feature = "tui")]
struct StatusMessage {
    text: String,
    is_error: bool,
    time: std::time::SystemTime,
}

#[cfg(feature = "tui")]
impl StatusMessage {
    fn new(text: String, is_error: bool) -> Self {
        Self {
            text,
            is_error,
            time: std::time::SystemTime::now(),
        }
    }

    fn timestamp(&self) -> String {
        let elapsed = self
            .time
            .elapsed()
            .unwrap_or_default();
        let total_secs = elapsed.as_secs();
        if total_secs < 60 {
            format!("{}s ago", total_secs)
        } else if total_secs < 3600 {
            format!("{}m ago", total_secs / 60)
        } else {
            format!("{}h ago", total_secs / 3600)
        }
    }

    fn log_line(&self) -> String {
        let level = if self.is_error { "ERR" } else { "INF" };
        let secs = self
            .time
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        format!("[{}] [{}] {}", secs, level, self.text)
    }
}
