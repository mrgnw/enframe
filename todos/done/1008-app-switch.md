---
branch: app-switch
---

# App switch: the wordmark is the menu

Issue: https://github.com/mrgnw/enframe/issues/21 · Mockup: `mockups/nav/01-wordmark-menu.html` (https://mox.xcc.es/enframe/nav/01-wordmark-menu.html) · Model: Sonnet · Size: S

## Why

enframe has two screens a person moves between, the framer and Stabilize, and today the only
way across is the command bar. Mockup 1 was chosen on 2026-10-08: the wordmark each screen
already shows becomes the switch. Nothing is added to the chrome; a caret is the only cue.

## What

One component, `AppSwitch`, rendered where each screen shows its name:

- The framer, library and editor pages get an `enframe` wordmark top-left (they have none
  today), in the light shell's type (`--display`, 600, 15px, -0.02em).
- The Stabilize screens (`Marking.svelte` line ~138, `Preview.svelte` line ~94) replace
  their `.wordmark` span with the same component in the dark shell.
- Click opens a two-item menu below: `enframe` and `Stabilize`, current item ticked and
  inert, the other a link with its shortcut (`⌘1` → `/`, `⌘2` → `/stabilize`). Esc, outside
  click and choosing close it. The caret rotates while open.
- `⌘1` / `⌘2` work from anywhere, added to the existing keydown handler in
  `CommandBar.svelte` next to `⌘K`; not while typing in a field.
- On phones the Stabilize wordmark is hidden today (`desk`); the component shows as a 28px
  icon-only pill there instead, same menu.
- Styling from the mockup: menu 164px min, 12px radius, 1px border, blur, the shell's
  `--nbg`/`--nb` equivalents (`--card`/`--border` in the light shell, `--surface`/`--border`
  in `stabilize.css`). Use bits-ui's `DropdownMenu` if the project already uses it for the
  command bar's menus; otherwise a plain `{#if}` with `transition:slide`, as `web-islands`
  does.

## Done when

- [x] The wordmark on every screen opens the menu and switches; `⌘1`/`⌘2` switch without it.
- [x] A Playwright test (extend `e2e/marking.spec.ts` or a small `e2e/nav.spec.ts`) switches
      Stabilize → enframe → Stabilize by menu and by key; screenshots of the open menu in
      both shells to `captures/nav/`.
- [x] `bun run check`, `bun run lint`, `bun run test`, `bun run test:e2e` pass; a line under
      Unreleased in CHANGELOG.md.

## Result

`AppSwitch.svelte` is the wordmark on all five screens: the layout renders it top-left for the framer, library and editor (offset past the traffic lights in the desktop shell); Marking and Preview render it in their dark dock, as a 28px icon pill on phones. ⌘1/⌘2 switch from anywhere except typed fields. Two z-index bugs found by the e2e were fixed: the Stabilize dock sat level with the features panel, and Standalone's corner slot covered the framer wordmark. e2e 7/7 with `e2e/app-switch.spec.ts`; screenshots in `captures/nav/`.
