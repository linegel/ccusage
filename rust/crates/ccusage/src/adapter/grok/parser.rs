use std::{fs, path::Path, sync::Arc, time::UNIX_EPOCH};

use jiff::tz::TimeZone as JiffTimeZone;
use serde::Deserialize;

use crate::{
    LoadedEntry, Result, TimestampMs, TokenUsageRaw, UsageEntry, UsageMessage, format_date_tz,
    format_rfc3339_millis, parse_ts_timestamp,
};

const DEFAULT_MODEL: &str = "grok-build";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GrokSignals {
    #[serde(default)]
    context_tokens_used: u64,
    #[serde(default)]
    total_tokens_before_compaction: u64,
    primary_model_id: Option<String>,
    #[serde(default)]
    models_used: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct GrokSummary {
    current_model_id: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
    last_active_at: Option<String>,
    git_root_dir: Option<String>,
    info: Option<GrokSummaryInfo>,
}

#[derive(Debug, Default, Deserialize)]
struct GrokSummaryInfo {
    id: Option<String>,
    cwd: Option<String>,
}

struct GrokSessionRecord {
    timestamp: TimestampMs,
    timestamp_text: String,
    session_id: String,
    project_path: String,
    model: String,
    total_tokens: u64,
}

pub(super) fn read_session(
    signals_path: &Path,
    tz: Option<&JiffTimeZone>,
    log_usage: &super::logs::LogUsageBySession,
) -> Result<Vec<LoadedEntry>> {
    let content = fs::read_to_string(signals_path)?;
    let Ok(signals) = serde_json::from_str::<GrokSignals>(&content) else {
        return Ok(Vec::new());
    };
    let total_tokens = signals
        .context_tokens_used
        .saturating_add(signals.total_tokens_before_compaction);
    let summary = read_summary(signals_path);
    let record = session_record(signals_path, signals, summary, total_tokens);
    if let Some(inferences) = log_usage.get(&record.session_id) {
        let mut entries = inferences
            .iter()
            .map(|inference| inference_to_loaded(&record, inference, tz))
            .collect::<Vec<_>>();
        let logged_tokens = inferences
            .iter()
            .map(|inference| {
                inference
                    .prompt_tokens
                    .saturating_add(inference.completion_tokens)
            })
            .sum::<u64>();
        // unified.jsonl is a short-retention rolling log; when it only covers
        // the tail of a session, keep the uncovered remainder of the recorded
        // session totals so the session never reports less than signals.json.
        let residual_tokens = total_tokens.saturating_sub(logged_tokens);
        if residual_tokens > 0 {
            let mut record = record;
            record.total_tokens = residual_tokens;
            entries.push(record_to_loaded(record, tz));
        }
        return Ok(entries);
    }
    if total_tokens == 0 {
        return Ok(Vec::new());
    }
    Ok(vec![record_to_loaded(record, tz)])
}

fn inference_to_loaded(
    record: &GrokSessionRecord,
    inference: &super::logs::LogInference,
    tz: Option<&JiffTimeZone>,
) -> LoadedEntry {
    let usage = TokenUsageRaw {
        input_tokens: inference
            .prompt_tokens
            .saturating_sub(inference.cached_prompt_tokens),
        output_tokens: inference.completion_tokens,
        cache_creation_input_tokens: 0,
        cache_read_input_tokens: inference.cached_prompt_tokens,
        speed: None,
        cache_creation: None,
    };
    loaded_entry(
        record,
        usage,
        0,
        inference.timestamp,
        inference.timestamp_text.clone(),
        tz,
    )
}

fn read_summary(signals_path: &Path) -> GrokSummary {
    let summary_path = signals_path.with_file_name("summary.json");
    let Ok(content) = fs::read_to_string(summary_path) else {
        return GrokSummary::default();
    };
    serde_json::from_str::<GrokSummary>(&content).unwrap_or_default()
}

fn session_record(
    signals_path: &Path,
    signals: GrokSignals,
    summary: GrokSummary,
    total_tokens: u64,
) -> GrokSessionRecord {
    let fallback_timestamp = file_modified_timestamp(signals_path);
    let timestamp_text = summary
        .last_active_at
        .as_deref()
        .or(summary.updated_at.as_deref())
        .or(summary.created_at.as_deref())
        .and_then(normalize_grok_timestamp)
        .unwrap_or_else(|| format_rfc3339_millis(fallback_timestamp));
    let timestamp = parse_ts_timestamp(&timestamp_text).unwrap_or(fallback_timestamp);
    let session_id = summary
        .info
        .as_ref()
        .and_then(|info| info.id.clone())
        .or_else(|| session_id_from_path(signals_path))
        .unwrap_or_else(|| "unknown".to_string());
    let project_path = summary
        .info
        .as_ref()
        .and_then(|info| info.cwd.clone())
        .or(summary.git_root_dir)
        .or_else(|| project_path_from_session_file(signals_path))
        .unwrap_or_else(|| "Grok Build".to_string());
    let model = signals
        .primary_model_id
        .or(summary.current_model_id)
        .or_else(|| signals.models_used.first().cloned())
        .filter(|model| !model.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_MODEL.to_string());
    GrokSessionRecord {
        timestamp,
        timestamp_text,
        session_id,
        project_path: project_path.trim_end_matches('/').to_string(),
        model,
        total_tokens,
    }
}

fn record_to_loaded(record: GrokSessionRecord, tz: Option<&JiffTimeZone>) -> LoadedEntry {
    let usage = TokenUsageRaw {
        input_tokens: 0,
        output_tokens: 0,
        cache_creation_input_tokens: 0,
        cache_read_input_tokens: 0,
        speed: None,
        cache_creation: None,
    };
    let timestamp = record.timestamp;
    let timestamp_text = record.timestamp_text.clone();
    let extra_total_tokens = record.total_tokens;
    loaded_entry(
        &record,
        usage,
        extra_total_tokens,
        timestamp,
        timestamp_text,
        tz,
    )
}

fn loaded_entry(
    record: &GrokSessionRecord,
    usage: TokenUsageRaw,
    extra_total_tokens: u64,
    timestamp: TimestampMs,
    timestamp_text: String,
    tz: Option<&JiffTimeZone>,
) -> LoadedEntry {
    let data = UsageEntry {
        session_id: Some(record.session_id.clone()),
        timestamp: timestamp_text,
        version: None,
        message: UsageMessage {
            usage,
            model: Some(record.model.clone()),
            id: None,
        },
        cost_usd: None,
        request_id: None,
        is_api_error_message: None,
        is_sidechain: None,
    };
    LoadedEntry {
        data,
        timestamp,
        date: format_date_tz(timestamp, tz),
        project: Arc::from("grok"),
        session_id: Arc::from(record.session_id.as_str()),
        project_path: Arc::from(record.project_path.as_str()),
        cost: 0.0,
        extra_total_tokens,
        credits: None,
        message_count: None,
        model: Some(record.model.clone()),
        usage_limit_reset_time: None,
        missing_pricing_model: None,
    }
}

fn normalize_grok_timestamp(value: &str) -> Option<String> {
    if parse_ts_timestamp(value).is_some() {
        return Some(value.to_string());
    }
    let dot = value.find('.')?;
    let timezone_offset = value[dot + 1..]
        .find(['Z', '+', '-'])
        .map(|offset| dot + 1 + offset)?;
    let fraction = &value[dot + 1..timezone_offset];
    if fraction.len() < 3 {
        return None;
    }
    let normalized = format!(
        "{}.{}{}",
        &value[..dot],
        &fraction[..3],
        &value[timezone_offset..]
    );
    parse_ts_timestamp(&normalized).map(|_| normalized)
}

fn file_modified_timestamp(path: &Path) -> TimestampMs {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .map(TimestampMs::from_millis)
        .unwrap_or(TimestampMs::UNIX_EPOCH)
}

fn session_id_from_path(path: &Path) -> Option<String> {
    path.parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

fn project_path_from_session_file(path: &Path) -> Option<String> {
    let encoded = path
        .parent()?
        .parent()?
        .file_name()
        .and_then(|name| name.to_str())?;
    let decoded = percent_decode(encoded);
    (!decoded.is_empty()).then_some(decoded)
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let Some(value) = hex_byte(bytes[index + 1], bytes[index + 2])
        {
            output.push(value);
            index += 3;
            continue;
        }
        output.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn hex_byte(high: u8, low: u8) -> Option<u8> {
    Some(hex_value(high)? * 16 + hex_value(low)?)
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use ccusage_test_support::fs_fixture;
    use jiff::tz::TimeZone;

    use super::*;

    #[test]
    fn loads_grok_session_from_signals_and_summary() {
        let fixture = fs_fixture!({
            "sessions/%2Fworkspace%2Fapi/session-a/signals.json": r#"{
                "contextTokensUsed": 100,
                "primaryModelId": "grok-build"
            }"#,
            "sessions/%2Fworkspace%2Fapi/session-a/summary.json": r#"{
                "last_active_at": "2026-05-22T00:00:00.000Z",
                "info": { "id": "session-a", "cwd": "/workspace/api" }
            }"#,
        });

        let entries = read_session(
            &fixture.path("sessions/%2Fworkspace%2Fapi/session-a/signals.json"),
            Some(&TimeZone::UTC),
            &super::super::logs::LogUsageBySession::new(),
        )
        .unwrap();

        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.date, "2026-05-22");
        assert_eq!(entry.extra_total_tokens, 100);
        assert_eq!(entry.model.as_deref(), Some("grok-build"));
        assert_eq!(entry.project.as_ref(), "grok");
    }

    #[test]
    fn splits_session_into_per_inference_entries_when_logs_cover_it() {
        let fixture = fs_fixture!({
            "sessions/%2Fworkspace%2Fapi/session-a/signals.json": r#"{
                "contextTokensUsed": 100,
                "primaryModelId": "grok-build-0.1"
            }"#,
            "sessions/%2Fworkspace%2Fapi/session-a/summary.json": r#"{
                "last_active_at": "2026-07-03T06:00:00.000Z",
                "info": { "id": "session-a", "cwd": "/workspace/api" }
            }"#,
        });
        let mut log_usage = super::super::logs::LogUsageBySession::new();
        log_usage.insert(
            "session-a".to_string(),
            vec![
                super::super::logs::LogInference {
                    timestamp: parse_ts_timestamp("2026-07-03T05:00:07.907Z").unwrap(),
                    timestamp_text: "2026-07-03T05:00:07.907Z".to_string(),
                    prompt_tokens: 70634,
                    cached_prompt_tokens: 69632,
                    completion_tokens: 2699,
                },
                super::super::logs::LogInference {
                    timestamp: parse_ts_timestamp("2026-07-04T05:00:10.616Z").unwrap(),
                    timestamp_text: "2026-07-04T05:00:10.616Z".to_string(),
                    prompt_tokens: 100,
                    cached_prompt_tokens: 40,
                    completion_tokens: 10,
                },
            ],
        );

        let entries = read_session(
            &fixture.path("sessions/%2Fworkspace%2Fapi/session-a/signals.json"),
            Some(&TimeZone::UTC),
            &log_usage,
        )
        .unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].date, "2026-07-03");
        assert_eq!(entries[0].data.message.usage.input_tokens, 1002);
        assert_eq!(entries[0].data.message.usage.cache_read_input_tokens, 69632);
        assert_eq!(entries[0].data.message.usage.output_tokens, 2699);
        assert_eq!(entries[0].extra_total_tokens, 0);
        assert_eq!(entries[0].model.as_deref(), Some("grok-build-0.1"));
        assert_eq!(entries[1].date, "2026-07-04");
        assert_eq!(entries[1].data.message.usage.input_tokens, 60);
    }

    #[test]
    fn keeps_residual_session_totals_when_logs_only_cover_the_tail() {
        let fixture = fs_fixture!({
            "sessions/%2Fworkspace%2Fapi/session-a/signals.json": r#"{
                "contextTokensUsed": 1000000,
                "primaryModelId": "grok-build-0.1"
            }"#,
            "sessions/%2Fworkspace%2Fapi/session-a/summary.json": r#"{
                "last_active_at": "2026-07-05T06:00:00.000Z",
                "info": { "id": "session-a", "cwd": "/workspace/api" }
            }"#,
        });
        let mut log_usage = super::super::logs::LogUsageBySession::new();
        log_usage.insert(
            "session-a".to_string(),
            vec![super::super::logs::LogInference {
                timestamp: parse_ts_timestamp("2026-07-05T05:00:00.000Z").unwrap(),
                timestamp_text: "2026-07-05T05:00:00.000Z".to_string(),
                prompt_tokens: 70000,
                cached_prompt_tokens: 60000,
                completion_tokens: 3000,
            }],
        );

        let entries = read_session(
            &fixture.path("sessions/%2Fworkspace%2Fapi/session-a/signals.json"),
            Some(&TimeZone::UTC),
            &log_usage,
        )
        .unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].data.message.usage.input_tokens, 10000);
        assert_eq!(entries[1].extra_total_tokens, 1000000 - 73000);
        assert_eq!(entries[1].data.message.usage.input_tokens, 0);
    }
}
