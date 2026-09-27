# Omalogi xtask

`xtask` is the repository-level entry point for development tasks that need Rust
or cross-language orchestration. Run it from the repository root:

```sh
cargo xtask <command>
# or, without the cargo alias:
cargo run -p xtask -- <command>
```

## Commands

- `ci [--list] [--dry-run] [JOB…]` — reproduce the `ci.yml` jobs this host can
  run; a job it cannot is skipped with a reason, never passed.
- `linux package` — build release binaries and package `.deb`, `.rpm`, and
  `.pkg.tar.zst` artifacts with nfpm.
- `release changelog` — write the next workspace version's section into
  `CHANGELOG.md` with git-cliff.
- `release check-publish` — verify that every crates.io package has a publishable,
  versioned workspace dependency closure.

## Layout

```text
xtask/
  README.md
  src/
    main.rs                  # CLI shape and dispatch only
    commands/
      mod.rs
      ci.rs                  # CI job runner: CLI, host, step execution, summary
      ci/
        jobs.rs              # one row of facts per ci.yml job + host gating
        jobs/
          steps.rs           # what each job runs
          tests.rs
        list.rs              # renders --list from those rows (comfy-table)
        list/
          tests.rs
      linux.rs               # Linux domain entry
      linux/
        package.rs
        package/tests.rs
      release.rs             # release metadata entry
      release/
        changelog.rs
        changelog/tests.rs
        check_publish.rs
        check_publish/tests.rs
    support/
      mod.rs
      fs.rs                  # shared filesystem/process guards only
      manifest.rs            # the root Cargo.toml's [workspace.package]
```

Unit tests are a sibling file throughout this crate: `foo.rs` declares
`#[cfg(test)] mod tests;` and the tests live in `foo/tests.rs`. That keeps a
module's source to what it does, and the `#[cfg(test)]` on the declaration is
what carries the `clippy.toml` unwrap/expect exemption into the file — a test
helper outside any `#[test]` fn still gets it. Note that `include_str!` in such
a file resolves relative to `foo/`, one level deeper than the module it came
from.

A test that reads a file from the repository has one more constraint: the Nix
package builds from a source derivation that deliberately omits documentation
and CI metadata — editing a workflow must not rebuild the application — and it
runs `cargo test` inside that sandbox. Either add the file to the fileset in
`packaging/linux/package.nix`, the way `nfpm.yaml` is there for the packaged-bins
test, or let the test skip when the file's whole directory is absent, the way
the `ci.yml` drift tests do.

Keep command modules aligned with the CLI hierarchy. A platform action belongs
under its platform (`linux package`); release metadata belongs under `release`; shared helpers belong in `support` only when they are reused by
multiple commands or handle real error/resource boundaries.

## Maintenance rules

- Use `xshell` for short-lived external tools such as `cargo` and `nfpm`.
- Use `std::process::Command` only when a command needs explicit process
  lifetime, streaming, or stdout/stderr control.
- Use crates for structured data and platform-neutral formats (`serde_json` for
  JSON, `toml` for manifests) rather than shelling out.
- Do not shell out just to avoid a small, appropriate Rust dependency.
- Do not reintroduce thin wrappers around tasks already owned by a dedicated
  package script, Cargo subcommand, or external tool.
- Inline single-use helpers unless the name captures a durable domain concept,
  hides meaningful resource handling, or reduces repeated complexity.
