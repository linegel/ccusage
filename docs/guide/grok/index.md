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
└── sessions/
    └── <encoded-cwd>/
        └── <session-id>/
            ├── signals.json
            └── summary.json
```

`signals.json` supplies the primary usage data; `summary.json` provides session metadata (ID, timestamps, project path, model info).

## Report Views

| Focused view           | Description                       | See also                                |
| ---------------------- | --------------------------------- | --------------------------------------- |
| `ccusage grok daily`   | Aggregate usage by date           | [Daily Usage](/guide/daily-reports)     |
| `ccusage grok monthly` | Aggregate usage by month          | [Monthly Usage](/guide/monthly-reports) |
| `ccusage grok session` | Group usage by Grok Build session | [Session Usage](/guide/session-reports) |

These views support `--json`, `--compact`, `--breakdown`, `--offline`, and standard date filters (daily / monthly / session only; weekly is available only via unified reports). Grok Build data is automatically included in unified reports (see [All Sources (Default)](/guide/all-reports)).

## What Gets Calculated

- **Total tokens** — `contextTokensUsed` plus `totalTokensBeforeCompaction` from `signals.json` is reported as `totalTokens`.
- **Model attribution** — `primaryModelId`, `current_model_id`, or the first value from `modelsUsed` (from summary or signals) is shown as the model name (prefixed `[grok] ` for disambiguation in combined reports).
- **Session metadata** — `summary.json` supplies timestamps, session IDs, and project/cwd paths when available.
- **Cost** — Grok Build session state does not currently expose per-request input/output/cache token counts or recorded USD cost, so cost is reported as `$0.00` (and input/output/cache fields are zero). This is expected. See the adapter notes and [Source Support Q&A](/guide/source-support-qa) for history.

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

::: details Costs showing as $0.00 (and zero input/output/cache)
This is expected for Grok Build rows today. ccusage reports the persisted context token totals from `signals.json`, but Grok Build does not yet persist a stable per-request token breakdown or recorded USD cost that ccusage can price. When Grok Build adds that data, the adapter can be updated to populate the standard fields and enable costing. See the [Grok Build adapter README](https://github.com/ccusage/ccusage/tree/main/rust/crates/ccusage/src/adapter/grok) in the source for details.
:::

## Next Steps
- [All Sources (Default)](/guide/all-reports) — How Grok Build appears in unified views
- [Getting Started](/guide/getting-started)
- [Environment Variables](/guide/environment-variables) (GROK_HOME)
- [Source Support Q&A](/guide/source-support-qa)
