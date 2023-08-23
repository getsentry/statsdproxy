use anyhow::Error;
use serde::Deserialize;
use std::fs::File;

#[derive(Debug, Deserialize, PartialEq, Default)]
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
    AggregateMetrics(AggregateMetricsConfig),
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

fn default_true() -> bool {
    true
}
fn default_flush_interval() -> u64 {
    1
}
fn default_flush_offset() -> i64 {
    0
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct AggregateMetricsConfig {
    #[serde(default = "default_true")]
    pub aggregate_counters: bool,
    #[serde(default = "default_true")]
    pub aggregate_gauges: bool,
    #[serde(default = "default_flush_interval")]
    pub flush_interval: u64,
    #[serde(default = "default_flush_offset")]
    pub flush_offset: i64,
    #[serde(default)]
    pub max_map_size: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config() {
        let config = Config::new("example.yaml").unwrap();
        insta::assert_debug_snapshot!(config, @r###"
        Config {
            middlewares: [
                DenyTag(
                    DenyTagConfig {
                        tags: [
                            "a",
                            "b",
                            "c",
                        ],
                    },
                ),
                AllowTag(
                    AllowTagConfig {
                        tags: [
                            "x",
                            "y",
                            "z",
                        ],
                    },
                ),
                CardinalityLimit(
                    CardinalityLimitConfig {
                        limits: [
                            LimitConfig {
                                window: 3600,
                                limit: 3,
                            },
                        ],
                    },
                ),
                AggregateMetrics(
                    AggregateMetricsConfig {
                        aggregate_counters: true,
                        aggregate_gauges: true,
                        flush_interval: 1,
                        flush_offset: 0,
                        max_map_size: None,
                    },
                ),
            ],
        }
        "###);
    }
}
