//! JWTs a client signs for itself (RFC 7523): the JWT-bearer grant a
//! dynamically registered client gets its tokens with, and a
//! `private_key_jwt` client assertion. The signing is the CLI's
//! ([`JwtSigner`]): this crate holds no key type and no crypto library.

use base64::Engine as _;
use pk_cli_auth::oauth::{random_token, TokenRequest};
use pk_cli_core::CliError;
use pk_cli_secrets::Secret;
use serde_json::{json, Value};

/// The JWT-bearer grant type (RFC 7523 §2.1).
pub const JWT_BEARER: &str = "urn:ietf:params:oauth:grant-type:jwt-bearer";
/// The longest lifetime an assertion gets: 5 minutes, Epic's limit.
pub const MAX_LIFETIME_SECS: u64 = 300;
/// The lifetime [`crate::jwt_bearer_tokens`] gives: 4 minutes, a minute
/// inside Epic's limit for a clock running ahead of the server's.
pub const DEFAULT_LIFETIME_SECS: u64 = 240;

/// Signs a JWT's signing input with the client's private key.
pub trait JwtSigner {
    /// The JWS `alg`: `RS384`, `ES384`.
    fn alg(&self) -> &str;
    /// The key id, when the registered JWK Set names one.
    fn kid(&self) -> Option<String> {
        None
    }
    /// The signature over `signing_input` (`<header>.<claims>`, base64url),
    /// raw bytes.
    fn sign(&self, signing_input: &[u8]) -> Result<Vec<u8>, CliError>;
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// A signed JWT with `iss` = `sub` = `client_id`, `aud` = `audience` (the
/// token endpoint), a random `jti`, `iat` = `now` and `exp` = `now` +
/// `lifetime_secs` (at most [`MAX_LIFETIME_SECS`]).
pub fn signed_assertion(
    signer: &dyn JwtSigner,
    client_id: &str,
    audience: &str,
    now: u64,
    lifetime_secs: u64,
) -> Result<Secret, CliError> {
    let mut header = json!({ "alg": signer.alg(), "typ": "JWT" });
    if let Some(kid) = signer.kid() {
        header["kid"] = Value::from(kid);
    }
    let claims = json!({
        "iss": client_id,
        "sub": client_id,
        "aud": audience,
        "jti": random_token(16)?,
        "iat": now,
        "exp": now + lifetime_secs.min(MAX_LIFETIME_SECS),
    });
    let input = format!(
        "{}.{}",
        b64(header.to_string().as_bytes()),
        b64(claims.to_string().as_bytes())
    );
    let signature = signer.sign(input.as_bytes())?;
    Ok(Secret::new(format!("{input}.{}", b64(&signature))))
}

/// The JWT-bearer grant for `assertion` (RFC 7523 §2.1).
pub fn jwt_bearer_grant(assertion: &Secret) -> TokenRequest {
    TokenRequest::grant(JWT_BEARER).param("assertion", assertion.expose())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Signs nothing: the signature is the input's length, for shape tests.
    struct Fake;
    impl JwtSigner for Fake {
        fn alg(&self) -> &str {
            "RS384"
        }
        fn kid(&self) -> Option<String> {
            Some("k1".into())
        }
        fn sign(&self, input: &[u8]) -> Result<Vec<u8>, CliError> {
            Ok(input.len().to_string().into_bytes())
        }
    }

    #[test]
    fn an_assertion_names_the_client_and_expires_within_five_minutes() {
        let jwt = signed_assertion(
            &Fake,
            "client-1",
            "https://auth.example.test/token",
            1000,
            3600,
        )
        .unwrap();
        let parts: Vec<&str> = jwt.expose().split('.').collect();
        assert_eq!(parts.len(), 3);
        let claims = pk_cli_auth::token::claims(jwt.expose()).unwrap();
        assert_eq!(claims["iss"], "client-1");
        assert_eq!(claims["sub"], "client-1");
        assert_eq!(claims["aud"], "https://auth.example.test/token");
        assert_eq!(claims["exp"], 1300);
        assert!(claims["jti"].as_str().unwrap().len() >= 16);
        let header: Value = serde_json::from_slice(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(parts[0])
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            (header["alg"].as_str(), header["kid"].as_str()),
            (Some("RS384"), Some("k1"))
        );
        let again = signed_assertion(&Fake, "client-1", "aud", 1000, 60).unwrap();
        assert_ne!(
            pk_cli_auth::token::claims(again.expose()).unwrap()["jti"],
            claims["jti"],
            "every assertion has its own jti"
        );
    }
}
