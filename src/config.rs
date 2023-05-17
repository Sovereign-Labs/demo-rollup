use jupiter::da_service::DaServiceConfig;
use jupiter::types::{NamespaceId, NAMESPACE_ID_LEN};
use serde::{Deserialize, Deserializer};
use demo_app::config::{Config as RunnerConfig, from_toml_path};


#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RollupConfig {
    pub start_height: u64,
    pub da: DaServiceConfig,
    pub runner: RunnerConfig,
}


#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::NamedTempFile;
    use demo_app::config::StorageConfig;

    fn create_config_from(content: &str) -> NamedTempFile {
        let mut config_file = NamedTempFile::new().unwrap();
        config_file.write_all(content.as_bytes()).unwrap();
        config_file
    }

    #[test]
    fn test_correct_config() {
        let config = r#"
            start_height = 31337
            [da]
            celestia_rpc_auth_token = "SECRET_RPC_TOKEN"
            celestia_rpc_address = "http://localhost:11111/"
            max_celestia_response_body_size = 980
            [runner.storage]
            path = "/tmp"
        "#;

        let config_file = create_config_from(config);

        let config: RollupConfig = from_toml_path(config_file.path()).unwrap();
        let expected = RollupConfig {
            namespace_id: NamespaceId([115, 111, 118, 45, 116, 101, 115, 116]),
            start_height: 31337,
            da: DaServiceConfig {
                celestia_rpc_auth_token: "SECRET_RPC_TOKEN".to_string(),
                celestia_rpc_address: "http://localhost:11111/".into(),
                max_celestia_response_body_size: 980,
            },
            runner: RunnerConfig {
                storage: StorageConfig {
                    path: PathBuf::from("/tmp"),
                },
            },
        };
        assert_eq!(config, expected);
    }
}


