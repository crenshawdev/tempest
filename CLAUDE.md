## Subagent Model Routing

When dispatching subagents, always set `model` explicitly, never rely on inherit:

- `haiku` for mechanical impl (1-2 files, complete spec)
- `sonnet` for integration, multi-file, judgment calls
- `opus` for architecture, design, final review
