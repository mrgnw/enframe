# F1 ⚑ · Keep or retire the Python reference

Depends on: a decision from the user · Size: S
Touches: `anchor.py`, `analysis/check.sh`, README

## Decision needed

`anchor.py` is the reference that `check.sh` compares the Rust output against (Girona within 2.95e-4). It also has the only interactive box window. Options:

1. **Keep it frozen as the reference** (today). Each Rust change that alters tracking on purpose needs the comparison updated, but there is always an independent second implementation.
2. **Retire it.** Replace the comparison with committed expected outputs, and keep the Python-dump fixtures the Rust tests use. One codebase to maintain, but no independent check.

## Done when (after the decision)

- [ ] `check.sh` and the README reflect the choice. If retired, `anchor.py` and its Python-only analysis scripts are deleted in one commit.
- [ ] `bash analysis/check.sh` passes.
