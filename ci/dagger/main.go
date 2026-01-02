// Dagger CI Pipeline for dotconfig
//
// Uses Daggerverse modules for Rust builds and container creation.
// Run with: dagger call <function>

package main

import (
	"context"
	"fmt"

	"dagger/dotconfig-ci/internal/dagger"
)

// DotconfigCi provides CI/CD functions for the dotconfig project
type DotconfigCi struct{}

// Binaries to build
var binaries = []string{"platform_operator", "resource_stats_operator"}

// Build compiles all Rust binaries
func (m *DotconfigCi) Build(
	ctx context.Context,
	// Source directory
	source *dagger.Directory,
	// Build in release mode
	// +optional
	// +default=true
	release bool,
) *dagger.Directory {
	rust := dag.Rust()

	// Setup the Rust project with caching
	project := rust.
		WithSource(source).
		WithCaching()

	var built *dagger.Directory
	if release {
		built = project.BuildRelease()
	} else {
		built = project.Build()
	}

	return built
}

// Test runs all tests
func (m *DotconfigCi) Test(
	ctx context.Context,
	// Source directory
	source *dagger.Directory,
) (string, error) {
	rust := dag.Rust()

	output, err := rust.
		WithSource(source).
		WithCaching().
		Test().
		Stdout(ctx)

	if err != nil {
		return "", fmt.Errorf("tests failed: %w", err)
	}

	return output, nil
}

// Lint runs clippy
func (m *DotconfigCi) Lint(
	ctx context.Context,
	// Source directory
	source *dagger.Directory,
) (string, error) {
	rust := dag.Rust()

	output, err := rust.
		WithSource(source).
		WithCaching().
		Container().
		WithExec([]string{"rustup", "component", "add", "clippy"}).
		WithExec([]string{"cargo", "clippy", "--all-targets", "--", "-D", "warnings"}).
		Stdout(ctx)

	if err != nil {
		return "", fmt.Errorf("clippy failed: %w", err)
	}

	return output, nil
}

// Check runs cargo check
func (m *DotconfigCi) Check(
	ctx context.Context,
	// Source directory
	source *dagger.Directory,
) (string, error) {
	rust := dag.Rust()

	output, err := rust.
		WithSource(source).
		WithCaching().
		Container().
		WithExec([]string{"cargo", "check", "--all-targets"}).
		Stdout(ctx)

	if err != nil {
		return "", fmt.Errorf("check failed: %w", err)
	}

	return output, nil
}

// Container builds a minimal container image for a binary
func (m *DotconfigCi) Container(
	ctx context.Context,
	// Source directory
	source *dagger.Directory,
	// Binary name to containerize
	binary string,
) *dagger.Container {
	rust := dag.Rust()

	// Build static binary with musl
	builder := rust.
		WithSource(source).
		WithCaching().
		Container().
		WithExec([]string{"rustup", "target", "add", "x86_64-unknown-linux-musl"}).
		WithExec([]string{
			"cargo", "build",
			"--bin", binary,
			"--release",
			"--target", "x86_64-unknown-linux-musl",
		})

	binaryFile := builder.File(fmt.Sprintf("/src/target/x86_64-unknown-linux-musl/release/%s", binary))

	// Use wolfi for minimal secure base image
	return dag.Wolfi().
		Container().
		WithFile(fmt.Sprintf("/app/%s", binary), binaryFile).
		WithEntrypoint([]string{fmt.Sprintf("/app/%s", binary)}).
		WithLabel("org.opencontainers.image.source", "https://github.com/yurikrupnik/dotconfig").
		WithLabel("org.opencontainers.image.title", binary)
}

// Containers builds container images for all binaries
func (m *DotconfigCi) Containers(
	ctx context.Context,
	// Source directory
	source *dagger.Directory,
) []*dagger.Container {
	var containers []*dagger.Container
	for _, binary := range binaries {
		containers = append(containers, m.Container(ctx, source, binary))
	}
	return containers
}

// Publish builds and publishes container images to a registry
func (m *DotconfigCi) Publish(
	ctx context.Context,
	// Source directory
	source *dagger.Directory,
	// Container registry (e.g., ghcr.io/yurikrupnik)
	registry string,
	// Image tag
	// +optional
	// +default="latest"
	tag string,
	// Registry username
	// +optional
	username string,
	// Registry password
	// +optional
	password *dagger.Secret,
) ([]string, error) {
	var refs []string

	for _, binary := range binaries {
		container := m.Container(ctx, source, binary)

		if password != nil && username != "" {
			container = container.WithRegistryAuth(registry, username, password)
		}

		imageRef := fmt.Sprintf("%s/%s:%s", registry, binary, tag)
		ref, err := container.Publish(ctx, imageRef)
		if err != nil {
			return refs, fmt.Errorf("failed to publish %s: %w", binary, err)
		}
		refs = append(refs, ref)
	}

	return refs, nil
}

// All runs the complete CI pipeline
func (m *DotconfigCi) All(
	ctx context.Context,
	// Source directory
	source *dagger.Directory,
) (string, error) {
	// Run check
	_, err := m.Check(ctx, source)
	if err != nil {
		return "", err
	}

	// Run lint
	_, err = m.Lint(ctx, source)
	if err != nil {
		return "", err
	}

	// Run tests
	_, err = m.Test(ctx, source)
	if err != nil {
		return "", err
	}

	// Build
	_ = m.Build(ctx, source, true)

	return "Pipeline passed!", nil
}

// Dev starts a development container with Rust toolchain
func (m *DotconfigCi) Dev(
	ctx context.Context,
	// Source directory
	source *dagger.Directory,
) *dagger.Container {
	return dag.Rust().
		WithSource(source).
		WithCaching().
		Container().
		WithExec([]string{"rustup", "component", "add", "clippy", "rustfmt"}).
		WithDefaultTerminalCmd([]string{"/bin/bash"})
}
