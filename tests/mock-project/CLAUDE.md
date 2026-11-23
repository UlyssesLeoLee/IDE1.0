# IDE1.0 Mock Workspace Invariants (CLAUDE.md)

- Always preserve pure safe Rust (`#![forbid(unsafe_code)]`) across workspace sources.
- Multi-agent parallel edits must never inject progress logs into fenced source blocks.
- Keep `.mcp.json` external servers disabled in offline CI fixtures (`"disabled": true`).
- Markdown documents use UTF-8 LF endings; `.nema` documents enforce strict 5W1H / Table domain syntax.
