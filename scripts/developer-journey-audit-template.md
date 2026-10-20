# Developer journey audit — Lurker neutral crates

> 层级：L3

This template is the report `scripts/developer-journey-rehearsal.py` fills. The
rehearsal replays the journey of an external developer who adopts the neutral
Lurker crates: clone, read the documentation, scaffold a project, run it,
attach an engine driver and debug a configuration.

The journey budget is **five minutes (300 s)** for the whole chain. A step
that cannot run in the current environment is reported as `blocked` with its
reason; it never disappears from the report.

| Field | Value |
| --- | --- |
| Date | {{DATE}} |
| Profile | {{PROFILE}} |
| Repository | {{REPOSITORY}} |
| Revision | {{REVISION}} |
| Budget | {{BUDGET_SECONDS}} s |
| Total | {{TOTAL_SECONDS}} s |
| Verdict | {{VERDICT}} |

## Step ledger

{{STEPS_TABLE}}

## Friction log

{{FRICTION}}

## Evidence

- Rehearsal command: `{{COMMAND}}`
- Report path: `{{REPORT_PATH}}`
- Machine readable summary: `{{SUMMARY_PATH}}`

## Audit questions

1. Did every step of the journey complete without a manual detour? {{Q1}}
2. Did the documentation answer "where do I start" without reading the source? {{Q2}}
3. Did the scaffolded project build and run on the first attempt? {{Q3}}
4. Could an engine driver be attached to the scaffolded project? {{Q4}}
5. Did a corrupted configuration produce a typed, actionable diagnosis? {{Q5}}
6. What was the slowest step, and what would shorten it? {{Q6}}
7. Did the whole journey stay inside the five minute budget? {{Q7}}
8. Which friction remains for the next rehearsal? {{Q8}}
