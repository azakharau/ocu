use std::env;

use chrono::{TimeZone, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct UsageSnapshot {
    pub(crate) buckets: Vec<UsageBucket>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct UsageBucket {
    pub(crate) title: &'static str,
    pub(crate) windows: [WindowUsage; 2],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct WindowUsage {
    pub(crate) label: &'static str,
    pub(crate) reset_at: Option<i64>,
    pub(crate) used_percent: u16,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum SnapshotError {
    #[error("usage payload is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Deserialize)]
struct ApiPayload {
    #[serde(default)]
    rate_limit: ApiRateLimit,
    #[serde(default)]
    additional_rate_limits: Vec<ApiAdditionalRateLimit>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ApiAdditionalRateLimit {
    #[serde(default)]
    rate_limit: ApiRateLimit,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ApiRateLimit {
    #[serde(default)]
    primary_window: ApiWindow,
    #[serde(default)]
    secondary_window: ApiWindow,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ApiWindow {
    #[serde(default, deserialize_with = "deserialize_optional_i64")]
    reset_at: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_optional_f64")]
    used_percent: Option<f64>,
}

pub(crate) fn parse_snapshot(payload: &str) -> Result<UsageSnapshot, SnapshotError> {
    let payload: ApiPayload = serde_json::from_str(payload)?;
    let spark_limit = payload
        .additional_rate_limits
        .first()
        .map(|limit| &limit.rate_limit)
        .cloned()
        .unwrap_or_default();

    Ok(UsageSnapshot {
        buckets: vec![
            bucket_from_rate_limit("Main Codex bucket", &payload.rate_limit),
            bucket_from_rate_limit("Codex 5.3 Spark", &spark_limit),
        ],
    })
}

impl WindowUsage {
    pub(crate) fn reset_at_text(&self) -> String {
        self.reset_at
            .and_then(|timestamp| Utc.timestamp_opt(timestamp, 0).single())
            .map(|datetime| datetime.with_timezone(&local_timezone()))
            .map(|datetime| datetime.format("%b %-d, %Y %H:%M:%S").to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }
}

fn local_timezone() -> Tz {
    env::var("TZ")
        .ok()
        .and_then(|timezone| timezone.parse::<Tz>().ok())
        .unwrap_or(chrono_tz::UTC)
}

fn bucket_from_rate_limit(title: &'static str, rate_limit: &ApiRateLimit) -> UsageBucket {
    UsageBucket {
        title,
        windows: [
            window_from_api("5h", &rate_limit.primary_window),
            window_from_api("weekly", &rate_limit.secondary_window),
        ],
    }
}

fn window_from_api(label: &'static str, window: &ApiWindow) -> WindowUsage {
    WindowUsage {
        label,
        reset_at: window.reset_at,
        used_percent: normalized_percent(window.used_percent),
    }
}

fn normalized_percent(value: Option<f64>) -> u16 {
    let Some(value) = value else {
        return 0;
    };
    if !value.is_finite() {
        return 0;
    }
    value.floor().clamp(0.0, 100.0) as u16
}

fn deserialize_optional_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(value.and_then(|value| match value {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_f64().map(|n| n as i64)),
        Value::String(text) => text.parse::<i64>().ok(),
        _ => None,
    }))
}

fn deserialize_optional_f64<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(value.and_then(|value| match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.parse::<f64>().ok(),
        _ => None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_snapshot_should_build_two_codex_buckets() {
        let payload = r#"{
            "rate_limit": {
                "primary_window": { "reset_at": 1781517330, "used_percent": 15.9 },
                "secondary_window": { "reset_at": "1781763927", "used_percent": "38.2" }
            },
            "additional_rate_limits": [{
                "rate_limit": {
                    "primary_window": { "reset_at": null, "used_percent": null },
                    "secondary_window": { "reset_at": 1782143467, "used_percent": 3 }
                }
            }]
        }"#;

        let snapshot = parse_snapshot(payload).expect("valid payload");

        assert_eq!(snapshot.buckets.len(), 2);
        assert_eq!(snapshot.buckets[0].title, "Main Codex bucket");
        assert_eq!(snapshot.buckets[0].windows[0].used_percent, 15);
        assert_eq!(snapshot.buckets[0].windows[1].used_percent, 38);
        assert_eq!(snapshot.buckets[1].title, "Codex 5.3 Spark");
    }

    #[test]
    fn normalized_percent_should_clamp_invalid_and_out_of_range_values() {
        assert_eq!(normalized_percent(None), 0);
        assert_eq!(normalized_percent(Some(-10.0)), 0);
        assert_eq!(normalized_percent(Some(101.8)), 100);
    }
}
