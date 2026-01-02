// Package bucket provides CUE-based bucket configuration management
// for AWS S3, GCP Cloud Storage, and Azure Blob Storage using Crossplane APIs.
package bucket

import (
	"embed"
	"encoding/json"
	"fmt"

	"cuelang.org/go/cue"
	"cuelang.org/go/cue/cuecontext"
	"cuelang.org/go/cue/load"
	"cuelang.org/go/encoding/yaml"
)

//go:embed schema/*.cue
var schemaFS embed.FS

// Provider represents cloud provider types
type Provider string

const (
	ProviderAWS   Provider = "aws"
	ProviderGCP   Provider = "gcp"
	ProviderAzure Provider = "azure"
)

// BucketInput represents the unified input for creating buckets across providers
type BucketInput struct {
	Name         string            `json:"name"`
	Provider     Provider          `json:"provider"`
	Region       string            `json:"region"`
	Tags         map[string]string `json:"tags,omitempty"`
	Versioning   *bool             `json:"versioning,omitempty"`
	Encryption   *bool             `json:"encryption,omitempty"`
	PublicAccess *bool             `json:"publicAccess,omitempty"`
	ForceDestroy *bool             `json:"forceDestroy,omitempty"`

	// Provider-specific overrides
	AWS   map[string]interface{} `json:"aws,omitempty"`
	GCP   map[string]interface{} `json:"gcp,omitempty"`
	Azure map[string]interface{} `json:"azure,omitempty"`
}

// Manager handles CUE-based bucket configuration
type Manager struct {
	ctx    *cue.Context
	schema cue.Value
}

// NewManager creates a new bucket configuration manager
func NewManager() (*Manager, error) {
	ctx := cuecontext.New()

	// Load the embedded CUE schema
	schemaContent, err := schemaFS.ReadFile("schema/schema.cue")
	if err != nil {
		return nil, fmt.Errorf("failed to read embedded schema: %w", err)
	}

	schema := ctx.CompileBytes(schemaContent)
	if schema.Err() != nil {
		return nil, fmt.Errorf("failed to compile schema: %w", schema.Err())
	}

	return &Manager{
		ctx:    ctx,
		schema: schema,
	}, nil
}

// NewManagerFromPath creates a manager from external CUE files
func NewManagerFromPath(paths ...string) (*Manager, error) {
	ctx := cuecontext.New()

	cfg := &load.Config{}
	instances := load.Instances(paths, cfg)

	if len(instances) == 0 {
		return nil, fmt.Errorf("no CUE instances found")
	}

	inst := instances[0]
	if inst.Err != nil {
		return nil, fmt.Errorf("failed to load CUE instance: %w", inst.Err)
	}

	schema := ctx.BuildInstance(inst)
	if schema.Err() != nil {
		return nil, fmt.Errorf("failed to build schema: %w", schema.Err())
	}

	return &Manager{
		ctx:    ctx,
		schema: schema,
	}, nil
}

// ValidateInput validates a BucketInput against the CUE schema
func (m *Manager) ValidateInput(input *BucketInput) error {
	inputJSON, err := json.Marshal(input)
	if err != nil {
		return fmt.Errorf("failed to marshal input: %w", err)
	}

	inputValue := m.ctx.CompileBytes(inputJSON)
	if inputValue.Err() != nil {
		return fmt.Errorf("failed to compile input: %w", inputValue.Err())
	}

	// Get the #BucketInput definition from schema
	bucketInputDef := m.schema.LookupPath(cue.ParsePath("#BucketInput"))
	if bucketInputDef.Err() != nil {
		return fmt.Errorf("failed to lookup #BucketInput: %w", bucketInputDef.Err())
	}

	// Unify input with schema
	unified := bucketInputDef.Unify(inputValue)
	if err := unified.Validate(); err != nil {
		return fmt.Errorf("validation failed: %w", err)
	}

	return nil
}

// GenerateCrossplaneManifest generates a Crossplane manifest for the given input
func (m *Manager) GenerateCrossplaneManifest(input *BucketInput) ([]byte, error) {
	inputJSON, err := json.Marshal(map[string]interface{}{
		"input": input,
	})
	if err != nil {
		return nil, fmt.Errorf("failed to marshal input: %w", err)
	}

	inputValue := m.ctx.CompileBytes(inputJSON)
	if inputValue.Err() != nil {
		return nil, fmt.Errorf("failed to compile input: %w", inputValue.Err())
	}

	// Get the #Bucket definition and unify with input
	bucketDef := m.schema.LookupPath(cue.ParsePath("#Bucket"))
	if bucketDef.Err() != nil {
		return nil, fmt.Errorf("failed to lookup #Bucket: %w", bucketDef.Err())
	}

	unified := bucketDef.Unify(inputValue)
	if err := unified.Validate(); err != nil {
		return nil, fmt.Errorf("validation failed: %w", err)
	}

	// Extract the output
	output := unified.LookupPath(cue.ParsePath("output"))
	if output.Err() != nil {
		return nil, fmt.Errorf("failed to lookup output: %w", output.Err())
	}

	// Convert to YAML
	yamlBytes, err := yaml.Encode(output)
	if err != nil {
		return nil, fmt.Errorf("failed to encode YAML: %w", err)
	}

	return yamlBytes, nil
}

// GenerateJSON generates JSON output for the Crossplane manifest
func (m *Manager) GenerateJSON(input *BucketInput) ([]byte, error) {
	inputJSON, err := json.Marshal(map[string]interface{}{
		"input": input,
	})
	if err != nil {
		return nil, fmt.Errorf("failed to marshal input: %w", err)
	}

	inputValue := m.ctx.CompileBytes(inputJSON)
	if inputValue.Err() != nil {
		return nil, fmt.Errorf("failed to compile input: %w", inputValue.Err())
	}

	// Get the #Bucket definition and unify with input
	bucketDef := m.schema.LookupPath(cue.ParsePath("#Bucket"))
	if bucketDef.Err() != nil {
		return nil, fmt.Errorf("failed to lookup #Bucket: %w", bucketDef.Err())
	}

	unified := bucketDef.Unify(inputValue)
	if err := unified.Validate(); err != nil {
		return nil, fmt.Errorf("validation failed: %w", err)
	}

	// Extract the output
	output := unified.LookupPath(cue.ParsePath("output"))
	if output.Err() != nil {
		return nil, fmt.Errorf("failed to lookup output: %w", output.Err())
	}

	return json.MarshalIndent(output, "", "  ")
}

// ValidateAWSBucket validates an AWS S3 bucket configuration
func (m *Manager) ValidateAWSBucket(manifest []byte) error {
	value := m.ctx.CompileBytes(manifest)
	if value.Err() != nil {
		return fmt.Errorf("failed to compile manifest: %w", value.Err())
	}

	awsDef := m.schema.LookupPath(cue.ParsePath("#AWSS3Bucket"))
	if awsDef.Err() != nil {
		return fmt.Errorf("failed to lookup #AWSS3Bucket: %w", awsDef.Err())
	}

	unified := awsDef.Unify(value)
	return unified.Validate()
}

// ValidateGCPBucket validates a GCP Cloud Storage bucket configuration
func (m *Manager) ValidateGCPBucket(manifest []byte) error {
	value := m.ctx.CompileBytes(manifest)
	if value.Err() != nil {
		return fmt.Errorf("failed to compile manifest: %w", value.Err())
	}

	gcpDef := m.schema.LookupPath(cue.ParsePath("#GCPBucket"))
	if gcpDef.Err() != nil {
		return fmt.Errorf("failed to lookup #GCPBucket: %w", gcpDef.Err())
	}

	unified := gcpDef.Unify(value)
	return unified.Validate()
}

// ValidateAzureStorageAccount validates an Azure Storage Account configuration
func (m *Manager) ValidateAzureStorageAccount(manifest []byte) error {
	value := m.ctx.CompileBytes(manifest)
	if value.Err() != nil {
		return fmt.Errorf("failed to compile manifest: %w", value.Err())
	}

	azureDef := m.schema.LookupPath(cue.ParsePath("#AzureStorageAccount"))
	if azureDef.Err() != nil {
		return fmt.Errorf("failed to lookup #AzureStorageAccount: %w", azureDef.Err())
	}

	unified := azureDef.Unify(value)
	return unified.Validate()
}

// GetSchema returns the raw schema for introspection
func (m *Manager) GetSchema() cue.Value {
	return m.schema
}

// ListDefinitions lists all available definitions in the schema
func (m *Manager) ListDefinitions() ([]string, error) {
	var defs []string

	iter, err := m.schema.Fields(cue.Definitions(true))
	if err != nil {
		return nil, fmt.Errorf("failed to iterate definitions: %w", err)
	}

	for iter.Next() {
		defs = append(defs, iter.Selector().String())
	}

	return defs, nil
}
