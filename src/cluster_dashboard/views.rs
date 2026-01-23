//! View definitions for the cluster dashboard

/// Tab/view identifiers
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Overview,
    Nodes,
    Applications,
    Dependencies,
    Security,
    FinOps,
    PortForwards,
    Providers,
    Help,
    VulnerabilityDetail,
    AppDetail,
    ProviderDetail,
}

impl View {
    /// Get the display title for this view
    pub fn title(&self) -> &'static str {
        match self {
            View::Overview => "Overview",
            View::Nodes => "Nodes",
            View::Applications => "Applications",
            View::Dependencies => "Dependencies",
            View::Security => "Security",
            View::FinOps => "FinOps",
            View::PortForwards => "Port Forwards",
            View::Providers => "Providers",
            View::Help => "Help",
            View::VulnerabilityDetail => "Vulnerability Detail",
            View::AppDetail => "App Detail",
            View::ProviderDetail => "Provider Detail",
        }
    }

    /// Get the keyboard shortcut key for this view
    pub fn key(&self) -> char {
        match self {
            View::Overview => '1',
            View::Nodes => '2',
            View::Applications => '3',
            View::Dependencies => '4',
            View::Security => '5',
            View::FinOps => '6',
            View::PortForwards => '7',
            View::Providers => '8',
            View::Help => '?',
            _ => ' ',
        }
    }

    /// Get all navigable views (shown in tab bar)
    pub fn navigable_views() -> &'static [View] {
        &[
            View::Overview,
            View::Nodes,
            View::Applications,
            View::Dependencies,
            View::Security,
            View::FinOps,
            View::PortForwards,
            View::Providers,
        ]
    }

    /// Check if this is a detail view
    pub fn is_detail_view(&self) -> bool {
        matches!(self, View::VulnerabilityDetail | View::AppDetail | View::ProviderDetail)
    }
}
