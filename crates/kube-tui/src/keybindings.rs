use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, InputMode, View};
use crate::resources::ResourceType;

/// Result of handling a key event
pub enum KeyAction {
    /// No-op, already handled internally
    Handled,
    /// Need to refresh current resource list
    Refresh,
    /// Need to switch resource type and refresh
    SwitchResource(ResourceType),
    /// Show YAML for selected resource
    ShowYaml,
    /// Show describe for selected resource
    ShowDescribe,
    /// Show logs for selected pod
    ShowLogs,
    /// Shell into selected pod
    ShellIntoPod,
    /// Switch to a k8s context by name
    SwitchContext(String),
    /// Switch namespace
    SwitchNamespace(String),
    /// Delete selected resource (requires confirmation)
    DeleteResource,
}

pub fn handle_key(app: &mut App, key: KeyEvent) -> Option<KeyAction> {
    match app.input_mode {
        InputMode::Command => handle_command_mode(app, key),
        InputMode::Filter => handle_filter_mode(app, key),
        InputMode::Normal => match app.view {
            View::ResourceList => handle_resource_list(app, key),
            View::Yaml | View::Describe => handle_detail_view(app, key),
            View::Logs => handle_logs_view(app, key),
            View::NamespaceSelect => handle_namespace_select(app, key),
            View::ContextSelect => handle_context_select(app, key),
        },
    }
}

// ── Command mode (:) ──────────────────────────────────────────────────

fn handle_command_mode(app: &mut App, key: KeyEvent) -> Option<KeyAction> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.command_input.clear();
            Some(KeyAction::Handled)
        }
        KeyCode::Enter => {
            let cmd = app.command_input.trim().to_string();
            app.command_input.clear();
            app.input_mode = InputMode::Normal;
            execute_command(app, &cmd)
        }
        KeyCode::Backspace => {
            app.command_input.pop();
            Some(KeyAction::Handled)
        }
        KeyCode::Char(c) => {
            app.command_input.push(c);
            Some(KeyAction::Handled)
        }
        _ => Some(KeyAction::Handled),
    }
}

fn execute_command(app: &mut App, cmd: &str) -> Option<KeyAction> {
    match cmd {
        "q" | "quit" => {
            app.should_quit = true;
            Some(KeyAction::Handled)
        }
        "ctx" | "context" | "contexts" => {
            app.push_view(View::ContextSelect);
            Some(KeyAction::Handled)
        }
        "ns" => {
            app.push_view(View::NamespaceSelect);
            Some(KeyAction::Handled)
        }
        _ => {
            // Try resource type
            if let Some(rt) = ResourceType::from_command(cmd) {
                app.table_state.select(Some(0));
                Some(KeyAction::SwitchResource(rt))
            } else {
                app.status_message = Some(format!("Unknown command: {}", cmd));
                Some(KeyAction::Handled)
            }
        }
    }
}

// ── Filter mode (/) ──────────────────────────────────────────────────

fn handle_filter_mode(app: &mut App, key: KeyEvent) -> Option<KeyAction> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.filter_input.clear();
            app.table_state.select(Some(0));
            Some(KeyAction::Handled)
        }
        KeyCode::Enter => {
            app.input_mode = InputMode::Normal;
            app.table_state.select(Some(0));
            Some(KeyAction::Handled)
        }
        KeyCode::Backspace => {
            app.filter_input.pop();
            app.table_state.select(Some(0));
            Some(KeyAction::Handled)
        }
        KeyCode::Char(c) => {
            app.filter_input.push(c);
            app.table_state.select(Some(0));
            Some(KeyAction::Handled)
        }
        _ => Some(KeyAction::Handled),
    }
}

// ── Resource list (normal mode) ──────────────────────────────────────

fn handle_resource_list(app: &mut App, key: KeyEvent) -> Option<KeyAction> {
    match key.code {
        // Quit
        KeyCode::Char('q') => {
            app.should_quit = true;
            Some(KeyAction::Handled)
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_quit = true;
            Some(KeyAction::Handled)
        }

        // Mode switches
        KeyCode::Char(':') => {
            app.input_mode = InputMode::Command;
            app.command_input.clear();
            Some(KeyAction::Handled)
        }
        KeyCode::Char('/') => {
            app.input_mode = InputMode::Filter;
            app.filter_input.clear();
            Some(KeyAction::Handled)
        }

        // Navigation
        KeyCode::Char('j') | KeyCode::Down => {
            app.move_selection(1);
            Some(KeyAction::Handled)
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.move_selection(-1);
            Some(KeyAction::Handled)
        }
        KeyCode::Char('g') | KeyCode::Home => {
            app.table_state.select(Some(0));
            Some(KeyAction::Handled)
        }
        KeyCode::Char('G') | KeyCode::End => {
            let len = app.filtered_resources().len();
            if len > 0 {
                app.table_state.select(Some(len - 1));
            }
            Some(KeyAction::Handled)
        }
        KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.move_selection(20);
            Some(KeyAction::Handled)
        }
        KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.move_selection(-20);
            Some(KeyAction::Handled)
        }
        KeyCode::PageDown => {
            app.move_selection(20);
            Some(KeyAction::Handled)
        }
        KeyCode::PageUp => {
            app.move_selection(-20);
            Some(KeyAction::Handled)
        }

        // Ctrl-d delete (must be before plain 'd')
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(KeyAction::DeleteResource)
        }

        // Actions on selected resource
        KeyCode::Enter | KeyCode::Char('d') => Some(KeyAction::ShowDescribe),
        KeyCode::Char('y') => Some(KeyAction::ShowYaml),
        KeyCode::Char('l') => {
            if app.resource_type == ResourceType::Pods {
                Some(KeyAction::ShowLogs)
            } else {
                app.status_message = Some("Logs only available for Pods".into());
                Some(KeyAction::Handled)
            }
        }
        KeyCode::Char('s') => {
            if app.resource_type == ResourceType::Pods {
                Some(KeyAction::ShellIntoPod)
            } else {
                app.status_message = Some("Shell only available for Pods".into());
                Some(KeyAction::Handled)
            }
        }

        // Refresh
        KeyCode::Char('r') => Some(KeyAction::Refresh),

        // Quick resource switches (k9s style number keys)
        KeyCode::Char('1') => Some(KeyAction::SwitchResource(ResourceType::Pods)),
        KeyCode::Char('2') => Some(KeyAction::SwitchResource(ResourceType::Deployments)),
        KeyCode::Char('3') => Some(KeyAction::SwitchResource(ResourceType::Services)),
        KeyCode::Char('4') => Some(KeyAction::SwitchResource(ResourceType::Nodes)),
        KeyCode::Char('5') => Some(KeyAction::SwitchResource(ResourceType::Namespaces)),
        KeyCode::Char('6') => Some(KeyAction::SwitchResource(ResourceType::ConfigMaps)),
        KeyCode::Char('7') => Some(KeyAction::SwitchResource(ResourceType::Secrets)),
        KeyCode::Char('8') => Some(KeyAction::SwitchResource(ResourceType::Ingresses)),
        KeyCode::Char('9') => Some(KeyAction::SwitchResource(ResourceType::DaemonSets)),
        KeyCode::Char('0') => Some(KeyAction::SwitchResource(ResourceType::StatefulSets)),

        // Namespace / context
        KeyCode::Char('n') => {
            app.push_view(View::NamespaceSelect);
            Some(KeyAction::Handled)
        }
        KeyCode::Char('c') => {
            app.push_view(View::ContextSelect);
            Some(KeyAction::Handled)
        }

        // Esc clears filter or status
        KeyCode::Esc => {
            if !app.filter_input.is_empty() {
                app.filter_input.clear();
                app.table_state.select(Some(0));
            }
            app.status_message = None;
            Some(KeyAction::Handled)
        }

        _ => Some(KeyAction::Handled),
    }
}

// ── Detail views (YAML / Describe) ──────────────────────────────────

fn handle_detail_view(app: &mut App, key: KeyEvent) -> Option<KeyAction> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            app.pop_view();
            Some(KeyAction::Handled)
        }
        KeyCode::Char('j') | KeyCode::Down => {
            app.detail_scroll = app.detail_scroll.saturating_add(1);
            Some(KeyAction::Handled)
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.detail_scroll = app.detail_scroll.saturating_sub(1);
            Some(KeyAction::Handled)
        }
        KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.detail_scroll = app.detail_scroll.saturating_add(20);
            Some(KeyAction::Handled)
        }
        KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.detail_scroll = app.detail_scroll.saturating_sub(20);
            Some(KeyAction::Handled)
        }
        KeyCode::PageDown => {
            app.detail_scroll = app.detail_scroll.saturating_add(20);
            Some(KeyAction::Handled)
        }
        KeyCode::PageUp => {
            app.detail_scroll = app.detail_scroll.saturating_sub(20);
            Some(KeyAction::Handled)
        }
        KeyCode::Char('g') | KeyCode::Home => {
            app.detail_scroll = 0;
            Some(KeyAction::Handled)
        }
        _ => Some(KeyAction::Handled),
    }
}

// ── Logs view ────────────────────────────────────────────────────────

fn handle_logs_view(app: &mut App, key: KeyEvent) -> Option<KeyAction> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            app.pop_view();
            Some(KeyAction::Handled)
        }
        KeyCode::Char('j') | KeyCode::Down => {
            app.detail_scroll = app.detail_scroll.saturating_add(1);
            Some(KeyAction::Handled)
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.detail_scroll = app.detail_scroll.saturating_sub(1);
            Some(KeyAction::Handled)
        }
        KeyCode::Char('G') | KeyCode::End => {
            app.detail_scroll = app.logs_content.len().saturating_sub(1) as u16;
            Some(KeyAction::Handled)
        }
        KeyCode::Char('g') | KeyCode::Home => {
            app.detail_scroll = 0;
            Some(KeyAction::Handled)
        }
        KeyCode::PageDown | KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.detail_scroll = app.detail_scroll.saturating_add(20);
            Some(KeyAction::Handled)
        }
        KeyCode::PageUp | KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.detail_scroll = app.detail_scroll.saturating_sub(20);
            Some(KeyAction::Handled)
        }
        _ => Some(KeyAction::Handled),
    }
}

// ── Namespace selector ───────────────────────────────────────────────

fn handle_namespace_select(app: &mut App, key: KeyEvent) -> Option<KeyAction> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.pop_view();
            Some(KeyAction::Handled)
        }
        KeyCode::Char('j') | KeyCode::Down => {
            app.move_ns_selection(1);
            Some(KeyAction::Handled)
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.move_ns_selection(-1);
            Some(KeyAction::Handled)
        }
        KeyCode::Enter | KeyCode::Char('s') => {
            if let Some(idx) = app.namespace_state.selected() {
                if let Some(ns) = app.namespaces.get(idx).cloned() {
                    app.pop_view();
                    return Some(KeyAction::SwitchNamespace(ns));
                }
            }
            Some(KeyAction::Handled)
        }
        _ => Some(KeyAction::Handled),
    }
}

// ── Context selector ─────────────────────────────────────────────────

fn handle_context_select(app: &mut App, key: KeyEvent) -> Option<KeyAction> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.pop_view();
            Some(KeyAction::Handled)
        }
        KeyCode::Char('j') | KeyCode::Down => {
            app.move_ctx_selection(1);
            Some(KeyAction::Handled)
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.move_ctx_selection(-1);
            Some(KeyAction::Handled)
        }
        KeyCode::Enter | KeyCode::Char('s') => {
            if let Some(idx) = app.context_state.selected() {
                if let Some(ctx) = app.contexts.get(idx).cloned() {
                    app.pop_view();
                    return Some(KeyAction::SwitchContext(ctx));
                }
            }
            Some(KeyAction::Handled)
        }
        _ => Some(KeyAction::Handled),
    }
}
