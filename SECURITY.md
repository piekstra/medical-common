# Security policy

## Reporting

Report a vulnerability privately through GitHub's private vulnerability
reporting on this repository (Security, then "Report a vulnerability"). Do
not open a public issue, and do not include anyone's health information in
the report.

## Scope

These crates read health records from FHIR servers on behalf of the person
the records are about. In scope:

- a token, a client secret or a session sent anywhere but the server it
  belongs to, or shown in a log, an error message or `Debug` output;
- a request leaving the FHIR base's origin (a `next` link, a Binary
  reference) or a redirect followed;
- record content in an error message, a skip reason or a log;
- a stream that a reader accepts as whole when it was cut short or altered;
- an export file readable by anyone but its owner;
- personal data committed to this repository.

## Practices

- Credentials live in the OS keychain through cli-common's `pk-cli-secrets`,
  never on argv, in environment variables or in files.
- Every request goes to `https`, or `http` to this machine for a test
  server; the client sends its bearer only to the FHIR base's origin.
- CI runs gitleaks and cargo-deny on every pull request.
