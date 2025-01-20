use std::time::Duration;
#[cfg(feature = "cli")]
use {anyhow::Error, serde::Deserialize, std::fs::File};

#[cfg_attr(feature = "cli", derive(Deserialize))]
#[derive(Debug, PartialEq, Default)]
pub struct Config {
    pub middlewares: Vec<MiddlewareConfig>,
}

impl Config {
    #[cfg(feature = "cli")]
    pub fn new(path: &str) -> Result<Self, Error> {
        let f = File::open(path)?;
        let d: Config = serde_yaml::from_reader(f)?;
        Ok(d)
    }
}

#[cfg_attr(feature = "cli", derive(Deserialize))]
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "cli", serde(tag = "type", rename_all = "kebab-case"))]
pub enum MiddlewareConfig {
    DenyTag(DenyTagConfig),
    AllowTag(AllowTagConfig),
    CardinalityLimit(CardinalityLimitConfig),
    AggregateMetrics(AggregateMetricsConfig),
    Sample(SampleConfig),
    AddTag(AddTagConfig),
    TagCardinalityLimit(TagCardinalityLimitConfig),
}

#[cfg_attr(feature = "cli", derive(Deserialize))]
#[derive(Debug, PartialEq)]
pub struct DenyTagConfig {
    pub tags: Vec<String>,
}

#[cfg_attr(feature = "cli", derive(Deserialize))]
#[derive(Debug, PartialEq)]
pub struct AllowTagConfig {
    pub tags: Vec<String>,
}

#[cfg_attr(feature = "cli", derive(Deserialize))]
#[derive(Debug, PartialEq)]
pub struct LimitConfig {
    pub window: u16, // in seconds
    pub limit: u64,
}

#[cfg_attr(feature = "cli", derive(Deserialize))]
#[derive(Debug, PartialEq)]
pub struct CardinalityLimitConfig {
    pub limits: Vec<LimitConfig>,
}

#[cfg_attr(feature = "cli", derive(Deserialize))]
#[derive(Debug, Eq, Hash, PartialEq)]
pub struct TagLimitConfig {
    pub tag: String,
    pub limit: u64,
}

#[cfg_attr(feature = "cli", derive(Deserialize))]
#[derive(Debug, PartialEq)]
pub struct TagCardinalityLimitConfig {
    pub limits: Vec<TagLimitConfig>,
}

#[cfg_attr(feature = "cli", derive(Deserialize))]
#[derive(Debug, PartialEq)]
pub struct AddTagConfig {
    pub tags: Vec<String>,
}

#[cfg(feature = "cli")]
fn default_true() -> bool {
    true
}

#[cfg(feature = "cli")]
fn default_flush_interval() -> Duration {
    Duration::from_secs(1)
}

#[cfg(feature = "cli")]
fn default_flush_offset() -> i64 {
    0
}

#[cfg_attr(feature = "cli", derive(Deserialize))]
#[derive(Debug, PartialEq)]
pub struct AggregateMetricsConfig {
    #[cfg_attr(feature = "cli", serde(default = "default_true"))]
    pub aggregate_counters: bool,
    #[cfg_attr(feature = "cli", serde(default = "default_true"))]
    pub aggregate_gauges: bool,
    #[cfg_attr(feature = "cli", serde(default = "default_flush_interval"))]
    pub flush_interval: Duration,
    #[cfg_attr(feature = "cli", serde(default = "default_flush_offset"))]
    pub flush_offset: i64,
    #[cfg_attr(feature = "cli", serde(default))]
    pub max_map_size: Option<usize>,
}

#[cfg_attr(feature = "cli", derive(Deserialize))]
#[derive(Debug, PartialEq)]
pub struct SampleConfig {
    pub sample_rate: f64,
}

#[cfg(test)]
#[cfg(feature = "cli")]
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
                        flush_interval: 1s,
                        flush_offset: 0,
                        max_map_size: None,
                    },
                ),
            ],
        }
        "###);
    }
}
