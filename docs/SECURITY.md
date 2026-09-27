# Security Policy

Omalogi is a personal, pre-1.0 fork. Security fixes land on `master`; there are
no maintained release branches.

## Reporting a Vulnerability

Report suspected vulnerabilities privately through
[GitHub private vulnerability reporting](https://github.com/kurtlieber/omalogi/security/advisories/new).
Do not open a public issue for a suspected vulnerability.

Useful reports include:

- A short description of the issue and its impact.
- Steps to reproduce, proof-of-concept code, or affected configuration.
- The Omalogi commit, Omarchy/Hyprland version, device model, and connection type.
- Relevant logs with private data removed.

Examples of issues that should be reported privately:

- Arbitrary code execution or privilege escalation — for example through the
  Hyprland/Omarchy helper dispatch, shell-command actions, or the udev rules.
- Unsafe handling of configuration, profile, or asset data.
- Leaks of private configuration, logs, device identifiers, or user activity.
- Security-sensitive behavior in the input hook, IPC, packaging, or device
  communication paths.

A vulnerability in HID++ protocol or device handling that also affects upstream
[OpenLogi](https://github.com/AprilNEA/OpenLogi) should be reported to upstream
as well — see its security policy.

## Response

Reports are handled on a best-effort basis. Please allow reasonable time for a
fix before disclosing publicly. There is no bug bounty program.
