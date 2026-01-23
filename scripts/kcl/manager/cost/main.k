# Cost-Aware Resource Provisioning
# Platform Manager
#
# Usage:
#   kcl run main.k -D resource_type=registry -D name=my-registry
#   kcl run main.k -D resource_type=database -D name=my-db
#   kcl run main.k -D resource_type=storage -D name=my-bucket

import .registry
import .database
import .storage

# Input Parameters
resource_type: str = "registry"
name: str = "test-resource"
region: str = "us-central1"
current_cloud: str = "gcp"
project_id: str = "my-gcp-project"

# Registry params
estimated_storage_gb: int = 10

# Database params
engine: str = "postgres"
cpu_cores: int = 2
memory_gb: int = 4
storage_gb: int = 100
high_availability: bool = False

# Storage params
storage_class: str = "STANDARD"
estimated_egress_gb: int = 50
estimated_operations: int = 100000
versioning_enabled: bool = False

# ============================================================================
# Registry
# ============================================================================

_reg_req = registry.RegistryRequest {
    name = name
    region = region
    estimated_storage_gb = estimated_storage_gb
}

_reg_costs = registry.get_all_costs(_reg_req)
_reg_selected = registry.get_cheapest(_reg_req, current_cloud)
_reg_manifest = registry.gen_gcp(_reg_req, project_id) if _reg_selected == "gcp" else registry.gen_aws(_reg_req) if _reg_selected == "aws" else registry.gen_harbor(_reg_req)

# ============================================================================
# Database
# ============================================================================

_db_req = database.DatabaseRequest {
    name = name
    engine = engine
    region = region
    cpu_cores = cpu_cores
    memory_gb = memory_gb
    storage_gb = storage_gb
    high_availability = high_availability
}

_db_costs = database.get_all_costs(_db_req)
_db_selected = database.get_cheapest(_db_req, current_cloud)
_db_manifest = database.gen_cloudsql(_db_req, project_id) if _db_selected == "cloudsql" else database.gen_rds(_db_req) if _db_selected == "rds" else database.gen_cnpg(_db_req)

# ============================================================================
# Storage
# ============================================================================

_storage_req = storage.StorageRequest {
    name = name
    region = region
    storage_class = storage_class
    estimated_storage_gb = estimated_storage_gb
    estimated_egress_gb_per_month = estimated_egress_gb
    estimated_operations_per_month = estimated_operations
    versioning_enabled = versioning_enabled
}

_storage_costs = storage.get_all_costs(_storage_req)
_storage_selected = storage.get_cheapest(_storage_req, current_cloud)
_storage_manifest = storage.gen_gcs(_storage_req, project_id) if _storage_selected == "gcs" else storage.gen_s3(_storage_req) if _storage_selected == "s3" else storage.gen_azure_blob(_storage_req) if _storage_selected == "azure_blob" else storage.gen_minio(_storage_req)

# ============================================================================
# Output
# ============================================================================

_manifest = _reg_manifest if resource_type == "registry" else _db_manifest if resource_type == "database" else _storage_manifest
_costs = _reg_costs if resource_type == "registry" else _db_costs if resource_type == "database" else _storage_costs
_selected = _reg_selected if resource_type == "registry" else _db_selected if resource_type == "database" else _storage_selected

# Cost comparison for visibility
cost_comparison = {
    c.provider: {
        name = c.provider_name
        monthly_cost = c.monthly_cost
        eligible = c.is_eligible
    }
    for c in _costs
}

# Output manifests
items = [
    _manifest
    {
        apiVersion = "v1"
        kind = "ConfigMap"
        metadata = {
            name = "{}-cost-analysis".format(name)
            namespace = "platform-system"
        }
        data = {
            resource_type = resource_type
            selected_provider = _selected
            current_cloud = current_cloud
        }
    }
]
