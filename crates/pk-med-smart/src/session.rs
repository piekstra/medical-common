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

/// The keychain account of a sign-in: `smart/<organization>/<patient>`, each
/// part with `%` and `/` percent-encoded, so two different pairs never name
/// one item (`a/b` and `c` against `a` and `b/c`). A FHIR id has neither
/// character, so a patient id is kept as it is.
pub fn account(organization: &str, patient: &str) -> String {
    format!(
        "smart/{}/{}",
        account_part(organization),
        account_part(patient)
    )
}

fn account_part(part: &str) -> String {
    part.replace('%', "%25").replace('/', "%2F")
}

/// The [`SmartSession::format`] this release writes, and the newest it reads.
pub const SESSION_FORMAT: u32 = 1;

fn first_format() -> u32 {
    1
}

/// What a SMART sign-in leaves to keep. `Debug` shows no token or key.
#[derive(Clone, Serialize, Deserialize)]
pub struct SmartSession {
    /// The item's shape: [`SESSION_FORMAT`] when written by this release. A
    /// release reading an item with a newer format refuses it rather than
    /// read it in its own shape and write it back.
    #[serde(default = "first_format")]
    pub format: u32,
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
    /// The scope asked for at sign-in. The scope granted is the token set's
    /// (`tokens.scope`), and a refresh asks for that
    /// ([`crate::VendorQuirks::refresh_scope`]).
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

/// Read a stored session. The error names a line and a column, never a
/// value (serde's own message would quote one: a token) and never the
/// account (it holds the patient's id). A newer [`SmartSession::format`]
/// is refused.
pub fn decode_session(text: &str) -> Result<SmartSession, CliError> {
    let session: SmartSession = serde_json::from_str(text).map_err(|e| {
        CliError::Keychain(format!(
            "a stored SMART session does not read (line {}, column {}); clear the stored \
             sign-in and sign in again",
            e.line(),
            e.column()
        ))
    })?;
    if session.format > SESSION_FORMAT {
        return Err(CliError::Keychain(format!(
            "a stored SMART session has format {}, written by a newer release of this CLI \
             (this one reads up to {SESSION_FORMAT}); update it",
            session.format
        )));
    }
    Ok(session)
}

/// The session as stored, stamped with this release's [`SESSION_FORMAT`]
/// whatever format it was read with: what is written is this release's
/// shape, and the stamp must say so.
fn encode_session(session: &SmartSession) -> Result<Secret, CliError> {
    let mut value = serde_json::to_value(session)
        .map_err(|_| CliError::Other("a SMART session did not serialize".into()))?;
    value["format"] = Value::from(SESSION_FORMAT);
    Ok(Secret::new(value.to_string()))
}

/// The keychain's error with the account, which holds the patient's id, put
/// as `smart/<organization>/…`.
fn without_account(e: CliError, account: &str) -> CliError {
    let text = e.to_string();
    if !text.contains(account) {
        return e;
    }
    let shown = match account.rsplit_once('/') {
        Some((head, _)) => format!("{head}/…"),
        None => "a SMART session".to_string(),
    };
    CliError::Keychain(text.replace(account, &shown))
}

/// One keychain item per session, written and read as JSON here rather than
/// through `get_json`, whose errors name the account and quote values.
impl SessionStore for CredentialStore {
    fn load(&self, account: &str) -> Result<Option<SmartSession>, CliError> {
        let Some(text) = self.get(account).map_err(|e| without_account(e, account))? else {
            return Ok(None);
        };
        decode_session(text.expose()).map(Some)
    }

    fn save(&self, session: &SmartSession) -> Result<(), CliError> {
        let account = session.account();
        self.set(&account, &encode_session(session)?)
            .map_err(|e| without_account(e, &account))
    }

    fn delete(&self, account: &str) -> Result<bool, CliError> {
        CredentialStore::delete(self, account).map_err(|e| without_account(e, account))
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
            .map(|text| decode_session(text))
            .transpose()
    }

    fn save(&self, session: &SmartSession) -> Result<(), CliError> {
        let text = encode_session(session)?;
        self.items
            .borrow_mut()
            .insert(session.account(), text.expose().to_string());
        Ok(())
    }

    fn delete(&self, account: &str) -> Result<bool, CliError> {
        Ok(self.items.borrow_mut().remove(account).is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_pairs_never_share_an_account() {
        assert_eq!(account("org-1", "pat-1"), "smart/org-1/pat-1");
        assert_ne!(account("a/b", "c"), account("a", "b/c"));
        assert_eq!(account("a/b", "c"), "smart/a%2Fb/c");
        assert_ne!(account("a%2Fb", "c"), account("a/b", "c"));
    }

    fn stored(format: u32) -> String {
        format!(
            r#"{{"format":{format},"organization":"org-1","patient":"111111111","fhir_base":"https://fhir.example.test","token_endpoint":"https://fhir.example.test/token","client_id":"c","scope":"openid","tokens":{{"access_token":"synthetic-secret-token","token_type":"Bearer","obtained_at":0}}}}"#
        )
    }

    #[test]
    fn a_newer_format_is_refused_and_an_older_item_reads() {
        assert_eq!(decode_session(&stored(1)).unwrap().format, 1);
        let unversioned = stored(1).replace(r#""format":1,"#, "");
        assert_eq!(decode_session(&unversioned).unwrap().format, 1);
        let err = decode_session(&stored(2)).unwrap_err();
        assert!(err.to_string().contains("newer release"), "{err}");
    }

    #[test]
    fn every_write_stamps_this_releases_format() {
        let store = MemoryStore::default();
        let mut session = decode_session(&stored(1)).unwrap();
        session.format = 0;
        store.save(&session).unwrap();
        let text = store
            .items
            .borrow()
            .get(&session.account())
            .unwrap()
            .clone();
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["format"], SESSION_FORMAT);
    }

    #[test]
    fn a_session_that_does_not_read_names_no_value() {
        let broken = stored(1).replace(
            r#""obtained_at":0"#,
            r#""obtained_at":"synthetic-secret-token""#,
        );
        let err = decode_session(&broken).unwrap_err().to_string();
        assert!(!err.contains("synthetic-secret-token"), "{err}");
        assert!(!err.contains("111111111"), "{err}");
    }

    #[test]
    fn a_keychain_error_names_no_patient() {
        let account = account("org-1", "111111111");
        let err = without_account(
            CliError::Keychain(format!(
                "stored item `{account}` under `piekstra.x` is damaged"
            )),
            &account,
        );
        let text = err.to_string();
        assert!(!text.contains("111111111"), "{text}");
        assert!(text.contains("smart/org-1/…"), "{text}");
    }
}
