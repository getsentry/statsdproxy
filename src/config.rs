#[cfg(feature = "cli")]
use std::fmt::Formatter;
use std::time::Duration;
#[cfg(feature = "cli")]
use serde::de::Visitor;
#[cfg(feature = "cli")]
use serde::{Deserializer};
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
    StripTag(StripTagConfig),
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
#[derive(Debug, Default, PartialEq)]
pub struct StripTagConfig {
    #[cfg_attr(feature = "cli", serde(default))]
    pub starts_with: Vec<String>,
    #[cfg_attr(feature = "cli", serde(default))]
    pub ends_with: Vec<String>,
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
    #[cfg_attr(feature = "cli", serde(default = "default_flush_interval", deserialize_with="deserialize_duration"))]
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

/// Deserializes a number or a time-string into a Duration struct.
/// Numbers without unit suffixes will be treated as seconds while suffixes will be
/// parsed using https://crates.io/crates/humantime
#[cfg(feature = "cli")]
fn deserialize_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error> where D:Deserializer<'de> {
    struct FlushIntervalVisitor;

    impl Visitor<'_> for FlushIntervalVisitor {
        type Value = Duration;

        fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
            formatter.write_str("a non negative number")
        }

        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Duration::from_millis(v))
        }
    }

    deserializer.deserialize_any(FlushIntervalVisitor)
}

#[cfg(test)]
#[cfg(feature = "cli")]
mod tests {
    use super::*;

    #[test]
    fn flush_duration_milliseconds() {
        let yaml = r#"
            middlewares:
              - type: aggregate-metrics
                flush_interval: 125
        "#;
        let config = serde_yaml::from_str::<Config>(yaml).unwrap();
        assert!(matches!(&config.middlewares[0], MiddlewareConfig::AggregateMetrics(c) if c.flush_interval == Duration::from_millis(125)));
    }

    #[test]
    fn flush_duration_negative_number() {
        let yaml = r#"
            middleware:
              - type: aggregate-metrics
                flush_interval: -1000
        "#;
        let config = serde_yaml::from_str::<Config>(yaml);
        assert!(config.is_err());
    }

    #[test]
    fn test_empty_strip_config() {
        let yaml = r#"
            middlewares:
              - type: strip-tag
        "#;
        let config = serde_yaml::from_str::<Config>(yaml).unwrap();
        let empty_config = MiddlewareConfig::StripTag(StripTagConfig {
            starts_with: Vec::new(),
            ends_with: Vec::new(),
        });
        assert_eq!(config.middlewares[0], empty_config);
    }

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
                StripTag(
                    StripTagConfig {
                        starts_with: [
                            "foo",
                        ],
                        ends_with: [
                            "bar",
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
