//! Where a FHIR server signs people in: SMART's
//! `.well-known/smart-configuration`, or the `oauth-uris` extension of its
//! CapabilityStatement (`/metadata`), for a server that publishes only that.

use pk_cli_core::transport::{require_secure_url, HttpRequest, Transport};
use pk_cli_core::CliError;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::quirks::VendorQuirks;

const OAUTH_URIS: &str = "http://fhir-registry.smarthealthit.org/StructureDefinition/oauth-uris";

/// A server's SMART configuration (SMART App Launch 2.x §2.4). Members this
/// type does not name are kept in `extra`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmartConfiguration {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registration_endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jwks_uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revocation_endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes_supported: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub code_challenge_methods_supported: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub grant_types_supported: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub token_endpoint_auth_methods_supported: Vec<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl SmartConfiguration {
    /// Read `.well-known/smart-configuration`'s body.
    pub fn parse(body: &str) -> Result<Self, CliError> {
        let config: SmartConfiguration = serde_json::from_str(body).map_err(|e| {
            CliError::Upstream(format!("the SMART configuration does not read: {e}"))
        })?;
        config.check()?;
        Ok(config)
    }

    /// Read the `oauth-uris` extension of a CapabilityStatement's
    /// `rest[].security`. The capabilities are unknown from it.
    pub fn from_capability_statement(body: &str) -> Result<Self, CliError> {
        let statement: Value = serde_json::from_str(body).map_err(|e| {
            CliError::Upstream(format!("the CapabilityStatement does not read: {e}"))
        })?;
        let uris = statement
            .get("rest")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|rest| rest.pointer("/security/extension"))
            .filter_map(Value::as_array)
            .flatten()
            .find(|ext| ext.get("url").and_then(Value::as_str) == Some(OAUTH_URIS))
            .and_then(|ext| ext.get("extension"))
            .and_then(Value::as_array)
            .ok_or_else(|| {
                CliError::Upstream(
                    "the CapabilityStatement names no SMART endpoints (oauth-uris)".into(),
                )
            })?;
        let uri = |name: &str| {
            uris.iter()
                .find(|e| e.get("url").and_then(Value::as_str) == Some(name))
                .and_then(|e| e.get("valueUri").and_then(Value::as_str))
                .map(str::to_string)
        };
        let config = SmartConfiguration {
            issuer: None,
            authorization_endpoint: uri("authorize").unwrap_or_default(),
            token_endpoint: uri("token").unwrap_or_default(),
            registration_endpoint: uri("register"),
            jwks_uri: None,
            revocation_endpoint: uri("revoke"),
            capabilities: Vec::new(),
            scopes_supported: Vec::new(),
            code_challenge_methods_supported: Vec::new(),
            grant_types_supported: Vec::new(),
            token_endpoint_auth_methods_supported: Vec::new(),
            extra: Map::new(),
        };
        config.check()?;
        Ok(config)
    }

    /// Whether the server lists this capability (`permission-v2`,
    /// `client-confidential-symmetric`).
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|c| c == capability)
    }

    /// Whether PKCE S256 is usable: listed, or no methods listed at all (a
    /// CapabilityStatement says nothing about them).
    pub fn supports_s256(&self) -> bool {
        self.code_challenge_methods_supported.is_empty()
            || self
                .code_challenge_methods_supported
                .iter()
                .any(|m| m == "S256")
    }

    /// Every URL it keeps: the issuer and the authorization, token,
    /// registration, revocation and JWK Set endpoints.
    fn urls(&self) -> [(&'static str, Option<&String>); 6] {
        [
            ("authorization endpoint", Some(&self.authorization_endpoint)),
            ("token endpoint", Some(&self.token_endpoint)),
            ("registration endpoint", self.registration_endpoint.as_ref()),
            ("revocation endpoint", self.revocation_endpoint.as_ref()),
            ("JWK Set", self.jwks_uri.as_ref()),
            ("issuer", self.issuer.as_ref()),
        ]
    }

    /// Each URL must be `https`, or `http` to this machine for a test
    /// server: a server's metadata decides where the sign-in, the code and
    /// the tokens go.
    fn check(&self) -> Result<(), CliError> {
        for (what, url) in self.urls() {
            let Some(url) = url else { continue };
            if url.is_empty() {
                if what == "authorization endpoint" || what == "token endpoint" {
                    return Err(CliError::Upstream(format!(
                        "the SMART configuration names no {what}"
                    )));
                }
                continue;
            }
            require_secure_url(url).map_err(|_| {
                CliError::Upstream(format!(
                    "the SMART configuration's {what} is not https; refusing to sign in through it"
                ))
            })?;
        }
        Ok(())
    }

    /// For the configuration of the server at `fhir_base`: an `https` server
    /// names only `https` URLs. Plain `http` to this machine is for a test
    /// server whose own base is one, and an `https` server naming it would
    /// send the code, a client secret and the tokens to whatever local
    /// process holds the port. [`discover`] checks this.
    pub fn check_for(&self, fhir_base: &str) -> Result<(), CliError> {
        self.check()?;
        if !is_https(fhir_base) {
            return Ok(());
        }
        for (what, url) in self.urls() {
            if let Some(url) = url.filter(|u| !u.is_empty()) {
                if !is_https(url) {
                    return Err(CliError::Upstream(format!(
                        "the https FHIR server's SMART configuration names a {what} that is not \
                         https; refusing to sign in through it"
                    )));
                }
            }
        }
        Ok(())
    }
}

fn is_https(url: &str) -> bool {
    url.get(..8)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("https://"))
}

/// Find the server's SMART configuration. `.well-known/smart-configuration`
/// first, then `/metadata` when it answers a 3xx (redirects are not
/// followed), a 4xx or something that does not read; a quirk can ask for
/// `/metadata` first. A 429 or a 5xx is the error: the server said nothing
/// about where to sign in. The configuration is checked against the base
/// ([`SmartConfiguration::check_for`]).
pub async fn discover<T: Transport>(
    transport: &T,
    fhir_base: &str,
    quirks: &dyn VendorQuirks,
    binary: &str,
    version: &str,
) -> Result<SmartConfiguration, CliError> {
    let base = fhir_base.trim_end_matches('/');
    require_secure_url(base)?;
    let well_known = format!("{base}/.well-known/smart-configuration");
    let metadata = format!("{base}/metadata");
    let order: [(&str, &str); 2] = if quirks.metadata_first() {
        [(&metadata, "metadata"), (&well_known, "well-known")]
    } else {
        [(&well_known, "well-known"), (&metadata, "metadata")]
    };
    let mut first_error = None;
    for (url, kind) in order {
        let accept = if kind == "metadata" {
            "application/fhir+json"
        } else {
            "application/json"
        };
        let mut request = HttpRequest::get(url)
            .header("Accept", accept)
            .user_agent(binary, version);
        for (name, value) in quirks.discovery_headers() {
            request = request.header(name, value);
        }
        let origin = request.origin();
        let response = transport.send(request).await?;
        let outcome = if response.is_success() {
            if kind == "metadata" {
                SmartConfiguration::from_capability_statement(&response.body)
            } else {
                SmartConfiguration::parse(&response.body)
            }
        } else if (300..400).contains(&response.status) {
            Err(CliError::Upstream(format!(
                "the FHIR server at {origin} answered HTTP {} for its SMART {kind} document: a \
                 redirect, which is not followed",
                response.status
            )))
        } else if (400..500).contains(&response.status) && response.status != 429 {
            Err(CliError::Upstream(format!(
                "the FHIR server at {origin} answered HTTP {} for its SMART {kind} document",
                response.status
            )))
        } else {
            return Err(CliError::Upstream(format!(
                "the FHIR server at {origin} answered HTTP {} for its SMART {kind} document. Try \
                 again later",
                response.status
            )));
        };
        match outcome.and_then(|config| config.check_for(base).map(|()| config)) {
            Ok(config) => return Ok(config),
            Err(e) => {
                first_error.get_or_insert(e);
            }
        }
    }
    Err(first_error.expect("two attempts"))
}
