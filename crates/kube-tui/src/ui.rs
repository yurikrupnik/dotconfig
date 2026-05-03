use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, Wrap},
    Frame,
};

use crate::app::{App, InputMode, View};
use crate::resources::ResourceType;

pub fn render(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // top bar
            Constraint::Min(5),   // main content
            Constraint::Length(1), // status / input bar
        ])
        .split(f.area());

    render_top_bar(f, app, chunks[0]);

    match app.view {
        View::ResourceList => render_resource_table(f, app, chunks[1]),
        View::Yaml => render_yaml(f, app, chunks[1]),
        View::Describe => render_describe(f, app, chunks[1]),
        View::Logs => render_logs(f, app, chunks[1]),
        View::NamespaceSelect => {
            render_resource_table(f, app, chunks[1]);
            render_namespace_popup(f, app);
        }
        View::ContextSelect => {
            render_resource_table(f, app, chunks[1]);
            render_context_popup(f, app);
        }
    }

    render_bottom_bar(f, app, chunks[2]);
}

fn render_top_bar(f: &mut Frame, app: &App, area: Rect) {
    let spans = vec![
        Span::styled(" kube-tui ", Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled(
            format!("ctx:{}", app.current_context),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw(" "),
        Span::styled(
            format!(
                "ns:{}",
                if app.current_namespace.is_empty() {
                    "<all>"
                } else {
                    &app.current_namespace
                }
            ),
            Style::default().fg(Color::Green),
        ),
        Span::raw(" "),
        Span::styled(
            format!("[{}]", app.resource_type.label()),
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            format!("{} items", app.filtered_resources().len()),
            Style::default().fg(Color::DarkGray),
        ),
    ];

    let bar = Paragraph::new(Line::from(spans));
    f.render_widget(bar, area);
}

fn render_bottom_bar(f: &mut Frame, app: &App, area: Rect) {
    let content = match app.input_mode {
        InputMode::Command => Line::from(vec![
            Span::styled(":", Style::default().fg(Color::Yellow)),
            Span::raw(&app.command_input),
            Span::styled("_", Style::default().fg(Color::Gray)),
        ]),
        InputMode::Filter => Line::from(vec![
            Span::styled("/", Style::default().fg(Color::Yellow)),
            Span::raw(&app.filter_input),
            Span::styled("_", Style::default().fg(Color::Gray)),
        ]),
        InputMode::Normal => {
            if let Some(ref msg) = app.status_message {
                Line::from(Span::styled(msg.as_str(), Style::default().fg(Color::Yellow)))
            } else {
                match app.view {
                    View::ResourceList => Line::from(vec![
                        Span::styled(":", Style::default().fg(Color::DarkGray)),
                        Span::styled("cmd", Style::default().fg(Color::Gray)),
                        Span::raw("  "),
                        Span::styled("/", Style::default().fg(Color::DarkGray)),
                        Span::styled("filter", Style::default().fg(Color::Gray)),
                        Span::raw("  "),
                        Span::styled("d", Style::default().fg(Color::DarkGray)),
                        Span::styled("esc", Style::default().fg(Color::Gray)),
                        Span::raw("  "),
                        Span::styled("y", Style::default().fg(Color::DarkGray)),
                        Span::styled("aml", Style::default().fg(Color::Gray)),
                        Span::raw("  "),
                        Span::styled("l", Style::default().fg(Color::DarkGray)),
                        Span::styled("ogs", Style::default().fg(Color::Gray)),
                        Span::raw("  "),
                        Span::styled("s", Style::default().fg(Color::DarkGray)),
                        Span::styled("hell", Style::default().fg(Color::Gray)),
                        Span::raw("  "),
                        Span::styled("n", Style::default().fg(Color::DarkGray)),
                        Span::styled("s", Style::default().fg(Color::Gray)),
                        Span::raw("  "),
                        Span::styled("c", Style::default().fg(Color::DarkGray)),
                        Span::styled("tx", Style::default().fg(Color::Gray)),
                        Span::raw("  "),
                        Span::styled("r", Style::default().fg(Color::DarkGray)),
                        Span::styled("efresh", Style::default().fg(Color::Gray)),
                        Span::raw("  "),
                        Span::styled("q", Style::default().fg(Color::DarkGray)),
                        Span::styled("uit", Style::default().fg(Color::Gray)),
                    ]),
                    View::Yaml | View::Describe | View::Logs => Line::from(vec![
                        Span::styled("j/k", Style::default().fg(Color::DarkGray)),
                        Span::styled(" scroll", Style::default().fg(Color::Gray)),
                        Span::raw("  "),
                        Span::styled("q/Esc", Style::default().fg(Color::DarkGray)),
                        Span::styled(" back", Style::default().fg(Color::Gray)),
                    ]),
                    View::NamespaceSelect | View::ContextSelect => Line::from(vec![
                        Span::styled("j/k", Style::default().fg(Color::DarkGray)),
                        Span::styled(" navigate", Style::default().fg(Color::Gray)),
                        Span::raw("  "),
                        Span::styled("Enter/s", Style::default().fg(Color::DarkGray)),
                        Span::styled(" select", Style::default().fg(Color::Gray)),
                        Span::raw("  "),
                        Span::styled("Esc", Style::default().fg(Color::DarkGray)),
                        Span::styled(" back", Style::default().fg(Color::Gray)),
                    ]),
                }
            }
        }
    };

    let bar = Paragraph::new(content);
    f.render_widget(bar, area);
}

fn render_resource_table(f: &mut Frame, app: &mut App, area: Rect) {
    let columns = app.resource_type.columns();
    let filtered = app.filtered_resources();

    let header_cells: Vec<Cell> = columns
        .iter()
        .map(|c| {
            Cell::from(*c).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
        })
        .collect();
    let header = Row::new(header_cells).height(1);

    let rows: Vec<Row> = filtered
        .iter()
        .map(|r| {
            let cells = build_row_cells(app.resource_type, r);
            Row::new(cells)
        })
        .collect();

    let widths: Vec<Constraint> = columns
        .iter()
        .map(|_| Constraint::Percentage(100 / columns.len() as u16))
        .collect();

    let title = if app.filter_input.is_empty() {
        app.resource_type.label().to_string()
    } else {
        format!("{} (filter: {})", app.resource_type.label(), app.filter_input)
    };

    let table = Table::new(rows, &widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    f.render_stateful_widget(table, area, &mut app.table_state);
}

fn build_row_cells(rt: ResourceType, r: &crate::app::ResourceRow) -> Vec<Cell<'static>> {
    match rt {
        ResourceType::Pods => vec![
            Cell::from(r.name.clone()),
            Cell::from(r.namespace.clone()),
            status_cell(&r.status),
            Cell::from(r.ready.clone()),
            Cell::from(r.restarts.clone()),
            Cell::from(r.age.clone()),
        ],
        ResourceType::Deployments => vec![
            Cell::from(r.name.clone()),
            Cell::from(r.namespace.clone()),
            Cell::from(r.ready.clone()),
            status_cell(&r.status),
            Cell::from(r.age.clone()),
        ],
        ResourceType::Services => vec![
            Cell::from(r.name.clone()),
            Cell::from(r.namespace.clone()),
            Cell::from(r.extra.get("type").cloned().unwrap_or_default()),
            Cell::from(r.extra.get("cluster_ip").cloned().unwrap_or_default()),
            Cell::from(r.age.clone()),
        ],
        ResourceType::Nodes => vec![
            Cell::from(r.name.clone()),
            status_cell(&r.status),
            Cell::from(r.extra.get("roles").cloned().unwrap_or_default()),
            Cell::from(r.extra.get("version").cloned().unwrap_or_default()),
            Cell::from(r.age.clone()),
        ],
        ResourceType::Namespaces => vec![
            Cell::from(r.name.clone()),
            status_cell(&r.status),
            Cell::from(r.age.clone()),
        ],
        ResourceType::ConfigMaps => vec![
            Cell::from(r.name.clone()),
            Cell::from(r.namespace.clone()),
            Cell::from(r.extra.get("data").cloned().unwrap_or_default()),
            Cell::from(r.age.clone()),
        ],
        ResourceType::Secrets => vec![
            Cell::from(r.name.clone()),
            Cell::from(r.namespace.clone()),
            Cell::from(r.extra.get("type").cloned().unwrap_or_default()),
            Cell::from(r.extra.get("data").cloned().unwrap_or_default()),
            Cell::from(r.age.clone()),
        ],
        ResourceType::Ingresses => vec![
            Cell::from(r.name.clone()),
            Cell::from(r.namespace.clone()),
            Cell::from(r.extra.get("hosts").cloned().unwrap_or_default()),
            Cell::from(r.age.clone()),
        ],
        ResourceType::DaemonSets => vec![
            Cell::from(r.name.clone()),
            Cell::from(r.namespace.clone()),
            Cell::from(r.extra.get("desired").cloned().unwrap_or_default()),
            Cell::from(r.ready.clone()),
            Cell::from(r.age.clone()),
        ],
        ResourceType::StatefulSets => vec![
            Cell::from(r.name.clone()),
            Cell::from(r.namespace.clone()),
            Cell::from(r.ready.clone()),
            Cell::from(r.age.clone()),
        ],
        ResourceType::ReplicaSets => vec![
            Cell::from(r.name.clone()),
            Cell::from(r.namespace.clone()),
            Cell::from(r.extra.get("desired").cloned().unwrap_or_default()),
            Cell::from(r.ready.clone()),
            Cell::from(r.age.clone()),
        ],
        ResourceType::Jobs => vec![
            Cell::from(r.name.clone()),
            Cell::from(r.namespace.clone()),
            Cell::from(r.extra.get("completions").cloned().unwrap_or_default()),
            Cell::from(r.age.clone()),
        ],
        ResourceType::CronJobs => vec![
            Cell::from(r.name.clone()),
            Cell::from(r.namespace.clone()),
            Cell::from(r.extra.get("schedule").cloned().unwrap_or_default()),
            Cell::from(r.age.clone()),
        ],
        ResourceType::ServiceAccounts => vec![
            Cell::from(r.name.clone()),
            Cell::from(r.namespace.clone()),
            Cell::from(r.extra.get("secrets").cloned().unwrap_or_default()),
            Cell::from(r.age.clone()),
        ],
        ResourceType::PersistentVolumeClaims => vec![
            Cell::from(r.name.clone()),
            Cell::from(r.namespace.clone()),
            status_cell(&r.status),
            Cell::from(r.extra.get("capacity").cloned().unwrap_or_default()),
            Cell::from(r.age.clone()),
        ],
    }
}

fn status_cell(status: &str) -> Cell<'static> {
    let color = match status {
        "Running" | "Active" | "Ready" | "Available" | "Bound" => Color::Green,
        "Pending" | "Progressing" | "ContainerCreating" => Color::Yellow,
        "Failed" | "Error" | "CrashLoopBackOff" | "NotReady" | "Lost" => Color::Red,
        "Terminating" | "Completed" | "Succeeded" => Color::Cyan,
        _ => Color::White,
    };
    Cell::from(status.to_string()).style(Style::default().fg(color))
}

fn render_yaml(f: &mut Frame, app: &App, area: Rect) {
    let title = format!("YAML - {}", app.selected_resource().as_ref().map(|r| r.name.as_str()).unwrap_or(""));
    let lines: Vec<Line> = app
        .yaml_content
        .lines()
        .skip(app.detail_scroll as usize)
        .map(|l| {
            // Syntax highlight YAML keys
            if let Some(colon_pos) = l.find(':') {
                let (key, rest) = l.split_at(colon_pos);
                Line::from(vec![
                    Span::styled(key.to_string(), Style::default().fg(Color::Cyan)),
                    Span::raw(rest.to_string()),
                ])
            } else if l.trim_start().starts_with('-') {
                Line::from(Span::styled(l.to_string(), Style::default().fg(Color::Yellow)))
            } else if l.trim_start().starts_with('#') {
                Line::from(Span::styled(l.to_string(), Style::default().fg(Color::DarkGray)))
            } else {
                Line::from(l.to_string())
            }
        })
        .collect();

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}

fn render_describe(f: &mut Frame, app: &App, area: Rect) {
    let title = format!(
        "Describe - {}",
        app.selected_resource().as_ref().map(|r| r.name.as_str()).unwrap_or("")
    );
    let lines: Vec<Line> = app
        .describe_content
        .lines()
        .skip(app.detail_scroll as usize)
        .map(|l| Line::from(l.to_string()))
        .collect();

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}

fn render_logs(f: &mut Frame, app: &App, area: Rect) {
    let title = format!(
        "Logs - {}",
        app.selected_resource().as_ref().map(|r| r.name.as_str()).unwrap_or("")
    );
    let lines: Vec<Line> = app
        .logs_content
        .iter()
        .skip(app.detail_scroll as usize)
        .map(|l| Line::from(l.as_str().to_string()))
        .collect();

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}

fn centered_popup(area: Rect, width_pct: u16, height_pct: u16) -> Rect {
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height_pct) / 2),
            Constraint::Percentage(height_pct),
            Constraint::Percentage((100 - height_pct) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - width_pct) / 2),
            Constraint::Percentage(width_pct),
            Constraint::Percentage((100 - width_pct) / 2),
        ])
        .split(v[1])[1]
}

fn render_namespace_popup(f: &mut Frame, app: &mut App) {
    let area = centered_popup(f.area(), 40, 60);
    f.render_widget(Clear, area);

    let items: Vec<ListItem> = app
        .namespaces
        .iter()
        .map(|ns| {
            let is_current = *ns == app.current_namespace;
            let marker = if is_current { "* " } else { "  " };
            let style = if is_current {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("{}{}", marker, ns)).style(style)
        })
        .collect();

    let mut all_items = vec![
        ListItem::new("  <all>").style(
            if app.current_namespace.is_empty() {
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            },
        ),
    ];
    all_items.extend(items);

    let list = List::new(all_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Select Namespace")
                .style(Style::default().bg(Color::Black)),
        )
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");

    f.render_stateful_widget(list, area, &mut app.namespace_state);
}

fn render_context_popup(f: &mut Frame, app: &mut App) {
    let area = centered_popup(f.area(), 50, 60);
    f.render_widget(Clear, area);

    let items: Vec<ListItem> = app
        .contexts
        .iter()
        .map(|ctx| {
            let is_current = *ctx == app.current_context;
            let marker = if is_current { "* " } else { "  " };
            let style = if is_current {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("{}{}", marker, ctx)).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Select Context")
                .style(Style::default().bg(Color::Black)),
        )
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");

    f.render_stateful_widget(list, area, &mut app.context_state);
}
