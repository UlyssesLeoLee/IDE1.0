# ADR-001: User-Configured Model Profile Registry & Synthetic Model Eradication

## Status
Accepted

## Date
2026-11-19

## Context
Previously, the IDE model selection dropdown presented hardcoded placeholder/popular models (e.g. standard mock model names) even when the user had not configured or tested them. This created:
1. False affordances where users selected unauthenticated or non-existent endpoints.
2. Loss of user model configuration across IDE restarts (model profiles were not cleanly multi-tenant and didn't persist user-defined names, base URLs, or efforts).
3. Inability for individual sub-agents to bind to dedicated models while retaining a global default.

## Decision
1. **Zero Synthetic Model Invariant**: `AiConfig::configured_models()` returns ONLY models explicitly configured or saved by the user. If no models are configured, the list is strictly empty (`[]`), prompting the user to add their own model via the settings drawer.
2. **First-Class `ModelProfile` Abstraction**:
   - Fields: `name: String`, `base_url: String`, `model: String`, `key: String`, `effort: String`, `efforts: Vec<String>`.
   - Security Invariant: `std::fmt::Debug` MUST redact the `key` field (`<redacted>`).
   - Storage: Persisted under `savedModels` in `ai.json` with backward-compatible migration from legacy single-model configs.
3. **Atomic Upsert & Selection**:
   - `upsert_current_model(custom_name)` merges active parameters into the profile registry.
   - `select_saved_model(model_name)` cleanly swaps the active model configuration.
   - `remove_saved_model(model_name)` drops profiles and safely falls back to remaining profiles or clear state.
   - `resolve_for_model(model_override)` allows individual agents to override the model while falling back cleanly to global configuration when none is set.

## Consequences
- **Positive**: Eliminates confusion, completely removes cheap mock placeholders, gives users full control over custom LLM endpoints (Local Ollama, vLLM, DeepSeek, OpenAI, Anthropic, Gemini, etc.), and guarantees secure persistence across restarts.
- **Tradeoffs**: Users on a fresh, clean install must enter at least one model configuration before the model selector dropdown populates. A prominent, intuitive "添加或管理模型…" CTA button is provided in empty state.
