use crate::operator::types::KclSpec;
use crate::operator::{OperatorError, Result};
use std::collections::BTreeMap;
use tokio::process::Command;
use tracing::{debug, info};

/// Result of KCL execution
pub struct KclExecutionResult {
    /// Generated Kubernetes manifests
    pub manifests: Vec<serde_json::Value>,
}

/// KCL executor for generating Kubernetes manifests from KCL sources
pub struct KclExecutor {
    /// Path to KCL binary
    kcl_binary: String,
}

impl KclExecutor {
    /// Create a new KCL executor with the default configuration
    pub fn new() -> Self {
        Self {
            kcl_binary: std::env::var("KCL_BINARY").unwrap_or_else(|_| "kcl".to_string()),
        }
    }

    /// Create a new KCL executor with custom binary path
    pub fn with_binary(binary: impl Into<String>) -> Self {
        Self {
            kcl_binary: binary.into(),
        }
    }

    /// Execute KCL and return generated manifests
    pub async fn execute(&self, spec: &KclSpec) -> Result<KclExecutionResult> {
        let mut args = vec!["run".to_string()];

        // Add a source (OCI, git, or local path)
        args.push(spec.source.clone());

        // Add arguments as -D key=value
        if let Some(arguments) = &spec.arguments {
            for (key, value) in arguments {
                let value_str = match value {
                    serde_json::Value::String(s) => s.clone(),
                    _ => serde_json::to_string(value)
                        .map_err(|e| OperatorError::Kcl(format!("Failed to serialize argument: {}", e)))?,
                };
                args.push("-D".to_string());
                args.push(format!("{}={}", key, value_str));
            }
        }

        // Add settings
        if let Some(settings) = &spec.settings {
            if settings.disable_none {
                args.push("--disable-none".to_string());
            }
            if settings.sort_keys {
                args.push("--sort-keys".to_string());
            }
        }

        debug!("Executing KCL: {} {}", self.kcl_binary, args.join(" "));

        let output = Command::new(&self.kcl_binary)
            .args(&args)
            .output()
            .await
            .map_err(|e| OperatorError::Kcl(format!("Failed to execute KCL: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OperatorError::Kcl(format!("KCL execution failed: {}", stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let manifests = self.parse_manifests(&stdout)?;

        info!("KCL generated {} manifests", manifests.len());

        Ok(KclExecutionResult { manifests })
    }

    /// Execute a pipeline of KCL functions following the function_registry.k pattern
    pub async fn execute_pipeline(
        &self,
        spec: &KclSpec,
    ) -> Result<KclExecutionResult> {
        if let Some(steps) = &spec.pipeline_steps {
            let mut all_manifests = Vec::new();

            for step in steps {
                info!("Executing pipeline step: {}", step.function_name);

                // Build KCL source that calls the function registry
                let kcl_source = format!(
                    r#"
import function_registry as fr

output = fr.execute_function(fr.registry, "{}", {})
items = output if typeof(output) == "list" else [output]
"#,
                    step.function_name,
                    serde_json::to_string(&step.input)
                        .map_err(|e| OperatorError::Kcl(format!("Failed to serialize step input: {}", e)))?
                );

                // Create a temporary spec for this step
                let step_spec = KclSpec {
                    source: spec.source.clone(),
                    arguments: Some({
                        let mut args = BTreeMap::new();
                        args.insert("__pipeline_source__".to_string(), serde_json::Value::String(kcl_source));
                        args
                    }),
                    pipeline_steps: None,
                    settings: spec.settings.clone(),
                };

                let result = self.execute(&step_spec).await?;
                all_manifests.extend(result.manifests);
            }

            Ok(KclExecutionResult { manifests: all_manifests })
        } else {
            // No pipeline, just execute the source directly
            self.execute(spec).await
        }
    }

    /// Parse multi-document YAML output into individual manifests
    fn parse_manifests(&self, yaml: &str) -> Result<Vec<serde_json::Value>> {
        use serde::Deserialize;

        let mut manifests = Vec::new();

        for doc in serde_yaml::Deserializer::from_str(yaml) {
            let value = serde_yaml::Value::deserialize(doc)
                .map_err(OperatorError::Yaml)?;

            // Convert to JSON for easier manipulation
            let json_value: serde_json::Value = serde_yaml::from_value(value)
                .map_err(OperatorError::Yaml)?;

            // Handle KCL items array pattern
            if let Some(items) = json_value.get("items") {
                if let Some(arr) = items.as_array() {
                    for item in arr {
                        // Skip null/none values
                        if !item.is_null() {
                            manifests.push(item.clone());
                        }
                    }
                    continue;
                }
            }

            // Skip null values
            if !json_value.is_null() {
                manifests.push(json_value);
            }
        }

        Ok(manifests)
    }
}

impl Default for KclExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_manifests_single() {
        let executor = KclExecutor::new();
        let yaml = r#"
apiVersion: v1
kind: ConfigMap
metadata:
  name: test
data:
  key: value
"#;

        let result = executor.parse_manifests(yaml).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["kind"], "ConfigMap");
    }

    #[test]
    fn test_parse_manifests_items_array() {
        let executor = KclExecutor::new();
        let yaml = r#"
items:
  - apiVersion: v1
    kind: ConfigMap
    metadata:
      name: test1
  - apiVersion: v1
    kind: Secret
    metadata:
      name: test2
"#;

        let result = executor.parse_manifests(yaml).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0]["kind"], "ConfigMap");
        assert_eq!(result[1]["kind"], "Secret");
    }

    #[test]
    fn test_parse_manifests_multi_doc() {
        let executor = KclExecutor::new();
        let yaml = r#"
apiVersion: v1
kind: ConfigMap
metadata:
  name: test1
---
apiVersion: v1
kind: Secret
metadata:
  name: test2
"#;

        let result = executor.parse_manifests(yaml).unwrap();
        assert_eq!(result.len(), 2);
    }
}
