//! Every identity-bearing value in the fixtures has a known dummy shape
//! (`fixtures/README.md`). Checked positively: a value passes only when it
//! matches a dummy, so a real one fails without the test naming it.

use serde_json::Value;

fn fixtures() -> Vec<(String, Value)> {
    let dir = format!("{}/fixtures/synthetic", env!("CARGO_MANIFEST_DIR"));
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        out.push((name, serde_json::from_str(&text).unwrap()));
    }
    assert_eq!(out.len(), 18);
    out
}

/// Every string at a member called `key`, anywhere.
fn strings_at(value: &Value, key: &str, found: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                if k == key {
                    match v {
                        Value::String(s) => found.push(s.clone()),
                        Value::Array(items) => {
                            found.extend(items.iter().filter_map(Value::as_str).map(str::to_string))
                        }
                        _ => {}
                    }
                }
                strings_at(v, key, found);
            }
        }
        Value::Array(items) => items.iter().for_each(|v| strings_at(v, key, found)),
        _ => {}
    }
}

fn all(key: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (_, value) in fixtures() {
        strings_at(&value, key, &mut found);
    }
    found
}

#[test]
fn names_are_the_dummy_ones() {
    for family in all("family") {
        assert!(
            ["Testperson", "Doctor"].contains(&family.as_str()),
            "family {family}"
        );
    }
    for given in all("given") {
        assert!(
            ["Robin", "Example"].contains(&given.as_str()),
            "given {given}"
        );
    }
    // A reference's display names a person or organization.
    fn reference_displays(value: &Value, found: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                if map.contains_key("reference") {
                    if let Some(d) = map.get("display").and_then(Value::as_str) {
                        found.push(d.to_string());
                    }
                }
                map.values().for_each(|v| reference_displays(v, found));
            }
            Value::Array(items) => items.iter().for_each(|v| reference_displays(v, found)),
            _ => {}
        }
    }
    let mut displays = Vec::new();
    for (_, value) in fixtures() {
        reference_displays(&value, &mut displays);
    }
    assert!(!displays.is_empty());
    for display in displays {
        assert_eq!(display, "Robin Testperson");
    }
    for name in all("name") {
        assert!(
            name.starts_with("Synthetic ") || name == "Example Health",
            "organization or class name {name}"
        );
    }
}

#[test]
fn contact_details_are_the_dummy_ones() {
    for date in all("birthDate") {
        assert_eq!(date, "1970-01-01");
    }
    let mut telecoms = Vec::new();
    for (_, value) in fixtures() {
        strings_at(&value, "value", &mut telecoms);
    }
    for value in telecoms {
        if value.contains('@') {
            assert!(value.ends_with("@example.com"), "email {value}");
        }
        let digits: String = value.chars().filter(char::is_ascii_digit).collect();
        if value.starts_with("555") {
            assert!(
                value.starts_with("555-01") && digits.len() == 7,
                "phone {value}"
            );
        }
    }
    for line in all("line") {
        assert_eq!(line, "1 Example St");
    }
    for city in all("city") {
        assert_eq!(city, "Exampleville");
    }
    for code in all("postalCode") {
        assert_eq!(code, "00000");
    }
    for state in all("state") {
        assert_eq!(state, "ZZ");
    }
}

#[test]
fn identifiers_are_dummy_shapes() {
    let mut found = Vec::new();
    for (_, value) in fixtures() {
        if let Value::Array(resources) = value {
            for r in resources {
                for id in r
                    .get("identifier")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    found.push(id["value"].as_str().unwrap().to_string());
                }
                for key in ["subscriberId", "lotNumber"] {
                    if let Some(v) = r.get(key).and_then(Value::as_str) {
                        found.push(v.to_string());
                    }
                }
            }
        }
    }
    assert!(!found.is_empty());
    for value in found {
        let digits: String = value.chars().filter(char::is_ascii_digit).collect();
        let dummy_digits = digits.is_empty()
            || digits.chars().all(|c| c == '0')
            || digits[..digits.len() - 1].chars().all(|c| c == '0');
        let dummy = value.starts_with("urn:oid:2.25.1")
            || (dummy_digits
                && ["MRN-", "MEMBER-", "GROUP-", "LOT-", "0000"]
                    .iter()
                    .any(|p| value.starts_with(p)));
        assert!(dummy, "identifier {value}");
    }
}

#[test]
fn dicom_uids_are_under_the_dummy_root() {
    for uid in all("uid") {
        assert!(uid == "2.25.1" || uid.starts_with("2.25.1."), "uid {uid}");
    }
}

#[test]
fn dates_of_care_are_in_2019_or_2020() {
    let mut dates = Vec::new();
    for key in [
        "start",
        "end",
        "effectiveDateTime",
        "issued",
        "onsetDateTime",
        "recordedDate",
        "authoredOn",
        "occurrenceDateTime",
        "performedDateTime",
        "date",
        "created",
        "servicedDate",
        "timingDate",
        "recorded",
        "started",
        "lastUpdated",
    ] {
        dates.extend(all(key));
    }
    assert!(dates.len() > 20);
    for date in dates {
        assert!(
            date.starts_with("2019-") || date.starts_with("2020-"),
            "date {date}"
        );
    }
}
