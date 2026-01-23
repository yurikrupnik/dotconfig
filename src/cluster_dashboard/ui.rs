//! UI rendering for the cluster dashboard

use crate::cluster_dashboard::{
    data::{AppStatus, AuthProviderType, DashboardData, DependencyStatus, NodeStatus, ProviderStatus, Severity},
    Dashboard, View,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{
        Block, Borders, Cell, Clear, Gauge, List, ListItem, Paragraph, Row,
        Table, Tabs, Wrap,
    },
    Frame,
};

/// Render the entire dashboard
pub fn render(frame: &mut Frame, dashboard: &Dashboard, data: &DashboardData) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header/tabs
            Constraint::Min(0),    // Main content
            Constraint::Length(3), // Status bar
        ])
        .split(frame.area());

    render_header(frame, chunks[0], dashboard);
    render_main_content(frame, chunks[1], dashboard, data);
    render_status_bar(frame, chunks[2], dashboard, data);

    // Render search popup if active
    if dashboard.is_searching {
        render_search_popup(frame, dashboard);
    }
}

fn render_header(frame: &mut Frame, area: Rect, dashboard: &Dashboard) {
    let titles: Vec<Line> = [
        View::Overview,
        View::Nodes,
        View::Applications,
        View::Dependencies,
        View::Security,
        View::FinOps,
        View::PortForwards,
        View::Providers,
    ]
    .iter()
    .map(|v| {
        Line::from(format!(" {} {} ", v.key(), v.title()))
    })
    .collect();

    let selected = match dashboard.current_view {
        View::Overview => 0,
        View::Nodes => 1,
        View::Applications => 2,
        View::Dependencies => 3,
        View::Security => 4,
        View::FinOps => 5,
        View::PortForwards => 6,
        View::Providers | View::ProviderDetail => 7,
        _ => 0,
    };

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title(" Cluster Dashboard "))
        .select(selected)
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_widget(tabs, area);
}

fn render_main_content(frame: &mut Frame, area: Rect, dashboard: &Dashboard, data: &DashboardData) {
    match dashboard.current_view {
        View::Overview => render_overview(frame, area, data),
        View::Nodes => render_nodes(frame, area, dashboard, data),
        View::Applications => render_applications(frame, area, dashboard, data),
        View::Dependencies => render_dependencies(frame, area, dashboard, data),
        View::Security => render_security(frame, area, dashboard, data),
        View::FinOps => render_finops(frame, area, dashboard, data),
        View::PortForwards => render_port_forwards(frame, area, dashboard, data),
        View::Providers => render_providers(frame, area, dashboard, data),
        View::Help => render_help(frame, area),
        View::VulnerabilityDetail => render_vulnerability_detail(frame, area, dashboard, data),
        View::AppDetail => render_app_detail(frame, area, dashboard, data),
        View::ProviderDetail => render_provider_detail(frame, area, dashboard, data),
    }
}

fn render_overview(frame: &mut Frame, area: Rect, data: &DashboardData) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Min(0),
        ])
        .split(chunks[0]);

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Min(0),
        ])
        .split(chunks[1]);

    // Cluster Info
    let cluster_info = vec![
        format!("Cluster: {}", data.cluster_info.name),
        format!("Version: {}", data.cluster_info.version),
        format!("Provider: {}", data.cluster_info.provider),
        format!("Nodes: {}", data.cluster_info.node_count),
        format!("Namespaces: {}", data.cluster_info.namespace_count),
        format!("Pods: {}/{}", data.cluster_info.running_pods, data.cluster_info.pod_count),
    ];
    let cluster_block = Paragraph::new(cluster_info.join("\n"))
        .block(Block::default().borders(Borders::ALL).title(" Cluster "));
    frame.render_widget(cluster_block, left_chunks[0]);

    // Node Summary
    let ready_nodes = data.nodes.iter().filter(|n| n.status == NodeStatus::Ready).count();
    let node_info = vec![
        format!("Total: {}", data.nodes.len()),
        format!("Ready: {}", ready_nodes),
        format!("Not Ready: {}", data.nodes.len() - ready_nodes),
    ];
    let node_block = Paragraph::new(node_info.join("\n"))
        .block(Block::default().borders(Borders::ALL).title(" Nodes "));
    frame.render_widget(node_block, left_chunks[1]);

    // Applications Summary
    let healthy = data.applications.iter().filter(|a| a.status == AppStatus::Healthy).count();
    let degraded = data.applications.iter().filter(|a| a.status == AppStatus::Degraded).count();
    let unhealthy = data.applications.iter().filter(|a| a.status == AppStatus::Unhealthy).count();
    let app_info = vec![
        format!("Total: {}", data.applications.len()),
        format!("Healthy: {}", healthy),
        format!("Degraded: {}", degraded),
        format!("Unhealthy: {}", unhealthy),
    ];
    let app_block = Paragraph::new(app_info.join("\n"))
        .block(Block::default().borders(Borders::ALL).title(" Applications "));
    frame.render_widget(app_block, left_chunks[2]);

    // Security Score
    let security_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(" Security Score "))
        .gauge_style(
            Style::default().fg(if data.security.score >= 80 {
                Color::Green
            } else if data.security.score >= 60 {
                Color::Yellow
            } else {
                Color::Red
            }),
        )
        .percent(data.security.score as u16)
        .label(format!("{}%", data.security.score));
    frame.render_widget(security_gauge, right_chunks[0]);

    // FinOps Summary
    let finops_info = vec![
        format!("Monthly Cost: ${:.2}", data.finops.total_monthly_cost),
        format!("Hourly Cost: ${:.2}", data.finops.total_hourly_cost),
        format!("Savings Opportunities: ${:.2}", data.finops.savings_opportunities),
    ];
    let finops_block = Paragraph::new(finops_info.join("\n"))
        .block(Block::default().borders(Borders::ALL).title(" FinOps "));
    frame.render_widget(finops_block, right_chunks[1]);

    // Dependencies
    let available = data.dependencies.iter().filter(|d| d.status == DependencyStatus::Available).count();
    let deps_info = vec![
        format!("Operators Checked: {}", data.dependencies.len()),
        format!("Available: {}", available),
        format!("Missing: {}", data.dependencies.len() - available),
    ];
    let deps_block = Paragraph::new(deps_info.join("\n"))
        .block(Block::default().borders(Borders::ALL).title(" Dependencies "));
    frame.render_widget(deps_block, right_chunks[2]);
}

fn render_nodes(frame: &mut Frame, area: Rect, dashboard: &Dashboard, data: &DashboardData) {
    let header = Row::new(vec![
        Cell::from("Name"),
        Cell::from("Status"),
        Cell::from("CPU"),
        Cell::from("Memory"),
        Cell::from("Instance Type"),
        Cell::from("Zone"),
    ])
    .style(Style::default().add_modifier(Modifier::BOLD))
    .height(1);

    let rows: Vec<Row> = data
        .nodes
        .iter()
        .enumerate()
        .map(|(i, node)| {
            let style = if i == dashboard.selected_index {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };

            let status_style = match node.status {
                NodeStatus::Ready => Style::default().fg(Color::Green),
                NodeStatus::NotReady => Style::default().fg(Color::Red),
                NodeStatus::Unknown => Style::default().fg(Color::Yellow),
            };

            Row::new(vec![
                Cell::from(node.name.clone()),
                Cell::from(format!("{:?}", node.status)).style(status_style),
                Cell::from(node.cpu_allocatable.clone()),
                Cell::from(node.memory_allocatable.clone()),
                Cell::from(node.instance_type.clone()),
                Cell::from(node.zone.clone()),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(25),
            Constraint::Percentage(10),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(20),
            Constraint::Percentage(15),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(" Nodes "));

    frame.render_widget(table, area);
}

fn render_applications(frame: &mut Frame, area: Rect, dashboard: &Dashboard, data: &DashboardData) {
    let header = Row::new(vec![
        Cell::from("Name"),
        Cell::from("Namespace"),
        Cell::from("Status"),
        Cell::from("Replicas"),
        Cell::from("Image"),
    ])
    .style(Style::default().add_modifier(Modifier::BOLD))
    .height(1);

    let rows: Vec<Row> = data
        .applications
        .iter()
        .enumerate()
        .map(|(i, app)| {
            let style = if i == dashboard.selected_index {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };

            let status_style = match app.status {
                AppStatus::Healthy => Style::default().fg(Color::Green),
                AppStatus::Degraded => Style::default().fg(Color::Yellow),
                AppStatus::Unhealthy => Style::default().fg(Color::Red),
                AppStatus::Stopped => Style::default().fg(Color::Gray),
                AppStatus::Unknown => Style::default().fg(Color::Yellow),
            };

            Row::new(vec![
                Cell::from(app.name.clone()),
                Cell::from(app.namespace.clone()),
                Cell::from(format!("{:?}", app.status)).style(status_style),
                Cell::from(format!("{}/{}", app.ready_replicas, app.replicas)),
                Cell::from(truncate_string(&app.image, 40)),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(20),
            Constraint::Percentage(15),
            Constraint::Percentage(12),
            Constraint::Percentage(10),
            Constraint::Percentage(43),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(" Applications "));

    frame.render_widget(table, area);
}

fn render_dependencies(frame: &mut Frame, area: Rect, dashboard: &Dashboard, data: &DashboardData) {
    let header = Row::new(vec![
        Cell::from("Name"),
        Cell::from("Kind"),
        Cell::from("API Group"),
        Cell::from("Status"),
    ])
    .style(Style::default().add_modifier(Modifier::BOLD))
    .height(1);

    let rows: Vec<Row> = data
        .dependencies
        .iter()
        .enumerate()
        .map(|(i, dep)| {
            let style = if i == dashboard.selected_index {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };

            let status_style = match dep.status {
                DependencyStatus::Available => Style::default().fg(Color::Green),
                DependencyStatus::Missing => Style::default().fg(Color::Red),
                DependencyStatus::Degraded => Style::default().fg(Color::Yellow),
                DependencyStatus::Unknown => Style::default().fg(Color::Gray),
            };

            Row::new(vec![
                Cell::from(dep.name.clone()),
                Cell::from(dep.kind.clone()),
                Cell::from(dep.group.clone()),
                Cell::from(format!("{:?}", dep.status)).style(status_style),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(25),
            Constraint::Percentage(15),
            Constraint::Percentage(40),
            Constraint::Percentage(20),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(" Dependencies / Operators "));

    frame.render_widget(table, area);
}

fn render_security(frame: &mut Frame, area: Rect, dashboard: &Dashboard, data: &DashboardData) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(0)])
        .split(area);

    // Security score and summary
    let score_area = chunks[0];
    let score_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(score_area);

    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(" Score "))
        .gauge_style(
            Style::default().fg(if data.security.score >= 80 {
                Color::Green
            } else if data.security.score >= 60 {
                Color::Yellow
            } else {
                Color::Red
            }),
        )
        .percent(data.security.score as u16)
        .label(format!("{}%", data.security.score));
    frame.render_widget(gauge, score_chunks[0]);

    let summary = format!(
        "Issues: {} | Critical: {} | High: {} | Medium: {}",
        data.security.issues.len(),
        data.security.issues.iter().filter(|i| i.severity == Severity::Critical).count(),
        data.security.issues.iter().filter(|i| i.severity == Severity::High).count(),
        data.security.issues.iter().filter(|i| i.severity == Severity::Medium).count(),
    );
    let summary_block = Paragraph::new(summary)
        .block(Block::default().borders(Borders::ALL).title(" Summary "));
    frame.render_widget(summary_block, score_chunks[1]);

    // Issues list
    let header = Row::new(vec![
        Cell::from("Severity"),
        Cell::from("Category"),
        Cell::from("Resource"),
        Cell::from("Message"),
    ])
    .style(Style::default().add_modifier(Modifier::BOLD))
    .height(1);

    let rows: Vec<Row> = data
        .security
        .issues
        .iter()
        .enumerate()
        .map(|(i, issue)| {
            let style = if i == dashboard.selected_index {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };

            let severity_style = match issue.severity {
                Severity::Critical => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                Severity::High => Style::default().fg(Color::Red),
                Severity::Medium => Style::default().fg(Color::Yellow),
                Severity::Warning => Style::default().fg(Color::LightYellow),
                Severity::Low => Style::default().fg(Color::Cyan),
                Severity::Info => Style::default().fg(Color::Gray),
            };

            Row::new(vec![
                Cell::from(format!("{:?}", issue.severity)).style(severity_style),
                Cell::from(format!("{:?}", issue.category)),
                Cell::from(issue.resource.clone()),
                Cell::from(truncate_string(&issue.message, 40)),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(12),
            Constraint::Percentage(15),
            Constraint::Percentage(25),
            Constraint::Percentage(48),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(" Security Issues "));

    frame.render_widget(table, chunks[1]);
}

fn render_finops(frame: &mut Frame, area: Rect, dashboard: &Dashboard, data: &DashboardData) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Left: Cost by namespace
    let header = Row::new(vec![
        Cell::from("Namespace"),
        Cell::from("Hourly"),
        Cell::from("Monthly"),
    ])
    .style(Style::default().add_modifier(Modifier::BOLD))
    .height(1);

    let rows: Vec<Row> = data
        .finops
        .cost_by_namespace
        .iter()
        .enumerate()
        .map(|(i, ns_cost)| {
            let style = if i == dashboard.selected_index {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(ns_cost.namespace.clone()),
                Cell::from(format!("${:.2}", ns_cost.hourly_cost)),
                Cell::from(format!("${:.2}", ns_cost.monthly_cost)),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(50),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(" Cost by Namespace "));

    frame.render_widget(table, chunks[0]);

    // Right: Recommendations
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(0)])
        .split(chunks[1]);

    let summary = vec![
        format!("Total Monthly: ${:.2}", data.finops.total_monthly_cost),
        format!("Projected: ${:.2}", data.finops.projected_monthly_cost),
        format!("Savings Available: ${:.2}", data.finops.savings_opportunities),
    ];
    let summary_block = Paragraph::new(summary.join("\n"))
        .block(Block::default().borders(Borders::ALL).title(" Summary "));
    frame.render_widget(summary_block, right_chunks[0]);

    let items: Vec<ListItem> = data
        .finops
        .recommendations
        .iter()
        .map(|rec| {
            ListItem::new(format!(
                "[${:.0}] {:?}: {}",
                rec.potential_savings, rec.category, rec.description
            ))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Recommendations "));
    frame.render_widget(list, right_chunks[1]);
}

fn render_port_forwards(frame: &mut Frame, area: Rect, dashboard: &Dashboard, data: &DashboardData) {
    if data.port_forwards.is_empty() {
        let message = Paragraph::new("No port forwards configured.\n\nPress 'a' to add a port forward.")
            .block(Block::default().borders(Borders::ALL).title(" Port Forwards "));
        frame.render_widget(message, area);
        return;
    }

    let header = Row::new(vec![
        Cell::from("Name"),
        Cell::from("Namespace"),
        Cell::from("Target"),
        Cell::from("Local:Remote"),
        Cell::from("Status"),
    ])
    .style(Style::default().add_modifier(Modifier::BOLD))
    .height(1);

    let rows: Vec<Row> = data
        .port_forwards
        .iter()
        .enumerate()
        .map(|(i, pf)| {
            let style = if i == dashboard.selected_index {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };

            let status_style = if pf.active {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Gray)
            };

            Row::new(vec![
                Cell::from(pf.name.clone()),
                Cell::from(pf.namespace.clone()),
                Cell::from(format!("{}/{}", pf.target_type, pf.target_name)),
                Cell::from(format!("{}:{}", pf.local_port, pf.remote_port)),
                Cell::from(if pf.active { "Active" } else { "Stopped" }).style(status_style),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(20),
            Constraint::Percentage(15),
            Constraint::Percentage(30),
            Constraint::Percentage(15),
            Constraint::Percentage(20),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(" Port Forwards [Enter to toggle] "));

    frame.render_widget(table, area);
}

fn render_help(frame: &mut Frame, area: Rect) {
    let help_text = vec![
        "Keyboard Shortcuts:",
        "",
        "  Navigation:",
        "    1-8     Switch between views",
        "    j/↓     Move down",
        "    k/↑     Move up",
        "    PgUp    Page up",
        "    PgDn    Page down",
        "    Home    Go to top",
        "    Esc     Go back / Cancel",
        "",
        "  Actions:",
        "    Enter   Select / Toggle",
        "    r       Refresh data",
        "    /       Search",
        "    ?       Show this help",
        "    q       Quit",
        "",
        "  Views:",
        "    1 Overview     - Cluster summary",
        "    2 Nodes        - Node status and resources",
        "    3 Applications - Deployments and their status",
        "    4 Dependencies - Operators and CRDs",
        "    5 Security     - Vulnerabilities and issues",
        "    6 FinOps       - Cost analysis",
        "    7 Port Forwards - Local development tunnels",
        "    8 Providers    - Crossplane provider configs & auth",
    ];

    let paragraph = Paragraph::new(help_text.join("\n"))
        .block(Block::default().borders(Borders::ALL).title(" Help "))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn render_vulnerability_detail(
    frame: &mut Frame,
    area: Rect,
    dashboard: &Dashboard,
    data: &DashboardData,
) {
    if dashboard.selected_index >= data.vulnerabilities.len() {
        let message = Paragraph::new("No vulnerability selected")
            .block(Block::default().borders(Borders::ALL).title(" Vulnerability Detail "));
        frame.render_widget(message, area);
        return;
    }

    let vuln = &data.vulnerabilities[dashboard.selected_index];
    let details = vec![
        format!("ID: {}", vuln.id),
        format!("Severity: {:?}", vuln.severity),
        format!("Package: {}", vuln.package),
        format!("Installed: {}", vuln.installed_version),
        format!("Fixed: {}", vuln.fixed_version.as_deref().unwrap_or("N/A")),
        format!("Image: {}", vuln.image),
        format!("Resource: {}", vuln.resource),
        String::new(),
        "Description:".to_string(),
        vuln.description.clone(),
    ];

    let paragraph = Paragraph::new(details.join("\n"))
        .block(Block::default().borders(Borders::ALL).title(" Vulnerability Detail "))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn render_app_detail(
    frame: &mut Frame,
    area: Rect,
    dashboard: &Dashboard,
    data: &DashboardData,
) {
    if dashboard.selected_index >= data.applications.len() {
        let message = Paragraph::new("No application selected")
            .block(Block::default().borders(Borders::ALL).title(" Application Detail "));
        frame.render_widget(message, area);
        return;
    }

    let app = &data.applications[dashboard.selected_index];
    let details = vec![
        format!("Name: {}", app.name),
        format!("Namespace: {}", app.namespace),
        format!("Kind: {}", app.kind),
        format!("Status: {:?}", app.status),
        format!("Replicas: {}/{}", app.ready_replicas, app.replicas),
        format!("Image: {}", app.image),
        format!("Restarts: {}", app.restart_count),
        String::new(),
        "Labels:".to_string(),
    ];

    let mut all_lines: Vec<String> = details;
    for (k, v) in &app.labels {
        all_lines.push(format!("  {}: {}", k, v));
    }

    let paragraph = Paragraph::new(all_lines.join("\n"))
        .block(Block::default().borders(Borders::ALL).title(" Application Detail "))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn render_providers(frame: &mut Frame, area: Rect, dashboard: &Dashboard, data: &DashboardData) {
    let has_configs = !data.provider_configs.is_empty();
    let has_auth = !data.auth_providers.is_empty();

    if !has_configs && !has_auth {
        let message = Paragraph::new(
            "No Crossplane Providers or Auth Providers found.\n\n\
            This view displays:\n\
            • Crossplane ProviderConfigs (AWS, GCP, Azure, etc.)\n\
            • Crossplane Provider packages\n\
            • External Secrets SecretStores & ClusterSecretStores\n\
            • ServiceAccounts with IRSA/Workload Identity\n\n\
            Install Crossplane or External Secrets Operator to see providers here."
        )
        .block(Block::default().borders(Borders::ALL).title(" Providers & Auth "));
        frame.render_widget(message, area);
        return;
    }

    // Main layout: summary + two tables (configs and auth)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),   // Summary
            Constraint::Percentage(45), // Provider Configs
            Constraint::Percentage(45), // Auth Providers
        ])
        .split(area);

    // Summary section
    let config_healthy = data.provider_configs.iter().filter(|p| p.status == ProviderStatus::Healthy).count();
    let config_error = data.provider_configs.iter().filter(|p| p.status == ProviderStatus::Error).count();
    let auth_healthy = data.auth_providers.iter().filter(|p| p.status == ProviderStatus::Healthy).count();
    let auth_error = data.auth_providers.iter().filter(|p| p.status == ProviderStatus::Error).count();

    let summary = format!(
        "Provider Configs: {} ({}✓ {}✗) | Auth Providers: {} ({}✓ {}✗) | [Tab to switch sections]",
        data.provider_configs.len(), config_healthy, config_error,
        data.auth_providers.len(), auth_healthy, auth_error,
    );
    let summary_block = Paragraph::new(summary)
        .block(Block::default().borders(Borders::ALL).title(" Summary "));
    frame.render_widget(summary_block, chunks[0]);

    // Provider Configs table
    let config_header = Row::new(vec![
        Cell::from("Name"),
        Cell::from("Type"),
        Cell::from("Status"),
        Cell::from("Credentials"),
        Cell::from("Secret Ref"),
        Cell::from("Resources"),
    ])
    .style(Style::default().add_modifier(Modifier::BOLD))
    .height(1);

    let config_rows: Vec<Row> = data
        .provider_configs
        .iter()
        .enumerate()
        .map(|(i, provider)| {
            let style = if i == dashboard.selected_index && dashboard.selected_index < data.provider_configs.len() {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };

            let status_style = match provider.status {
                ProviderStatus::Healthy => Style::default().fg(Color::Green),
                ProviderStatus::Degraded => Style::default().fg(Color::Yellow),
                ProviderStatus::Error => Style::default().fg(Color::Red),
                ProviderStatus::Unknown => Style::default().fg(Color::Gray),
            };

            let status_text = match provider.status {
                ProviderStatus::Healthy => "✓",
                ProviderStatus::Degraded => "◐",
                ProviderStatus::Error => "✗",
                ProviderStatus::Unknown => "?",
            };

            Row::new(vec![
                Cell::from(truncate_string(&provider.name, 20)),
                Cell::from(provider.provider_type.to_string()),
                Cell::from(status_text).style(status_style),
                Cell::from(truncate_string(&provider.credentials_source, 15)),
                Cell::from(provider.secret_ref.clone().unwrap_or_else(|| "-".to_string())),
                Cell::from(provider.associated_resources.to_string()),
            ])
            .style(style)
        })
        .collect();

    let config_table = Table::new(
        config_rows,
        [
            Constraint::Percentage(20),
            Constraint::Percentage(12),
            Constraint::Percentage(8),
            Constraint::Percentage(15),
            Constraint::Percentage(30),
            Constraint::Percentage(15),
        ],
    )
    .header(config_header)
    .block(Block::default().borders(Borders::ALL).title(" Crossplane ProviderConfigs "));

    frame.render_widget(config_table, chunks[1]);

    // Auth Providers table
    let auth_header = Row::new(vec![
        Cell::from("Name"),
        Cell::from("Type"),
        Cell::from("Status"),
        Cell::from("Backend"),
        Cell::from("Namespace"),
        Cell::from("Service Account"),
    ])
    .style(Style::default().add_modifier(Modifier::BOLD))
    .height(1);

    let auth_rows: Vec<Row> = data
        .auth_providers
        .iter()
        .enumerate()
        .map(|(i, auth)| {
            let adjusted_index = i + data.provider_configs.len();
            let style = if adjusted_index == dashboard.selected_index {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };

            let status_style = match auth.status {
                ProviderStatus::Healthy => Style::default().fg(Color::Green),
                ProviderStatus::Degraded => Style::default().fg(Color::Yellow),
                ProviderStatus::Error => Style::default().fg(Color::Red),
                ProviderStatus::Unknown => Style::default().fg(Color::Gray),
            };

            let status_text = match auth.status {
                ProviderStatus::Healthy => "✓",
                ProviderStatus::Degraded => "◐",
                ProviderStatus::Error => "✗",
                ProviderStatus::Unknown => "?",
            };

            let type_color = match auth.auth_type {
                AuthProviderType::CrossplaneProvider => Color::Cyan,
                AuthProviderType::SecretStore | AuthProviderType::ClusterSecretStore => Color::Magenta,
                AuthProviderType::AwsIrsa => Color::Yellow,
                AuthProviderType::GcpWorkloadIdentity => Color::Blue,
                AuthProviderType::AzureWorkloadIdentity => Color::LightBlue,
                AuthProviderType::Vault => Color::LightGreen,
                AuthProviderType::Other(_) => Color::Gray,
            };

            Row::new(vec![
                Cell::from(truncate_string(&auth.name, 20)),
                Cell::from(auth.auth_type.to_string()).style(Style::default().fg(type_color)),
                Cell::from(status_text).style(status_style),
                Cell::from(truncate_string(&auth.backend, 25)),
                Cell::from(auth.namespace.clone().unwrap_or_else(|| "(cluster)".to_string())),
                Cell::from(auth.service_account.clone().unwrap_or_else(|| "-".to_string())),
            ])
            .style(style)
        })
        .collect();

    let auth_table = Table::new(
        auth_rows,
        [
            Constraint::Percentage(18),
            Constraint::Percentage(18),
            Constraint::Percentage(8),
            Constraint::Percentage(24),
            Constraint::Percentage(16),
            Constraint::Percentage(16),
        ],
    )
    .header(auth_header)
    .block(Block::default().borders(Borders::ALL).title(" Auth Providers (Crossplane Pkgs, SecretStores, Workload Identity) "));

    frame.render_widget(auth_table, chunks[2]);
}

fn render_provider_detail(
    frame: &mut Frame,
    area: Rect,
    dashboard: &Dashboard,
    data: &DashboardData,
) {
    let total_items = data.provider_configs.len() + data.auth_providers.len();

    if dashboard.selected_index >= total_items {
        let message = Paragraph::new("No provider selected. Press Esc to go back.")
            .block(Block::default().borders(Borders::ALL).title(" Provider Detail "));
        frame.render_widget(message, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(14), Constraint::Min(0)])
        .split(area);

    // Determine if we're showing a ProviderConfig or an AuthProvider
    if dashboard.selected_index < data.provider_configs.len() {
        // Show ProviderConfig detail
        let provider = &data.provider_configs[dashboard.selected_index];

        let status_indicator = match provider.status {
            ProviderStatus::Healthy => "● Healthy",
            ProviderStatus::Degraded => "◐ Degraded",
            ProviderStatus::Error => "○ Error",
            ProviderStatus::Unknown => "? Unknown",
        };

        let details = vec![
            format!("Name: {}", provider.name),
            format!("Type: ProviderConfig"),
            format!("Provider: {}", provider.provider_type),
            format!("Status: {}", status_indicator),
            format!("Credentials Source: {}", provider.credentials_source),
            format!("Secret Reference: {}", provider.secret_ref.as_deref().unwrap_or("N/A")),
            format!("Associated Resources: {}", provider.associated_resources),
            format!("Last Sync: {}", provider.last_sync.as_deref().unwrap_or("N/A")),
            String::new(),
            format!("Message: {}", provider.message.as_deref().unwrap_or("None")),
        ];

        let info_block = Paragraph::new(details.join("\n"))
            .block(Block::default().borders(Borders::ALL).title(" ProviderConfig Detail "))
            .wrap(Wrap { trim: false });
        frame.render_widget(info_block, chunks[0]);

        let cmd1 = format!("  kubectl describe providerconfig {} -o yaml", provider.name);
        let cmd2 = format!("  kubectl get providerconfig {} -o yaml", provider.name);
        let actions = vec![
            "kubectl Commands:".to_string(),
            String::new(),
            cmd1,
            cmd2,
            String::new(),
            "Navigation:".to_string(),
            "  Esc - Return to providers list".to_string(),
            "  r   - Refresh data".to_string(),
        ];

        let actions_block = Paragraph::new(actions.join("\n"))
            .block(Block::default().borders(Borders::ALL).title(" Actions "))
            .wrap(Wrap { trim: false });
        frame.render_widget(actions_block, chunks[1]);
    } else {
        // Show AuthProvider detail
        let auth_index = dashboard.selected_index - data.provider_configs.len();
        let auth = &data.auth_providers[auth_index];

        let status_indicator = match auth.status {
            ProviderStatus::Healthy => "● Healthy",
            ProviderStatus::Degraded => "◐ Degraded",
            ProviderStatus::Error => "○ Error",
            ProviderStatus::Unknown => "? Unknown",
        };

        let details = vec![
            format!("Name: {}", auth.name),
            format!("Type: {}", auth.auth_type),
            format!("Status: {}", status_indicator),
            format!("Backend: {}", auth.backend),
            format!("Namespace: {}", auth.namespace.as_deref().unwrap_or("(cluster-scoped)")),
            format!("Secret Reference: {}", auth.secret_ref.as_deref().unwrap_or("N/A")),
            format!("Service Account: {}", auth.service_account.as_deref().unwrap_or("N/A")),
            format!("Last Sync: {}", auth.last_sync.as_deref().unwrap_or("N/A")),
            String::new(),
            format!("Message: {}", auth.message.as_deref().unwrap_or("None")),
        ];

        let info_block = Paragraph::new(details.join("\n"))
            .block(Block::default().borders(Borders::ALL).title(" Auth Provider Detail "))
            .wrap(Wrap { trim: false });
        frame.render_widget(info_block, chunks[0]);

        let kubectl_cmd = match auth.auth_type {
            AuthProviderType::CrossplaneProvider => {
                format!("  kubectl describe provider.pkg.crossplane.io {}", auth.name)
            }
            AuthProviderType::ClusterSecretStore => {
                format!("  kubectl describe clustersecretstore {}", auth.name)
            }
            AuthProviderType::SecretStore => {
                let ns = auth.namespace.as_deref().unwrap_or("default");
                format!("  kubectl describe secretstore {} -n {}", auth.name, ns)
            }
            AuthProviderType::AwsIrsa | AuthProviderType::GcpWorkloadIdentity | AuthProviderType::AzureWorkloadIdentity => {
                let ns = auth.namespace.as_deref().unwrap_or("default");
                format!("  kubectl describe serviceaccount {} -n {}", auth.name, ns)
            }
            _ => format!("  kubectl get {} -o yaml", auth.name),
        };

        let actions = vec![
            "kubectl Commands:".to_string(),
            String::new(),
            kubectl_cmd,
            String::new(),
            "Navigation:".to_string(),
            "  Esc - Return to providers list".to_string(),
            "  r   - Refresh data".to_string(),
        ];

        let actions_block = Paragraph::new(actions.join("\n"))
            .block(Block::default().borders(Borders::ALL).title(" Actions "))
            .wrap(Wrap { trim: false });
        frame.render_widget(actions_block, chunks[1]);
    }
}

fn render_status_bar(frame: &mut Frame, area: Rect, dashboard: &Dashboard, data: &DashboardData) {
    let status = if let Some(ref msg) = dashboard.status_message {
        msg.clone()
    } else {
        format!(
            "Nodes: {} | Apps: {} | Pods: {} | Press ? for help, q to quit",
            data.cluster_info.node_count,
            data.applications.len(),
            data.cluster_info.pod_count,
        )
    };

    let status_bar = Paragraph::new(status)
        .style(Style::default().bg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(status_bar, area);
}

fn render_search_popup(frame: &mut Frame, dashboard: &Dashboard) {
    let area = centered_rect(60, 3, frame.area());
    frame.render_widget(Clear, area);

    let search = Paragraph::new(format!("Search: {}_", dashboard.search_query))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Search (Esc to cancel) "),
        );
    frame.render_widget(search, area);
}

fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((r.height - height) / 2),
            Constraint::Length(height),
            Constraint::Length((r.height - height) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
