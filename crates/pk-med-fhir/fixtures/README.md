# Fixtures

Synthetic data only. Nothing here was captured from a real server, patient
or payer: every resource was written by hand for these tests, in the shapes
FHIR R4, US Core 6 and CARIN Blue Button 2 define.

- `synthetic/<Type>.json`: one patient's records, a JSON array per resource
  type, served by the mock server (`pk_med_fhir::test_util`). Every resource
  type the crate names has at least one.
- `synthetic/document.pdf`: a one-page PDF reading "Synthetic test
  document". `synthetic/Binary.json` carries it in base64.
- `smart-configuration.json`, `capability-statement.json`: SMART discovery,
  both forms. `{{ORIGIN}}` and `{{BASE}}` are the mock server's origin and
  FHIR base, filled in when it starts.
- `operation-outcome-*.json`: the refusals the mock server sends.

Dummy values follow fixed shapes, and `tests/fixture_shapes.rs` checks them
positively:

| Field | Shape |
|---|---|
| a person's family name | `Testperson`, `Doctor` |
| a given name | `Robin`, `Example` |
| a birth date | `1970-01-01` |
| a phone number | `555-01xx` |
| an email address | `@example.com` |
| an address | `1 Example St`, `Exampleville`, state `ZZ`, postal code `00000` |
| an identifier | `MRN-0000001`, `MEMBER-0000`, `GROUP-0000`, `LOT-0000`, NPI `0000000001` |
| a DICOM UID | under `2.25.1` |
| a date of care | in 2019 or 2020 |

Decimals keep the spelling a server would use (`1.50`), so a test can check
that a raw resource keeps it.
