// CLI tool for generating bucket configurations across cloud providers
package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"os"

	bucket "github.com/yurikrupnik/dotconfig/scripts/cue/bucket"
)

func main() {
	var (
		name         = flag.String("name", "", "Bucket name (required)")
		provider     = flag.String("provider", "", "Cloud provider: aws, gcp, azure (required)")
		region       = flag.String("region", "", "Region/location (required)")
		tags         = flag.String("tags", "", "Tags as JSON object (optional)")
		versioning   = flag.Bool("versioning", false, "Enable versioning")
		encryption   = flag.Bool("encryption", false, "Enable encryption")
		publicAccess = flag.Bool("public", false, "Allow public access")
		forceDestroy = flag.Bool("force-destroy", false, "Force destroy on delete")
		outputFormat = flag.String("output", "yaml", "Output format: yaml, json")
		validate     = flag.Bool("validate", false, "Only validate, don't generate")
	)

	flag.Parse()

	if *name == "" || *provider == "" || *region == "" {
		fmt.Fprintln(os.Stderr, "Error: name, provider, and region are required")
		flag.Usage()
		os.Exit(1)
	}

	// Create bucket manager
	mgr, err := bucket.NewManager()
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error creating manager: %v\n", err)
		os.Exit(1)
	}

	// Parse tags if provided
	var tagsMap map[string]string
	if *tags != "" {
		if err := json.Unmarshal([]byte(*tags), &tagsMap); err != nil {
			fmt.Fprintf(os.Stderr, "Error parsing tags: %v\n", err)
			os.Exit(1)
		}
	}

	// Build input
	input := &bucket.BucketInput{
		Name:         *name,
		Provider:     bucket.Provider(*provider),
		Region:       *region,
		Tags:         tagsMap,
		Versioning:   versioning,
		Encryption:   encryption,
		PublicAccess: publicAccess,
		ForceDestroy: forceDestroy,
	}

	// Validate input
	if err := mgr.ValidateInput(input); err != nil {
		fmt.Fprintf(os.Stderr, "Validation error: %v\n", err)
		os.Exit(1)
	}

	if *validate {
		fmt.Println("Validation successful!")
		return
	}

	// Generate manifest
	var output []byte
	switch *outputFormat {
	case "yaml":
		output, err = mgr.GenerateCrossplaneManifest(input)
	case "json":
		output, err = mgr.GenerateJSON(input)
	default:
		fmt.Fprintf(os.Stderr, "Unknown output format: %s\n", *outputFormat)
		os.Exit(1)
	}

	if err != nil {
		fmt.Fprintf(os.Stderr, "Error generating manifest: %v\n", err)
		os.Exit(1)
	}

	fmt.Println(string(output))
}
