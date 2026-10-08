# Priority

The board is https://github.com/users/mrgnw/projects/10, and each card's issue carries its status (see PLAN.md, "Where things live"); the integration's cross-repo view is https://github.com/mrgnw/pinframe/issues/2. Work top to bottom. Run at most two at once, and only a pair the plan marks safe. See [PLAN.md](../PLAN.md) for the rules.

1. MVP, standalone: done (M1, M2, M3, spikes W1 and W2). `cd web && pnpm build && pnpm start`, then http://localhost:7341/.
2. [M5](M5-soft-marks.md): soft marks (M4 is done). Running.
3. UI in enframe, later: [E2](E2-serve-endpoint.md) (waits for enframe's `phase-2-remote-targets` merge), [E3](E3-marking-screen.md), [E4](E4-preview-export.md), in that order; then [E6](E6-multiple-features.md), several features per sequence.
