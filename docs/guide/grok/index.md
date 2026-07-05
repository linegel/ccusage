# Grok Build Data Source (Experimental)

> Grok Build support is experimental. Expect breaking changes while both ccusage and Grok Build continue to evolve.

ccusage can read Grok Build session state as one of its supported local data sources, using the same focused and all-source report views as the rest of ccusage.

## Focused Views

::: code-group

```bash [bunx (Recommended)]
bunx ccusage grok --help
```

```bash [npx]
npx ccusage@latest grok --help
```

```bash [pnpm]
pnpm dlx ccusage grok --help
```

:::

## Data Source

The CLI reads Grok Build session files from `GROK_HOME` (defaults to `~/.grok`). `GROK_HOME` can be one directory or a comma-separated list of directories.

```bash
GROK_HOME="$HOME/.grok,/backup/grok" ccusage grok daily
```

```text
~/.grok/
├── logs/
│   └── unified.jsonl
└── sessions/
    └── <encoded-cwd>/
        └── <session-id>/
            ├── signals.json
            └── summary.json
```

`logs/unified.jsonl` supplies per-request token usage while it still covers a session (Grok Build rotates it after a short window); `signals.json` supplies durable session-level context totals; `summary.json` provides session metadata (ID, timestamps, project path, model info).

## Report Views

| Focused view           | Description                       | See also                                |
| ---------------------- | --------------------------------- | --------------------------------------- |
| `ccusage grok daily`   | Aggregate usage by date           | [Daily Usage](/guide/daily-reports)     |
| `ccusage grok monthly` | Aggregate usage by month          | [Monthly Usage](/guide/monthly-reports) |
| `ccusage grok session` | Group usage by Grok Build session | [Session Usage](/guide/session-reports) |

These views support `--json`, `--compact`, `--breakdown`, `--offline`, and standard date filters (daily / monthly / session only; weekly is available only via unified reports). Grok Build data is automatically included in unified reports (see [All Sources (Default)](/guide/all-reports)).

## What Gets Calculated

- **Input/output/cache tokens** — when `logs/unified.jsonl` still covers a session, ccusage reads per-request `prompt_tokens`, `cached_prompt_tokens`, and `completion_tokens` from it (input is the non-cached prompt portion, matching how Codex rows are counted). Grok Build rotates this log after a short window, so only recent activity has a full breakdown.
- **Total tokens** — for sessions (or parts of sessions) the log no longer covers, `contextTokensUsed` plus `totalTokensBeforeCompaction` from `signals.json` is reported as `totalTokens` with zero input/output/cache fields, so no usage is lost.
- **Model attribution** — `primaryModelId`, `current_model_id`, or the first value from `modelsUsed` (from summary or signals) is shown as the model name (e.g. `grok-build-0.1`, `grok-composer-2.5-fast`).
- **Session metadata** — `summary.json` supplies timestamps, session IDs, and project/cwd paths when available.
- **Cost** — Grok Build does not record USD cost and no public pricing is available, so cost is reported as `$0.00`. This is expected. See the adapter notes and [Source Support Q&A](/guide/source-support-qa) for history.

## Environment Variables

| Variable    | Description                                                                                  |
| ----------- | -------------------------------------------------------------------------------------------- |
| `GROK_HOME` | Override the root directory, or comma-separated root directories, containing Grok Build data |
| `LOG_LEVEL` | Adjust verbosity (0 silent ... 5 trace)                                                      |

See [Environment Variables](/guide/environment-variables) and [Configuration Files](/guide/config-files) for precedence and JSON config (`grok` namespace).

## Troubleshooting

::: details No Grok Build usage data found
Ensure the data directory exists at `~/.grok/sessions/` (or your `GROK_HOME`) and contains session directories with `signals.json` files. Set `GROK_HOME` if your Grok Build data lives elsewhere or in multiple archive roots. Empty entries and non-directories are skipped.
:::

::: details Costs showing as $0.00
This is expected for Grok Build rows. Grok Build does not record USD cost and there is no public per-token pricing for its models, so ccusage cannot price the usage. See the [Grok Build adapter README](https://github.com/ccusage/ccusage/tree/main/rust/crates/ccusage/src/adapter/grok) in the source for details.
:::

::: details Input/output/cache showing zero for older days
Grok Build only keeps per-request token usage in `logs/unified.jsonl`, which it rotates after a short window. Days no longer covered by that log fall back to the durable session context totals from `signals.json`, which have no input/output/cache breakdown — only `totalTokens`. Recent activity shows the full breakdown.
:::

## Next Steps
- [All Sources (Default)](/guide/all-reports) — How Grok Build appears in unified views
- [Getting Started](/guide/getting-started)
- [Environment Variables](/guide/environment-variables) (GROK_HOME)
- [Source Support Q&A](/guide/source-support-qa)
