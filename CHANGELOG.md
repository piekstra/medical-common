# Changelog

## Unreleased

- `pk-med-fhir`: `FhirClient::bearer_with`, a token source called before
  each request and once more after a 401 to renew; `FhirClient::on_retry`,
  told of each wait before a retry in one line with no query, id or token;
  `UNREACHABLE`, the reason on an error for a request that got no complete
  HTTP answer after its retries (a failed, timed-out or broken connection;
  a reason the transport set is kept). Both closures are `Send + Sync`. One entry whose resource does not read no
  longer fails its page: `RawEntry::unreadable` says why, and
  `SearchResult::unreadable` lists them. `SearchResult` and `RawEntry` gain
  a public field each.

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
