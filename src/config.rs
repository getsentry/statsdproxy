use anyhow::Error;
use serde::Deserialize;
use std::fs::File;

#[derive(Debug, Deserialize, PartialEq)]
pub struct Config {
    pub middlewares: Vec<MiddlewareConfig>,
}

impl Config {
    pub fn new(path: &str) -> Result<Self, Error> {
        let f = File::open(path)?;
        let d: Config = serde_yaml::from_reader(f)?;
        Ok(d)
    }
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum MiddlewareConfig {
    DenyTag(DenyTagConfig),
    AllowTag(AllowTagConfig),
    CardinalityLimit(CardinalityLimitConfig),
    AddTag(AddTagConfig),
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct DenyTagConfig {
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct AllowTagConfig {
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct AllowNameConfig {
    pub name: Vec<String>,
}

#[derive(Debug, Deserialize, Eq, Hash, PartialEq)]
pub struct LimitConfig {
    pub window: u16, // in seconds
    pub limit: u64,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct CardinalityLimitConfig {
    pub limits: Vec<LimitConfig>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct AddTagConfig {
    pub tags: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config() {
        let config = Config::new("example.yaml").unwrap();
        let expected = Config {
            middlewares: vec![
                MiddlewareConfig::DenyTag(DenyTagConfig {
                    tags: vec!["a".to_string(), "b".to_string(), "c".to_string()],
                }),
                MiddlewareConfig::CardinalityLimit(CardinalityLimitConfig {
                    limits: vec![LimitConfig {
                        window: 3600,
                        limit: 3,
                    }],
                }),
                MiddlewareConfig::AddTag(AddTagConfig {
                    tags: vec!["d".to_string(), "e:1".to_string()],
                }),
            ],
        };
        assert_eq!(config, expected);
    }
}
