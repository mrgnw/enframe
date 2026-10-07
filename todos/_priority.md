# Priority

Work top to bottom. Run at most two at once, and only a pair the plan marks safe. See [PLAN.md](../PLAN.md) for the rules.

1. MVP, standalone: done (M1, M2, M3). `cd web && pnpm build && pnpm start`, then http://localhost:7341/.
2. UI in enframe, later: [E2](E2-serve-endpoint.md) (waits for enframe's `phase-2-remote-targets` merge), [E3](E3-marking-screen.md), [E4](E4-preview-export.md), in that order.
