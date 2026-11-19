# Security policy

## Supported versions

Only the latest release receives fixes.

## Reporting a vulnerability

Please **do not open a public issue**. Use GitHub's *Report a vulnerability* button (Security → Advisories) on this repository. Include the affected version, platform, and steps to reproduce. You can expect an initial answer within a few days.

## Scope

DriftLab talks to HID devices locally and never sends controller data anywhere. Network access is limited to an on-demand request to the GitHub releases API (Settings → Check for updates). Relevant reports include: unsafe handling of HID report data (panics, out-of-bounds), unsafe writes to controller memory, insecure file handling (backups, profiles, exports), and supply-chain issues in the release pipeline.

## Release integrity

Release assets ship with `SHA256SUMS.txt` and a GitHub build-provenance attestation (`gh attestation verify <file> --repo LaFAirs/driftlab`).
