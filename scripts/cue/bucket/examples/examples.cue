package examples

import "github.com/yurikrupnik/dotconfig/scripts/cue/bucket/schema"

// Example AWS S3 bucket for application data
appDataBucketAWS: schema.#Bucket & {
	input: {
		name:       "myapp-data-prod"
		provider:   "aws"
		region:     "us-east-1"
		versioning: true
		encryption: true
		tags: {
			environment: "production"
			team:        "platform"
			app:         "myapp"
		}
	}
}

// Example GCP bucket for backups
backupBucketGCP: schema.#Bucket & {
	input: {
		name:         "myapp-backups-eu"
		provider:     "gcp"
		region:       "EU"
		versioning:   true
		forceDestroy: false
		tags: {
			environment: "production"
			purpose:     "backup"
		}
		gcp: {
			storageClass: "NEARLINE"
			lifecycleRule: [{
				action: type: "Delete"
				condition: age: 365
			}]
		}
	}
}

// Example Azure storage account for static assets
staticAssetsBucketAzure: schema.#Bucket & {
	input: {
		name:         "myappstatics"
		provider:     "azure"
		region:       "westeurope"
		publicAccess: true
		tags: {
			environment: "production"
			purpose:     "static-assets"
		}
		azure: {
			accountTier:            "Standard"
			accountReplicationType: "GRS"
			accessTier:             "Hot"
			blobProperties: {
				versioningEnabled: true
				cors: corsRule: [{
					allowedHeaders:  ["*"]
					allowedMethods:  ["GET", "HEAD"]
					allowedOrigins:  ["https://myapp.com"]
					exposedHeaders:  ["ETag"]
					maxAgeInSeconds: 3600
				}]
			}
		}
	}
}

// Example: Multi-region deployment buckets
multiRegionBuckets: {
	usEast: schema.#Bucket & {
		input: {
			name:     "myapp-assets-us-east"
			provider: "aws"
			region:   "us-east-1"
			tags: region: "us-east"
		}
	}
	euWest: schema.#Bucket & {
		input: {
			name:     "myapp-assets-eu-west"
			provider: "aws"
			region:   "eu-west-1"
			tags: region: "eu-west"
		}
	}
	apSouth: schema.#Bucket & {
		input: {
			name:     "myapp-assets-ap-south"
			provider: "aws"
			region:   "ap-south-1"
			tags: region: "ap-south"
		}
	}
}

// Example: Full AWS S3 bucket with all security settings
secureS3Bucket: {
	bucket: schema.#AWSS3Bucket & {
		metadata: {
			name: "secure-data-bucket"
			labels: {
				"app.kubernetes.io/managed-by": "crossplane"
				security:                       "high"
			}
		}
		spec: {
			deletionPolicy: "Orphan"
			forProvider: {
				bucket:       "secure-data-bucket-prod"
				region:       "us-east-1"
				forceDestroy: false
				tags: {
					environment: "production"
					compliance:  "pci-dss"
				}
			}
			providerConfigRef: name: "aws-production"
		}
	}

	versioning: schema.#AWSS3BucketVersioning & {
		metadata: name: "secure-data-bucket-versioning"
		spec: {
			forProvider: {
				bucketRef: name: "secure-data-bucket"
				region: "us-east-1"
				versioningConfiguration: {
					status:    "Enabled"
					mfaDelete: "Disabled"
				}
			}
			providerConfigRef: name: "aws-production"
		}
	}

	publicAccessBlock: schema.#AWSS3BucketPublicAccessBlock & {
		metadata: name: "secure-data-bucket-pab"
		spec: {
			forProvider: {
				bucketRef: name:         "secure-data-bucket"
				region:                  "us-east-1"
				blockPublicAcls:         true
				blockPublicPolicy:       true
				ignorePublicAcls:        true
				restrictPublicBuckets:   true
			}
			providerConfigRef: name: "aws-production"
		}
	}

	encryption: schema.#AWSS3BucketServerSideEncryption & {
		metadata: name: "secure-data-bucket-sse"
		spec: {
			forProvider: {
				bucketRef: name: "secure-data-bucket"
				region: "us-east-1"
				rule: [{
					bucketKeyEnabled: true
					applyServerSideEncryptionByDefault: {
						sseAlgorithm: "aws:kms"
					}
				}]
			}
			providerConfigRef: name: "aws-production"
		}
	}
}
