// CNCF Platform Stack - Dagger Module
//
// This module provides CI/CD functions for the CNCF observability and platform stack.
// It can generate Kubernetes manifests using KCL, validate configurations,
// and deploy to clusters.
//
// Components managed:
// - Prometheus, Grafana, Jaeger, OpenTelemetry, Loki (Observability)
// - KEDA, FluxCD, Dapr (Platform)

package main

import (
	"context"
	"dagger/observability/internal/dagger"
	"fmt"
)

type Observability struct{}

// KCL container with the KCL CLI installed
func (m *Observability) KclContainer() *dagger.Container {
	return dag.Container().
		From("kcllang/kcl:latest").
		WithWorkdir("/src")
}

// Kubectl container for Kubernetes operations
func (m *Observability) KubectlContainer(
	// Optional kubeconfig file
	// +optional
	kubeconfig *dagger.File,
) *dagger.Container {
	ctr := dag.Container().
		From("bitnami/kubectl:latest").
		WithWorkdir("/manifests")

	if kubeconfig != nil {
		ctr = ctr.WithMountedFile("/root/.kube/config", kubeconfig)
	}

	return ctr
}

// Generate Kubernetes manifests from KCL source
func (m *Observability) Generate(
	ctx context.Context,
	// Source directory containing KCL files
	source *dagger.Directory,
	// Enable Prometheus
	// +optional
	// +default=true
	prometheus bool,
	// Enable Grafana
	// +optional
	// +default=true
	grafana bool,
	// Enable Jaeger
	// +optional
	// +default=true
	jaeger bool,
	// Enable OpenTelemetry Collector
	// +optional
	// +default=true
	otel bool,
	// Enable Loki
	// +optional
	// +default=true
	loki bool,
	// Enable KEDA
	// +optional
	// +default=true
	keda bool,
	// Enable FluxCD
	// +optional
	// +default=true
	fluxcd bool,
	// Enable Dapr
	// +optional
	// +default=true
	dapr bool,
) (string, error) {
	args := []string{"run", "main.k"}

	// Add component flags
	if !prometheus {
		args = append(args, "-D", "prometheus=false")
	}
	if !grafana {
		args = append(args, "-D", "grafana=false")
	}
	if !jaeger {
		args = append(args, "-D", "jaeger=false")
	}
	if !otel {
		args = append(args, "-D", "otel=false")
	}
	if !loki {
		args = append(args, "-D", "loki=false")
	}
	if !keda {
		args = append(args, "-D", "keda=false")
	}
	if !fluxcd {
		args = append(args, "-D", "fluxcd=false")
	}
	if !dapr {
		args = append(args, "-D", "dapr=false")
	}

	return m.KclContainer().
		WithMountedDirectory("/src", source).
		WithExec([]string{"kcl", "mod", "update"}).
		WithExec(args).
		Stdout(ctx)
}

// Generate and export manifests to a file
func (m *Observability) GenerateFile(
	ctx context.Context,
	// Source directory containing KCL files
	source *dagger.Directory,
	// Output filename
	// +optional
	// +default="platform-stack.yaml"
	output string,
) *dagger.File {
	if output == "" {
		output = "platform-stack.yaml"
	}

	return m.KclContainer().
		WithMountedDirectory("/src", source).
		WithExec([]string{"kcl", "mod", "update"}).
		WithExec([]string{"sh", "-c", fmt.Sprintf("kcl run main.k > /tmp/%s", output)}).
		File(fmt.Sprintf("/tmp/%s", output))
}

// Validate KCL configuration
func (m *Observability) Validate(
	ctx context.Context,
	// Source directory containing KCL files
	source *dagger.Directory,
) (string, error) {
	return m.KclContainer().
		WithMountedDirectory("/src", source).
		WithExec([]string{"kcl", "mod", "update"}).
		WithExec([]string{"kcl", "run", "main.k", "--output", "/dev/null"}).
		WithExec([]string{"echo", "Validation successful!"}).
		Stdout(ctx)
}

// Run KCL tests
func (m *Observability) Test(
	ctx context.Context,
	// Source directory containing KCL files
	source *dagger.Directory,
) (string, error) {
	return m.KclContainer().
		WithMountedDirectory("/src", source).
		WithExec([]string{"kcl", "mod", "update"}).
		WithExec([]string{"kcl", "test", "..."}).
		Stdout(ctx)
}

// Lint KCL files
func (m *Observability) Lint(
	ctx context.Context,
	// Source directory containing KCL files
	source *dagger.Directory,
) (string, error) {
	return m.KclContainer().
		WithMountedDirectory("/src", source).
		WithExec([]string{"kcl", "lint", "."}).
		Stdout(ctx)
}

// Format KCL files
func (m *Observability) Format(
	ctx context.Context,
	// Source directory containing KCL files
	source *dagger.Directory,
) *dagger.Directory {
	return m.KclContainer().
		WithMountedDirectory("/src", source).
		WithExec([]string{"kcl", "fmt", "."}).
		Directory("/src")
}

// Dry-run apply to Kubernetes cluster
func (m *Observability) DryRun(
	ctx context.Context,
	// Source directory containing KCL files
	source *dagger.Directory,
	// Kubeconfig file for cluster access
	kubeconfig *dagger.File,
) (string, error) {
	manifests := m.GenerateFile(ctx, source, "manifests.yaml")

	return m.KubectlContainer(kubeconfig).
		WithMountedFile("/manifests/manifests.yaml", manifests).
		WithExec([]string{"kubectl", "apply", "-f", "manifests.yaml", "--dry-run=client"}).
		Stdout(ctx)
}

// Apply manifests to Kubernetes cluster
func (m *Observability) Apply(
	ctx context.Context,
	// Source directory containing KCL files
	source *dagger.Directory,
	// Kubeconfig file for cluster access
	kubeconfig *dagger.File,
) (string, error) {
	manifests := m.GenerateFile(ctx, source, "manifests.yaml")

	return m.KubectlContainer(kubeconfig).
		WithMountedFile("/manifests/manifests.yaml", manifests).
		WithExec([]string{"kubectl", "apply", "-f", "manifests.yaml"}).
		Stdout(ctx)
}

// Delete resources from Kubernetes cluster
func (m *Observability) Delete(
	ctx context.Context,
	// Source directory containing KCL files
	source *dagger.Directory,
	// Kubeconfig file for cluster access
	kubeconfig *dagger.File,
) (string, error) {
	manifests := m.GenerateFile(ctx, source, "manifests.yaml")

	return m.KubectlContainer(kubeconfig).
		WithMountedFile("/manifests/manifests.yaml", manifests).
		WithExec([]string{"kubectl", "delete", "-f", "manifests.yaml", "--ignore-not-found"}).
		Stdout(ctx)
}

// CI pipeline: lint, test, validate, and generate
func (m *Observability) Ci(
	ctx context.Context,
	// Source directory containing KCL files
	source *dagger.Directory,
) (string, error) {
	ctr := m.KclContainer().
		WithMountedDirectory("/src", source).
		WithExec([]string{"kcl", "mod", "update"}).
		WithExec([]string{"echo", "=== Linting ==="}).
		WithExec([]string{"kcl", "lint", "."}).
		WithExec([]string{"echo", "=== Testing ==="}).
		WithExec([]string{"kcl", "test", "..."}).
		WithExec([]string{"echo", "=== Validating ==="}).
		WithExec([]string{"kcl", "run", "main.k", "--output", "/dev/null"}).
		WithExec([]string{"echo", "=== CI Pipeline Complete ==="})

	return ctr.Stdout(ctx)
}

// CD pipeline: generate and apply to cluster
func (m *Observability) Cd(
	ctx context.Context,
	// Source directory containing KCL files
	source *dagger.Directory,
	// Kubeconfig file for cluster access
	kubeconfig *dagger.File,
	// Perform dry-run only
	// +optional
	// +default=false
	dryRun bool,
) (string, error) {
	manifests := m.GenerateFile(ctx, source, "manifests.yaml")

	applyCmd := []string{"kubectl", "apply", "-f", "manifests.yaml"}
	if dryRun {
		applyCmd = append(applyCmd, "--dry-run=client")
	}

	return m.KubectlContainer(kubeconfig).
		WithMountedFile("/manifests/manifests.yaml", manifests).
		WithExec([]string{"echo", "=== Applying to cluster ==="}).
		WithExec(applyCmd).
		WithExec([]string{"echo", "=== CD Pipeline Complete ==="}).
		Stdout(ctx)
}

// Generate only observability components (Prometheus, Grafana, Jaeger, OTel, Loki)
func (m *Observability) GenerateObservability(
	ctx context.Context,
	// Source directory containing KCL files
	source *dagger.Directory,
) (string, error) {
	return m.Generate(ctx, source, true, true, true, true, true, false, false, false)
}

// Generate only platform components (KEDA, FluxCD, Dapr)
func (m *Observability) GeneratePlatform(
	ctx context.Context,
	// Source directory containing KCL files
	source *dagger.Directory,
) (string, error) {
	return m.Generate(ctx, source, false, false, false, false, false, true, true, true)
}

// Show module information
func (m *Observability) Info(ctx context.Context) (string, error) {
	info := `
CNCF Platform Stack - Dagger Module
====================================

Observability Components:
  - Prometheus (CNCF Graduated) - Metrics
  - Grafana - UI Dashboard
  - Jaeger (CNCF Graduated) - Tracing
  - OpenTelemetry (CNCF Incubating) - Telemetry
  - Loki - Logs

Platform Components:
  - KEDA (CNCF Graduated) - Event-driven Autoscaling
  - FluxCD (CNCF Graduated) - GitOps
  - Dapr (CNCF Graduated) - Distributed Runtime

Available Commands:
  dagger call generate --source=.
  dagger call generate-file --source=.
  dagger call validate --source=.
  dagger call test --source=.
  dagger call lint --source=.
  dagger call format --source=.
  dagger call ci --source=.
  dagger call cd --source=. --kubeconfig=~/.kube/config
  dagger call dry-run --source=. --kubeconfig=~/.kube/config
  dagger call apply --source=. --kubeconfig=~/.kube/config
  dagger call delete --source=. --kubeconfig=~/.kube/config
  dagger call generate-observability --source=.
  dagger call generate-platform --source=.
`
	return info, nil
}
