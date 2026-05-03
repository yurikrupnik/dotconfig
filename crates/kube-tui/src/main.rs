mod app;
mod keybindings;
mod resources;
mod ui;

use std::io;
use std::time::Duration;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::{App, View};
use keybindings::{handle_key, KeyAction};
use resources::{
    create_client, fetch_namespaces, fetch_resources, get_all_contexts, get_pod_logs,
    get_resource_yaml, set_context, ResourceType,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Init tracing to file to avoid messing with TUI
    tracing_subscriber::fmt()
        .with_writer(io::stderr)
        .with_max_level(tracing::Level::WARN)
        .init();

    let mut app = App::new();

    // Load contexts
    match get_all_contexts() {
        Ok((contexts, current)) => {
            let idx = contexts.iter().position(|c| c == &current).unwrap_or(0);
            app.contexts = contexts;
            app.current_context = current;
            app.context_state.select(Some(idx));
        }
        Err(e) => {
            app.status_message = Some(format!("Failed to read kubeconfig: {}", e));
        }
    }

    // Create initial client and load data
    let mut client = create_client(None).await.ok();

    if let Some(ref c) = client {
        if let Ok(ns) = fetch_namespaces(c).await {
            app.namespaces = ns;
        }
        load_resources(&mut app, c).await;
    } else {
        app.status_message = Some("Failed to connect to cluster".into());
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Main loop
    loop {
        terminal.draw(|f| ui::render(f, &mut app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if let Some(action) = handle_key(&mut app, key) {
                    match action {
                        KeyAction::Handled => {}
                        KeyAction::Refresh => {
                            if let Some(ref c) = client {
                                app.status_message = Some("Refreshing...".into());
                                load_resources(&mut app, c).await;
                                app.status_message = Some("Refreshed".into());
                            }
                        }
                        KeyAction::SwitchResource(rt) => {
                            app.resource_type = rt;
                            app.filter_input.clear();
                            app.table_state.select(Some(0));
                            if let Some(ref c) = client {
                                app.status_message =
                                    Some(format!("Loading {}...", rt.label()));
                                load_resources(&mut app, c).await;
                                app.status_message = None;
                            }
                        }
                        KeyAction::ShowYaml => {
                            if let (Some(res), Some(ref c)) =
                                (app.selected_resource(), &client)
                            {
                                app.status_message = Some("Loading YAML...".into());
                                match get_resource_yaml(
                                    c,
                                    app.resource_type,
                                    &res.namespace,
                                    &res.name,
                                )
                                .await
                                {
                                    Ok(yaml) => {
                                        app.yaml_content = yaml;
                                        app.push_view(View::Yaml);
                                        app.status_message = None;
                                    }
                                    Err(e) => {
                                        app.status_message =
                                            Some(format!("YAML error: {}", e));
                                    }
                                }
                            }
                        }
                        KeyAction::ShowDescribe => {
                            if let (Some(res), Some(ref c)) =
                                (app.selected_resource(), &client)
                            {
                                // Use YAML as describe for now (rich describe needs kubectl)
                                match get_resource_yaml(
                                    c,
                                    app.resource_type,
                                    &res.namespace,
                                    &res.name,
                                )
                                .await
                                {
                                    Ok(yaml) => {
                                        app.describe_content = yaml;
                                        app.push_view(View::Describe);
                                    }
                                    Err(e) => {
                                        app.status_message =
                                            Some(format!("Describe error: {}", e));
                                    }
                                }
                            }
                        }
                        KeyAction::ShowLogs => {
                            if let (Some(res), Some(ref c)) =
                                (app.selected_resource(), &client)
                            {
                                app.status_message = Some("Loading logs...".into());
                                match get_pod_logs(c, &res.namespace, &res.name, 500).await {
                                    Ok(logs) => {
                                        app.logs_content = logs;
                                        app.detail_scroll = 0;
                                        app.push_view(View::Logs);
                                        app.status_message = None;
                                    }
                                    Err(e) => {
                                        app.status_message =
                                            Some(format!("Logs error: {}", e));
                                    }
                                }
                            }
                        }
                        KeyAction::ShellIntoPod => {
                            if let Some(res) = app.selected_resource() {
                                // Restore terminal, exec kubectl, then re-init
                                disable_raw_mode()?;
                                execute!(
                                    terminal.backend_mut(),
                                    LeaveAlternateScreen,
                                    DisableMouseCapture
                                )?;

                                let ns = if res.namespace.is_empty() {
                                    "default".to_string()
                                } else {
                                    res.namespace.clone()
                                };

                                let status = tokio::process::Command::new("kubectl")
                                    .args([
                                        "exec",
                                        "-it",
                                        "-n",
                                        &ns,
                                        &res.name,
                                        "--",
                                        "/bin/sh",
                                    ])
                                    .status()
                                    .await;

                                // Re-init terminal
                                enable_raw_mode()?;
                                execute!(
                                    terminal.backend_mut(),
                                    EnterAlternateScreen,
                                    EnableMouseCapture
                                )?;
                                terminal.clear()?;

                                match status {
                                    Ok(s) if s.success() => {
                                        app.status_message =
                                            Some(format!("Shell closed for {}", res.name));
                                    }
                                    Ok(s) => {
                                        app.status_message = Some(format!(
                                            "Shell exited with code {}",
                                            s.code().unwrap_or(-1)
                                        ));
                                    }
                                    Err(e) => {
                                        app.status_message =
                                            Some(format!("Shell error: {}", e));
                                    }
                                }
                            }
                        }
                        KeyAction::SwitchContext(ctx_name) => {
                            match set_context(&ctx_name) {
                                Ok(()) => {
                                    app.current_context = ctx_name.clone();
                                    app.status_message =
                                        Some(format!("Switched to context: {}", ctx_name));
                                    // Reconnect client
                                    match create_client(Some(&ctx_name)).await {
                                        Ok(c) => {
                                            if let Ok(ns) = fetch_namespaces(&c).await {
                                                app.namespaces = ns;
                                            }
                                            app.current_namespace.clear();
                                            load_resources(&mut app, &c).await;
                                            client = Some(c);
                                        }
                                        Err(e) => {
                                            app.status_message = Some(format!(
                                                "Failed to connect to {}: {}",
                                                ctx_name, e
                                            ));
                                            client = None;
                                        }
                                    }
                                }
                                Err(e) => {
                                    app.status_message =
                                        Some(format!("Context switch failed: {}", e));
                                }
                            }
                        }
                        KeyAction::SwitchNamespace(ns) => {
                            if ns == "<all>" {
                                app.current_namespace.clear();
                            } else {
                                app.current_namespace = ns.clone();
                            }
                            app.table_state.select(Some(0));
                            if let Some(ref c) = client {
                                load_resources(&mut app, c).await;
                            }
                            app.status_message = Some(format!(
                                "Namespace: {}",
                                if app.current_namespace.is_empty() {
                                    "<all>"
                                } else {
                                    &app.current_namespace
                                }
                            ));
                        }
                        KeyAction::DeleteResource => {
                            app.status_message =
                                Some("Delete not yet implemented (safety first!)".into());
                        }
                    }
                }
            }
        }

        if app.should_quit {
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

async fn load_resources(app: &mut App, client: &kube::Client) {
    match fetch_resources(client, app.resource_type, &app.current_namespace).await {
        Ok(rows) => {
            app.resources = rows;
            if app.table_state.selected().is_none() && !app.resources.is_empty() {
                app.table_state.select(Some(0));
            }
        }
        Err(e) => {
            app.status_message = Some(format!("Error: {}", e));
            app.resources.clear();
        }
    }
}
