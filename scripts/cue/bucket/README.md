# Bucket CUE Package

Go package with CUE integration for managing cloud storage buckets across AWS S3, GCP Cloud Storage, and Azure Blob Storage using Crossplane APIs.

## Features

- Unified bucket abstraction across 3 cloud providers
- CUE schema validation with type safety
- Go API for programmatic usage
- CLI tool for generating Crossplane manifests
- Input DTOs based on Crossplane/Pulumi APIs

## Installation

```bash
go get github.com/yurikrupnik/dotconfig/scripts/cue/bucket
```

## Usage

### Go API

```go
package main

import (
    "fmt"
    bucket "github.com/yurikrupnik/dotconfig/scripts/cue/bucket"
)

func main() {
    mgr, _ := bucket.NewManager()

    input := &bucket.BucketInput{
        Name:     "my-bucket",
        Provider: bucket.ProviderAWS,
        Region:   "us-east-1",
        Tags: map[string]string{
            "environment": "production",
        },
    }

    // Validate
    if err := mgr.ValidateInput(input); err != nil {
        panic(err)
    }

    // Generate Crossplane manifest
    yaml, _ := mgr.GenerateCrossplaneManifest(input)
    fmt.Println(string(yaml))
}
```

### CLI Tool

```bash
# Generate AWS S3 bucket manifest
go run ./cmd -name my-bucket -provider aws -region us-east-1

# Generate GCP bucket with tags
go run ./cmd -name my-gcs-bucket -provider gcp -region EU \
  -tags '{"env":"prod"}' -versioning

# Generate Azure storage account (JSON output)
go run ./cmd -name mystorageacct -provider azure -region westeurope \
  -output json

# Validate only
go run ./cmd -name test-bucket -provider aws -region us-east-1 -validate
```

## Schema Definitions

### Unified Input (#BucketInput)

| Field | Type | Description |
|-------|------|-------------|
| name | string | Bucket name (lowercase, alphanumeric, hyphens) |
| provider | "aws" \| "gcp" \| "azure" | Cloud provider |
| region | string | Provider-specific region |
| tags | map[string]string | Resource tags/labels |
| versioning | bool | Enable versioning |
| encryption | bool | Enable encryption |
| publicAccess | bool | Allow public access (default: false) |
| forceDestroy | bool | Force destroy on delete |

### Provider-Specific Schemas

- `#AWSS3Bucket` - AWS S3 Bucket (s3.aws.upbound.io/v1beta1)
- `#AWSS3BucketVersioning` - S3 versioning configuration
- `#AWSS3BucketPublicAccessBlock` - S3 public access block
- `#AWSS3BucketServerSideEncryption` - S3 SSE configuration
- `#GCPBucket` - GCP Cloud Storage (storage.gcp.upbound.io/v1beta1)
- `#AzureStorageAccount` - Azure Storage Account (storage.azure.upbound.io/v1beta1)
- `#AzureContainer` - Azure Blob Container

## Crossplane Provider Requirements

Install the required Crossplane providers:

```bash
# AWS S3
kubectl apply -f https://marketplace.upbound.io/providers/upbound/provider-aws-s3/v1.14.0/package.yaml

# GCP Storage
kubectl apply -f https://marketplace.upbound.io/providers/upbound/provider-gcp-storage/v1.8.0/package.yaml

# Azure Storage
kubectl apply -f https://marketplace.upbound.io/providers/upbound/provider-azure-storage/v1.6.0/package.yaml
```

## Examples

See `examples/examples.cue` for comprehensive examples including:
- Multi-region deployments
- Secure S3 bucket with versioning, encryption, and public access block
- GCP bucket with lifecycle rules
- Azure storage with CORS configuration
