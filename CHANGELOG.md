# Changelog

## v0.4.0 — 2026-11-12

Pins cli-common v0.31.1, as v0.3.0 does.

- `pk-med-smart`: an OpenID sign-in sends a nonce (DESIGN.md §3.1). When
  the scopes ask for `openid`, `begin` adds a fresh `nonce`, 32 random
  bytes in base64url. `exchange` refuses an ID token that does not carry
  it, or carries another (exit 3, `CredentialRefused`), as OpenID Connect
  Core §3.1.3.7 requires. `SmartSignIn::nonce_checked()` says whether the
  check runs. `VendorQuirks::send_nonce` (default `true`) turns it off for
  a server that refuses the parameter. athenahealth requires a nonce with
  `openid`.
- `pk-med-fhir`: a vendor's retry guidance can be followed (DESIGN.md
  §2.2). `RetryPolicy` gains:
  - `server_errors(&[502, 503, 504])`: which 5xx are asked again (default:
    every 5xx);
  - `retry_after_required(true)`: a 429 is asked again only with a
    `Retry-After` within `max_wait` (default `false`);
  - `jitter(0.25)`: each wait lengthened by a random 0–25 %, never past
    `max_wait` (default `0`);
  - builder methods `attempts`, `delay` and `max_wait`.
- **Breaking:** `RetryPolicy` is `#[non_exhaustive]`, so a struct literal
  no longer compiles. Build it from `RetryPolicy::default()` or
  `RetryPolicy::none()` with its methods. Its fields stay public to read
  and assign.
- `test-util`: the mock's ID token carries the authorization's `nonce`.
  `MockServer::id_token_nonce_next` gives the next one another nonce, or
  none.
- `pk-med-dicom`: the doc comment on `DicomError::Limit` names
  `MAX_INFLATED_BYTES`.

## v0.3.0 — 2026-11-12

Pins cli-common v0.31.1, as v0.2.0 does.

- `pk-med-dicom` (DESIGN.md §4): our own DICOM Part 10 header reader. No
  one else's DICOM library is used, and no pixel is decoded.
  - `read_header` and `read_file`: the preamble, or a file with none; the
    file meta information; and the data set in implicit or explicit VR
    little endian, explicit VR big endian, deflated, or a compressed
    syntax's explicit little endian. Sequences of defined and undefined
    length are read, and an undefined-length `UN` is read as a sequence.
    Reading stops at Pixel Data. Limits are enforced on value size, header
    size (each element and item counting 128 bytes), inflated size
    (256 MiB) and nesting.
  - `Header`: `get(tag) -> Option<&str>` for a `Tag` or `0xGGGGEEEE`,
    `element(tag)`, and typed getters (`study_uid`, `modality`,
    `study_date` as `YYYY-MM-DD`, `patient_name` as a person writes it,
    ...). `Debug` and errors show no value.
  - `read_dicomdir` and `dicomdir_entries`: the files a DICOMDIR names,
    with their study and series. Records are walked by offset, refusing a
    dangling offset or a cycle, or in sequence order when the first root
    offset is zero. A path that leaves the disc is refused.
  - `StudySummary::from_headers`: studies and series with their counts.
    It is not a wire shape (no `Serialize`): a CLI wraps it in its own
    schema-tagged DTO.
  - `DicomError` and `Value` are `#[non_exhaustive]`. In a deflated data
    set, a stream that does not inflate is `Malformed`, and an error of the
    file under it stays `Io`.
  - `is_dicom`, `is_dicom_file`, `iso_date`, `person_name`, and
    `MEDIA_STORAGE_DIRECTORY`.
  - Feature `test-util`: `synthetic::SyntheticStudy` writes a synthetic
    study (`IMAGES/S<n>/I<m>` and a DICOMDIR) in any of four transfer
    syntaxes, and `encode`, `part10` and `Node` build any data set.

## v0.2.0 — 2026-11-12

Pins cli-common v0.31.1 for every `pk-cli-*` (`private_file` syncs before it
renames). A consumer moves its own `pk-cli-*` to v0.31.1 in the same change.

- `pk-med-smart`: SMART App Launch (DESIGN.md §3).
  - `discover`: the well-known document, falling back to `/metadata` on a
    3xx or a 4xx. Every URL it keeps must be https; plain http to this
    machine only for a server whose base is one.
  - `ScopeSet` and `ScopeVersion`: v1 and v2, explicit per-type scopes, a
    vendor's own string.
  - `begin` / `SmartSignIn`: PKCE S256, `aud` (no trailing slash),
    registered ports, the issuer checked when discovery names one
    (`issuer_checked`). `exchange` for public and `client_secret_basic`
    clients. `LaunchContext`.
  - `SmartSession` (`format` 1), one keychain item per organization and
    patient at `smart/<org>/<patient>` with `%` and `/` encoded, through
    `SessionStore` (`CredentialStore`, `MemoryStore`). Errors name no value
    and no patient id.
  - `Renewer::fresh`, `renew` and `after_refusal` under `RefreshLock`. A
    refresh asks for the scope granted. A renewer for another client is
    refused. A registered client's signer is made from the session read
    under the lock (`SignerFor`). A refused grant stamps
    `SmartSession::refused_at` and keeps the sign-in. `fresh` hands out no
    refused token: the next use tries one grant, and an accepted one
    clears it.
  - `register_dynamic_client` (registration only; the first JWT-bearer
    grant is the `Renewer`'s, so a failed grant loses no client) and
    `jwt_bearer_tokens`, with `JwtSigner` and `signed_assertion` (Epic),
    assertions living 4 minutes.
  - `VendorQuirks`.
  - Feature `rsa`: `RsaKey`, an RS384 key of at least 2048 bits (rsa 0.10),
    with `RsaKey::for_client`.
- `pk-med-fhir`: `FhirClient::bearer_with`, a token source called before
  each request and once more after a 401 to renew; `FhirClient::on_retry`,
  told of each wait before a retry in one line with no query, id or token;
  `UNREACHABLE`, the reason on an error for a request that got no complete
  HTTP answer after its retries (a failed, timed-out or broken connection;
  a reason the transport set is kept). Both closures are `Send + Sync`. One entry whose resource does not read no
  longer fails its page: `RawEntry::unreadable` says why, and
  `SearchResult::unreadable` lists them. `SearchResult` and `RawEntry` gain
  a public field each.
- `pk-med-fhir`: "sign in again" and "try again later" are the error's
  remedy (`outcome::SIGN_IN_AGAIN`, `outcome::TRY_AGAIN`), no longer part
  of its message, so a CLI replaces them with its own.
- `pk-med-records`: `create_export_file` returns an `ExportFile`. The
  stream is written beside `records.ndjson`. `ExportFile::commit(&finished)`
  syncs, renames and syncs the directory: a complete stream replaces
  `records.ndjson`, and an incomplete one goes to
  `records.incomplete.ndjson` (`INCOMPLETE_FILE`), keeping the last
  complete export. Dropped uncommitted, the file is removed, and a killed
  run's file is removed by the next export (each export locks its own).
  Breaking: it returned `(PathBuf, File)`, and renamed the empty file into
  place before anything was written.

## v0.1.0 — 2026-11-12

First release. Pins cli-common v0.31.0.

- `pk-med-records`: the `health-records/v1` profile (DESIGN.md §1).
  `Organization` (`org-list/v1`), `PatientContext` (`patient-list/v1`),
  `Provenance` (`provenance/v1`) with `Provenance::from_stream`,
  `RecordsExport` (`records-export/v1`) and `create_export_file` (created new under a temporary
  name and renamed into place, mode 0600; nothing already there is
  written through). The `records-stream/v1` stream: `Header`
  (with optional `method` and `since_unbounded`), `Trailer` with counts, skips and
  `complete`, `Skip` and its codes, `RecordsWriter` (dedupe by type and
  id, 64 MiB line limit, `complete: false` on a failure after the header)
  and `RecordsReader` (bounded lines; refuses a missing header or
  trailer, a count mismatch, a line after the trailer, an incomplete
  stream). `RecordsWriter::finish` returns `Finished` (the trailer and
  the stream's whole size); the reader classifies a line exactly as the
  writer does, so every line written reads back. Feature `test-util`:
  `write_synthetic_stream`.
- `pk-med-fhir`: our own FHIR R4 / US Core types for 19 resource types
  and their datatypes, each keeping unknown elements in `extra`;
  ExplanationOfBenefit and Coverage in the CARIN Blue Button shape;
  `RawResource` and `RawBundle` keep the server's own text, so decimals
  keep their spelling; `Resource::find_contained`; `Binary::bytes`;
  `Money::to_money` (plain decimal text); `normalize_base`;
  `describe_json_error`, so no error quotes a record's value. Repeating
  primitives are `Vec<Option<T>>`, keeping FHIR's null placeholders. `FhirClient` over
  `pk_cli_core::transport::Transport`: read, search with `next` paging
  under a page cap, Binary as FHIR JSON, `get_raw`; requests only to the
  base's origin; retries a failed connection, a 429 or a 5xx under `RetryPolicy` with
  `Retry-After`; `.` and `..` segments refused however they are spelled; HTTP
  status and OperationOutcome mapped to exit codes 3, 4 and 5. Feature
  `test-util`: a loopback mock SMART + FHIR server and a synthetic
  patient with every resource type.
