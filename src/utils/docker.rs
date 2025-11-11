use std::path::Path;

pub fn resolve_compose_file(file: Option<String>) -> anyhow::Result<String> {
    if let Some(file_path) = file {
        if Path::new(&file_path).exists() {
            Ok(file_path)
        } else {
            anyhow::bail!("Compose file not found: {}", file_path);
        }
    } else {
        let compose_files = [
            "docker-compose.yml",
            "docker-compose.yaml",
            "compose.yml",
            "compose.yaml",
        ];

        for file in &compose_files {
            if Path::new(file).exists() {
                return Ok(file.to_string());
            }
        }

        Ok("./compose.yaml".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    #[serial]
    fn test_resolve_compose_file_provided_exists() {
        let temp_dir = TempDir::new().unwrap();
        let compose_file = temp_dir.path().join("my-compose.yml");
        fs::write(&compose_file, "version: '3'\n").unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let result = resolve_compose_file(Some(compose_file.to_string_lossy().to_string()));

        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), compose_file.to_string_lossy().to_string());
    }

    #[test]
    #[serial]
    fn test_resolve_compose_file_provided_not_exists() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let result = resolve_compose_file(Some("nonexistent.yml".to_string()));

        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Compose file not found: nonexistent.yml"));
    }

    #[test]
    #[serial]
    fn test_resolve_compose_file_precedence_docker_compose_yml() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        fs::write(temp_dir.path().join("docker-compose.yml"), "version: '3'\n").unwrap();
        fs::write(temp_dir.path().join("compose.yaml"), "version: '3'\n").unwrap();

        let result = resolve_compose_file(None);

        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "docker-compose.yml");
    }

    #[test]
    #[serial]
    fn test_resolve_compose_file_precedence_docker_compose_yaml() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        fs::write(
            temp_dir.path().join("docker-compose.yaml"),
            "version: '3'\n",
        )
        .unwrap();
        fs::write(temp_dir.path().join("compose.yml"), "version: '3'\n").unwrap();

        let result = resolve_compose_file(None);

        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "docker-compose.yaml");
    }

    #[test]
    #[serial]
    fn test_resolve_compose_file_only_compose_yaml() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        fs::write(temp_dir.path().join("compose.yaml"), "version: '3'\n").unwrap();

        let result = resolve_compose_file(None);

        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "compose.yaml");
    }

    #[test]
    #[serial]
    fn test_resolve_compose_file_none_found_fallback() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let result = resolve_compose_file(None);

        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "./compose.yaml");
    }
}
