# Contributing to medical-common

This repo holds the `health-records/v1` profile ([DESIGN.md](DESIGN.md)) and
the crates behind it. A piece two health-records CLIs need belongs here, not
copied into each.

## Before you start

- Open or comment on an issue describing the change.
- Branch from `main`; `main` takes pull requests only.
- Read DESIGN.md: DTO shapes, the stream and the command spellings are
  contracts consumed by tag-pinned CLIs.

## Local checks (must pass)

```sh
make verify      # format check, clippy -D warnings, tests, feature checks, cargo-deny: what CI runs
make fmt         # apply formatting
```

## Ground rules

- **No real data, ever.** Fixtures are synthetic, in the dummy shapes listed
  in `crates/pk-med-fhir/fixtures/README.md`. Never commit a capture from a
  real server, sandbox captures included: write the fixture by hand.
- **Schemas are immutable.** A shape change is a new `/v2` schema, except the
  additive carve-out (an optional, defaulted, omitted-when-empty field).
- **Secrets never on argv, never in logs.**
- **No FHIR, SMART or DICOM library of anyone else's.**

## Releasing

Bump `workspace.package.version`, update `CHANGELOG.md`, merge, tag
`vX.Y.Z` on the merge commit, push the tag.

## Licence of contributions

Dual-licensed MIT OR Apache-2.0, like the project.
