# xtask — packaging and CI tooling

- `xtask/README.md` is the contract for this crate — module layout mirrors the CLI
  hierarchy, `xshell` for short-lived external tools (`cargo`, `nfpm`),
  `std::process::Command` only for real process control, crates (not shell-outs)
  for structured data, no thin wrappers around tools that already own a task. Read it
  before adding a command.
- xtask is linted like product code: the workspace `clippy::pedantic` +
  `unwrap_used`/`expect_used` warns run with `-D warnings` — use `?` and combinators,
  not `unwrap`/`expect`, even in "script" code.
- Icons: `assets/icon/omalogi.png` (1024²) and its `omalogi-<size>.png` hicolor
  renditions are committed; `packaging/linux/nfpm.yaml` and `package.nix` install
  them, and the GUI embeds the master. There is no icon build step.
- Package contents are declarative, not coded: `packaging/linux/nfpm.yaml` (plus udev
  rules, systemd unit, and desktop entry beside it). `PKG_ARCH` overrides the package
  architecture.
- `cargo xtask ci` is the local CI runner (`xtask/src/commands/ci/`). Facts about a
  job — CI name, the names it answers to, the hosts CI gives it, whether a bare run
  includes it — are one `Spec` row returned from one match in `ci/jobs.rs`; behaviour
  is `ci/jobs/steps.rs`. Host gating is a runtime `Host` value, not `cfg!`, so which
  job skips where is data a test reads. Adding a job to `ci.yml` means a `Job`
  variant + `Spec` row, its steps, and a row in `.agents/rules/ci.md`; `--list`
  renders itself from the rows. `--dry-run` prints the real argv.
- Shell that is really program logic belongs here instead: `cargo xtask release changelog`
  replaced a script whose version parsing and changelog editing were two embedded Python
  heredocs. It reads the version from the tree at run time — `env!("CARGO_PKG_VERSION")`
  would bake in a stale one.
