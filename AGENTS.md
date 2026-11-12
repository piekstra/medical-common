# Agent guide: medical-common

The shared crates of the household's health-records CLIs (`mychart`,
`athena`, `uhc`, the `medrec` store). Read [DESIGN.md](DESIGN.md) before
changing any public shape: the CLIs pin tags of this repo.

## Rules

- **Medical data is the most sensitive data the household has.** This repo
  is public and carries none: not in code, fixtures, docs, commits, pull
  requests or issues ([PRIVACY.md](PRIVACY.md)). Fixtures are synthetic,
  written by hand in FHIR's shapes; `tests/fixture_shapes.rs` checks their
  dummy values positively and `tests/no_personal_data.rs` scans every
  tracked file for personal-data shapes.
- **Never read, print or summarize a real record, document or patient
  name** while working here. Develop against the mock server
  (`pk_med_fhir::test_util`) or a vendor's sandbox, whose patients are
  synthetic. A check against real data reports counts, statuses and exit
  codes only, or is left to the user in their own terminal.
- **No records leave the machine** except to the program that asked. No
  third-party service, no FHIR or SMART library or SDK, no health-data
  aggregator, no cloud OCR or AI. General-purpose crates (serde, sha2,
  base64) are fine.
- **Secrets never on argv, never in logs, never in `Debug`.** Tokens live in
  the macOS Keychain through `pk-cli-secrets`; types holding one redact it.
  Tests and `make verify` never read the real keychain.
- **Every public DTO shape is a contract** with a `"schema": "<name>/v1"`
  tag. A field may be added within `/v1` only if it is optional, defaulted
  on read and omitted when empty; anything else is `/v2`. A record's raw
  text is never re-serialized: streams carry `RawResource`.
- **One cli-common tag** for every `pk-cli-*` crate, the same tag the
  consuming CLIs pin (DESIGN.md §3). Never commit a `[patch]` to a local
  path; a temporary, uncommitted `.cargo/config.toml` patch is fine.
- Exit codes are piekstra-cli/1's: 3 auth, 4 not found, 5 upstream.

## Workflow

- `make verify` (format check, clippy `-D warnings`, tests with and without
  `test-util`, cargo-deny) must pass before committing; CI runs the same
  targets.
- Every change goes through a pull request reviewed by `cr`
  (`piekstra-dev`); merge once it approves with checks green.
- Releases: bump `workspace.package.version`, update `CHANGELOG.md`, merge,
  tag `vX.Y.Z` on the merge commit, push the tag. Consumers pin
  `tag = "vX.Y.Z"`.
