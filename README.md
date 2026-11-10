# SmartTaskRouter

**AI Model & Agent Orchestrator** — intelligent, cost-aware routing for connected AI providers and coding agents.

> Status: product planning and UI/UX design; implementation has not started.

## Product vision

SmartTaskRouter is a desktop-first AI gateway inspired by 9Router. Its differentiator is understanding a user's task and recommending the **appropriate connected model, agent, prompt and GitHub skill**, prioritizing free or local models when they can meet the quality requirements.

- **Connected-model-first:** route only to providers/models explicitly connected by the user.
- **Free-first:** prefer capable free/local models; never silently charge the user.
- **Task-aware:** analyze complexity, risk, tool requirements and expected context.
- **Transparent:** show the route, reasoning, estimated range of cost and fallback policy.
- **Measurable:** measure successful task completion cost, not only price per token.
- **Secure:** user-owned credentials, minimal context and reviewed third-party skills.

## UI screens

| Screen | Purpose |
| --- | --- |
| Overview | Gateway health, connected providers, recent tasks and savings |
| Smart Routing | Task input, analysis, suggested route, selected skills and approval |
| Providers | Connections, credentials, quota and provider health |
| Models | Searchable catalog: free/local/paid, capability, context, price |
| Model Combos | Free First, Balanced, Quality and user-defined routing chains |
| Agents | Connected CLI/IDE agents and multistep execution |
| Skills & Prompts | Curated GitHub skill discovery, review and optional install |
| Task History | Reproducible task traces, retries, model changes and results |
| Usage & Analytics | Token and monetary usage, latency, budget and success rate |
| Routing Rules | Per-workspace limits, fallback policy and approval gates |
| CLI & Endpoints | OpenAI/Anthropic-compatible endpoint and client setup |
| Settings | App theme, diagnostics, security, storage and local runtime |

## Main experience

1. Connect providers (OpenRouter, Gemini, OpenAI, Anthropic, Ollama and others).
2. Choose Free First, Balanced or Quality; enter a task.
3. Analyze task complexity using heuristics/local classification before paid inference.
4. Filter models by capability, quota, privacy, budget and tool support.
5. Recommend specific models, execution agent and relevant verified GitHub skills.
6. Show an execution plan and ask permission before paid escalation or sensitive actions.
7. Execute, validate, retry selectively and display actual usage.

## Implementation notes

- Concept UI follows 9Router's information architecture with an orange-accented light desktop dashboard and optional dark theme.
- Quotas and displayed cost must come from provider APIs or be explicitly labeled as unknown/estimated.
- Support gateway-only mode independently from agent execution.
- Never execute unreviewed GitHub scripts or silently install community skills.
- Do not promise any specific cost saving until measured against a reproducible baseline.

See [full implementation plan](docs/SMARTROUTER_UI_UX_FUNCTIONAL_PLAN.md).
