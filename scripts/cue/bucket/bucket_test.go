package bucket

import (
	"encoding/json"
	"testing"
)

func TestNewManager(t *testing.T) {
	mgr, err := NewManager()
	if err != nil {
		t.Fatalf("Failed to create manager: %v", err)
	}

	if mgr == nil {
		t.Fatal("Manager is nil")
	}
}

func TestListDefinitions(t *testing.T) {
	mgr, err := NewManager()
	if err != nil {
		t.Fatalf("Failed to create manager: %v", err)
	}

	defs, err := mgr.ListDefinitions()
	if err != nil {
		t.Fatalf("Failed to list definitions: %v", err)
	}

	// Check that we have the expected definitions
	expectedDefs := []string{
		"#Provider",
		"#Metadata",
		"#AWSS3Bucket",
		"#GCPBucket",
		"#AzureStorageAccount",
		"#BucketInput",
		"#Bucket",
	}

	defMap := make(map[string]bool)
	for _, d := range defs {
		defMap[d] = true
	}

	for _, expected := range expectedDefs {
		if !defMap[expected] {
			t.Errorf("Missing expected definition: %s", expected)
		}
	}
}

func TestValidateInput_AWS(t *testing.T) {
	mgr, err := NewManager()
	if err != nil {
		t.Fatalf("Failed to create manager: %v", err)
	}

	versioning := true
	input := &BucketInput{
		Name:       "my-test-bucket",
		Provider:   ProviderAWS,
		Region:     "us-east-1",
		Versioning: &versioning,
		Tags: map[string]string{
			"environment": "test",
			"team":        "platform",
		},
	}

	if err := mgr.ValidateInput(input); err != nil {
		t.Errorf("Validation failed: %v", err)
	}
}

func TestValidateInput_GCP(t *testing.T) {
	mgr, err := NewManager()
	if err != nil {
		t.Fatalf("Failed to create manager: %v", err)
	}

	input := &BucketInput{
		Name:     "my-gcp-bucket",
		Provider: ProviderGCP,
		Region:   "US",
		Tags: map[string]string{
			"environment": "production",
		},
	}

	if err := mgr.ValidateInput(input); err != nil {
		t.Errorf("Validation failed: %v", err)
	}
}

func TestValidateInput_Azure(t *testing.T) {
	mgr, err := NewManager()
	if err != nil {
		t.Fatalf("Failed to create manager: %v", err)
	}

	forceDestroy := false
	input := &BucketInput{
		Name:         "myazurestorage",
		Provider:     ProviderAzure,
		Region:       "eastus",
		ForceDestroy: &forceDestroy,
	}

	if err := mgr.ValidateInput(input); err != nil {
		t.Errorf("Validation failed: %v", err)
	}
}

func TestValidateInput_InvalidName(t *testing.T) {
	mgr, err := NewManager()
	if err != nil {
		t.Fatalf("Failed to create manager: %v", err)
	}

	input := &BucketInput{
		Name:     "INVALID_NAME", // Uppercase not allowed
		Provider: ProviderAWS,
		Region:   "us-east-1",
	}

	if err := mgr.ValidateInput(input); err == nil {
		t.Error("Expected validation to fail for invalid bucket name")
	}
}

func TestValidateInput_InvalidProvider(t *testing.T) {
	mgr, err := NewManager()
	if err != nil {
		t.Fatalf("Failed to create manager: %v", err)
	}

	input := &BucketInput{
		Name:     "my-bucket",
		Provider: "invalid",
		Region:   "us-east-1",
	}

	if err := mgr.ValidateInput(input); err == nil {
		t.Error("Expected validation to fail for invalid provider")
	}
}

func TestGenerateCrossplaneManifest_AWS(t *testing.T) {
	mgr, err := NewManager()
	if err != nil {
		t.Fatalf("Failed to create manager: %v", err)
	}

	input := &BucketInput{
		Name:     "my-s3-bucket",
		Provider: ProviderAWS,
		Region:   "us-west-2",
		Tags: map[string]string{
			"app": "myapp",
		},
	}

	yamlOutput, err := mgr.GenerateCrossplaneManifest(input)
	if err != nil {
		t.Fatalf("Failed to generate manifest: %v", err)
	}

	if len(yamlOutput) == 0 {
		t.Error("Generated manifest is empty")
	}

	// Verify it contains expected fields
	yamlStr := string(yamlOutput)
	if !contains(yamlStr, "s3.aws.upbound.io/v1beta1") {
		t.Error("Missing AWS S3 apiVersion")
	}
	if !contains(yamlStr, "kind: Bucket") {
		t.Error("Missing kind: Bucket")
	}
}

func TestGenerateCrossplaneManifest_GCP(t *testing.T) {
	mgr, err := NewManager()
	if err != nil {
		t.Fatalf("Failed to create manager: %v", err)
	}

	versioning := true
	input := &BucketInput{
		Name:       "my-gcs-bucket",
		Provider:   ProviderGCP,
		Region:     "europe-west1",
		Versioning: &versioning,
	}

	yamlOutput, err := mgr.GenerateCrossplaneManifest(input)
	if err != nil {
		t.Fatalf("Failed to generate manifest: %v", err)
	}

	yamlStr := string(yamlOutput)
	if !contains(yamlStr, "storage.gcp.upbound.io/v1beta1") {
		t.Error("Missing GCP Storage apiVersion")
	}
}

func TestGenerateCrossplaneManifest_Azure(t *testing.T) {
	mgr, err := NewManager()
	if err != nil {
		t.Fatalf("Failed to create manager: %v", err)
	}

	input := &BucketInput{
		Name:     "myazurestorage",
		Provider: ProviderAzure,
		Region:   "westeurope",
	}

	yamlOutput, err := mgr.GenerateCrossplaneManifest(input)
	if err != nil {
		t.Fatalf("Failed to generate manifest: %v", err)
	}

	yamlStr := string(yamlOutput)
	if !contains(yamlStr, "storage.azure.upbound.io/v1beta1") {
		t.Error("Missing Azure Storage apiVersion")
	}
	if !contains(yamlStr, "kind: Account") {
		t.Error("Missing kind: Account")
	}
}

func TestGenerateJSON(t *testing.T) {
	mgr, err := NewManager()
	if err != nil {
		t.Fatalf("Failed to create manager: %v", err)
	}

	input := &BucketInput{
		Name:     "json-test-bucket",
		Provider: ProviderAWS,
		Region:   "eu-west-1",
	}

	jsonOutput, err := mgr.GenerateJSON(input)
	if err != nil {
		t.Fatalf("Failed to generate JSON: %v", err)
	}

	// Verify it's valid JSON
	var result map[string]interface{}
	if err := json.Unmarshal(jsonOutput, &result); err != nil {
		t.Errorf("Invalid JSON output: %v", err)
	}
}

func contains(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || len(s) > 0 && containsHelper(s, substr))
}

func containsHelper(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}
