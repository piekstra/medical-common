//! Resources as the server wrote them.
//!
//! A FHIR `decimal` carries its precision in its spelling (`1.50` is not
//! `1.5`), and `serde_json::Value` keeps only the value. So what is stored and
//! streamed is the server's own JSON text, minified without parsing a number:
//! [`RawResource`]. [`RawBundle`] reads a search page while keeping each entry's
//! resource that way.

use pk_cli_core::CliError;
use serde::Deserialize;
use serde_json::value::RawValue;
use serde_json::Value;

use crate::datatypes::Meta;
use crate::resources::{BundleEntrySearch, BundleLink};
use crate::Resource;

/// One FHIR resource: its type and id, and its JSON text exactly as the server
/// wrote it, minified (no whitespace outside strings) and nothing else changed.
#[derive(Clone, PartialEq, Eq)]
pub struct RawResource {
    /// `resourceType`.
    pub resource_type: String,
    /// `id`, empty for a resource that has none.
    pub id: String,
    /// The minified JSON text.
    pub raw: String,
    version_id: Option<String>,
    last_updated: Option<String>,
}

/// The members read to name a resource; every other member is skipped.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Peek {
    resource_type: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    meta: Option<PeekMeta>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PeekMeta {
    #[serde(default)]
    version_id: Option<String>,
    #[serde(default)]
    last_updated: Option<String>,
}

impl RawResource {
    /// Read one resource from JSON text. Text that is not a JSON object with a
    /// string `resourceType` is [`CliError::Upstream`]: a server sent it.
    pub fn parse(text: &str) -> Result<Self, CliError> {
        let peek: Peek = serde_json::from_str(text)
            .map_err(|e| CliError::Upstream(format!("a FHIR resource is not JSON: {e}")))?;
        let resource_type = peek
            .resource_type
            .filter(|t| !t.is_empty())
            .ok_or_else(|| CliError::Upstream("a FHIR resource has no resourceType".into()))?;
        let meta = peek.meta;
        Ok(RawResource {
            resource_type,
            id: peek.id.unwrap_or_default(),
            raw: minify(text),
            version_id: meta.as_ref().and_then(|m| m.version_id.clone()),
            last_updated: meta.and_then(|m| m.last_updated),
        })
    }

    /// From a parsed value. Its numbers are already in `serde_json`'s spelling,
    /// so prefer [`RawResource::parse`] on the server's text.
    pub fn from_value(value: &Value) -> Result<Self, CliError> {
        let text = serde_json::to_string(value)
            .map_err(|e| CliError::Other(format!("serializing a FHIR resource: {e}")))?;
        Self::parse(&text)
    }

    /// From one of this crate's typed resources.
    pub fn from_resource<R: Resource>(resource: &R) -> Result<Self, CliError> {
        let text = serde_json::to_string(resource)
            .map_err(|e| CliError::Other(format!("serializing a FHIR {}: {e}", R::TYPE)))?;
        Self::parse(&text)
    }

    /// The resource as a JSON value (numbers in `serde_json`'s spelling).
    pub fn value(&self) -> Result<Value, CliError> {
        serde_json::from_str(&self.raw)
            .map_err(|e| CliError::Upstream(format!("a FHIR resource is not JSON: {e}")))
    }

    /// The resource as one of this crate's types, refusing another
    /// `resourceType`.
    pub fn parse_as<R: Resource>(&self) -> Result<R, CliError> {
        if self.resource_type != R::TYPE {
            return Err(CliError::Upstream(format!(
                "expected a FHIR {}, found {}",
                R::TYPE,
                self.resource_type
            )));
        }
        crate::from_str(&self.raw)
    }

    /// `meta.versionId`.
    pub fn version_id(&self) -> Option<&str> {
        self.version_id.as_deref()
    }

    /// `meta.lastUpdated`.
    pub fn last_updated(&self) -> Option<&str> {
        self.last_updated.as_deref()
    }

    /// `resourceType/id`, the resource's relative reference.
    pub fn reference(&self) -> String {
        format!("{}/{}", self.resource_type, self.id)
    }

    /// `meta` read as [`Meta`].
    pub fn meta(&self) -> Option<Meta> {
        let value = self.value().ok()?;
        serde_json::from_value(value.get("meta")?.clone()).ok()
    }
}

/// `Debug` names the resource and the text's length, never its content.
impl std::fmt::Debug for RawResource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawResource")
            .field("resource_type", &self.resource_type)
            .field("id", &self.id)
            .field("bytes", &self.raw.len())
            .finish()
    }
}

/// JSON text with the whitespace outside strings removed, and every other
/// character kept as written: numbers keep their spelling, strings their
/// escapes. The text is assumed to be valid JSON ([`RawResource::parse`] checks
/// it first).
pub fn minify(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_string = false;
    let mut escaped = false;
    for c in text.chars() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if !matches!(c, ' ' | '\t' | '\n' | '\r') {
            out.push(c);
        }
    }
    out
}

/// A Bundle page with each entry's resource kept as the server wrote it.
#[derive(Debug, Clone)]
pub struct RawBundle {
    /// `Bundle.type`: `searchset`, `collection`, ...
    pub bundle_type: Option<String>,
    pub total: Option<u64>,
    pub link: Vec<BundleLink>,
    pub entry: Vec<RawEntry>,
}

/// One entry of a [`RawBundle`].
#[derive(Debug, Clone)]
pub struct RawEntry {
    pub full_url: Option<String>,
    /// `None` for an entry with no resource (a deleted one in a history).
    pub resource: Option<RawResource>,
    pub search: Option<BundleEntrySearch>,
}

impl RawEntry {
    /// `search.mode`: `match`, `include` or `outcome`.
    pub fn mode(&self) -> Option<&str> {
        self.search.as_ref().and_then(|s| s.mode.as_deref())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BundleWire<'a> {
    resource_type: Option<String>,
    #[serde(default, rename = "type")]
    bundle_type: Option<String>,
    #[serde(default)]
    total: Option<u64>,
    #[serde(default)]
    link: Vec<BundleLink>,
    #[serde(default, borrow)]
    entry: Vec<EntryWire<'a>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntryWire<'a> {
    #[serde(default)]
    full_url: Option<String>,
    #[serde(default, borrow)]
    resource: Option<&'a RawValue>,
    #[serde(default)]
    search: Option<BundleEntrySearch>,
}

impl RawBundle {
    /// Read a Bundle from JSON text. Anything else is [`CliError::Upstream`].
    pub fn parse(text: &str) -> Result<Self, CliError> {
        let wire: BundleWire = serde_json::from_str(text)
            .map_err(|e| CliError::Upstream(format!("a FHIR Bundle did not read: {e}")))?;
        match wire.resource_type.as_deref() {
            Some("Bundle") => {}
            Some(other) => {
                return Err(CliError::Upstream(format!(
                    "expected a FHIR Bundle, found {other}"
                )))
            }
            None => {
                return Err(CliError::Upstream(
                    "expected a FHIR Bundle, found no resourceType".into(),
                ))
            }
        }
        let entry = wire
            .entry
            .into_iter()
            .map(|e| {
                Ok(RawEntry {
                    full_url: e.full_url,
                    resource: e
                        .resource
                        .map(|r| RawResource::parse(r.get()))
                        .transpose()?,
                    search: e.search,
                })
            })
            .collect::<Result<_, CliError>>()?;
        Ok(RawBundle {
            bundle_type: wire.bundle_type,
            total: wire.total,
            link: wire.link,
            entry,
        })
    }

    /// The URL of the link with this `relation` (`next`, `self`).
    pub fn link(&self, relation: &str) -> Option<&str> {
        self.link
            .iter()
            .find(|l| l.relation.as_deref() == Some(relation))
            .and_then(|l| l.url.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minify_keeps_numbers_and_strings_as_written() {
        let text = "{ \"resourceType\" : \"Observation\",\n  \"id\": \"o 1\",\n  \"valueQuantity\": { \"value\": 1.50, \"unit\": \"a \\\" b\\\\\" } }";
        assert_eq!(
            minify(text),
            r#"{"resourceType":"Observation","id":"o 1","valueQuantity":{"value":1.50,"unit":"a \" b\\"}}"#
        );
    }

    #[test]
    fn a_raw_resource_names_itself_and_keeps_its_text() {
        let raw = RawResource::parse(
            r#"{"resourceType":"Observation","id":"o1","meta":{"versionId":"3","lastUpdated":"2020-01-01T00:00:00Z"},"valueQuantity":{"value":1.50}}"#,
        )
        .unwrap();
        assert_eq!(raw.resource_type, "Observation");
        assert_eq!(raw.id, "o1");
        assert_eq!(raw.version_id(), Some("3"));
        assert_eq!(raw.last_updated(), Some("2020-01-01T00:00:00Z"));
        assert!(raw.raw.contains("1.50"));
        assert_eq!(raw.reference(), "Observation/o1");
        assert!(!format!("{raw:?}").contains("1.50"));
        let obs: crate::Observation = raw.parse_as().unwrap();
        assert_eq!(obs.id.as_deref(), Some("o1"));
        assert!(raw.parse_as::<crate::Patient>().is_err());
    }

    #[test]
    fn text_that_is_not_a_resource_is_upstream() {
        for text in ["[]", "{}", r#"{"resourceType":""}"#, "nope"] {
            assert_eq!(
                RawResource::parse(text).unwrap_err().exit_code(),
                5,
                "{text}"
            );
        }
    }

    #[test]
    fn a_bundle_keeps_each_entry_raw() {
        let bundle = RawBundle::parse(
            r#"{"resourceType":"Bundle","type":"searchset","total":2,
                "link":[{"relation":"self","url":"https://h.test/r4/Observation?patient=p"},{"relation":"next","url":"https://h.test/r4/Observation?page=2"}],
                "entry":[
                  {"fullUrl":"https://h.test/r4/Observation/o1","resource":{"resourceType":"Observation","id":"o1","valueQuantity":{"value":2.0}},"search":{"mode":"match"}},
                  {"resource":{"resourceType":"OperationOutcome","issue":[{"severity":"warning","code":"informational"}]},"search":{"mode":"outcome"}},
                  {"fullUrl":"urn:uuid:x"}
                ]}"#,
        )
        .unwrap();
        assert_eq!(bundle.total, Some(2));
        assert_eq!(
            bundle.link("next"),
            Some("https://h.test/r4/Observation?page=2")
        );
        assert_eq!(bundle.entry.len(), 3);
        assert!(bundle.entry[0]
            .resource
            .as_ref()
            .unwrap()
            .raw
            .contains("2.0"));
        assert_eq!(bundle.entry[1].mode(), Some("outcome"));
        assert!(bundle.entry[2].resource.is_none());
        assert!(RawBundle::parse(r#"{"resourceType":"Patient"}"#).is_err());
    }
}
