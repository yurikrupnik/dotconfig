# Base schema and utilities for dependency modules
# Each dependency is a "lego block" that can be composed into an app

import k8s.api.core.v1 as corev1

# Provider types for external services
schema ProviderConfig:
    """Configuration for cloud provider"""
    name: str                      # neon, atlas, aiven, supabase, etc.
    internal: bool = False         # Use in-cluster (CNPG, etc.)
    region?: str
    tier?: str = "free"            # free, standard, premium
    credentials_secret?: str       # Secret name containing provider credentials

# Base dependency output
schema DependencyOutput:
    """Output from a resolved dependency"""
    name: str
    env_vars: [corev1.EnvVar]           # Env vars to inject into app
    resources: [any]                     # K8s/Crossplane resources to create
    secrets: [corev1.Secret]             # Secrets to create
    labels: {str: str}                   # Labels to add to the app
    wait_for?: [str]                     # Resources to wait for before app starts

# Base dependency configuration
schema DependencyConfig:
    """Base configuration all dependencies extend"""
    enabled: bool = True
    provider?: ProviderConfig
    version?: str
    namespace?: str

# Utility to create env var from secret
create_env_from_secret = lambda name: str, secret_name: str, key: str -> corev1.EnvVar {
    corev1.EnvVar {
        name: name
        valueFrom: {
            secretKeyRef: {
                name: secret_name
                key: key
            }
        }
    }
}

# Utility to create plain env var
create_env = lambda name: str, value: str -> corev1.EnvVar {
    corev1.EnvVar {
        name: name
        value: value
    }
}

# Utility to merge multiple dependency outputs
merge_outputs = lambda outputs: [DependencyOutput] -> DependencyOutput {
    DependencyOutput {
        name: "merged"
        env_vars: [e for o in outputs for e in o.env_vars]
        resources: [r for o in outputs for r in o.resources]
        secrets: [s for o in outputs for s in o.secrets]
        labels: {k: v for o in outputs for k, v in o.labels}
        wait_for: [w for o in outputs if o.wait_for for w in o.wait_for]
    }
}
