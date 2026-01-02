package schema

import "strings"

// Provider enum for cloud providers
#Provider: "aws" | "gcp" | "azure"

// Common metadata for all Kubernetes resources
#Metadata: {
	name:       string & =~"^[a-z0-9][a-z0-9-]*[a-z0-9]$"
	namespace?: string
	labels?: [string]: string
	annotations?: [string]: string
}

// Reference to another resource
#Reference: {
	name:   string
	policy?: {
		resolution?: "Required" | "Optional"
		resolve?:    "Always" | "IfNotPresent"
	}
}

// Selector for resources
#Selector: {
	matchControllerRef?: bool
	matchLabels?: [string]: string
	policy?: {
		resolution?: "Required" | "Optional"
		resolve?:    "Always" | "IfNotPresent"
	}
}

// Provider configuration reference
#ProviderConfigRef: {
	name:   string
	policy?: {
		resolution?: "Required" | "Optional"
		resolve?:    "Always" | "IfNotPresent"
	}
}

// =============================================================================
// AWS S3 Bucket - Based on Crossplane provider-aws-s3
// https://marketplace.upbound.io/providers/upbound/provider-aws-s3
// =============================================================================

#AWSS3Bucket: {
	apiVersion: "s3.aws.upbound.io/v1beta1"
	kind:       "Bucket"
	metadata:   #Metadata
	spec: {
		deletionPolicy?:    "Delete" | "Orphan" | *"Delete"
		forProvider:        #AWSS3BucketParams
		initProvider?:      #AWSS3BucketParams
		managementPolicies?: [...("Create" | "Update" | "Delete" | "LateInitialize" | "Observe" | "*")]
		providerConfigRef?: #ProviderConfigRef
		writeConnectionSecretToRef?: {
			name:      string
			namespace: string
		}
	}
}

#AWSS3BucketParams: {
	bucket?:            string & strings.MinRunes(3) & strings.MaxRunes(63)
	bucketPrefix?:      string
	forceDestroy?:      bool | *false
	objectLockEnabled?: bool | *false
	region:             #AWSRegion
	tags?: [string]:    string
}

#AWSRegion: "us-east-1" | "us-east-2" | "us-west-1" | "us-west-2" |
	"eu-west-1" | "eu-west-2" | "eu-west-3" | "eu-central-1" | "eu-north-1" |
	"ap-south-1" | "ap-southeast-1" | "ap-southeast-2" | "ap-northeast-1" | "ap-northeast-2" |
	"sa-east-1" | "ca-central-1" | "me-south-1" | "af-south-1"

#AWSS3BucketVersioning: {
	apiVersion: "s3.aws.upbound.io/v1beta1"
	kind:       "BucketVersioning"
	metadata:   #Metadata
	spec: {
		deletionPolicy?: "Delete" | "Orphan" | *"Delete"
		forProvider: {
			bucket?:         string
			bucketRef?:      #Reference
			bucketSelector?: #Selector
			region:          #AWSRegion
			versioningConfiguration: {
				status:     "Enabled" | "Suspended" | "Disabled"
				mfaDelete?: "Enabled" | "Disabled"
			}
		}
		providerConfigRef?: #ProviderConfigRef
	}
}

#AWSS3BucketPublicAccessBlock: {
	apiVersion: "s3.aws.upbound.io/v1beta1"
	kind:       "BucketPublicAccessBlock"
	metadata:   #Metadata
	spec: {
		deletionPolicy?: "Delete" | "Orphan" | *"Delete"
		forProvider: {
			bucket?:                string
			bucketRef?:             #Reference
			bucketSelector?:        #Selector
			blockPublicAcls?:       bool | *true
			blockPublicPolicy?:     bool | *true
			ignorePublicAcls?:      bool | *true
			restrictPublicBuckets?: bool | *true
			region:                 #AWSRegion
		}
		providerConfigRef?: #ProviderConfigRef
	}
}

#AWSS3BucketServerSideEncryption: {
	apiVersion: "s3.aws.upbound.io/v1beta1"
	kind:       "BucketServerSideEncryptionConfiguration"
	metadata:   #Metadata
	spec: {
		deletionPolicy?: "Delete" | "Orphan" | *"Delete"
		forProvider: {
			bucket?:         string
			bucketRef?:      #Reference
			bucketSelector?: #Selector
			region:          #AWSRegion
			rule: [...{
				bucketKeyEnabled?: bool
				applyServerSideEncryptionByDefault?: {
					kmsMasterKeyId?: string
					sseAlgorithm:    "AES256" | "aws:kms" | "aws:kms:dsse"
				}
			}]
		}
		providerConfigRef?: #ProviderConfigRef
	}
}

// =============================================================================
// GCP Cloud Storage Bucket - Based on Crossplane provider-gcp-storage
// https://marketplace.upbound.io/providers/upbound/provider-gcp-storage
// =============================================================================

#GCPBucket: {
	apiVersion: "storage.gcp.upbound.io/v1beta1"
	kind:       "Bucket"
	metadata:   #Metadata
	spec: {
		deletionPolicy?:    "Delete" | "Orphan" | *"Delete"
		forProvider:        #GCPBucketParams
		initProvider?:      #GCPBucketParams
		managementPolicies?: [...("Create" | "Update" | "Delete" | "LateInitialize" | "Observe" | "*")]
		providerConfigRef?: #ProviderConfigRef
		writeConnectionSecretToRef?: {
			name:      string
			namespace: string
		}
	}
}

#GCPBucketParams: {
	// The bucket's location. Multi-region or single region.
	location: #GCPLocation

	// The project ID to create the bucket in.
	project?: string

	// Access control setting for objects in the bucket.
	uniformBucketLevelAccess?: bool | *true

	// Public access prevention config.
	publicAccessPrevention?: "inherited" | "enforced" | *"enforced"

	// Storage class of the bucket.
	storageClass?: "STANDARD" | "NEARLINE" | "COLDLINE" | "ARCHIVE" | "MULTI_REGIONAL" | "REGIONAL" | *"STANDARD"

	// Whether versioning is enabled for the bucket.
	versioning?: {
		enabled: bool
	}

	// Lifecycle rules for objects in the bucket.
	lifecycleRule?: [...#GCPLifecycleRule]

	// CORS configuration.
	cors?: [...#GCPCorsRule]

	// Encryption configuration.
	encryption?: {
		defaultKmsKeyName: string
	}

	// Logging configuration.
	logging?: {
		logBucket:       string
		logObjectPrefix?: string
	}

	// Retention policy for objects in the bucket.
	retentionPolicy?: {
		isLocked?:        bool
		retentionPeriod: int & >=0
	}

	// Labels for the bucket.
	labels?: [string]: string

	// Force destroy even if bucket is not empty.
	forceDestroy?: bool | *false
}

#GCPLocation: "US" | "EU" | "ASIA" |
	"us-central1" | "us-east1" | "us-east4" | "us-west1" | "us-west2" | "us-west3" | "us-west4" |
	"europe-west1" | "europe-west2" | "europe-west3" | "europe-west4" | "europe-west6" | "europe-north1" |
	"asia-east1" | "asia-east2" | "asia-northeast1" | "asia-northeast2" | "asia-northeast3" |
	"asia-south1" | "asia-south2" | "asia-southeast1" | "asia-southeast2" |
	"australia-southeast1" | "australia-southeast2" |
	"southamerica-east1" | "northamerica-northeast1" | "northamerica-northeast2"

#GCPLifecycleRule: {
	action: {
		type:         "Delete" | "SetStorageClass" | "AbortIncompleteMultipartUpload"
		storageClass?: "NEARLINE" | "COLDLINE" | "ARCHIVE" | "STANDARD"
	}
	condition: {
		age?:                     int & >=0
		createdBefore?:           string
		customTimeBefore?:        string
		daysSinceCustomTime?:     int
		daysSinceNoncurrentTime?: int
		matchesPrefix?:           [...string]
		matchesSuffix?:           [...string]
		matchesStorageClass?:     [...string]
		noncurrentTimeBefore?:    string
		numNewerVersions?:        int
		withState?:               "LIVE" | "ARCHIVED" | "ANY"
	}
}

#GCPCorsRule: {
	maxAgeSeconds?: int
	method?:        [...("GET" | "HEAD" | "PUT" | "POST" | "DELETE")]
	origin?:        [...string]
	responseHeader?: [...string]
}

// =============================================================================
// Azure Blob Storage Account - Based on Crossplane provider-azure-storage
// https://marketplace.upbound.io/providers/upbound/provider-azure-storage
// =============================================================================

#AzureStorageAccount: {
	apiVersion: "storage.azure.upbound.io/v1beta1"
	kind:       "Account"
	metadata:   #Metadata
	spec: {
		deletionPolicy?:    "Delete" | "Orphan" | *"Delete"
		forProvider:        #AzureStorageAccountParams
		initProvider?:      #AzureStorageAccountParams
		managementPolicies?: [...("Create" | "Update" | "Delete" | "LateInitialize" | "Observe" | "*")]
		providerConfigRef?: #ProviderConfigRef
		writeConnectionSecretToRef?: {
			name:      string
			namespace: string
		}
	}
}

#AzureStorageAccountParams: {
	// The name of the resource group in which the storage account is created.
	resourceGroupName?:         string
	resourceGroupNameRef?:      #Reference
	resourceGroupNameSelector?: #Selector

	// Specifies the supported Azure location where the resource exists.
	location: #AzureLocation

	// Defines the Kind of account.
	accountKind?: "BlobStorage" | "BlockBlobStorage" | "FileStorage" | "Storage" | "StorageV2" | *"StorageV2"

	// Defines the Tier to use for this storage account.
	accountTier: "Standard" | "Premium"

	// Defines the type of replication to use.
	accountReplicationType: "LRS" | "GRS" | "RAGRS" | "ZRS" | "GZRS" | "RAGZRS"

	// Defines the access tier for BlobStorage, FileStorage and StorageV2 accounts.
	accessTier?: "Hot" | "Cool" | *"Hot"

	// Boolean flag which forces HTTPS.
	enableHttpsTrafficOnly?: bool | *true

	// The minimum supported TLS version.
	minTlsVersion?: "TLS1_0" | "TLS1_1" | "TLS1_2" | *"TLS1_2"

	// Whether public network access is allowed.
	publicNetworkAccessEnabled?: bool | *true

	// Allow or disallow nested items within this Account to opt into being public.
	allowNestedItemsToBePublic?: bool | *false

	// Allow or disallow public access to all blobs or containers.
	allowBlobPublicAccess?: bool | *false

	// Indicates whether the storage account permits requests to be authorized with the account access key.
	sharedAccessKeyEnabled?: bool | *true

	// Is infrastructure encryption enabled?
	infrastructureEncryptionEnabled?: bool | *false

	// Blob service properties.
	blobProperties?: {
		containerDeleteRetentionPolicy?: {
			days: int & >=1 & <=365
		}
		deleteRetentionPolicy?: {
			days: int & >=1 & <=365
		}
		versioningEnabled?:       bool
		changeFeedEnabled?:       bool
		changeFeedRetentionInDays?: int
		lastAccessTimeEnabled?:   bool
		cors?: {
			corsRule?: [...{
				allowedHeaders:    [...string]
				allowedMethods:    [...("DELETE" | "GET" | "HEAD" | "MERGE" | "POST" | "OPTIONS" | "PUT" | "PATCH")]
				allowedOrigins:    [...string]
				exposedHeaders:    [...string]
				maxAgeInSeconds:   int
			}]
		}
	}

	// Network rules for the storage account.
	networkRules?: {
		defaultAction:             "Allow" | "Deny"
		bypass?:                   [...("AzureServices" | "Logging" | "Metrics" | "None")]
		ipRules?:                  [...string]
		virtualNetworkSubnetIds?: [...string]
	}

	// Identity configuration for the storage account.
	identity?: {
		type:        "SystemAssigned" | "UserAssigned" | "SystemAssigned, UserAssigned"
		identityIds?: [...string]
	}

	// Tags for the storage account.
	tags?: [string]: string
}

#AzureLocation: "eastus" | "eastus2" | "westus" | "westus2" | "westus3" |
	"centralus" | "northcentralus" | "southcentralus" | "westcentralus" |
	"canadacentral" | "canadaeast" |
	"westeurope" | "northeurope" | "uksouth" | "ukwest" | "francecentral" | "francesouth" |
	"germanywestcentral" | "germanynorth" | "switzerlandnorth" | "switzerlandwest" |
	"norwayeast" | "norwaywest" | "swedencentral" |
	"eastasia" | "southeastasia" | "japaneast" | "japanwest" |
	"koreacentral" | "koreasouth" | "centralindia" | "southindia" | "westindia" |
	"australiaeast" | "australiasoutheast" | "australiacentral" |
	"brazilsouth" | "brazilsoutheast" |
	"southafricanorth" | "southafricawest" |
	"uaenorth" | "uaecentral"

#AzureContainer: {
	apiVersion: "storage.azure.upbound.io/v1beta1"
	kind:       "Container"
	metadata:   #Metadata
	spec: {
		deletionPolicy?: "Delete" | "Orphan" | *"Delete"
		forProvider: {
			storageAccountName?:         string
			storageAccountNameRef?:      #Reference
			storageAccountNameSelector?: #Selector
			containerAccessType?:        "blob" | "container" | "private" | *"private"
			metadata?: [string]:         string
		}
		providerConfigRef?: #ProviderConfigRef
	}
}

// =============================================================================
// Unified Bucket Abstraction
// =============================================================================

#BucketInput: {
	// Common fields across all providers
	name:     string & =~"^[a-z0-9][a-z0-9-]*[a-z0-9]$"
	provider: #Provider

	// Provider-specific region/location
	region: string

	// Common optional fields
	tags?: [string]: string
	versioning?:     bool
	encryption?:     bool
	publicAccess?:   bool | *false
	forceDestroy?:   bool | *false

	// Provider-specific overrides (optional)
	aws?: #AWSS3BucketParams
	gcp?: #GCPBucketParams
	azure?: #AzureStorageAccountParams
}

// Generate resources based on provider
#Bucket: {
	input: #BucketInput

	// Output the appropriate Crossplane resource based on provider
	output: {
		if input.provider == "aws" {
			#AWSS3Bucket & {
				metadata: name: input.name
				spec: forProvider: {
					bucket:          input.name
					region:          input.region
					forceDestroy:    input.forceDestroy
					if input.tags != _|_ {
						tags: input.tags
					}
					if input.aws != _|_ {
						input.aws
					}
				}
			}
		}
		if input.provider == "gcp" {
			#GCPBucket & {
				metadata: name: input.name
				spec: forProvider: {
					location:     input.region
					forceDestroy: input.forceDestroy
					if input.versioning != _|_ {
						versioning: enabled: input.versioning
					}
					if input.tags != _|_ {
						labels: input.tags
					}
					if input.gcp != _|_ {
						input.gcp
					}
				}
			}
		}
		if input.provider == "azure" {
			#AzureStorageAccount & {
				metadata: name: input.name
				spec: forProvider: {
					location:               input.region
					accountTier:            "Standard"
					accountReplicationType: "LRS"
					if input.tags != _|_ {
						tags: input.tags
					}
					if input.publicAccess != _|_ {
						allowBlobPublicAccess: input.publicAccess
					}
					if input.azure != _|_ {
						input.azure
					}
				}
			}
		}
	}
}
