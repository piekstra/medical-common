//! SMART App Launch for a standalone patient app, on cli-common's
//! `pk_cli_auth::oauth`. No SMART library of anyone else's.
//!
//! 1. [`discover`] the server's endpoints.
//! 2. [`begin`] a sign-in with a [`ScopeSet`]: PKCE S256, `state`, `aud`, the
//!    loopback redirect. Print and open [`SmartSignIn::url`], then
//!    [`SmartSignIn::wait_for_code`] and [`SmartSignIn::exchange`] it.
//! 3. Store the [`SmartSession`] (one keychain item per organization and
//!    patient, [`account`]) under `RefreshLock::store_signin`.
//! 4. Before each use, [`Renewer::fresh`] renews a lapsed token under the
//!    refresh lock: a refresh token, or a registered client's signed
//!    JWT-bearer grant. [`register_dynamic_client`] registers the client and
//!    returns the session to store; [`Renewer::renew`] makes its first grant
//!    ([`JwtSigner`]; feature `rsa` adds an RS384 key, `RsaKey`).
//!
//! What a vendor does differently goes in a [`VendorQuirks`]. Tokens never
//! appear in `Debug` output or messages.

pub mod assertion;
pub mod discovery;
pub mod launch;
pub mod quirks;
pub mod register;
pub mod renew;
#[cfg(feature = "rsa")]
pub mod rsa_key;
pub mod scopes;
pub mod session;

pub use assertion::{
    jwt_bearer_grant, signed_assertion, JwtSigner, DEFAULT_LIFETIME_SECS, JWT_BEARER,
    MAX_LIFETIME_SECS,
};
pub use discovery::{discover, SmartConfiguration};
pub use launch::{begin, LaunchRequest, SmartApp, SmartSignIn};
pub use quirks::{NoQuirks, VendorQuirks};
pub use register::{jwt_bearer_tokens, register_dynamic_client};
pub use renew::{Renewer, SignerFor};
#[cfg(feature = "rsa")]
pub use rsa_key::RsaKey;
pub use scopes::{ScopeSet, ScopeVersion};
pub use session::{
    account, decode_session, DynamicClient, LaunchContext, MemoryStore, SessionStore, SmartSession,
    SESSION_FORMAT,
};
