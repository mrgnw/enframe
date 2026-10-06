# E0 ⚑ · Answer the UI plan's open questions

Depends on: the user · Blocks: E2, E3

`docs/ui-plan.md` ends with seven open questions. The two that change code:

1. **Does a box mark mean its centre?** Today `--at P=x,y,w,h` is a top-left box, while `--at P=x,y` is the anchor's centre. The UI can hide this, but the sidecar and the CLI should agree.
2. **Does dragging a box in the UI set zoom?** Today a box mark keeps the zoom unless a fifth number is given.

Answer these (and the other five) in `docs/ui-plan.md`, then E2 and E3 can start.
