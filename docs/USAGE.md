# Usage (CLI)

The `omalogi` command-line tool. For install and configuration, see the
[README](../README.md).

```sh
omalogi list                 # paired devices: slot, codename, kind, online, battery
omalogi assets sync          # pre-fetch device renders from the fastest available mirror
omalogi diag features        # dump every HID++ feature the active device reports
omalogi diag controls        # dump reprogrammable controls and capability flags
omalogi diag dpi             # read → write → read-back → restore DPI (smoke test)
omalogi diag smartshift      # toggle SmartShift and restore (smoke test)
omalogi diag lighting ff0000 # solid colour for a wired RGB keyboard (any RRGGBB hex)
omalogi reload               # apply a hand-edited config.toml to the running agent
```

Running `omalogi` with no subcommand defaults to `list`. Set
`OPENLOGI_LOG=debug` for verbose tracing in the CLI, GUI, or agent.

Asset synchronization probes the upstream OpenLogi project's asset mirrors —
`assets.openlogi.org`, the versioned Cloudflare Pages release alias, and the
pinned jsDelivr npm release — concurrently. The first
mirror with a valid catalog supplies every file for that synchronization run.
Set `OPENLOGI_ASSETS` or pass `omalogi assets sync --base <URL>` to use one
uniform asset origin instead of automatic mirror selection.
