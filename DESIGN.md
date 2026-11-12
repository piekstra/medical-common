# medical-common: the `health-records/v1` profile

Status: v0.1.0 ships `pk-med-records` and `pk-med-fhir`. `pk-med-smart`
(SMART App Launch) and `pk-med-dicom` (DICOM headers) follow in later
versions.

This document is the specification of the profile, in the style of
cli-common's domain profiles (`documents/v1`, `utility/v1`, cli-common
`DESIGN.md` §1.8). It builds on **piekstra-cli/1**: global flags, `--json`,
one JSON document per invocation, exit codes 0–6, secrets in the keychain
only. Nothing here restates those rules; where this profile is silent,
piekstra-cli/1 holds.

## 1. The `health-records/v1` profile (crate `pk-med-records`)

For any CLI that reads a person's health records from a provider, or their
coverage and claims from an insurer: `mychart` (Epic), `athena`
(athenahealth), `uhc` (UnitedHealthcare), and the CLIs after them. The
records store (`medrec`) is the consumer. Every command is a read, so the
confirmation rules of piekstra-cli/1 §1.3 do not apply. A CLI declares the
profile in `info` (`profiles: ["health-records/v1", "documents/v1"]`).

### 1.1 Commands

```text
<bin> orgs list                                   # org-list/v1
<bin> orgs search <TEXT>                          # org-list/v1
<bin> auth login --org <ORG> [--label <WHO>]      # browser sign-in, PKCE, loopback redirect
<bin> patients list                               # patient-list/v1
<bin> records export --patient <ID> [--since <YYYY-MM-DD>] --ndjson    # records-stream/v1 on stdout
<bin> records export --patient <ID> [--since <YYYY-MM-DD>] -o <DIR>    # the same stream to <DIR>/records.ndjson; records-export/v1 on stdout
<bin> documents list | download                   # documents/v1 (§1.7)
<bin> api GET <PATH>                              # passthrough, GET only
```

- `orgs search` matches `name`, `aliases` and `location`, case-insensitive.
- `--patient` takes a `PatientContext.id`, or its `label`. With one
  context signed in, it may be omitted.
- `--since` bounds each search by the date parameter the source documents
  for the type, and leaves a type the source documents none for unbounded
  rather than guess one (`_lastUpdated` can turn a whole search into a 400).
  The stream header names those types in `since_unbounded`, so a reader
  knows their resources may be older than `since`. It never drops a
  resource the source returns. (At Epic, for example, `date` is documented
  on Observation, DiagnosticReport, DocumentReference, Encounter,
  Immunization and Procedure.)
- An insurer CLI's export carries `Coverage` and `ExplanationOfBenefit`
  (CARIN Blue Button). A CLI whose source is not a FHIR server (a member
  site, downloaded files) maps what it reads into those FHIR shapes and sets
  the header's `method` to `portal`, so the store sees one format.
- `--ndjson` and `-o` are exclusive; one of them is required. `--json` with
  `--ndjson` is a usage error (exit 2): the stream is the output.
- `auth status` reports `method: "browser-session"` for a SMART sign-in
  (cli-common `pk_cli_auth::oauth`).

### 1.2 `Organization` (`org-list/v1`)

`orgs list` and `orgs search` emit `Paged<Organization>`
(`{"schema": "org-list/v1", "items": [...]}`), built by
`pk_med_records::org_list`. The same object is the stream header's
`organization`.

| Field | | |
|---|---|---|
| `id` | required | the CLI's own stable id: Epic's organization id, a slug |
| `name` | required | the organization's name as its directory gives it |
| `fhir_base` | required | the FHIR base, normalized by `pk_med_fhir::normalize_base` (scheme and host lower case, no trailing slash); a source that is not a FHIR server puts its site's base here |
| `vendor` | optional | `epic`, `athenahealth`, `cerner`, `uhc`, ... |
| `location` | optional | city and state, or a region, to tell same-named organizations apart |
| `aliases` | optional | other names: a brand, a former name |

### 1.3 `PatientContext` (`patient-list/v1`)

A patient at a provider, or a member at an insurer, that the CLI holds a
sign-in for. `patients list` emits `Paged<PatientContext>` (`patient-list/v1`,
`pk_med_records::patient_list`). The same object is the stream header's
`patient`.

| Field | | |
|---|---|---|
| `id` | required | the FHIR `Patient.id` at the source |
| `organization` | optional | the `Organization.id` it is at |
| `label` | optional | the household's own name for it (`--label`), never read from the record |
| `relationship` | optional | `self`, or `proxy` for a parent's access to a child's record |
| `kind` | optional | `patient` at a provider, `member` at an insurer |
| `member_role` | optional | `subscriber` or `dependent` |
| `fhir_user` | optional | the signed-in user's `fhirUser` (`Patient/…`, `RelatedPerson/…`) |
| `name` | optional | the name the source shows, for the person's own terminal; never in a fixture, a log or an error message |
| `session` | optional | the keychain account holding this sign-in (`smart/<org>/<patient>`), never the token |

### 1.4 The records stream (`records-stream/v1`)

`records export --ndjson` writes NDJSON to stdout: a header line, one FHIR
resource per line, and a trailer line. The writer is
`pk_med_records::RecordsWriter`; the reader is `RecordsReader`.

```text
{"schema":"records-stream/v1","source":{"cli":"mychart","version":"0.1.0"},"organization":{...},"patient":{...},"pulled_at":"2020-01-03T00:00:00Z","fhir_version":"4.0.1"}
{"resourceType":"Patient","id":"pat-0001",...}
{"resourceType":"DocumentReference","id":"doc-0001",...}
{"resourceType":"Binary","id":"bin-0001","contentType":"application/pdf","data":"JVBERi0..."}
{"schema":"records-stream-end/v1","complete":true,"counts":{"Binary":1,"DocumentReference":1,"Patient":1},"skipped":[],"finished_at":"2020-01-03T00:01:00Z"}
```

**Header** (`Header`):

| Field | | |
|---|---|---|
| `schema` | required | `records-stream/v1` |
| `source` | required | `{cli, version}`: the program that wrote the stream |
| `organization` | required | an `Organization` (§1.2) |
| `patient` | required | a `PatientContext` (§1.3) |
| `since` | optional | `--since`, `YYYY-MM-DD` |
| `since_unbounded` | optional | with `since`: the types left unbounded because the source documents no date parameter for them |
| `pulled_at` | required | when the pull started, RFC 3339 UTC |
| `fhir_version` | defaulted | `4.0.1`; a reader takes an absent one as `4.0.1` |
| `method` | optional | how the resources were obtained: absent means `fhir-api`; `portal` for resources mapped from a member or patient site or from files downloaded there |

**Resource lines.** A line is a resource if and only if it has a top-level
`resourceType`. Each is the server's own JSON text, minified (whitespace
outside strings removed) and otherwise unchanged, so a decimal keeps its
spelling (`1.50` stays `1.50`; FHIR gives the spelling meaning). A producer
writes `pk_med_fhir::RawResource` and never re-serializes a parsed value.

- Every type the source serves goes in, `Binary` included. A `Binary` is
  fetched as FHIR JSON (content in base64 `data`) and written right after
  the first `DocumentReference` or `DiagnosticReport` that names it.
- A resource appears once per stream, by `(resourceType, id)`; the writer
  drops repeats. A resource with no `id` is always written.
- A producer that maps a non-FHIR source keeps each resource's id stable
  across runs and import routes (derived from the source's own claim or
  document number), so the store's same-resource merge works. It marks each
  mapped resource with `meta.source` (`uhc:portal`, `uhc:eob-pdf`) and a
  `meta.tag` saying it was mapped.

**Trailer** (`Trailer`):

| Field | | |
|---|---|---|
| `schema` | required | `records-stream-end/v1` |
| `complete` | required | `true` when the pull finished; `false` when it stopped on an error after the header |
| `counts` | required | resource lines written, by `resourceType` |
| `skipped` | required | what was left out and why (`Skip`), possibly empty |
| `finished_at` | optional | RFC 3339 UTC; a writer always sets it |
| `error` | optional | with `complete: false`: `{code, message, exit_code?}`, `code` being piekstra-cli/1's error code (`auth`, `not_found`, `upstream`, ...) |

**`Skip`**: `{resource_type?, id?, search?, reason, code}`. `search` names a
search left out (`Observation?category=laboratory`) with no patient id or
token in it; `reason` is for a person and carries no record content. `code`
is from an open set; a reader treats an unknown code as a skip like any
other:

| `code` | Meaning |
|---|---|
| `no_file` | the resource names a file the source did not serve |
| `verify_failed` | a file failed its check (`documents/v1`'s `verify_download`) |
| `upstream` | the source failed: a 5xx, a 429 not waited out |
| `forbidden` | the source refused access: a 403, a scope not granted |
| `not_found` | a 404 or 410 for something a resource named |
| `too_large` | over the line limit |
| `unsupported` | the source does not offer the search or the type |
| `page_cap` | pages were left at the page cap |

**Failure.** Nothing goes to stdout before the header, so a failure before
the first request (no sign-in, a bad argument) is an ordinary error: the
error DTO, exit 2, 3, 4 or 5. Once the header is out, the stream is the
output: a failure ends it with a trailer of `complete: false` and its
`error`, and the CLI exits with that error's code (3, 4 or 5), writing no
second JSON document. A failure of one search or one file is a `Skip`, not a
stop.

**Limits.** No line is over `MAX_LINE_BYTES`, 64 MiB (a Binary of about 48
MiB). The writer skips a larger resource as `too_large`; the reader refuses
a larger line as `LineTooLong` without holding it in memory.

**Reading.** `RecordsReader` yields the header, each resource as a
`RawResource`, then the trailer. It refuses, as `StreamError`, each mapped
to exit 5:

| Error | When |
|---|---|
| `MissingHeader` | the first non-blank line is not a `records-stream/v1` header |
| `BadHeader` | the header names the schema but does not read |
| `BadJson { line }` | a line is not JSON, or a resource line not a resource |
| `LineTooLong { line }` | a line is over the limit |
| `NotAResource { line }` | a line has neither `resourceType` nor the trailer's schema |
| `AfterTrailer { line }` | a line follows the trailer |
| `MissingTrailer` | the input ended with no trailer: the stream was cut short |
| `CountMismatch` | the trailer's count for a type is not the number of lines read |
| `Incomplete { trailer }` | the trailer says `complete: false`; every resource before it was yielded |

Blank lines and CRLF endings are accepted. A store files what it read from
an incomplete stream as such, or not at all; it never takes a stream without
its trailer as whole.

### 1.5 `records export -o <DIR>` (`records-export/v1`)

The same stream, to `<DIR>/records.ndjson`, created with mode 0600 (and the
directory with 0700 when missing) by `pk_med_records::create_export_file`,
which refuses to write through a symbolic link. stdout gets one
`records-export/v1` document: `{schema, path, bytes, complete, counts,
skipped?}` (`RecordsExport::new(path, bytes, &trailer)`). Development and
tests use it with synthetic data only.

### 1.6 Provenance (`provenance/v1`)

What the store keeps on every record it files. `Provenance::from_stream`
fills it from a stream's header and one resource; the store adds the rest.

| Field | | |
|---|---|---|
| `source` | required | `{kind, cli?, cli_version?, organization_id?, fhir_base?}`; `kind` is `fhir`, `portal`, `scan`, `drive`, `email`, `media`, `manual` or `huishouden` |
| `method` | required | `fhir-api`, `portal`, `scan`, `file`, `media`, `manual` or `huishouden` |
| `source_resource` | optional | `{type, id, version_id?, last_updated?}` of the resource at the source |
| `fetched_at` | optional | the stream's `pulled_at` |
| `filed_at` | required | when the store filed it, RFC 3339 |
| `raw_path` | optional | the raw file in the records folder: `<Person>/Records/<source>/<Type>/<id>.json`, or the document as received |
| `classified_by` | optional | `medrec` (its rules), `organize-scans`, or `hand` (the command line or review) |
| `derived_from` | optional | `{document_id, rule_id}` for a field derived from a document |
| `filed_by` | required | the program and version that filed it (`medrec 0.3.0`) |

From a stream, `method` is the header's (`fhir-api` when absent) and
`source.kind` is `fhir` for `fhir-api`, otherwise the method itself.

### 1.7 How it composes with `documents/v1`

A provider CLI also implements cli-common's `documents/v1` over its
`DocumentReference` and `Binary` resources, and an insurer CLI over its EOB
files, so `organize-scans` and anything else that speaks `documents/v1`
works unchanged:

- `Document.id` is the `DocumentReference.id`; a reference with more than
  one attachment lists each as `<id>#<n>`, `n` counting from 1 in
  `content` order.
- `Document.name` is the attachment's `title`, else the reference's type
  text; `date` is `DocumentReference.date` as `YYYY-MM-DD`; `category` its
  first category's text; `file` the name the download writes.
- `documents download` fetches the attachment's `Binary` (through
  `FhirClient::binary`), decodes it (`Binary::bytes`) and passes the bytes
  through `verify_download` before writing, as `documents/v1` requires.
  A file that fails is skipped with `verify_failed`, in a batch and in a
  stream alike.
- The records stream carries the same `Binary` resources verbatim, so the
  store does not call `documents download` for a pulled patient; the two
  routes agree on ids, and a store matches a filed document to its
  reference by `Document.id`.

### 1.8 Money

Amounts are `pk_cli_core::Money`: a string decimal and a currency, never a
float. `pk_med_fhir::Money::to_money` converts a FHIR `Money`: a whole or
one-place amount gains its second place (`12.5` is `12.50`), more places
are kept as written and never rounded, and a missing currency is `USD`, the
only one CARIN Blue Button allows.

### 1.9 Versioning

Each schema above is `/v1` and follows cli-common's rule: a field may be
added within `/v1` only if it is optional, defaulted when read and omitted
when empty. Anything else (a removal, a rename, a new meaning) is `/v2`. A
reader ignores members it does not know, in the header, the trailer and
every DTO. The Rust structs have public fields; build them with their
constructors (`Header::new`, `Skip::resource`, `Skip::search`) so a field
added within `/v1` does not break a caller.

## 2. FHIR types and client (crate `pk-med-fhir`)

### 2.1 Types

Our own serde types for the FHIR R4 / US Core subset the household's
records use: Patient, Encounter, Observation, DiagnosticReport, Condition,
MedicationRequest, MedicationStatement, AllergyIntolerance, Immunization,
Procedure, DocumentReference, Binary, Coverage, ExplanationOfBenefit (CARIN
Blue Button: lines, adjudication amounts, totals, payment, benefit
balances), ImagingStudy, Practitioner, Organization, Provenance, Bundle and
OperationOutcome, and the datatypes they share. No FHIR library of anyone
else's.

- Fields are the elements a consumer reads, `snake_case` over FHIR's
  `camelCase`; every other element (extensions, `_field` primitive
  extensions, choice types not named, later profiles' elements) is kept in
  each type's `extra` map, so a resource read and written back holds the
  same elements (a test checks this for every synthetic resource).
- A decimal is a `serde_json::Number`: the value survives a typed round
  trip, its spelling may not. The spelling is kept by `RawResource`, which
  never parses a number; streams and stores are written from it.
- `Resource::contained()` and `find_contained::<R>("#id")` read contained
  resources, as CARIN EOBs carry their payer and coverage.
- `RawBundle` reads a search page keeping each entry's resource raw.

### 2.2 `FhirClient`

A read client generic over `pk_cli_core::transport::Transport`, so the same
code runs over `pk_cli_http::BlockingTransport` (driven with
`pk_cli_core::block_on`) and any other transport.

- `read(type, id)`, `read_as::<R>(id)`, `search(type, &params)`,
  `binary(reference)`, `get_raw(path)`.
- `search` adds no parameter of its own, follows `Bundle.link[next]` as the
  server wrote it, up to a page cap (`page_cap`, default 100), and reports
  `pages`, `truncated` (pages left at the cap), `total`, and its entries
  sorted by search mode: `resources` (`match`), `included` (`include`),
  `outcomes` (`outcome`).
- Every request goes to the base's own origin; the bearer and extra headers
  (`header("Epic-Client-ID", …)`) never leave it. A `next` link or a Binary
  reference on another origin is refused (exit 5). The base must be `https`,
  or `http` to this machine for a test server.
- Ids and types are checked before any request (exit 2).
- A 429, a 5xx, or an edge's HTML 403 is asked again under `RetryPolicy`
  (3 requests, 1 s then 2 s apart, or the server's `Retry-After`, 30 s at
  most; a server asking for longer is not waited for).

Refusals map to exit codes (`pk_med_fhir::outcome::status_error`):

| Answer | Exit |
|---|---|
| 401 | 3, `Reason::CredentialRefused` |
| 403 with FHIR or JSON | 3, `Reason::Forbidden` |
| 403 with an HTML page, 429, 5xx | 5, with the wait asked for |
| 404, 410 | 4 |
| 400, 422, any other 4xx | 5, `Reason::BadRequest` |
| a 3xx | 5: redirects are not followed |

An OperationOutcome in the body refines a 4xx by its first error issue's
code: `login`, `unknown`, `expired` give 3 (`CredentialRefused`);
`forbidden`, `suppressed`, `security` 3 (`Forbidden`); `not-found`,
`deleted` 4; `throttled`, `transient`, `timeout`, `exception` 5. An
OperationOutcome with errors in place of a resource with a 2xx is mapped the
same way. Messages name the server's origin and the resource type, never a
query, an id or a token, and quote the outcome's text cut to printable
characters.

### 2.3 `test-util`: the mock server

Feature `test-util` adds `pk_med_fhir::test_util`: a loopback mock SMART +
FHIR server (`MockServer::start`) and the synthetic patient it serves, one
or more resources of every type above, a Binary carrying a one-page PDF,
and paging. It answers SMART discovery (both forms), the authorize step
(PKCE S256 required, `state` and `iss` returned, a declined consent on
request), the token endpoint (authorization code with the verifier checked,
rotating refresh, JWT bearer) and registration. `Fault` makes chosen
requests fail: a 429 with `Retry-After`, an OperationOutcome error, a 401,
a 503. Every value in it is synthetic: `crates/pk-med-fhir/fixtures/README.md`
lists the dummy shapes, and a test checks them positively.

`pk_med_records::test_util::write_synthetic_stream` writes that patient as
a complete stream, for a store's tests.

## 3. Consumption

Pin a tag, never a branch or a path:

```toml
pk-med-records = { git = "https://github.com/piekstra/medical-common", tag = "v0.1.0" }
pk-med-fhir = { git = "https://github.com/piekstra/medical-common", tag = "v0.1.0" }
```

Each medical-common tag pins one cli-common tag for every `pk-cli-*` crate,
and a consumer pins the same cli-common tag: two cli-common tags in one
build are two copies of every `pk-cli-core` type, and `CliError` from one
does not convert to the other.

| medical-common | cli-common |
|---|---|
| v0.1.0 | v0.31.0 |
