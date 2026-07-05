use crate::{LoadedEntry, Result, cli::SharedArgs, parse_tz};

use super::{
    logs::load_inference_usage,
    parser::read_session,
    paths::{discover_log_files, discover_signal_files},
};

pub(crate) fn load_entries(shared: &SharedArgs) -> Result<Vec<LoadedEntry>> {
    crate::progress::track_usage_load(crate::progress::UsageLoadAgent::Grok, shared.json, || {
        load_entries_inner(shared)
    })
}

fn load_entries_inner(shared: &SharedArgs) -> Result<Vec<LoadedEntry>> {
    let tz = parse_tz(shared.timezone.as_deref());
    let log_usage = load_inference_usage(&discover_log_files()?);
    let mut entries = Vec::new();
    for file in discover_signal_files()? {
        entries.extend(read_session(&file, tz.as_ref(), &log_usage)?);
    }
    entries.sort_by_key(|entry| entry.timestamp);
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use ccusage_test_support::{EnvVarGuard, fs_fixture};

    use super::super::paths::GROK_HOME_ENV;
    use super::*;

    #[test]
    fn loads_entries_from_grok_home_sessions() {
        let _guard = super::super::GROK_HOME_LOCK.lock().unwrap();
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
        let _cleanup = EnvVarGuard::set(GROK_HOME_ENV, fixture.root());
        let shared = SharedArgs {
            timezone: Some("UTC".to_string()),
            ..SharedArgs::default()
        };

        let entries = load_entries(&shared).unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].date, "2026-05-22");
        assert_eq!(entries[0].extra_total_tokens, 100);
    }

    #[test]
    fn prefers_per_inference_log_usage_over_session_totals() {
        let _guard = super::super::GROK_HOME_LOCK.lock().unwrap();
        let fixture = fs_fixture!({
            "sessions/%2Fworkspace%2Fapi/session-a/signals.json": r#"{
                "contextTokensUsed": 100,
                "primaryModelId": "grok-build-0.1"
            }"#,
            "sessions/%2Fworkspace%2Fapi/session-a/summary.json": r#"{
                "last_active_at": "2026-07-03T06:00:00.000Z",
                "info": { "id": "session-a", "cwd": "/workspace/api" }
            }"#,
            "logs/unified.jsonl": concat!(
                r#"{"ts":"2026-07-03T05:00:07.907Z","src":"shell","sid":"session-a","msg":"shell.turn.inference_done","ctx":{"prompt_tokens":70634,"cached_prompt_tokens":69632,"completion_tokens":2699}}"#,
                "\n",
            ),
        });
        let _cleanup = EnvVarGuard::set(GROK_HOME_ENV, fixture.root());
        let shared = SharedArgs {
            timezone: Some("UTC".to_string()),
            ..SharedArgs::default()
        };

        let entries = load_entries(&shared).unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].date, "2026-07-03");
        assert_eq!(entries[0].data.message.usage.input_tokens, 1002);
        assert_eq!(entries[0].data.message.usage.cache_read_input_tokens, 69632);
        assert_eq!(entries[0].data.message.usage.output_tokens, 2699);
        assert_eq!(entries[0].extra_total_tokens, 0);
    }
}
