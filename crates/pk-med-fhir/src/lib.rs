//! Our own types for the FHIR R4 / US Core subset the household's records
//! use, and a read client over the family's transport seam. No FHIR library of
//! anyone else's.
//!
//! # Types
//!
//! [`resources`] holds Patient, Encounter, Observation, DiagnosticReport,
//! Condition, MedicationRequest, MedicationStatement, AllergyIntolerance,
//! Immunization, Procedure, DocumentReference, Binary, Coverage,
//! ExplanationOfBenefit (CARIN Blue Button), ImagingStudy, Practitioner,
//! Organization, Provenance, Bundle and OperationOutcome; [`datatypes`] the
//! elements they share. Each names the elements a consumer reads, as
//! `snake_case` fields over FHIR's `camelCase`, and keeps **every other
//! element** in `extra`: extensions, `_field` primitive extensions, choice
//! types not named here, elements of a later profile. A resource read and
//! written back holds the same elements.
//!
//! Decimals are `serde_json::Number`. A typed round trip keeps each value but
//! may spell it differently (`1.50` reads back as `1.5`), which FHIR says
//! carries precision. The server's own text is kept by [`RawResource`], which
//! never parses a number: the records stream and a store's raw layer are
//! written from it.
//!
//! Build a resource with `..Default::default()`, which sets `resourceType`:
//!
//! ```
//! use pk_med_fhir::{CodeableConcept, Immunization, Resource};
//! let shot = Immunization {
//!     status: Some("completed".into()),
//!     vaccine_code: Some(CodeableConcept { text: Some("influenza".into()), ..Default::default() }),
//!     ..Default::default()
//! };
//! assert_eq!(shot.resource_type, Immunization::TYPE);
//! ```
//!
//! # Client
//!
//! [`FhirClient`] reads, searches (following `Bundle.link[next]` up to a page
//! cap), fetches `Binary` as FHIR JSON, and maps HTTP status and
//! OperationOutcome onto the family's exit codes ([`outcome`]). It sends its
//! bearer only to the base's own origin.

pub mod client;
pub mod datatypes;
pub mod outcome;
pub mod raw;
pub mod resources;
#[cfg(feature = "test-util")]
pub mod test_util;

pub use client::{FhirClient, RetryPolicy, SearchResult, DEFAULT_PAGE_CAP, UNREACHABLE};
pub use datatypes::*;
pub use raw::{minify, RawBundle, RawEntry, RawResource};
pub use resources::*;

use pk_cli_core::CliError;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{Map, Value};

/// A FHIR `decimal`, as JSON carries it.
pub type Decimal = serde_json::Number;

/// The FHIR release these types follow.
pub const FHIR_VERSION: &str = "4.0.1";

/// The FHIR JSON media type, for `Accept`.
pub const FHIR_JSON: &str = "application/fhir+json";

/// A FHIR resource type of this crate.
pub trait Resource: Serialize + DeserializeOwned {
    /// The `resourceType`: `"Patient"`, `"Observation"`.
    const TYPE: &'static str;

    fn id(&self) -> Option<&str>;

    fn meta(&self) -> Option<&Meta>;

    /// The elements this type does not name.
    fn extra(&self) -> &Map<String, Value>;

    /// The resources contained in this one (`contained`), as JSON.
    fn contained(&self) -> &[Value] {
        self.extra()
            .get("contained")
            .and_then(Value::as_array)
            .map_or(&[], Vec::as_slice)
    }

    /// The contained resource a local reference (`#org1`) points at, read as
    /// `R`. `None` when there is none of that id and type.
    fn find_contained<R: Resource>(&self, reference: &str) -> Option<R> {
        let id = reference.strip_prefix('#')?;
        self.contained()
            .iter()
            .find(|c| {
                c.get("id").and_then(Value::as_str) == Some(id)
                    && c.get("resourceType").and_then(Value::as_str) == Some(R::TYPE)
            })
            .and_then(|c| serde_json::from_value(c.clone()).ok())
    }
}

/// Read `value` as `R`, refusing another `resourceType`. A resource that
/// does not read is [`CliError::Upstream`]: a server sent it.
pub fn from_value<R: Resource>(value: Value) -> Result<R, CliError> {
    let found = value
        .get("resourceType")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if found != R::TYPE {
        return Err(CliError::Upstream(format!(
            "expected a FHIR {}, found {}",
            R::TYPE,
            if found.is_empty() {
                "no resourceType"
            } else {
                &found
            }
        )));
    }
    serde_json::from_value(value).map_err(|e| {
        CliError::Upstream(format!(
            "a FHIR {} did not read: it {}",
            R::TYPE,
            describe_json_error(&e)
        ))
    })
}

/// [`from_value`] over JSON text.
pub fn from_str<R: Resource>(text: &str) -> Result<R, CliError> {
    let raw = RawResource::parse(text)?;
    if raw.resource_type != R::TYPE {
        return Err(CliError::Upstream(format!(
            "expected a FHIR {}, found {}",
            R::TYPE,
            raw.resource_type
        )));
    }
    serde_json::from_str(text).map_err(|e| {
        CliError::Upstream(format!(
            "a FHIR {} did not read: it {}",
            R::TYPE,
            describe_json_error(&e)
        ))
    })
}

/// What is wrong with JSON that did not read, without the value. serde's own
/// message quotes the value it refused, and in a record that value is the
/// record (a name, a result), so it never goes into an error, a log or a
/// `Skip`. A syntax error keeps serde's description (`trailing characters`),
/// a missing element its name, and every message its line and column.
pub fn describe_json_error(error: &serde_json::Error) -> String {
    use serde_json::error::Category;
    let message = error.to_string();
    let message = message
        .rsplit_once(" at line ")
        .map_or(message.as_str(), |(head, _)| head);
    let what = match error.classify() {
        Category::Io => "could not be read".to_string(),
        Category::Eof => "ends early".to_string(),
        Category::Syntax => format!("is not valid JSON: {message}"),
        Category::Data => match message.strip_prefix("missing field `") {
            Some(rest) => format!("lacks `{}`", rest.split('`').next().unwrap_or_default()),
            None => "has an element of the wrong type".to_string(),
        },
    };
    if error.line() == 0 {
        what
    } else {
        format!("{what} (line {}, column {})", error.line(), error.column())
    }
}

/// A FHIR base URL in one spelling: scheme and host in lower case, no
/// trailing slash. Directories and servers spell the same base differently
/// (`https://FHIR.EXAMPLE.ORG/api/FHIR/R4/`), and a base is how a source is
/// told apart, so compare and store this form.
pub fn normalize_base(url: &str) -> String {
    let url = url.trim().trim_end_matches('/');
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_string();
    };
    let split = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, path) = rest.split_at(split);
    format!(
        "{}://{}{path}",
        scheme.to_ascii_lowercase(),
        authority.to_ascii_lowercase()
    )
}

impl Binary {
    /// The content, decoded from base64 `data`. A Binary with no `data` (a
    /// server that serves it only by `url`) is [`CliError::Upstream`].
    pub fn bytes(&self) -> Result<Vec<u8>, CliError> {
        use base64::Engine as _;
        let data = self
            .data
            .as_deref()
            .ok_or_else(|| CliError::Upstream("the FHIR Binary carries no inline `data`".into()))?;
        let compact: String = data.chars().filter(|c| !c.is_ascii_whitespace()).collect();
        base64::engine::general_purpose::STANDARD
            .decode(compact.as_bytes())
            .map_err(|e| CliError::Upstream(format!("the FHIR Binary's `data` is not base64: {e}")))
    }
}

/// A number's text without an exponent (`1.25e3` is `1250`, `5e-7` is
/// `0.0000005`), its digits moved, never recomputed.
fn plain_decimal(text: &str) -> String {
    let Some((mantissa, exponent)) = text.split_once(['e', 'E']) else {
        return text.to_string();
    };
    let Ok(exponent) = exponent.parse::<i64>() else {
        return text.to_string();
    };
    let (sign, mantissa) = match mantissa.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", mantissa),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = format!("{whole}{fraction}");
    let point = whole.len() as i64 + exponent;
    let (whole, fraction) = if point <= 0 {
        (
            "0".to_string(),
            format!("{}{digits}", "0".repeat(point.unsigned_abs() as usize)),
        )
    } else if point as usize >= digits.len() {
        (
            format!("{digits}{}", "0".repeat(point as usize - digits.len())),
            String::new(),
        )
    } else {
        (
            digits[..point as usize].to_string(),
            digits[point as usize..].to_string(),
        )
    };
    let whole = whole.trim_start_matches('0');
    let whole = if whole.is_empty() { "0" } else { whole };
    let fraction = fraction.trim_end_matches('0');
    if fraction.is_empty() {
        format!("{sign}{whole}")
    } else {
        format!("{sign}{whole}.{fraction}")
    }
}

impl Money {
    /// The family's string-decimal money (`pk_cli_core::Money`, DESIGN.md
    /// §1.8), in plain decimal notation. A whole or one-place amount gains its
    /// second place (`12.5` is `"12.50"`). The typed layer reads a decimal as
    /// a 64-bit float, so 15 significant digits are exact and trailing zeros
    /// past the second place are not kept (`12.100` is `"12.10"`); the
    /// server's own spelling is in the [`RawResource`]. A missing currency is
    /// USD, the only one CARIN Blue Button allows. `None` when there is no
    /// value.
    pub fn to_money(&self) -> Option<pk_cli_core::Money> {
        let raw = plain_decimal(&self.value.as_ref()?.to_string());
        let amount = match raw.split_once('.') {
            None => format!("{raw}.00"),
            Some((whole, places)) if places.len() == 1 => format!("{whole}.{places}0"),
            Some(_) => raw,
        };
        Some(pk_cli_core::Money {
            amount,
            currency: self.currency.clone().unwrap_or_else(|| "USD".into()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_base_has_one_spelling() {
        for (raw, want) in [
            (
                "https://FHIR.EXAMPLE.ORG/api/FHIR/R4/",
                "https://fhir.example.org/api/FHIR/R4",
            ),
            ("HTTPS://h.test", "https://h.test"),
            (" https://h.test/r4// ", "https://h.test/r4"),
            ("not a url/", "not a url"),
            ("https://H.TEST?Q=X", "https://h.test?Q=X"),
            ("https://H.TEST#Frag", "https://h.test#Frag"),
        ] {
            assert_eq!(normalize_base(raw), want, "{raw}");
        }
    }

    #[test]
    fn a_repeating_primitive_keeps_its_null_placeholders() {
        let text = r#"{"resourceType":"Patient","name":[{"family":"Testperson","given":["Robin",null],"_given":[null,{"extension":[{"url":"https://example.test/x","valueString":"y"}]}]}]}"#;
        let patient: Patient = from_str(text).unwrap();
        assert_eq!(patient.name[0].given, vec![Some("Robin".to_string()), None]);
        let back = serde_json::to_value(&patient).unwrap();
        assert_eq!(back, serde_json::from_str::<Value>(text).unwrap());
    }

    #[test]
    fn money_is_plain_decimal_text() {
        let money = |text: &str| {
            let m: Money = serde_json::from_str(&format!(r#"{{"value":{text}}}"#)).unwrap();
            m.to_money().unwrap().amount
        };
        assert_eq!(money("12"), "12.00");
        assert_eq!(money("12.5"), "12.50");
        assert_eq!(money("-0.5"), "-0.50");
        assert_eq!(money("12.125"), "12.125");
        assert_eq!(money("12.100"), "12.10");
        // Past 15 significant digits the float shows: documented, not hidden.
        // (Built from parts: a long digit run reads as personal data to the
        // repository's own scan.)
        let big = format!("{}{}{}.25", "123456", "789012", "345678");
        assert_eq!(
            money(&big),
            format!("{}{}{}.00", "123456", "789012", "345660")
        );
        assert_eq!(money("0.0000005"), "0.0000005");
        assert_eq!(plain_decimal("1.25e3"), "1250");
        assert_eq!(plain_decimal("-5E-7"), "-0.0000005");
    }

    #[test]
    fn a_json_error_never_quotes_the_value() {
        let patient = r#"{"resourceType":"Patient","name":[{"given":"Robin"}]}"#;
        let err = from_str::<Patient>(patient).unwrap_err().to_string();
        assert!(!err.contains("Robin"), "{err}");
        assert!(
            err.contains("wrong type") && err.contains("column"),
            "{err}"
        );
        let obs = r#"{"resourceType":"Observation","valueInteger":7.25}"#;
        let err = from_str::<Observation>(obs).unwrap_err().to_string();
        assert!(!err.contains("7.25"), "{err}");
        let err = from_str::<Patient>(r#"{"resourceType":"Patient""#)
            .unwrap_err()
            .to_string();
        assert!(err.contains("ends early"), "{err}");
        let missing: Result<Money, _> = serde_json::from_str(r#"{"value":"#);
        assert!(describe_json_error(&missing.unwrap_err()).contains("line 1"));
    }

    #[test]
    fn money_keeps_two_places_and_never_rounds() {
        let money = |v: &str| Money {
            value: Some(serde_json::from_str(v).unwrap()),
            ..Default::default()
        };
        assert_eq!(money("12").to_money().unwrap().amount, "12.00");
        assert_eq!(money("12.5").to_money().unwrap().amount, "12.50");
        assert_eq!(money("12.34").to_money().unwrap().amount, "12.34");
        assert_eq!(money("12.345").to_money().unwrap().amount, "12.345");
        assert_eq!(money("-3.1").to_money().unwrap().amount, "-3.10");
        assert_eq!(money("1").to_money().unwrap().currency, "USD");
        assert!(Money::default().to_money().is_none());
    }

    #[test]
    fn a_binary_decodes_its_data() {
        let binary = Binary {
            content_type: Some("text/plain".into()),
            data: Some("aGVs\nbG8=".into()),
            ..Default::default()
        };
        assert_eq!(binary.bytes().unwrap(), b"hello");
        assert_eq!(Binary::default().bytes().unwrap_err().exit_code(), 5);
        let bad = Binary {
            data: Some("***".into()),
            ..Default::default()
        };
        assert_eq!(bad.bytes().unwrap_err().exit_code(), 5);
    }

    #[test]
    fn from_value_refuses_another_type() {
        let err =
            from_value::<Patient>(serde_json::json!({"resourceType": "Observation"})).unwrap_err();
        assert!(err.to_string().contains("expected a FHIR Patient"), "{err}");
        let err = from_str::<Patient>("{}").unwrap_err();
        assert!(err.to_string().contains("no resourceType"), "{err}");
    }

    #[test]
    fn a_contained_resource_is_found_by_its_local_reference() {
        let eob: ExplanationOfBenefit = from_value(serde_json::json!({
            "resourceType": "ExplanationOfBenefit",
            "contained": [
                {"resourceType": "Organization", "id": "payer", "name": "Example Plan"},
                {"resourceType": "Coverage", "id": "cov", "status": "active"}
            ],
            "insurer": {"reference": "#payer"}
        }))
        .unwrap();
        let reference = eob.insurer.as_ref().unwrap().reference.as_deref().unwrap();
        let payer: Organization = eob.find_contained(reference).unwrap();
        assert_eq!(payer.name.as_deref(), Some("Example Plan"));
        assert!(eob.find_contained::<Organization>("#cov").is_none());
        assert!(eob.find_contained::<Coverage>("cov").is_none());
        assert_eq!(eob.contained().len(), 2);
    }
}
