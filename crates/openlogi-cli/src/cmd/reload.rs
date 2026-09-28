//! `openlogi reload`: apply a hand-edited `config.toml` to the running agent.
//!
//! The GUI asks the agent to reload after every save; an edit made in a text
//! editor (a `[commands]` override, say) needs the same nudge. A config that
//! fails to parse is reported with its TOML location and the agent keeps the
//! config it already had.

use anyhow::{Result, anyhow};
use tarpc::context;

use crate::agent::{self, CallFailure};

/// Ask the running agent to re-read `config.toml`.
pub async fn run() -> Result<()> {
    let client = agent::connect()
        .await
        .map_err(|error| anyhow!("could not reach the running Agent: {error}"))?;
    match agent::call(client.reload_config(context::current())).await {
        Ok(Ok(())) => {
            println!("config.toml reloaded");
            Ok(())
        }
        Ok(Err(error)) => Err(anyhow!(
            "the Agent kept its current config: {}",
            error.message
        )),
        Err(CallFailure::TimedOut) => Err(anyhow!("the running Agent timed out while reloading")),
        Err(CallFailure::Disconnected) => {
            Err(anyhow!("the running Agent disconnected while reloading"))
        }
    }
}
