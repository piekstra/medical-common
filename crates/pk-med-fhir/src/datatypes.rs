// Generated from a field list, then edited by hand where noted. Each type
// keeps every element it does not name in `extra`, so a resource read and
// written back holds the same elements.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::Decimal;

/// Resource metadata: the server's version and update time, profiles, labels.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_id: Option<String>,
    /// FHIR `instant`, as the server wrote it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub profile: Vec<Option<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub security: Vec<Coding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tag: Vec<Coding>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A code from a code system: LOINC, SNOMED CT, RxNorm, CVX, ICD-10.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Coding {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_selected: Option<bool>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A concept as codings and text.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeableConcept {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub coding: Vec<Coding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A reference to another resource: `Patient/123`, an absolute URL, or `#contained`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reference {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub type_: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identifier: Option<Identifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A business identifier (an MRN, a member id).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Identifier {
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "use")]
    pub use_: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub type_: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<Period>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assigner: Option<Box<Reference>>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A time range; either end may be open.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Period {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A measured amount. `value` is a JSON number; see the crate docs on decimals.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quantity {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comparator: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// UCUM (`http://unitsofmeasure.org`) for most results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// An amount in a currency (FHIR `Money`). [`Money::to_money`] gives the family's string-decimal form.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Money {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A low and high quantity.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Range {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low: Option<Quantity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high: Option<Quantity>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Content inline (`data`, base64) or by reference (`url`, often `Binary/<id>`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creation: Option<String>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A note with its author and time.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author_reference: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author_string: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A person's name.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HumanName {
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "use")]
    pub use_: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub given: Vec<Option<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prefix: Vec<Option<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suffix: Vec<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<Period>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A phone number, email address or other contact.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactPoint {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "use")]
    pub use_: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<Period>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A postal address.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Address {
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "use")]
    pub use_: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub type_: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub line: Vec<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub district: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<Period>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// How a medication is or was taken. `timing` and `dose_and_rate` stay JSON.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dosage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sequence: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timing: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub as_needed_boolean: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dose_and_rate: Vec<Value>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
