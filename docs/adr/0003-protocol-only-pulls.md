# ADR-0003: Protocol-only upstream pulls

Status: superseded by [ADR-0004](0004-merge-upstream-release-tags.md)

## Context

Upstream ships fast (v0.8.9, ~1,510 commits) but is maintainer-silent on
Linux window-manager PRs. Merging `upstream/master` wholesale would drag in
macOS/Windows/GUI churn we deleted and re-break the fork every release.

## Decision

Pull only the protocol crates on upstream release tags
(`hidpp`, `device`, `device-registry`, `core`): cherry-pick or subtree-merge
those paths, ignore `desktop`, `overlay`, `hook`, `inject`, `agent`,
`camera`, and anything platform-gated. Procedure in
[PROTOCOL-PULLS.md](PROTOCOL-PULLS.md).

## Consequences

- New mice/protocols keep working; our Hyprland table never conflicts.
- GUI improvements upstream are deliberately left behind until
  cherry-picked by hand.
- A protocol change that touches shell-crate APIs needs a manual shim —
  expected to be rare (the `Effect` IR is the stable seam).
