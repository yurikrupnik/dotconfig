# Storage Cost Analyzer and Generators
# Uses pricing from pricing_config.k

import .pricing_config as pricing

schema StorageRequest:
    name: str
    region: str = "us-central1"
    storage_class: str = "STANDARD"  # STANDARD, NEARLINE, COLDLINE, ARCHIVE
    estimated_storage_gb: int = 100
    estimated_egress_gb_per_month: int = 50
    estimated_operations_per_month: int = 100000
    versioning_enabled: bool = False
    lifecycle_rules_enabled: bool = True

schema StorageCostEstimate:
    provider: str
    provider_name: str
    monthly_cost: float
    cost_breakdown: {str: float}
    is_eligible: bool
    pricing_source: str

# Provider display names
_PROVIDER_NAMES = {
    gcs = "GCP Cloud Storage"
    s3 = "AWS S3"
    azure_blob = "Azure Blob Storage"
    minio = "MinIO (Self-hosted)"
}

# Calculate cost using pricing config
_calc_cost = lambda req: StorageRequest, provider_id: str -> StorageCostEstimate {
    _p = pricing.PRICING.storage[provider_id]

    # Storage cost (with versioning overhead estimate)
    _versioning_multiplier = 1.2 if req.versioning_enabled else 1.0
    _storage = req.estimated_storage_gb * _p.storage_per_gb_month * _versioning_multiplier

    # Egress cost
    _egress = req.estimated_egress_gb_per_month * _p.egress_per_gb

    # Operations cost (per 10k operations)
    _operations = (req.estimated_operations_per_month / 10000) * _p.operations_per_10k

    _total = _storage + _egress + _operations

    StorageCostEstimate {
        provider = provider_id
        provider_name = _PROVIDER_NAMES[provider_id]
        monthly_cost = _total
        cost_breakdown = {
            storage = _storage
            egress = _egress
            operations = _operations
        }
        is_eligible = True
        pricing_source = pricing.PRICING.metadata.source
    }
}

# Get all costs
get_all_costs = lambda req: StorageRequest -> [StorageCostEstimate] {
    [
        _calc_cost(req, "gcs"),
        _calc_cost(req, "s3"),
        _calc_cost(req, "azure_blob"),
        _calc_cost(req, "minio")
    ]
}

# Get cheapest provider
get_cheapest = lambda req: StorageRequest, current_cloud: str -> str {
    _gcs = _calc_cost(req, "gcs")
    _s3 = _calc_cost(req, "s3")
    _azure = _calc_cost(req, "azure_blob")
    _minio = _calc_cost(req, "minio")

    _min_cost = min([_gcs.monthly_cost, _s3.monthly_cost, _azure.monthly_cost, _minio.monthly_cost])

    _cheapest = "minio" if _minio.monthly_cost == _min_cost else "s3" if _s3.monthly_cost == _min_cost else "gcs" if _gcs.monthly_cost == _min_cost else "azure_blob"

    # Map current cloud to storage provider
    _cloud_map = {gcp = "gcs", aws = "s3", azure = "azure_blob"}
    _current_provider = _cloud_map[current_cloud] if current_cloud in _cloud_map else ""
    _current_cost = _gcs.monthly_cost if _current_provider == "gcs" else _s3.monthly_cost if _current_provider == "s3" else _azure.monthly_cost if _current_provider == "azure_blob" else _minio.monthly_cost

    # Prefer current cloud if within 15%
    _current_provider if _current_cost <= _min_cost * 1.15 and _current_provider != "" else _cheapest
}

# Get pricing metadata
get_pricing_info = lambda -> {str:str} {
    {
        source = pricing.PRICING.metadata.source
        last_updated = pricing.PRICING.metadata.last_updated
        region = pricing.PRICING.metadata.region
    }
}

# ============================================================================
# GENERATORS
# ============================================================================

gen_gcs = lambda req: StorageRequest, project_id: str -> {str:} {
    {
        apiVersion = "storage.gcp.upbound.io/v1beta1"
        kind = "Bucket"
        metadata = {
            name = req.name
            labels = {
                "platform.yurikrupnik.com/provider" = "gcs"
                "platform.yurikrupnik.com/cost-optimized" = "true"
            }
            annotations = {
                "platform.yurikrupnik.com/pricing-source" = pricing.PRICING.metadata.source
                "platform.yurikrupnik.com/pricing-date" = pricing.PRICING.metadata.last_updated
            }
        }
        spec = {
            forProvider = {
                location = req.region.upper().replace("-", "_") if "-" in req.region else req.region.upper()
                project = project_id
                storageClass = req.storage_class
                versioning = [{enabled = req.versioning_enabled}]
                lifecycleRule = [
                    {
                        action = [{type = "Delete"}]
                        condition = [{age = 365}]
                    }
                ] if req.lifecycle_rules_enabled else []
            }
            providerConfigRef = {name = "gcp-provider-config"}
        }
    }
}

gen_s3 = lambda req: StorageRequest -> {str:} {
    # Map storage class to S3 equivalent
    _s3_storage_class = "STANDARD" if req.storage_class == "STANDARD" else "STANDARD_IA" if req.storage_class == "NEARLINE" else "GLACIER" if req.storage_class == "COLDLINE" else "DEEP_ARCHIVE" if req.storage_class == "ARCHIVE" else "STANDARD"

    {
        apiVersion = "s3.aws.upbound.io/v1beta1"
        kind = "Bucket"
        metadata = {
            name = req.name
            labels = {
                "platform.yurikrupnik.com/provider" = "s3"
                "platform.yurikrupnik.com/cost-optimized" = "true"
            }
            annotations = {
                "platform.yurikrupnik.com/pricing-source" = pricing.PRICING.metadata.source
                "platform.yurikrupnik.com/pricing-date" = pricing.PRICING.metadata.last_updated
            }
        }
        spec = {
            forProvider = {
                region = req.region
            }
            providerConfigRef = {name = "aws-provider-config"}
        }
    }
}

gen_s3_versioning = lambda req: StorageRequest -> {str:} {
    {
        apiVersion = "s3.aws.upbound.io/v1beta1"
        kind = "BucketVersioning"
        metadata = {
            name = "{}-versioning".format(req.name)
        }
        spec = {
            forProvider = {
                bucketSelector = {matchLabels = {"platform.yurikrupnik.com/bucket" = req.name}}
                versioningConfiguration = [{status = "Enabled" if req.versioning_enabled else "Suspended"}]
            }
            providerConfigRef = {name = "aws-provider-config"}
        }
    }
}

gen_azure_blob = lambda req: StorageRequest -> {str:} {
    # Map storage class to Azure access tier
    _access_tier = "Hot" if req.storage_class == "STANDARD" else "Cool" if req.storage_class == "NEARLINE" else "Cold" if req.storage_class == "COLDLINE" else "Archive" if req.storage_class == "ARCHIVE" else "Hot"

    {
        apiVersion = "storage.azure.upbound.io/v1beta1"
        kind = "Account"
        metadata = {
            name = req.name
            labels = {
                "platform.yurikrupnik.com/provider" = "azure_blob"
                "platform.yurikrupnik.com/cost-optimized" = "true"
            }
            annotations = {
                "platform.yurikrupnik.com/pricing-source" = pricing.PRICING.metadata.source
                "platform.yurikrupnik.com/pricing-date" = pricing.PRICING.metadata.last_updated
            }
        }
        spec = {
            forProvider = {
                location = req.region
                accountTier = "Standard"
                accountReplicationType = "LRS"
                accessTier = _access_tier
                blobProperties = [{
                    versioningEnabled = req.versioning_enabled
                    deleteRetentionPolicy = [{days = 7}] if req.lifecycle_rules_enabled else []
                }]
            }
            providerConfigRef = {name = "azure-provider-config"}
        }
    }
}

gen_minio = lambda req: StorageRequest, namespace: str = "minio-system" -> {str:} {
    {
        apiVersion = "v1"
        kind = "ConfigMap"
        metadata = {
            name = "{}-bucket-config".format(req.name)
            namespace = namespace
            labels = {
                "platform.yurikrupnik.com/provider" = "minio"
                "platform.yurikrupnik.com/cost-optimized" = "true"
                "platform.yurikrupnik.com/bucket-name" = req.name
            }
            annotations = {
                "platform.yurikrupnik.com/pricing-source" = pricing.PRICING.metadata.source
                "platform.yurikrupnik.com/pricing-date" = pricing.PRICING.metadata.last_updated
            }
        }
        data = {
            bucket_name = req.name
            region = req.region
            versioning = str(req.versioning_enabled).lower()
            lifecycle_enabled = str(req.lifecycle_rules_enabled).lower()
        }
    }
}

gen_minio_bucket_job = lambda req: StorageRequest, namespace: str = "minio-system" -> {str:} {
    _versioning_cmd = "mc version enable myminio/{}".format(req.name) if req.versioning_enabled else "echo 'versioning disabled'"
    _full_cmd = "mc alias set myminio $MINIO_ENDPOINT $MINIO_ACCESS_KEY $MINIO_SECRET_KEY && mc mb --ignore-existing myminio/{} && {}".format(req.name, _versioning_cmd)

    {
        apiVersion = "batch/v1"
        kind = "Job"
        metadata = {
            name = "create-bucket-{}".format(req.name)
            namespace = namespace
            labels = {
                "platform.yurikrupnik.com/provider" = "minio"
                "platform.yurikrupnik.com/bucket-name" = req.name
            }
        }
        spec = {
            template = {
                spec = {
                    restartPolicy = "OnFailure"
                    containers = [{
                        name = "mc"
                        image = "minio/mc:latest"
                        command = ["/bin/sh", "-c"]
                        args = [_full_cmd]
                        envFrom = [{secretRef = {name = "minio-credentials"}}]
                    }]
                }
            }
        }
    }
}
