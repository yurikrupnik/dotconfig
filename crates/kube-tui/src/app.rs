use std::collections::HashMap;

use ratatui::widgets::{ListState, TableState};

use crate::resources::ResourceType;

/// Input mode — what keystrokes are routed to
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMode {
    /// Normal navigation (j/k/Enter/etc)
    Normal,
    /// `:` command bar — type resource name, :ns, :ctx, :q
    Command,
    /// `/` filter bar — type filter string
    Filter,
}

/// Which view the user is looking at
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    /// Table of resources
    ResourceList,
    /// YAML dump of selected resource
    Yaml,
    /// kubectl describe output
    Describe,
    /// Pod logs
    Logs,
    /// Namespace selector
    NamespaceSelect,
    /// Context selector
    ContextSelect,
}

/// A single Kubernetes resource row
#[derive(Debug, Clone)]
pub struct ResourceRow {
    pub name: String,
    pub namespace: String,
    pub status: String,
    pub age: String,
    pub ready: String,
    pub restarts: String,
    pub extra: HashMap<String, String>,
}

/// Application state
pub struct App {
    pub should_quit: bool,
    pub input_mode: InputMode,
    pub view: View,
    pub view_stack: Vec<View>,

    // Command / filter bar
    pub command_input: String,
    pub filter_input: String,

    // Current resource type being viewed
    pub resource_type: ResourceType,

    // Resource data
    pub resources: Vec<ResourceRow>,
    pub table_state: TableState,

    // Detail views
    pub yaml_content: String,
    pub describe_content: String,
    pub logs_content: Vec<String>,
    pub detail_scroll: u16,

    // Namespace
    pub namespaces: Vec<String>,
    pub current_namespace: String,
    pub namespace_state: ListState,

    // Contexts
    pub contexts: Vec<String>,
    pub current_context: String,
    pub context_state: ListState,

    // Status message
    pub status_message: Option<String>,

    // Whether a background watch is running
    pub watching: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            input_mode: InputMode::Normal,
            view: View::ResourceList,
            view_stack: Vec::new(),

            command_input: String::new(),
            filter_input: String::new(),

            resource_type: ResourceType::Pods,

            resources: Vec::new(),
            table_state: TableState::default(),

            yaml_content: String::new(),
            describe_content: String::new(),
            logs_content: Vec::new(),
            detail_scroll: 0,

            namespaces: Vec::new(),
            current_namespace: String::new(),
            namespace_state: ListState::default(),

            contexts: Vec::new(),
            current_context: String::new(),
            context_state: ListState::default(),

            status_message: None,
            watching: false,
        }
    }

    pub fn push_view(&mut self, view: View) {
        let old = std::mem::replace(&mut self.view, view);
        self.view_stack.push(old);
        self.detail_scroll = 0;
    }

    pub fn pop_view(&mut self) {
        if let Some(prev) = self.view_stack.pop() {
            self.view = prev;
            self.detail_scroll = 0;
        }
    }

    pub fn selected_resource(&self) -> Option<ResourceRow> {
        self.table_state
            .selected()
            .and_then(|i| self.filtered_resources().get(i).cloned())
            .cloned()
    }

    pub fn filtered_resources(&self) -> Vec<&ResourceRow> {
        if self.filter_input.is_empty() {
            self.resources.iter().collect()
        } else {
            let filter = self.filter_input.to_lowercase();
            self.resources
                .iter()
                .filter(|r| {
                    r.name.to_lowercase().contains(&filter)
                        || r.namespace.to_lowercase().contains(&filter)
                        || r.status.to_lowercase().contains(&filter)
                })
                .collect()
        }
    }

    pub fn move_selection(&mut self, delta: i32) {
        let len = self.filtered_resources().len();
        if len == 0 {
            return;
        }
        let current = self.table_state.selected().unwrap_or(0) as i32;
        let max = len as i32 - 1;
        let next = (current + delta).clamp(0, max) as usize;
        self.table_state.select(Some(next));
    }

    pub fn move_ns_selection(&mut self, delta: i32) {
        let len = self.namespaces.len();
        if len == 0 {
            return;
        }
        let current = self.namespace_state.selected().unwrap_or(0) as i32;
        let next = (current + delta).clamp(0, len as i32 - 1) as usize;
        self.namespace_state.select(Some(next));
    }

    pub fn move_ctx_selection(&mut self, delta: i32) {
        let len = self.contexts.len();
        if len == 0 {
            return;
        }
        let current = self.context_state.selected().unwrap_or(0) as i32;
        let next = (current + delta).clamp(0, len as i32 - 1) as usize;
        self.context_state.select(Some(next));
    }
}
