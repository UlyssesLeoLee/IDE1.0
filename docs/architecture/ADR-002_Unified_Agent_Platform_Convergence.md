# ADR-002: Convergence of Single AI Chat and Multi-Agent Collaboration Platform

## Status
Accepted

## Date
2026-11-19

## Context
Prior releases maintained two separate AI interfaces:
1. A right-dock "AI Assistant" chat panel for single-agent conversational assistance and inline editing.
2. A separate "Multi-Agent Collaboration Workbench" invoked from a top-right titlebar button, which opened another window/overlay duplicating chat mechanisms and team orchestration.

This separation created user friction:
- Cognitive fragmentation: users were unsure whether to query the single AI chat or open the workbench.
- Redundant UI real estate: the titlebar was cluttered with an extra button that opened redundant views.
- Underpowered single-agent mode: single-agent chat lacked precision switching between team agents, and multi-agent view had an inferior, duplicated text box.

## Decision
1. **Convergence into Single Unified Right Panel**:
   - Merge the Multi-Agent Collaboration Platform and AI Assistant into the expandable right dock (`Dock::Right`).
   - Remove the redundant titlebar workbench button and overflow menu item entirely.
2. **Searchable Agent Dropdown Selector**:
   - The top header of the unified panel features a pure-vector dropdown selector (`dropdown_button` + `Popup::menu`).
   - Supports real-time search filtering across agent names and roles.
   - Provides an explicit toggle between "全员协同调度" (Multi-Agent Team Orchestration) and any individual agent ("Agent #1", "Agent #2", etc.).
3. **Session & Draft Isolation per Agent (`AgentSession`)**:
   - Each agent maintains its own independent conversation history (`items: Vec<Item>`), draft text (`input: String`), task state, and optional dedicated model override (`model_override: Option<String>`).
   - Switching between agents immediately hot-swaps state without losing uncommitted input or polluting sibling agent context.
4. **Full Feature Inheritance**:
   - When focused on a single agent, that agent receives the full original AI chat feature set: code context attachment, inline diff/patch application, MCP tools, persona rules, and execution tracking.

## Consequences
- **Positive**: Clean titlebar, unified mental model, precision control over every individual agent, seamless orchestration from a single pane, and zero UI duplication.
- **Tradeoffs**: Required managing session state lifecycle and synchronization (`sync_active_agent_session`, `swap_agent_session`) across the application state.
