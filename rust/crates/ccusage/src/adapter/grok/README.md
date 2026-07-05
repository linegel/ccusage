# Grok Build Adapter

The Grok Build adapter reads local session state from:

```text
${GROK_HOME:-~/.grok}/sessions/<encoded-cwd>/<session-id>/
${GROK_HOME:-~/.grok}/logs/unified.jsonl
```

Two data sources are combined per session:

- `logs/unified.jsonl` provides per-request usage via
  `shell.turn.inference_done` records (`prompt_tokens`,
  `cached_prompt_tokens`, `completion_tokens`, keyed by `sid`). When a session
  is covered, ccusage emits one entry per inference with input tokens set to
  the non-cached prompt portion, cache read tokens set to
  `cached_prompt_tokens`, and output tokens set to `completion_tokens`
  (matching the Codex non-cached-input convention). Grok Build rotates this
  log after a short window, so only recent activity has full coverage.
- `signals.json` provides durable session-level context totals
  (`contextTokensUsed` + `totalTokensBeforeCompaction`), and `summary.json`
  supplies metadata such as the session id, model, project path, and last
  activity time. Sessions without any log coverage report the context total as
  `totalTokens` with zero input/output/cache fields. When the log only covers
  the tail of a session, the uncovered remainder of the context total is kept
  as a residual entry so the session never reports less than `signals.json`.

Cost is reported as `$0.00` because Grok Build does not record USD cost and no
public per-token pricing is available for its models.

Model names are reported as-is (e.g. `grok-build-0.1`,
`grok-composer-2.5-fast`); no disambiguation prefix is needed because the
names are already unambiguous in unified reports.
