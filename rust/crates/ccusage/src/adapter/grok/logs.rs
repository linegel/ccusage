use std::{collections::HashMap, fs, path::Path};

use serde::Deserialize;

use crate::{TimestampMs, parse_ts_timestamp};

const INFERENCE_DONE_MSG: &str = "shell.turn.inference_done";

#[derive(Debug, Deserialize)]
struct LogLine {
    ts: String,
    sid: Option<String>,
    msg: String,
    ctx: Option<LogContext>,
}

#[derive(Debug, Deserialize)]
struct LogContext {
    prompt_tokens: Option<u64>,
    cached_prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
}

#[derive(Debug)]
pub(super) struct LogInference {
    pub(super) timestamp: TimestampMs,
    pub(super) timestamp_text: String,
    pub(super) prompt_tokens: u64,
    pub(super) cached_prompt_tokens: u64,
    pub(super) completion_tokens: u64,
}

pub(super) type LogUsageBySession = HashMap<String, Vec<LogInference>>;

pub(super) fn load_inference_usage(log_files: &[std::path::PathBuf]) -> LogUsageBySession {
    let mut usage = LogUsageBySession::new();
    for file in log_files {
        collect_file(file, &mut usage);
    }
    usage
}

fn collect_file(path: &Path, usage: &mut LogUsageBySession) {
    let Ok(content) = fs::read_to_string(path) else {
        return;
    };
    for line in content.lines() {
        if !line.contains(INFERENCE_DONE_MSG) {
            continue;
        }
        let Ok(parsed) = serde_json::from_str::<LogLine>(line) else {
            continue;
        };
        if parsed.msg != INFERENCE_DONE_MSG {
            continue;
        }
        let (Some(sid), Some(ctx)) = (parsed.sid, parsed.ctx) else {
            continue;
        };
        let Some(timestamp) = parse_ts_timestamp(&parsed.ts) else {
            continue;
        };
        let prompt_tokens = ctx.prompt_tokens.unwrap_or(0);
        let completion_tokens = ctx.completion_tokens.unwrap_or(0);
        if prompt_tokens == 0 && completion_tokens == 0 {
            continue;
        }
        usage.entry(sid).or_default().push(LogInference {
            timestamp,
            timestamp_text: parsed.ts,
            prompt_tokens,
            cached_prompt_tokens: ctx.cached_prompt_tokens.unwrap_or(0),
            completion_tokens,
        });
    }
}

#[cfg(test)]
mod tests {
    use ccusage_test_support::fs_fixture;

    use super::*;

    #[test]
    fn collects_inference_usage_by_session() {
        let fixture = fs_fixture!({
            "logs/unified.jsonl": concat!(
                r#"{"ts":"2026-07-03T05:00:07.907Z","src":"shell","sid":"session-a","msg":"shell.turn.inference_done","ctx":{"loop_index":21,"prompt_tokens":70634,"cached_prompt_tokens":69632,"completion_tokens":2699,"reasoning_tokens":2696}}"#,
                "\n",
                r#"{"ts":"2026-07-03T05:00:08.000Z","src":"shell","sid":"session-a","msg":"shell.turn.inference_start","ctx":{"loop_index":22}}"#,
                "\n",
                r#"{"ts":"2026-07-03T05:00:10.616Z","src":"shell","sid":"session-b","msg":"shell.turn.inference_done","ctx":{"prompt_tokens":100,"cached_prompt_tokens":40,"completion_tokens":10}}"#,
                "\n",
                "not json\n",
            ),
        });

        let usage = load_inference_usage(&[fixture.path("logs/unified.jsonl")]);

        assert_eq!(usage.len(), 2);
        let a = &usage["session-a"];
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].prompt_tokens, 70634);
        assert_eq!(a[0].cached_prompt_tokens, 69632);
        assert_eq!(a[0].completion_tokens, 2699);
        assert_eq!(usage["session-b"][0].prompt_tokens, 100);
    }
}
