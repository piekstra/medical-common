//! What a vendor does differently, behind one small trait. A CLI implements it
//! for its vendor; [`NoQuirks`] is SMART as written.

use crate::scopes::ScopeSet;
use crate::session::SmartSession;

/// A vendor's departures from SMART App Launch. Every method has the
/// standard behaviour as its default.
pub trait VendorQuirks {
    /// Headers on the discovery requests (Epic's `Epic-Client-ID`).
    fn discovery_headers(&self) -> Vec<(String, String)> {
        Vec::new()
    }

    /// Ask `/metadata` before `.well-known/smart-configuration`.
    fn metadata_first(&self) -> bool {
        false
    }

    /// Extra authorization parameters (a practice or tenant selector).
    fn authorize_params(&self) -> Vec<(String, String)> {
        Vec::new()
    }

    /// The scopes actually asked for, given the ones requested: a vendor that
    /// refuses wildcards, or wants its own spelling.
    fn scopes(&self, requested: ScopeSet) -> ScopeSet {
        requested
    }

    /// The scope sent with a refresh, and with a registered client's
    /// JWT-bearer grant; `None` sends none. SMART leaves it optional, and a
    /// refresh may ask only for what was granted (RFC 6749 §6), which can be
    /// less than was asked for when the person unticked a category at
    /// consent. The default sends the granted scope (the token set's), else
    /// the scope asked for at sign-in when the server named none.
    fn refresh_scope(&self, session: &SmartSession) -> Option<String> {
        session
            .tokens
            .scope
            .clone()
            .filter(|s| !s.is_empty())
            .or_else(|| Some(session.scope.clone()).filter(|s| !s.is_empty()))
    }

    /// Whether `aud` goes on the authorization request. SMART requires it; a
    /// server that refuses it can turn it off.
    fn send_aud(&self) -> bool {
        true
    }
}

/// SMART as the specification writes it.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoQuirks;

impl VendorQuirks for NoQuirks {}
