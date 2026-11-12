//! A sign-in as the keychain keeps it: one JSON item per (organization,
//! patient), account `smart/<organization>/<patient>`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;

use pk_cli_auth::oauth::{TokenResponse, TokenSet};
use pk_cli_core::CliError;
use pk_cli_secrets::{CredentialStore, Secret};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use zeroize::Zeroize;

/// The keychain account of a sign-in.
pub fn account(organization: &str, patient: &str) -> String {
    format!("smart/{organization}/{patient}")
}

/// What a SMART sign-in leaves to keep. `Debug` shows no token or key.
#[derive(Clone, Serialize, Deserialize)]
pub struct SmartSession {
    /// The CLI's organization id.
    pub organization: String,
    /// The launch context's patient, the FHIR `Patient.id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patient: Option<String>,
    /// The `fhirUser` of the person signed in, from the token response or
    /// the ID token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fhir_user: Option<String>,
    /// The FHIR base signed in to, normalized.
    pub fhir_base: String,
    /// The discovered issuer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
    pub token_endpoint: String,
    /// The client the tokens were issued to.
    pub client_id: String,
    /// The scope asked for at sign-in, sent again on a refresh.
    #[serde(default)]
    pub scope: String,
    pub tokens: TokenSet,
    /// A client registered for this sign-in (Epic's dynamic registration).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic_client: Option<DynamicClient>,
    /// A vendor's own state (a practice chosen at sign-in).
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub extra: Map<String, Value>,
}

impl SmartSession {
    /// The keychain account this session is stored under. A session with no
    /// patient in its launch context uses `_`.
    pub fn account(&self) -> String {
        account(&self.organization, self.patient.as_deref().unwrap_or("_"))
    }

    /// `Authorization: Bearer …` for a FHIR request.
    pub fn bearer(&self) -> Secret {
        Secret::new(self.tokens.access_token.clone())
    }

    /// Whether it can be renewed without the person: a refresh token, or a
    /// registered client that signs its own grant.
    pub fn renewable(&self) -> bool {
        self.tokens.refresh_token.is_some() || self.dynamic_client.is_some()
    }
}

impl fmt::Debug for SmartSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SmartSession")
            .field("organization", &self.organization)
            .field("patient", &self.patient.is_some())
            .field("fhir_base", &self.fhir_base)
            .field("client_id", &self.client_id)
            .field("scope", &self.scope)
            .field("tokens", &self.tokens)
            .field("dynamic_client", &self.dynamic_client)
            .finish_non_exhaustive()
    }
}

/// A client registered for one sign-in (RFC 7591), which gets its access
/// tokens with a JWT it signs (RFC 7523). The key is opaque here: the CLI's
/// [`crate::JwtSigner`] made it and reads it. Zeroed on drop.
#[derive(Clone, Serialize, Deserialize)]
pub struct DynamicClient {
    pub client_id: String,
    /// The JWS algorithm the key signs with (`RS384`).
    pub alg: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kid: Option<String>,
    /// The private key in the signer's own encoding (PKCS#8 DER in base64).
    key: String,
}

impl DynamicClient {
    pub fn new(
        client_id: impl Into<String>,
        alg: impl Into<String>,
        kid: Option<String>,
        key: Secret,
    ) -> Self {
        DynamicClient {
            client_id: client_id.into(),
            alg: alg.into(),
            kid,
            key: key.expose().to_string(),
        }
    }

    pub fn key(&self) -> Secret {
        Secret::new(self.key.clone())
    }
}

impl fmt::Debug for DynamicClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DynamicClient")
            .field("client_id", &self.client_id)
            .field("alg", &self.alg)
            .finish_non_exhaustive()
    }
}

impl Drop for DynamicClient {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}

/// SMART's launch context, from a token response (`patient`, `encounter`,
/// `need_patient_banner`) and its ID token's `fhirUser`, read unverified and
/// for display: the FHIR server, not this, decides access. `Debug` shows
/// which members are present, not their ids.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct LaunchContext {
    pub patient: Option<String>,
    pub encounter: Option<String>,
    pub fhir_user: Option<String>,
    pub need_patient_banner: Option<bool>,
}

impl fmt::Debug for LaunchContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LaunchContext")
            .field("patient", &self.patient.is_some())
            .field("encounter", &self.encounter.is_some())
            .field("fhir_user", &self.fhir_user.is_some())
            .field("need_patient_banner", &self.need_patient_banner)
            .finish()
    }
}

impl LaunchContext {
    pub fn from_response(response: &TokenResponse) -> Self {
        Self::from_parts(
            &response.extra,
            response.id_token.as_ref().map(Secret::expose),
        )
    }

    pub fn from_tokens(tokens: &TokenSet) -> Self {
        Self::from_parts(&tokens.extra, tokens.id_token.as_deref())
    }

    fn from_parts(extra: &Map<String, Value>, id_token: Option<&str>) -> Self {
        let text = |name: &str| extra.get(name).and_then(Value::as_str).map(str::to_string);
        let fhir_user = text("fhirUser").or_else(|| {
            id_token.and_then(pk_cli_auth::token::claims).and_then(|c| {
                c.get("fhirUser")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
        });
        LaunchContext {
            patient: text("patient"),
            encounter: text("encounter"),
            fhir_user,
            need_patient_banner: extra.get("need_patient_banner").and_then(Value::as_bool),
        }
    }
}

/// Where sessions are kept: the keychain ([`CredentialStore`]), or memory for
/// a test ([`MemoryStore`]).
pub trait SessionStore {
    fn load(&self, account: &str) -> Result<Option<SmartSession>, CliError>;
    fn save(&self, session: &SmartSession) -> Result<(), CliError>;
    fn delete(&self, account: &str) -> Result<bool, CliError>;
}

/// One keychain item per session (`get_json`/`set_json`).
impl SessionStore for CredentialStore {
    fn load(&self, account: &str) -> Result<Option<SmartSession>, CliError> {
        self.get_json(account)
    }

    fn save(&self, session: &SmartSession) -> Result<(), CliError> {
        self.set_json(&session.account(), session)
    }

    fn delete(&self, account: &str) -> Result<bool, CliError> {
        CredentialStore::delete(self, account)
    }
}

/// Sessions in memory, for tests: nothing touches the keychain.
#[derive(Default)]
pub struct MemoryStore {
    items: RefCell<HashMap<String, String>>,
}

impl SessionStore for MemoryStore {
    fn load(&self, account: &str) -> Result<Option<SmartSession>, CliError> {
        self.items
            .borrow()
            .get(account)
            .map(|text| {
                // serde's message would quote the value it refused: a token.
                serde_json::from_str(text).map_err(|e| {
                    CliError::Other(format!(
                        "a stored session does not read (line {}, column {})",
                        e.line(),
                        e.column()
                    ))
                })
            })
            .transpose()
    }

    fn save(&self, session: &SmartSession) -> Result<(), CliError> {
        let text = serde_json::to_string(session)
            .map_err(|e| CliError::Other(format!("serializing a session: {e}")))?;
        self.items.borrow_mut().insert(session.account(), text);
        Ok(())
    }

    fn delete(&self, account: &str) -> Result<bool, CliError> {
        Ok(self.items.borrow_mut().remove(account).is_some())
    }
}
