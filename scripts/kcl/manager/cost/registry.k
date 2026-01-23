# Registry Cost Analyzer and Generators
# Uses pricing from pricing_config.k

import .pricing_config as pricing

schema RegistryRequest:
    name: str
    region: str = "us-central1"
    format: str = "DOCKER"
    estimated_storage_gb: int = 10
    estimated_pulls_per_month: int = 1000
    requires_vulnerability_scanning: bool = False

schema RegistryCostEstimate:
    provider: str
    provider_name: str
    monthly_cost: float
    cost_breakdown: {str: float}
    is_eligible: bool
    pricing_source: str

# Provider display names
_PROVIDER_NAMES = {
    gcp = "GCP Artifact Registry"
    aws = "AWS ECR"
    azure = "Azure ACR"
    harbor = "Harbor (Self-hosted)"
}

# Calculate cost using pricing config
_calc_cost = lambda req: RegistryRequest, provider_id: str -> RegistryCostEstimate {
    _p = pricing.PRICING.registry[provider_id]
    _storage = req.estimated_storage_gb * _p.storage_per_gb_month
    _egress = req.estimated_storage_gb * 0.5 * _p.egress_per_gb  # Assume 50% pulled
    _scan = (req.estimated_pulls_per_month / 10) * _p.vulnerability_scan_per_image if req.requires_vulnerability_scanning else 0
    _total = _storage + _egress + _scan

    RegistryCostEstimate {
        provider = provider_id
        provider_name = _PROVIDER_NAMES[provider_id]
        monthly_cost = _total
        cost_breakdown = {
            storage = _storage
            egress = _egress
            scanning = _scan
        }
        is_eligible = True
        pricing_source = pricing.PRICING.metadata.source
    }
}

# Get all costs
get_all_costs = lambda req: RegistryRequest -> [RegistryCostEstimate] {
    [_calc_cost(req, "gcp"), _calc_cost(req, "aws"), _calc_cost(req, "azure"), _calc_cost(req, "harbor")]
}

# Get cheapest provider
get_cheapest = lambda req: RegistryRequest, current_cloud: str -> str {
    _gcp = _calc_cost(req, "gcp")
    _aws = _calc_cost(req, "aws")
    _azure = _calc_cost(req, "azure")
    _harbor = _calc_cost(req, "harbor")

    _min_cost = min([_gcp.monthly_cost, _aws.monthly_cost, _azure.monthly_cost, _harbor.monthly_cost])
    _cheapest = "harbor" if _harbor.monthly_cost == _min_cost else "aws" if _aws.monthly_cost == _min_cost else "gcp" if _gcp.monthly_cost == _min_cost else "azure"

    _current_cost = _gcp.monthly_cost if current_cloud == "gcp" else _aws.monthly_cost if current_cloud == "aws" else _azure.monthly_cost if current_cloud == "azure" else _harbor.monthly_cost

    # Prefer current cloud if within 20%
    current_cloud if _current_cost <= _min_cost * 1.2 else _cheapest
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

gen_gcp = lambda req: RegistryRequest, project_id: str -> {str:} {
    {
        apiVersion = "artifact.gcp.upbound.io/v1beta1"
        kind = "RegistryRepository"
        metadata = {
            name = req.name
            labels = {
                "platform.yurikrupnik.com/provider" = "gcp"
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
                format = req.format
                project = project_id
            }
            providerConfigRef = {name = "gcp-provider-config"}
        }
    }
}

gen_aws = lambda req: RegistryRequest -> {str:} {
    {
        apiVersion = "ecr.aws.upbound.io/v1beta1"
        kind = "Repository"
        metadata = {
            name = req.name
            labels = {
                "platform.yurikrupnik.com/provider" = "aws"
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
                imageScanningConfiguration = [{scanOnPush = req.requires_vulnerability_scanning}]
            }
            providerConfigRef = {name = "aws-provider-config"}
        }
    }
}

gen_harbor = lambda req: RegistryRequest -> {str:} {
    {
        apiVersion = "goharbor.io/v1beta1"
        kind = "HarborProject"
        metadata = {
            name = req.name
            labels = {
                "platform.yurikrupnik.com/provider" = "harbor"
                "platform.yurikrupnik.com/cost-optimized" = "true"
            }
            annotations = {
                "platform.yurikrupnik.com/pricing-source" = pricing.PRICING.metadata.source
                "platform.yurikrupnik.com/pricing-date" = pricing.PRICING.metadata.last_updated
            }
        }
        spec = {
            harborRef = "harbor"
            project = {projectName = req.name}
        }
    }
}
