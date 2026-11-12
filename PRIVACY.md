# PRIVACY.md: the treat-as-public rule

This repository is public. Everything committed here is published, so it
holds nothing about any real person: medical records are the most sensitive
data a household has.

## Never committed

- Health records, documents or images, real or "anonymized": no captured
  FHIR responses, sandbox captures included, no PDFs from a portal, no DICOM
  files
- Names, birth dates, addresses, phone numbers, email addresses of anyone
- Medical record numbers, member, subscriber and group ids, claim numbers,
  NPIs of real clinicians
- Real amounts from real claims or bills
- Tokens, client secrets, session cookies, registration keys (these live in
  the OS keychain, owned by each CLI)
- Organization ids or endpoints that identify where a household member
  receives care

## Where the data lives instead

Real records live only in the household's records folder, written by the
`medrec` store on the user's own machine, and pass through these crates at
run time. Nothing here sends them anywhere but the program that asked.

## Enforcement

1. Fixtures are written by hand, never captured
   (`crates/pk-med-fhir/fixtures/README.md`), and
   `crates/pk-med-fhir/tests/fixture_shapes.rs` checks every
   identity-bearing value against a dummy shape: positively, so a real value
   fails without the test naming it.
2. `crates/pk-med-records/tests/no_personal_data.rs` scans every tracked
   file for the shapes personal data takes: phone and member numbers, Social
   Security numbers, emails, street addresses, birth dates.
3. DICOM test files are written at run time by `pk_med_dicom::synthetic`,
   every value a dummy; no DICOM file is committed.
4. CI runs gitleaks on every push and pull request.
5. Before every commit, read the diff for values that look real.

## Examples in docs

Use obviously fake values: `Robin Testperson`, `1970-01-01`, `555-0100`,
`1 Example St`, `MRN-0000001`, `https://fhir.example.test`. Keep realism in
the structure, not the values.
