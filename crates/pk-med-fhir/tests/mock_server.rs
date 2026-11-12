//! The client against the mock SMART + FHIR server, over the real HTTP
//! transport. Offline: everything is on 127.0.0.1.

use pk_cli_core::{block_on, Reason};
use pk_cli_http::BlockingTransport;
use pk_cli_secrets::Secret;
use pk_med_fhir::test_util::{self, http_get, Fault, MockServer, PATIENT_ID, PDF};
use pk_med_fhir::{
    Binary, ExplanationOfBenefit, FhirClient, Observation, Organization, Patient, RetryPolicy,
};

fn client(mock: &MockServer) -> FhirClient<BlockingTransport> {
    FhirClient::new(
        BlockingTransport::new().unwrap(),
        &mock.fhir_base(),
        "pk-test",
        "0.0.0",
    )
    .unwrap()
    .bearer(Secret::new(mock.access_token()))
    .sleeper(|_| {})
}

#[test]
fn reads_the_synthetic_patient() {
    let mock = MockServer::start();
    let c = client(&mock);
    let patient: Patient = block_on(c.read_as(PATIENT_ID)).unwrap();
    assert_eq!(patient.name[0].family.as_deref(), Some("Testperson"));
    let seen = mock.requests();
    assert_eq!(seen[0].header("accept"), Some("application/fhir+json"));
    assert!(seen[0]
        .header("user-agent")
        .unwrap()
        .starts_with("pk-test/"));
}

#[test]
fn a_search_reads_every_page_and_keeps_decimals() {
    let mock = MockServer::start();
    let r =
        block_on(client(&mock).search("Observation", &[("patient", PATIENT_ID.into())])).unwrap();
    assert_eq!(
        (r.resources.len(), r.pages, r.truncated, r.total),
        (3, 2, false, Some(3))
    );
    assert!(
        r.resources[0].raw.contains(r#""value":1.50"#),
        "{}",
        r.resources[0].raw
    );
    let obs: Observation = r.resources[0].parse_as().unwrap();
    assert_eq!(obs.id.as_deref(), Some("obs-0001"));
    let second = &mock.requests()[1];
    assert!(
        second.target.contains("patient=pat-0001"),
        "{}",
        second.target
    );
    assert!(second.target.contains("_page=2"), "{}", second.target);
}

#[test]
fn the_page_cap_leaves_the_rest_and_says_so() {
    let mock = MockServer::start();
    mock.page_size(1);
    let r = block_on(client(&mock).page_cap(2).search("Observation", &[])).unwrap();
    assert_eq!((r.resources.len(), r.pages, r.truncated), (2, 2, true));
}

#[test]
fn a_binary_comes_back_as_the_pdf() {
    let mock = MockServer::start();
    let c = client(&mock);
    let doc = block_on(c.search("DocumentReference", &[])).unwrap();
    let doc: pk_med_fhir::DocumentReference = doc.resources[0].parse_as().unwrap();
    let url = doc.content[0]
        .attachment
        .as_ref()
        .unwrap()
        .url
        .clone()
        .unwrap();
    let binary: Binary = block_on(c.binary(&url)).unwrap().parse_as().unwrap();
    assert_eq!(binary.bytes().unwrap(), PDF);
    assert_eq!(binary.content_type.as_deref(), Some("application/pdf"));
}

#[test]
fn no_token_is_exit_3() {
    let mock = MockServer::start();
    let c = FhirClient::new(
        BlockingTransport::new().unwrap(),
        &mock.fhir_base(),
        "pk-test",
        "0",
    )
    .unwrap()
    .bearer(Secret::new("not-issued"));
    let err = block_on(c.read("Patient", PATIENT_ID)).unwrap_err();
    assert_eq!(err.exit_code(), 3);
    assert_eq!(err.reason(), Some(Reason::CredentialRefused));
}

#[test]
fn an_unknown_id_is_exit_4() {
    let mock = MockServer::start();
    let err = block_on(client(&mock).read("Patient", "nobody")).unwrap_err();
    assert_eq!(err.exit_code(), 4);
}

#[test]
fn a_429_is_waited_out_within_the_policy() {
    let mock = MockServer::start();
    mock.fail(Fault::too_many_requests("/fhir/Patient", 1));
    assert!(block_on(client(&mock).read("Patient", PATIENT_ID)).is_ok());
    assert_eq!(mock.requests().len(), 2);

    mock.fail(Fault::too_many_requests("/fhir/Patient", 1).times(5));
    let err = block_on(
        client(&mock)
            .retry(RetryPolicy::none())
            .read("Patient", PATIENT_ID),
    )
    .unwrap_err();
    assert_eq!(err.exit_code(), 5);
    assert!(err.to_string().contains("wait 1 s"), "{err}");
}

#[test]
fn an_operation_outcome_error_keeps_its_text() {
    let mock = MockServer::start();
    mock.fail(Fault::not_supported("/fhir/Condition"));
    let err = block_on(client(&mock).search("Condition", &[("onset-date", "ge2019".into())]))
        .unwrap_err();
    assert_eq!(err.exit_code(), 5);
    assert_eq!(err.reason(), Some(Reason::BadRequest));
    assert!(err.to_string().contains("not supported"), "{err}");
    assert!(
        !err.to_string().contains("ge2019"),
        "the query stays out: {err}"
    );
}

#[test]
fn discovery_is_served_both_ways() {
    let mock = MockServer::start();
    let c = client(&mock);
    let smart: serde_json::Value =
        serde_json::from_str(&block_on(c.get_raw(".well-known/smart-configuration")).unwrap())
            .unwrap();
    assert_eq!(smart["issuer"], mock.fhir_base());
    assert_eq!(smart["token_endpoint"], format!("{}/token", mock.origin()));
    let metadata = block_on(c.get_raw("metadata")).unwrap();
    assert!(metadata.contains(&format!("{}/authorize", mock.origin())));
}

#[test]
fn the_authorize_step_redirects_with_code_state_and_iss() {
    let mock = MockServer::start();
    let url = format!(
        "{}/authorize?response_type=code&client_id={}&redirect_uri=http%3A%2F%2F127.0.0.1%3A9%2Fcallback&state=S1&code_challenge=abc&code_challenge_method=S256&aud={}",
        mock.origin(),
        test_util::CLIENT_ID,
        mock.fhir_base().replace(':', "%3A").replace('/', "%2F"),
    );
    let answer = http_get(&url).unwrap();
    assert_eq!(answer.status, 302);
    let location = answer.header("location").unwrap();
    assert!(
        location.starts_with("http://127.0.0.1:9/callback?code=synthetic-code-"),
        "{location}"
    );
    assert!(location.contains("&state=S1&iss="), "{location}");

    mock.deny_next_authorization();
    let answer = http_get(&url).unwrap();
    let location = answer.header("location").unwrap();
    assert!(location.contains("error=access_denied") && location.contains("state=S1"));
}

#[test]
fn every_synthetic_resource_round_trips_through_its_type() {
    fn check<R: pk_med_fhir::Resource>() {
        for raw in test_util::synthetic(R::TYPE) {
            let value = raw.value().unwrap();
            let typed: R = raw.parse_as().unwrap();
            assert_eq!(serde_json::to_value(&typed).unwrap(), value, "{}", R::TYPE);
        }
    }
    check::<Patient>();
    check::<Organization>();
    check::<pk_med_fhir::Practitioner>();
    check::<pk_med_fhir::Encounter>();
    check::<pk_med_fhir::Condition>();
    check::<Observation>();
    check::<pk_med_fhir::DiagnosticReport>();
    check::<pk_med_fhir::DocumentReference>();
    check::<Binary>();
    check::<pk_med_fhir::MedicationRequest>();
    check::<pk_med_fhir::MedicationStatement>();
    check::<pk_med_fhir::AllergyIntolerance>();
    check::<pk_med_fhir::Immunization>();
    check::<pk_med_fhir::Procedure>();
    check::<pk_med_fhir::ImagingStudy>();
    check::<pk_med_fhir::Coverage>();
    check::<ExplanationOfBenefit>();
    check::<pk_med_fhir::Provenance>();
    assert_eq!(test_util::synthetic_types().len(), 18);
}

#[test]
fn a_carin_eob_reads_its_amounts_and_contained_payer() {
    use pk_med_fhir::Resource;
    let eob: ExplanationOfBenefit = test_util::synthetic("ExplanationOfBenefit")[0]
        .parse_as()
        .unwrap();
    let line = &eob.item[0];
    let member = line
        .adjudication
        .iter()
        .find(|a| {
            a.category.as_ref().is_some_and(|c| {
                c.coding
                    .iter()
                    .any(|k| k.code.as_deref() == Some("memberliability"))
            })
        })
        .unwrap();
    assert_eq!(
        member.amount.as_ref().unwrap().to_money().unwrap().amount,
        "20.00"
    );
    let payer: Organization = eob
        .find_contained(eob.insurer.as_ref().unwrap().reference.as_deref().unwrap())
        .unwrap();
    assert_eq!(payer.name.as_deref(), Some("Synthetic Plan"));
    let balance = &eob.benefit_balance[0].financial[0];
    assert_eq!(
        balance
            .used_money
            .as_ref()
            .unwrap()
            .to_money()
            .unwrap()
            .amount,
        "100.00"
    );
}
