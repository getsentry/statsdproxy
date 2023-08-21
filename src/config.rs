use anyhow::Error;
use serde::Deserialize;
use std::fs::File;

#[derive(Debug, Deserialize, PartialEq)]
pub struct Config {
    middlewares: Vec<MiddlewareConfig>,
}

impl Config {
    pub fn new(path: &str) -> Result<Self, Error> {
        let f = File::open(path)?;
        let d: Config = serde_yaml::from_reader(f)?;
        Ok(d)
    }
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum MiddlewareConfig {
    #[serde(rename(deserialize = "deny-tag"))]
    DenyTagConfig(DenyTagConfig),
    #[serde(rename(deserialize = "allow-tag"))]
    AllowTagConfig(AllowTagConfig),
    #[serde(rename(deserialize = "cardinality-limit"))]
    CardinalityLimit(CardinalityLimit),
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct DenyTagConfig {
    tags: Vec<String>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct AllowTagConfig {
    tags: Vec<String>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct CardinalityLimit {}

mod tests {
    use super::*;

    #[test]
    fn config() {
        let config = Config::new("example.yaml").unwrap();
        let expected = Config {
            middlewares: vec![
                MiddlewareConfig::DenyTagConfig(DenyTagConfig {
                    tags: vec!["a".to_string(), "b".to_string(), "c".to_string()],
                }),
                MiddlewareConfig::CardinalityLimit(CardinalityLimit {}),
            ],
        };
        assert_eq!(config, expected);
    }
}
