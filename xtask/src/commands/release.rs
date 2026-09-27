pub(crate) mod changelog;
pub(crate) mod check_publish;

use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub(crate) enum Command {
    /// Write the next workspace version's section into CHANGELOG.md with git-cliff.
    Changelog(changelog::Args),
    /// Verify that crates.io packages have a publishable workspace dependency closure.
    CheckPublish,
}

pub(crate) fn run(command: Command) -> Result<()> {
    match command {
        Command::Changelog(args) => changelog::run(&args),
        Command::CheckPublish => check_publish::run(),
    }
}
