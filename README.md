# medical-common

The shared crates of the household's health-records CLIs, built on
[cli-common](https://github.com/piekstra/cli-common)'s `piekstra-cli/1`.

| Crate | Contents |
|---|---|
| `pk-med-records` | The `health-records/v1` profile: `Organization` (`org-list/v1`), `PatientContext` (`patient-list/v1`), `Provenance` (`provenance/v1`), and the `records-stream/v1` NDJSON stream with its writer and reader |
| `pk-med-fhir` | Our own FHIR R4 / US Core types, lossless on a round trip, CARIN Blue Button claims included; a read client over cli-common's transport seam (read, search with paging, Binary, OperationOutcome and status to exit codes, Retry-After); a mock SMART + FHIR server for tests (feature `test-util`) |
| `pk-med-dicom` | Our own DICOM Part 10 header reader (implicit and explicit VR, little and big endian, deflated, sequences of undefined length; it stops at Pixel Data and never decodes a pixel), DICOMDIR reading, study and series summaries, and a synthetic study writer (feature `test-util`) |
| `pk-med-smart` | SMART App Launch for a standalone patient app: discovery, v1 and v2 scopes, the patient launch context, public and confidential clients, the session as one keychain item per organization and patient, renewal under cli-common's refresh lock, Epic's dynamic client registration with a signed JWT-bearer grant (an RS384 key with feature `rsa`), vendor quirks |

[DESIGN.md](DESIGN.md) is the specification: the commands every provider and
insurer CLI implements, the DTOs, the stream, and how the profile composes
with cli-common's `documents/v1`.

These crates handle medical records, the most sensitive data a household
has. Nothing in this repository is real: every fixture is synthetic
([PRIVACY.md](PRIVACY.md)). The crates read records; they send them nowhere
but the program that asked.

## Use

```toml
[dependencies]
pk-med-records = { git = "https://github.com/piekstra/medical-common", tag = "v0.4.0" }
pk-med-fhir = { git = "https://github.com/piekstra/medical-common", tag = "v0.4.0" }
pk-med-smart = { git = "https://github.com/piekstra/medical-common", tag = "v0.4.0" }
pk-med-dicom = { git = "https://github.com/piekstra/medical-common", tag = "v0.4.0" }
# The same cli-common tag medical-common pins (DESIGN.md §5):
pk-cli-core = { git = "https://github.com/piekstra/cli-common", tag = "v0.31.1" }

[dev-dependencies]
pk-med-fhir = { git = "https://github.com/piekstra/medical-common", tag = "v0.4.0", features = ["test-util"] }
pk-med-dicom = { git = "https://github.com/piekstra/medical-common", tag = "v0.4.0", features = ["test-util"] }
```

A pull, in brief:

```rust,ignore
let client = FhirClient::new(&transport, &org.fhir_base, "mychart", VERSION)?
    .bearer(token);
let mut out = RecordsWriter::start(std::io::stdout().lock(), &header)?;
let page = block_on(client.search("Observation", &[("patient", id.clone())]))?;
for resource in &page.resources {
    out.resource(resource)?;
}
out.finish(now_rfc3339(), None)?;
```

## Develop

```sh
make verify   # fmt check, clippy -D warnings, tests, cargo-deny: what CI runs
```

Tests are offline: the mock server listens on 127.0.0.1, and nothing reads
the keychain.

## Licence

MIT OR Apache-2.0.
