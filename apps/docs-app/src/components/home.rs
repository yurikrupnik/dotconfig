use leptos::prelude::*;
use leptos_router::components::A;

use crate::components::SearchBox;

#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <div class="home-page">
            <header class="hero">
                <h1>"Platform Operator Documentation"</h1>
                <p class="hero-subtitle">
                    "Kubernetes operator for managing applications via Helm, KCL, GitOps, and External Secrets"
                </p>
                <SearchBox/>
            </header>

            <section class="quick-start">
                <h2>"Quick Start"</h2>
                <div class="cards">
                    <QuickStartCard
                        title="Installation"
                        description="Deploy the Platform Operator to your cluster"
                        href="/docs/getting-started/installation"
                    />
                    <QuickStartCard
                        title="PlatformApp"
                        description="Install apps via Helm and KCL manifests"
                        href="/docs/crds/platform-app"
                    />
                    <QuickStartCard
                        title="GitOpsApp"
                        description="Manage FluxCD resources declaratively"
                        href="/docs/crds/gitops-app"
                    />
                    <QuickStartCard
                        title="ExternalSecrets"
                        description="Sync secrets from cloud providers"
                        href="/docs/crds/external-secret"
                    />
                </div>
            </section>

            <section class="features">
                <h2>"Features"</h2>
                <ul class="feature-list">
                    <li>
                        <strong>"Multi-source App Installation"</strong>
                        " - Deploy applications using Helm charts, KCL manifests, or both"
                    </li>
                    <li>
                        <strong>"GitOps Integration"</strong>
                        " - Seamless FluxCD GitRepository and Kustomization management"
                    </li>
                    <li>
                        <strong>"External Secrets"</strong>
                        " - Support for GCP, AWS, Azure, Vault, and 1Password"
                    </li>
                    <li>
                        <strong>"Crossplane Resources"</strong>
                        " - Manage infrastructure as Kubernetes resources"
                    </li>
                    <li>
                        <strong>"Drift Detection"</strong>
                        " - Automatic reconciliation with configurable intervals"
                    </li>
                </ul>
            </section>

            <section class="crd-overview">
                <h2>"Custom Resources"</h2>
                <table class="crd-table">
                    <thead>
                        <tr>
                            <th>"CRD"</th>
                            <th>"Short Name"</th>
                            <th>"Description"</th>
                        </tr>
                    </thead>
                    <tbody>
                        <tr>
                            <td><code>"PlatformApp"</code></td>
                            <td><code>"papp"</code></td>
                            <td>"Install apps via Helm/KCL"</td>
                        </tr>
                        <tr>
                            <td><code>"GitOpsApp"</code></td>
                            <td><code>"gapp"</code></td>
                            <td>"Manage FluxCD resources"</td>
                        </tr>
                        <tr>
                            <td><code>"CrossplaneResource"</code></td>
                            <td><code>"cpr"</code></td>
                            <td>"Crossplane composites"</td>
                        </tr>
                        <tr>
                            <td><code>"ExternalSecretConfig"</code></td>
                            <td><code>"esc"</code></td>
                            <td>"External Secrets Operator"</td>
                        </tr>
                    </tbody>
                </table>
            </section>
        </div>
    }
}

#[component]
fn QuickStartCard(title: &'static str, description: &'static str, href: &'static str) -> impl IntoView {
    view! {
        <A href=href class="card">
            <h3>{title}</h3>
            <p>{description}</p>
        </A>
    }
}
