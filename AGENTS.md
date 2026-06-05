# Agents (Cursor)

This repo uses **Cursor rules** under [`.cursor/rules/`](.cursor/rules/) as role presets. Enable the rule that matches your task (Cursor rule picker / project rules).

| Role | Rule file | Use when |
|------|-----------|----------|
| **Stack** (always on) | `supazip-stack.mdc` | Automatic — workspace paths and stack |
| **Architect** | `agent-architect.mdc` | Design, boundaries, tech choices, memory bank setup and broad updates |
| **Code** | `agent-code.mdc` | Implementing or refactoring Rust in core/GUI/CLI |
| **Debug** | `agent-debug.mdc` | Failures, `cargo test` / `cargo check`, root cause analysis |
| **Ask** | `agent-ask.mdc` | Questions and explanations; minimal file churn |

## Memory bank

- Folder: [`memory-bank/`](memory-bank/).
- Workflow detail: project skill **supazip-memory-bank** (`.cursor/skills/supazip-memory-bank/SKILL.md`).
- Workspace map and crates: **supazip-workspace** (`.cursor/skills/supazip-workspace/SKILL.md`).

## MCP

- **Context7** is configured in [`.cursor/mcp.json`](.cursor/mcp.json) for up-to-date library documentation (egui, zip, tokio, etc.).
- Optional: set a free API key as environment variable `CONTEXT7_API_KEY` on your system for better rate limits (see [Context7 installation](https://context7.com/docs/installation)). Cursor inherits the environment when starting the MCP process; do not commit secrets into `mcp.json`.

## Legacy Copilot modes

GitHub Copilot chat mode sources live under [`.github/*.chatmode.md`](.github). Cursor uses the `.mdc` rules above instead; keep Copilot files in sync manually if you still use both.
