// Generated from a field list, then edited by hand where noted. Each type
// keeps every element it does not name in `extra`, so a resource read and
// written back holds the same elements.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::datatypes::*;
use crate::{Decimal, Resource};

/// US Core Patient: who the record is about.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Patient {
    /// Always `"Patient"`.
    pub resource_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifier: Vec<Identifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub name: Vec<HumanName>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub telecom: Vec<ContactPoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gender: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub birth_date: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub address: Vec<Address>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub general_practitioner: Vec<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub managing_organization: Option<Reference>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Patient {
    fn default() -> Self {
        Patient {
            resource_type: "Patient".into(),
            id: None,
            meta: None,
            identifier: Vec::new(),
            active: None,
            name: Vec::new(),
            telecom: Vec::new(),
            gender: None,
            birth_date: None,
            address: Vec::new(),
            general_practitioner: Vec::new(),
            managing_organization: None,
            extra: Map::new(),
        }
    }
}

impl Resource for Patient {
    const TYPE: &'static str = "Patient";
    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn extra(&self) -> &Map<String, Value> {
        &self.extra
    }
}

/// US Core Encounter: a visit, admission or virtual contact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Encounter {
    /// Always `"Encounter"`.
    pub resource_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifier: Vec<Identifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// R4's `class` is one `Coding` (ambulatory `AMB`, emergency `EMER`, inpatient `IMP`, virtual `VR`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<Coding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty", rename = "type")]
    pub type_: Vec<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<Reference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub participant: Vec<EncounterParticipant>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<Period>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reason_code: Vec<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reason_reference: Vec<Reference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub location: Vec<EncounterLocation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_provider: Option<Reference>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Encounter {
    fn default() -> Self {
        Encounter {
            resource_type: "Encounter".into(),
            id: None,
            meta: None,
            identifier: Vec::new(),
            status: None,
            class: None,
            type_: Vec::new(),
            subject: None,
            participant: Vec::new(),
            period: None,
            reason_code: Vec::new(),
            reason_reference: Vec::new(),
            location: Vec::new(),
            service_provider: None,
            extra: Map::new(),
        }
    }
}

impl Resource for Encounter {
    const TYPE: &'static str = "Encounter";
    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn extra(&self) -> &Map<String, Value> {
        &self.extra
    }
}

/// Who took part in an encounter.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncounterParticipant {
    #[serde(default, skip_serializing_if = "Vec::is_empty", rename = "type")]
    pub type_: Vec<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<Period>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub individual: Option<Reference>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Where an encounter took place.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncounterLocation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<Period>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// US Core Observation: a lab result, vital sign, or social-history fact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    /// Always `"Observation"`.
    pub resource_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifier: Vec<Identifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub category: Vec<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encounter: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_date_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_period: Option<Period>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issued: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub performer: Vec<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_quantity: Option<Quantity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_codeable_concept: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_string: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_boolean: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_integer: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_absent_reason: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interpretation: Vec<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub note: Vec<Annotation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_range: Vec<ObservationReferenceRange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub has_member: Vec<Reference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub derived_from: Vec<Reference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub component: Vec<ObservationComponent>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Observation {
    fn default() -> Self {
        Observation {
            resource_type: "Observation".into(),
            id: None,
            meta: None,
            identifier: Vec::new(),
            status: None,
            category: Vec::new(),
            code: None,
            subject: None,
            encounter: None,
            effective_date_time: None,
            effective_period: None,
            issued: None,
            performer: Vec::new(),
            value_quantity: None,
            value_codeable_concept: None,
            value_string: None,
            value_boolean: None,
            value_integer: None,
            data_absent_reason: None,
            interpretation: Vec::new(),
            note: Vec::new(),
            reference_range: Vec::new(),
            has_member: Vec::new(),
            derived_from: Vec::new(),
            component: Vec::new(),
            extra: Map::new(),
        }
    }
}

impl Resource for Observation {
    const TYPE: &'static str = "Observation";
    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn extra(&self) -> &Map<String, Value> {
        &self.extra
    }
}

/// A result's normal range.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservationReferenceRange {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low: Option<Quantity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high: Option<Quantity>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub type_: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One part of a multi-part result (a blood pressure's systolic and diastolic).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservationComponent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_quantity: Option<Quantity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_codeable_concept: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_string: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_absent_reason: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interpretation: Vec<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_range: Vec<ObservationReferenceRange>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// US Core DiagnosticReport: a lab panel, imaging or pathology report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticReport {
    /// Always `"DiagnosticReport"`.
    pub resource_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifier: Vec<Identifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub category: Vec<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encounter: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_date_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_period: Option<Period>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issued: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub performer: Vec<Reference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub result: Vec<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conclusion: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conclusion_code: Vec<CodeableConcept>,
    /// The report as a document, often `url: Binary/<id>`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub presented_form: Vec<Attachment>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for DiagnosticReport {
    fn default() -> Self {
        DiagnosticReport {
            resource_type: "DiagnosticReport".into(),
            id: None,
            meta: None,
            identifier: Vec::new(),
            status: None,
            category: Vec::new(),
            code: None,
            subject: None,
            encounter: None,
            effective_date_time: None,
            effective_period: None,
            issued: None,
            performer: Vec::new(),
            result: Vec::new(),
            conclusion: None,
            conclusion_code: Vec::new(),
            presented_form: Vec::new(),
            extra: Map::new(),
        }
    }
}

impl Resource for DiagnosticReport {
    const TYPE: &'static str = "DiagnosticReport";
    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn extra(&self) -> &Map<String, Value> {
        &self.extra
    }
}

/// US Core Condition: a problem, diagnosis or health concern.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Condition {
    /// Always `"Condition"`.
    pub resource_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifier: Vec<Identifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clinical_status: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_status: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub category: Vec<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encounter: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub onset_date_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub onset_period: Option<Period>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub onset_string: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abatement_date_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abatement_string: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorder: Option<Reference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub note: Vec<Annotation>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Condition {
    fn default() -> Self {
        Condition {
            resource_type: "Condition".into(),
            id: None,
            meta: None,
            identifier: Vec::new(),
            clinical_status: None,
            verification_status: None,
            category: Vec::new(),
            severity: None,
            code: None,
            subject: None,
            encounter: None,
            onset_date_time: None,
            onset_period: None,
            onset_string: None,
            abatement_date_time: None,
            abatement_string: None,
            recorded_date: None,
            recorder: None,
            note: Vec::new(),
            extra: Map::new(),
        }
    }
}

impl Resource for Condition {
    const TYPE: &'static str = "Condition";
    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn extra(&self) -> &Map<String, Value> {
        &self.extra
    }
}

/// US Core MedicationRequest: a prescription or medication order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MedicationRequest {
    /// Always `"MedicationRequest"`.
    pub resource_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifier: Vec<Identifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub category: Vec<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub medication_codeable_concept: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub medication_reference: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encounter: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authored_on: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requester: Option<Reference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dosage_instruction: Vec<Dosage>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub note: Vec<Annotation>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for MedicationRequest {
    fn default() -> Self {
        MedicationRequest {
            resource_type: "MedicationRequest".into(),
            id: None,
            meta: None,
            identifier: Vec::new(),
            status: None,
            intent: None,
            category: Vec::new(),
            medication_codeable_concept: None,
            medication_reference: None,
            subject: None,
            encounter: None,
            authored_on: None,
            requester: None,
            dosage_instruction: Vec::new(),
            note: Vec::new(),
            extra: Map::new(),
        }
    }
}

impl Resource for MedicationRequest {
    const TYPE: &'static str = "MedicationRequest";
    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn extra(&self) -> &Map<String, Value> {
        &self.extra
    }
}

/// MedicationStatement: a medication the patient is or was taking.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MedicationStatement {
    /// Always `"MedicationStatement"`.
    pub resource_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifier: Vec<Identifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub medication_codeable_concept: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub medication_reference: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_date_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_period: Option<Period>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_asserted: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub information_source: Option<Reference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dosage: Vec<Dosage>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub note: Vec<Annotation>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for MedicationStatement {
    fn default() -> Self {
        MedicationStatement {
            resource_type: "MedicationStatement".into(),
            id: None,
            meta: None,
            identifier: Vec::new(),
            status: None,
            category: None,
            medication_codeable_concept: None,
            medication_reference: None,
            subject: None,
            context: None,
            effective_date_time: None,
            effective_period: None,
            date_asserted: None,
            information_source: None,
            dosage: Vec::new(),
            note: Vec::new(),
            extra: Map::new(),
        }
    }
}

impl Resource for MedicationStatement {
    const TYPE: &'static str = "MedicationStatement";
    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn extra(&self) -> &Map<String, Value> {
        &self.extra
    }
}

/// US Core AllergyIntolerance: an allergy or intolerance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllergyIntolerance {
    /// Always `"AllergyIntolerance"`.
    pub resource_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifier: Vec<Identifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clinical_status: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_status: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub type_: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub category: Vec<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub criticality: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patient: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub onset_date_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded_date: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub note: Vec<Annotation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reaction: Vec<AllergyIntoleranceReaction>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for AllergyIntolerance {
    fn default() -> Self {
        AllergyIntolerance {
            resource_type: "AllergyIntolerance".into(),
            id: None,
            meta: None,
            identifier: Vec::new(),
            clinical_status: None,
            verification_status: None,
            type_: None,
            category: Vec::new(),
            criticality: None,
            code: None,
            patient: None,
            onset_date_time: None,
            recorded_date: None,
            note: Vec::new(),
            reaction: Vec::new(),
            extra: Map::new(),
        }
    }
}

impl Resource for AllergyIntolerance {
    const TYPE: &'static str = "AllergyIntolerance";
    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn extra(&self) -> &Map<String, Value> {
        &self.extra
    }
}

/// One reaction: what caused it and how it showed.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllergyIntoleranceReaction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub substance: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub manifestation: Vec<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub onset: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// US Core Immunization: a vaccine given or reported.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Immunization {
    /// Always `"Immunization"`.
    pub resource_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifier: Vec<Identifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// CVX in US Core.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vaccine_code: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patient: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encounter: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurrence_date_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurrence_string: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_source: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lot_number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expiration_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dose_quantity: Option<Quantity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub performer: Vec<ImmunizationPerformer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<Reference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub note: Vec<Annotation>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Immunization {
    fn default() -> Self {
        Immunization {
            resource_type: "Immunization".into(),
            id: None,
            meta: None,
            identifier: Vec::new(),
            status: None,
            vaccine_code: None,
            patient: None,
            encounter: None,
            occurrence_date_time: None,
            occurrence_string: None,
            primary_source: None,
            lot_number: None,
            expiration_date: None,
            site: None,
            route: None,
            dose_quantity: None,
            performer: Vec::new(),
            location: None,
            note: Vec::new(),
            extra: Map::new(),
        }
    }
}

impl Resource for Immunization {
    const TYPE: &'static str = "Immunization";
    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn extra(&self) -> &Map<String, Value> {
        &self.extra
    }
}

/// Who gave a vaccine.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImmunizationPerformer {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<Reference>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// US Core Procedure: something done to or for the patient.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Procedure {
    /// Always `"Procedure"`.
    pub resource_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifier: Vec<Identifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encounter: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub performed_date_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub performed_period: Option<Period>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub performer: Vec<ProcedurePerformer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<Reference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reason_code: Vec<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub body_site: Vec<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub note: Vec<Annotation>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Procedure {
    fn default() -> Self {
        Procedure {
            resource_type: "Procedure".into(),
            id: None,
            meta: None,
            identifier: Vec::new(),
            status: None,
            category: None,
            code: None,
            subject: None,
            encounter: None,
            performed_date_time: None,
            performed_period: None,
            performer: Vec::new(),
            location: None,
            reason_code: Vec::new(),
            body_site: Vec::new(),
            note: Vec::new(),
            extra: Map::new(),
        }
    }
}

impl Resource for Procedure {
    const TYPE: &'static str = "Procedure";
    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn extra(&self) -> &Map<String, Value> {
        &self.extra
    }
}

/// Who performed a procedure.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcedurePerformer {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<Reference>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// US Core DocumentReference: a document (a visit summary, a clinical note, a scan) and where its content is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentReference {
    /// Always `"DocumentReference"`.
    pub resource_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifier: Vec<Identifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub type_: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub category: Vec<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub author: Vec<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custodian: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<DocumentReferenceContent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<DocumentReferenceContext>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for DocumentReference {
    fn default() -> Self {
        DocumentReference {
            resource_type: "DocumentReference".into(),
            id: None,
            meta: None,
            identifier: Vec::new(),
            status: None,
            doc_status: None,
            type_: None,
            category: Vec::new(),
            subject: None,
            date: None,
            author: Vec::new(),
            custodian: None,
            description: None,
            content: Vec::new(),
            context: None,
            extra: Map::new(),
        }
    }
}

impl Resource for DocumentReference {
    const TYPE: &'static str = "DocumentReference";
    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn extra(&self) -> &Map<String, Value> {
        &self.extra
    }
}

/// One rendition of a document.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentReferenceContent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachment: Option<Attachment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<Coding>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// The clinical context of a document.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentReferenceContext {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub encounter: Vec<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<Period>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<Reference>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Binary: raw content (a PDF, an image) as base64 `data` with its `contentType`. Fetch it with `Accept: application/fhir+json` to get this form.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Binary {
    /// Always `"Binary"`.
    pub resource_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// Base64; [`Binary::bytes`] decodes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security_context: Option<Reference>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Binary {
    fn default() -> Self {
        Binary {
            resource_type: "Binary".into(),
            id: None,
            meta: None,
            content_type: None,
            data: None,
            security_context: None,
            extra: Map::new(),
        }
    }
}

impl Resource for Binary {
    const TYPE: &'static str = "Binary";
    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn extra(&self) -> &Map<String, Value> {
        &self.extra
    }
}

/// Coverage: an insurance plan the patient is covered by.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Coverage {
    /// Always `"Coverage"`.
    pub resource_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifier: Vec<Identifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub type_: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_holder: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscriber: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscriber_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beneficiary: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relationship: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<Period>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub payor: Vec<Reference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub class: Vec<CoverageClass>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cost_to_beneficiary: Vec<CoverageCost>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Coverage {
    fn default() -> Self {
        Coverage {
            resource_type: "Coverage".into(),
            id: None,
            meta: None,
            identifier: Vec::new(),
            status: None,
            type_: None,
            policy_holder: None,
            subscriber: None,
            subscriber_id: None,
            beneficiary: None,
            dependent: None,
            relationship: None,
            period: None,
            payor: Vec::new(),
            class: Vec::new(),
            order: None,
            network: None,
            cost_to_beneficiary: Vec::new(),
            extra: Map::new(),
        }
    }
}

impl Resource for Coverage {
    const TYPE: &'static str = "Coverage";
    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn extra(&self) -> &Map<String, Value> {
        &self.extra
    }
}

/// A plan, group or other class of a coverage.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageClass {
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub type_: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// What the beneficiary pays for a kind of service: a copay, a coinsurance.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageCost {
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub type_: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_money: Option<Money>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_quantity: Option<Quantity>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// ExplanationOfBenefit in the CARIN Blue Button shape: a payer's adjudicated claim, its lines,
/// the amounts decided on each (allowed, paid by the payer, the member's share) and the totals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplanationOfBenefit {
    /// Always `"ExplanationOfBenefit"`.
    pub resource_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifier: Vec<Identifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub type_: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_type: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "use")]
    pub use_: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patient: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub billable_period: Option<Period>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insurer: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payee: Option<EobPayee>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facility: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disposition: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub care_team: Vec<EobCareTeam>,
    /// CARIN carries the claim's received date, admission period and network status here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supporting_info: Vec<EobSupportingInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnosis: Vec<EobDiagnosis>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub procedure: Vec<EobProcedure>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub insurance: Vec<EobInsurance>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub item: Vec<EobItem>,
    /// Header-level adjudication: CARIN's adjudication date and denial reason, or the
    ///     /// amounts of a claim with no lines.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adjudication: Vec<EobAdjudication>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub total: Vec<EobTotal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment: Option<EobPayment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub process_note: Vec<EobProcessNote>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benefit_period: Option<Period>,
    /// Where a payer reports deductible and out-of-pocket accumulators.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub benefit_balance: Vec<EobBenefitBalance>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for ExplanationOfBenefit {
    fn default() -> Self {
        ExplanationOfBenefit {
            resource_type: "ExplanationOfBenefit".into(),
            id: None,
            meta: None,
            identifier: Vec::new(),
            status: None,
            type_: None,
            sub_type: None,
            use_: None,
            patient: None,
            billable_period: None,
            created: None,
            insurer: None,
            provider: None,
            payee: None,
            facility: None,
            outcome: None,
            disposition: None,
            related: Vec::new(),
            care_team: Vec::new(),
            supporting_info: Vec::new(),
            diagnosis: Vec::new(),
            procedure: Vec::new(),
            insurance: Vec::new(),
            item: Vec::new(),
            adjudication: Vec::new(),
            total: Vec::new(),
            payment: None,
            process_note: Vec::new(),
            benefit_period: None,
            benefit_balance: Vec::new(),
            extra: Map::new(),
        }
    }
}

impl Resource for ExplanationOfBenefit {
    const TYPE: &'static str = "ExplanationOfBenefit";
    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn extra(&self) -> &Map<String, Value> {
        &self.extra
    }
}

/// Who the payer pays.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EobPayee {
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub type_: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub party: Option<Reference>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A supporting fact of a claim (CARIN: `clmrecvddate`, `admissionperiod`, `billingnetworkcontractingstatus`, ...).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EobSupportingInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sequence: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timing_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timing_period: Option<Period>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_boolean: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_string: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_quantity: Option<Quantity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_reference: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<Coding>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A procedure on the claim.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EobProcedure {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sequence: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty", rename = "type")]
    pub type_: Vec<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub procedure_codeable_concept: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub procedure_reference: Option<Reference>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A note the payer added.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EobProcessNote {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub type_: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<CodeableConcept>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A benefit's balance: a deductible or out-of-pocket accumulator.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EobBenefitBalance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excluded: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub term: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub financial: Vec<EobBenefitFinancial>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One figure of a benefit balance: what is allowed and what is used.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EobBenefitFinancial {
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub type_: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_unsigned_int: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_string: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_money: Option<Money>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub used_unsigned_int: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub used_money: Option<Money>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A member of the claim's care team.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EobCareTeam {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sequence: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qualification: Option<CodeableConcept>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A diagnosis on the claim.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EobDiagnosis {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sequence: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnosis_codeable_concept: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Vec::is_empty", rename = "type")]
    pub type_: Vec<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_admission: Option<CodeableConcept>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// The coverage a claim was adjudicated under.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EobInsurance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focal: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage: Option<Reference>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One claim line.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EobItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sequence: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub care_team_sequence: Vec<Option<u32>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnosis_sequence: Vec<Option<u32>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub procedure_sequence: Vec<Option<u32>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub note_number: Vec<Option<u32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revenue: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product_or_service: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modifier: Vec<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serviced_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serviced_period: Option<Period>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location_codeable_concept: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantity: Option<Quantity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit_price: Option<Money>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net: Option<Money>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adjudication: Vec<EobAdjudication>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// An amount the payer decided on a line or a claim. `category` says which, from CARIN's
/// adjudication and adjudication-discriminator code systems: `submitted`, `eligible`, `paidtoprovider`,
/// `paidbypatient`, `deductible`, `copay`, `coinsurance`, `noncovered`, `memberliability`, `discount`,
/// `priorpayerpaid`, ...
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EobAdjudication {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<Money>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Decimal>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A claim's total in one category.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EobTotal {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<Money>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// What the payer paid and when.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EobPayment {
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub type_: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<Money>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// ImagingStudy: a DICOM study's series and instances, by UID.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagingStudy {
    /// Always `"ImagingStudy"`.
    pub resource_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifier: Vec<Identifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modality: Vec<Coding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encounter: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub based_on: Vec<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub referrer: Option<Reference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub endpoint: Vec<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_of_series: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_of_instances: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub series: Vec<ImagingStudySeries>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for ImagingStudy {
    fn default() -> Self {
        ImagingStudy {
            resource_type: "ImagingStudy".into(),
            id: None,
            meta: None,
            identifier: Vec::new(),
            status: None,
            modality: Vec::new(),
            subject: None,
            encounter: None,
            started: None,
            based_on: Vec::new(),
            referrer: None,
            endpoint: Vec::new(),
            number_of_series: None,
            number_of_instances: None,
            description: None,
            series: Vec::new(),
            extra: Map::new(),
        }
    }
}

impl Resource for ImagingStudy {
    const TYPE: &'static str = "ImagingStudy";
    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn extra(&self) -> &Map<String, Value> {
        &self.extra
    }
}

/// One series of a study.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagingStudySeries {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modality: Option<Coding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_of_instances: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_site: Option<Coding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub instance: Vec<ImagingStudyInstance>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One instance (image or object) of a series.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagingStudyInstance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sop_class: Option<Coding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// US Core Practitioner: a clinician.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Practitioner {
    /// Always `"Practitioner"`.
    pub resource_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifier: Vec<Identifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub name: Vec<HumanName>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub telecom: Vec<ContactPoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub address: Vec<Address>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gender: Option<String>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Practitioner {
    fn default() -> Self {
        Practitioner {
            resource_type: "Practitioner".into(),
            id: None,
            meta: None,
            identifier: Vec::new(),
            active: None,
            name: Vec::new(),
            telecom: Vec::new(),
            address: Vec::new(),
            gender: None,
            extra: Map::new(),
        }
    }
}

impl Resource for Practitioner {
    const TYPE: &'static str = "Practitioner";
    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn extra(&self) -> &Map<String, Value> {
        &self.extra
    }
}

/// US Core Organization: a provider organization, payer or practice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Organization {
    /// Always `"Organization"`.
    pub resource_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifier: Vec<Identifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty", rename = "type")]
    pub type_: Vec<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alias: Vec<Option<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub telecom: Vec<ContactPoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub address: Vec<Address>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part_of: Option<Reference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub endpoint: Vec<Reference>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Organization {
    fn default() -> Self {
        Organization {
            resource_type: "Organization".into(),
            id: None,
            meta: None,
            identifier: Vec::new(),
            active: None,
            type_: Vec::new(),
            name: None,
            alias: Vec::new(),
            telecom: Vec::new(),
            address: Vec::new(),
            part_of: None,
            endpoint: Vec::new(),
            extra: Map::new(),
        }
    }
}

impl Resource for Organization {
    const TYPE: &'static str = "Organization";
    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn extra(&self) -> &Map<String, Value> {
        &self.extra
    }
}

/// Provenance: who made or changed the resources it targets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provenance {
    /// Always `"Provenance"`.
    pub resource_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub target: Vec<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agent: Vec<ProvenanceAgent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entity: Vec<ProvenanceEntity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity: Option<CodeableConcept>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Provenance {
    fn default() -> Self {
        Provenance {
            resource_type: "Provenance".into(),
            id: None,
            meta: None,
            target: Vec::new(),
            recorded: None,
            agent: Vec::new(),
            entity: Vec::new(),
            activity: None,
            extra: Map::new(),
        }
    }
}

impl Resource for Provenance {
    const TYPE: &'static str = "Provenance";
    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn extra(&self) -> &Map<String, Value> {
        &self.extra
    }
}

/// An agent in a provenance.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvenanceAgent {
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub type_: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub who: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_behalf_of: Option<Reference>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// An entity a provenance used.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvenanceEntity {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub what: Option<Reference>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Bundle: a search page or a collection. Entries keep their resources as JSON; read them typed with [`crate::from_value`] or, losslessly, with [`crate::RawBundle`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bundle {
    /// Always `"Bundle"`.
    pub resource_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub type_: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub link: Vec<BundleLink>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entry: Vec<BundleEntry>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Bundle {
    fn default() -> Self {
        Bundle {
            resource_type: "Bundle".into(),
            id: None,
            meta: None,
            type_: None,
            timestamp: None,
            total: None,
            link: Vec::new(),
            entry: Vec::new(),
            extra: Map::new(),
        }
    }
}

impl Resource for Bundle {
    const TYPE: &'static str = "Bundle";
    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn extra(&self) -> &Map<String, Value> {
        &self.extra
    }
}

/// A paging link: `relation` is `self`, `next`, `previous`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleLink {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One entry of a bundle.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<BundleEntrySearch>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Why an entry is in a search page: `match`, `include` or `outcome`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleEntrySearch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<Decimal>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// OperationOutcome: errors and warnings. [`crate::outcome`] maps one onto the family's exit codes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationOutcome {
    /// Always `"OperationOutcome"`.
    pub resource_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub issue: Vec<OperationOutcomeIssue>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for OperationOutcome {
    fn default() -> Self {
        OperationOutcome {
            resource_type: "OperationOutcome".into(),
            id: None,
            meta: None,
            issue: Vec::new(),
            extra: Map::new(),
        }
    }
}

impl Resource for OperationOutcome {
    const TYPE: &'static str = "OperationOutcome";
    fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn extra(&self) -> &Map<String, Value> {
        &self.extra
    }
}

/// One issue: its severity (`fatal`, `error`, `warning`, `information`), its code, and text.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationOutcomeIssue {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<CodeableConcept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub location: Vec<Option<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expression: Vec<Option<String>>,
    /// Every element this type does not name: extensions, `_` primitive
    /// extensions, choice types and elements not typed here.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
