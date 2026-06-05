---
name: supazip-memory-bank
description: >-
  Memory bank workflow for this repo — read order, when to update files, UMB
  command. Use when memory-bank/ exists and the user works across sessions or
  asks for project context sync.
---

# Memory bank (SupaZip)

## Location

Directory: `memory-bank/` at the repository root.

## Read order

When using the memory bank for context, read in this order:

1. `productContext.md`
2. `activeContext.md`
3. `systemPatterns.md`
4. `decisionLog.md`
5. `progress.md`

## When to update (append; do not wipe files)

| File | When |
|------|------|
| `decisionLog.md` | Significant architecture or technology decisions |
| `productContext.md` | Product goals, features, or high-level architecture change |
| `systemPatterns.md` | New or changed recurring patterns / standards |
| `activeContext.md` | Focus of work shifts or major progress |
| `progress.md` | Task started, completed, or status changes |

Use a timestamp prefix for new entries: `[YYYY-MM-DD HH:MM:SS] - summary`.

## UMB

If the user says **"Update Memory Bank"** or **"UMB"**:

1. Acknowledge that the memory bank is being updated.
2. Review the conversation for decisions, progress, and pattern changes.
3. Append updates to every affected file; keep existing content unless correcting clear errors.
4. Stay consistent with prior memory-bank tone and structure.

## Ask mode note

In pure Q&A, prefer reading the memory bank over editing it; suggest updating it when the user records new decisions or progress.
