# Database Cost Analyzer and Generators
# Uses pricing from pricing_config.k

import .pricing_config as pricing

schema DatabaseRequest:
    name: str
    engine: str = "postgres"
    region: str = "us-central1"
    cpu_cores: int = 2
    memory_gb: int = 4
    storage_gb: int = 100
    iops: int = 3000
    high_availability: bool = False
    backup_enabled: bool = True

schema DatabaseCostEstimate:
    provider: str
    provider_name: str
    monthly_cost: float
    cost_breakdown: {str: float}
    is_eligible: bool
    pricing_source: str

HOURS_PER_MONTH = 730

# Provider display names and engine support
_PROVIDER_INFO = {
    cloudsql = {name = "GCP Cloud SQL", engines = ["postgres", "mysql"]}
    rds = {name = "AWS RDS", engines = ["postgres", "mysql"]}
    azure_sql = {name = "Azure Database", engines = ["postgres", "mysql"]}
    cnpg = {name = "CloudNativePG (In-cluster)", engines = ["postgres"]}
    neon = {name = "Neon Serverless", engines = ["postgres"]}
}

# Calculate cost using pricing config
_calc_cost = lambda req: DatabaseRequest, provider_id: str -> DatabaseCostEstimate {
    _p = pricing.PRICING.database[provider_id]
    _info = _PROVIDER_INFO[provider_id]
    _eligible = req.engine in _info.engines

    _compute = (req.cpu_cores * _p.cpu_per_hour + req.memory_gb * _p.memory_per_gb_hour) * HOURS_PER_MONTH
    _storage = req.storage_gb * _p.storage_per_gb_month
    _iops = (req.iops / 1000) * _p.iops_per_1000_hour * HOURS_PER_MONTH
    _backup = req.storage_gb * _p.backup_per_gb_month if req.backup_enabled else 0
    _ha = _compute * (_p.ha_multiplier - 1) if req.high_availability else 0
    _total = _compute + _storage + _iops + _backup + _ha

    DatabaseCostEstimate {
        provider = provider_id
        provider_name = _info.name
        monthly_cost = _total if _eligible else 999999.0
        cost_breakdown = {
            compute = _compute
            storage = _storage
            iops = _iops
            backup = _backup
            ha = _ha
        }
        is_eligible = _eligible
        pricing_source = pricing.PRICING.metadata.source
    }
}

# Get all costs
get_all_costs = lambda req: DatabaseRequest -> [DatabaseCostEstimate] {
    [
        _calc_cost(req, "cloudsql"),
        _calc_cost(req, "rds"),
        _calc_cost(req, "azure_sql"),
        _calc_cost(req, "cnpg"),
        _calc_cost(req, "neon")
    ]
}

# Get cheapest eligible provider
get_cheapest = lambda req: DatabaseRequest, current_cloud: str -> str {
    _costs = get_all_costs(req)
    _eligible_costs = [c.monthly_cost if c.is_eligible else 999999.0 for c in _costs]
    _min_cost = min(_eligible_costs)

    _cheapest = "cnpg"  # Default fallback
    if _eligible_costs[0] == _min_cost:
        _cheapest = "cloudsql"
    elif _eligible_costs[1] == _min_cost:
        _cheapest = "rds"
    elif _eligible_costs[2] == _min_cost:
        _cheapest = "azure_sql"
    elif _eligible_costs[3] == _min_cost:
        _cheapest = "cnpg"
    elif _eligible_costs[4] == _min_cost:
        _cheapest = "neon"

    # Current cloud mapping
    _cloud_map = {gcp = "cloudsql", aws = "rds", azure = "azure_sql"}
    _current_provider = _cloud_map[current_cloud] if current_cloud in _cloud_map else ""
    _current_idx = 0 if _current_provider == "cloudsql" else 1 if _current_provider == "rds" else 2 if _current_provider == "azure_sql" else -1
    _current_cost = _eligible_costs[_current_idx] if _current_idx >= 0 else 999999.0

    # Prefer current if within 15%
    _current_provider if _current_cost <= _min_cost * 1.15 and _current_cost < 999999 else _cheapest
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

gen_cloudsql = lambda req: DatabaseRequest, project_id: str -> {str:} {
    {
        apiVersion = "sql.gcp.upbound.io/v1beta1"
        kind = "DatabaseInstance"
        metadata = {
            name = req.name
            labels = {"platform.yurikrupnik.com/provider" = "cloudsql"}
            annotations = {
                "platform.yurikrupnik.com/pricing-source" = pricing.PRICING.metadata.source
                "platform.yurikrupnik.com/pricing-date" = pricing.PRICING.metadata.last_updated
            }
        }
        spec = {
            forProvider = {
                project = project_id
                region = req.region
                databaseVersion = "POSTGRES_15" if req.engine == "postgres" else "MYSQL_8_0"
                settings = [{
                    tier = "db-custom-{}-{}".format(req.cpu_cores, req.memory_gb * 1024)
                    diskSize = req.storage_gb
                    availabilityType = "REGIONAL" if req.high_availability else "ZONAL"
                }]
            }
            providerConfigRef = {name = "gcp-provider-config"}
        }
    }
}

gen_rds = lambda req: DatabaseRequest -> {str:} {
    {
        apiVersion = "rds.aws.upbound.io/v1beta1"
        kind = "Instance"
        metadata = {
            name = req.name
            labels = {"platform.yurikrupnik.com/provider" = "rds"}
            annotations = {
                "platform.yurikrupnik.com/pricing-source" = pricing.PRICING.metadata.source
                "platform.yurikrupnik.com/pricing-date" = pricing.PRICING.metadata.last_updated
            }
        }
        spec = {
            forProvider = {
                region = req.region
                engine = req.engine
                allocatedStorage = req.storage_gb
                iops = req.iops
                multiAz = req.high_availability
            }
            providerConfigRef = {name = "aws-provider-config"}
        }
    }
}

gen_cnpg = lambda req: DatabaseRequest -> {str:} {
    {
        apiVersion = "postgresql.cnpg.io/v1"
        kind = "Cluster"
        metadata = {
            name = req.name
            labels = {"platform.yurikrupnik.com/provider" = "cnpg"}
            annotations = {
                "platform.yurikrupnik.com/pricing-source" = pricing.PRICING.metadata.source
                "platform.yurikrupnik.com/pricing-date" = pricing.PRICING.metadata.last_updated
            }
        }
        spec = {
            instances = 3 if req.high_availability else 1
            storage = {size = "{}Gi".format(req.storage_gb)}
            resources = {
                requests = {
                    cpu = str(req.cpu_cores)
                    memory = "{}Gi".format(req.memory_gb)
                }
            }
        }
    }
}

gen_neon = lambda req: DatabaseRequest -> {str:} {
    {
        apiVersion = "neon.crossplane.io/v1alpha1"
        kind = "Project"
        metadata = {
            name = req.name
            labels = {"platform.yurikrupnik.com/provider" = "neon"}
            annotations = {
                "platform.yurikrupnik.com/pricing-source" = pricing.PRICING.metadata.source
                "platform.yurikrupnik.com/pricing-date" = pricing.PRICING.metadata.last_updated
            }
        }
        spec = {
            forProvider = {
                name = req.name
                regionId = req.region
                pgVersion = 15
            }
            providerConfigRef = {name = "neon-provider-config"}
        }
    }
}
