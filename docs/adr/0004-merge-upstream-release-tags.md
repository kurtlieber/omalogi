# ADR-0004: Merge upstream release tags

Status: accepted (supersedes ADR-0003)

## Context

ADR-0003 limited upstream pulls to four protocol crates via per-path
subtree merges. In practice that scope drops fixes Omalogi needs — the hidraw
transport (`openlogi-hid`) and gesture/diversion behaviour
(`openlogi-agent-core`) change as often as the protocol crates — and those
crates cannot be pulled alone because they share APIs with `openlogi-ipc`,
`openlogi-core`, and each other. The fork also keeps upstream's full history,
so a plain merge is available and cheap.

## Decision

Merge each upstream release tag with an ordinary `git merge`. Files are either
upstream-pulled (take upstream's version; a short list of documented deltas) or
Omalogi-owned (Linux-only rewrites; keep ours, port relevant fixes by hand).
Upstream-pulled crates keep their macOS/Windows code so merges stay clean.
Procedure and ownership table: [PROTOCOL-PULLS.md](../PROTOCOL-PULLS.md).

## Consequences

- New devices, protocol fixes, transport fixes, and agent behaviour arrive with
  no manual cherry-picking.
- Each merge re-deletes upstream's changes to removed platform files
  (`DU` conflicts) — mechanical, scripted in the runbook.
- Upstream API changes that reach Omalogi-owned crates need a shim on the
  Omalogi side, as before.
