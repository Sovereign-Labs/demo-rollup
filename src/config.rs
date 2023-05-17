use jupiter::da_service::DaServiceConfig;
use jupiter::types::{NamespaceId, NAMESPACE_ID_LEN};
use serde::{Deserialize, Deserializer};
use demo_app::config::{Config as RunnerConfig, FromTomlFile};


#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RollupConfig {
    #[serde(deserialize_with = "deserialize_namespace_id_from_hex")]
    pub namespace_id: NamespaceId,
    pub start_height: u64,
    pub da: DaServiceConfig,
    pub runner: RunnerConfig,
}


fn deserialize_namespace_id_from_hex<'de, D>(deserializer: D) -> Result<NamespaceId, D::Error>
    where
        D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if !s.starts_with("0x") {
        return Err(serde::de::Error::custom("namespace id must start with 0x"));
    }
    let data = hex::decode(&s[2..]).map_err(serde::de::Error::custom)?;
    if data.len() != NAMESPACE_ID_LEN {
        return Err(serde::de::Error::custom(format!("namespace id length should be {} bytes", NAMESPACE_ID_LEN)));
    }
    let mut raw = [0u8; NAMESPACE_ID_LEN];
    raw.copy_from_slice(&data[..]);
    Ok(NamespaceId(raw))
}

impl FromTomlFile for RollupConfig {}

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
            namespace_id = "0x736f762d74657374"
            [da]
            celestia_rpc_auth_token = "SECRET_RPC_TOKEN"
            celestia_rpc_address = "http://localhost:11111/"
            max_celestia_response_body_size = 980
            [runner.storage]
            path = "/tmp"
        "#;

        let config_file = create_config_from(config);

        let config = RollupConfig::from_path(config_file.path()).unwrap();
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


