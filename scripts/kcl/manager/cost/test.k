# Cost Analysis Tests

import .registry
import .database
import .storage

# ============================================================================
# TEST 1: Registry Cost Comparison (50GB)
# ============================================================================
_reg_req = registry.RegistryRequest {
    name = "my-registry"
    estimated_storage_gb = 50
}

registry_costs = registry.get_all_costs(_reg_req)
registry_cheapest = registry.get_cheapest(_reg_req, "gcp")

# ============================================================================
# TEST 2: Database Cost Comparison (4 CPU, 16GB, 200GB storage)
# ============================================================================
_db_req = database.DatabaseRequest {
    name = "orders-db"
    engine = "postgres"
    cpu_cores = 4
    memory_gb = 16
    storage_gb = 200
    high_availability = False
}

database_costs = database.get_all_costs(_db_req)
database_cheapest = database.get_cheapest(_db_req, "gcp")

# ============================================================================
# TEST 3: Database with HA (8 CPU, 32GB, 500GB)
# ============================================================================
_db_ha_req = database.DatabaseRequest {
    name = "prod-db"
    engine = "postgres"
    cpu_cores = 8
    memory_gb = 32
    storage_gb = 500
    high_availability = True
}

database_ha_costs = database.get_all_costs(_db_ha_req)
database_ha_cheapest = database.get_cheapest(_db_ha_req, "aws")

# ============================================================================
# TEST 4: Storage Cost Comparison (500GB, 100GB egress)
# ============================================================================
_storage_req = storage.StorageRequest {
    name = "app-data"
    estimated_storage_gb = 500
    estimated_egress_gb_per_month = 100
    estimated_operations_per_month = 500000
    versioning_enabled = False
}

storage_costs = storage.get_all_costs(_storage_req)
storage_cheapest = storage.get_cheapest(_storage_req, "gcp")

# ============================================================================
# TEST 5: Storage with Versioning (1TB, high egress)
# ============================================================================
_storage_versioned_req = storage.StorageRequest {
    name = "backup-data"
    estimated_storage_gb = 1000
    estimated_egress_gb_per_month = 500
    estimated_operations_per_month = 1000000
    versioning_enabled = True
    lifecycle_rules_enabled = True
}

storage_versioned_costs = storage.get_all_costs(_storage_versioned_req)
storage_versioned_cheapest = storage.get_cheapest(_storage_versioned_req, "aws")

# ============================================================================
# Summary
# ============================================================================
summary = {
    registry_50gb = {
        costs = {c.provider: c.monthly_cost for c in registry_costs}
        selected = registry_cheapest
        savings_vs_gcp = registry_costs[0].monthly_cost - [c for c in registry_costs if c.provider == registry_cheapest][0].monthly_cost
    }
    database_standard = {
        costs = {c.provider: c.monthly_cost for c in database_costs}
        selected = database_cheapest
    }
    database_ha = {
        costs = {c.provider: c.monthly_cost for c in database_ha_costs}
        selected = database_ha_cheapest
    }
    storage_500gb = {
        costs = {c.provider: c.monthly_cost for c in storage_costs}
        selected = storage_cheapest
    }
    storage_1tb_versioned = {
        costs = {c.provider: c.monthly_cost for c in storage_versioned_costs}
        selected = storage_versioned_cheapest
    }
}
