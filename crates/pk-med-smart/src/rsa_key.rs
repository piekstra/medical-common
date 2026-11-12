//! An RS384 key for a client that signs its own grants (feature `rsa`): what
//! Epic's patient apps register. Generated on this machine, kept in the
//! keychain inside the session ([`crate::DynamicClient`]), never sent: the
//! server gets the public half as a JWK Set.

use base64::Engine as _;
use pk_cli_core::CliError;
use pk_cli_secrets::Secret;
use rsa::pkcs8::{DecodePrivateKey, EncodePrivateKey};
use rsa::sha2::Sha384;
use rsa::traits::PublicKeyParts;
use rsa::PrivateKey;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use zeroize::Zeroizing;

use crate::assertion::JwtSigner;
use crate::session::DynamicClient;

/// The JWS algorithm: RSASSA-PKCS1-v1_5 with SHA-384.
pub const RS384: &str = "RS384";

/// An RSA private key that signs RS384. `Debug` shows only its key id.
pub struct RsaKey {
    key: PrivateKey,
    kid: String,
}

impl RsaKey {
    /// The smallest modulus accepted.
    pub const MIN_BITS: usize = 2048;

    /// A new key with a `bits`-bit modulus from the system's random source.
    /// Use the size the server documents (Epic: 2048); at least
    /// [`Self::MIN_BITS`].
    pub fn generate(bits: usize) -> Result<Self, CliError> {
        if bits < Self::MIN_BITS {
            return Err(CliError::Usage(format!(
                "an RSA key needs at least {} bits",
                Self::MIN_BITS
            )));
        }
        let key = PrivateKey::new(&mut getrandom::SysRng, bits)
            .map_err(|e| CliError::Other(format!("generating an RSA key: {e}")))?;
        Ok(Self::from_key(key))
    }

    /// The key as [`Self::to_pkcs8`] wrote it. A key under
    /// [`Self::MIN_BITS`] is refused, as [`Self::generate`] refuses one.
    pub fn from_pkcs8(encoded: &Secret) -> Result<Self, CliError> {
        let der = Zeroizing::new(
            base64::engine::general_purpose::STANDARD
                .decode(encoded.expose().trim())
                .map_err(|_| CliError::Other("the stored RSA key is not base64".into()))?,
        );
        let key = PrivateKey::from_pkcs8_der(&der)
            .map_err(|_| CliError::Other("the stored RSA key is not a PKCS#8 RSA key".into()))?;
        if modulus_bits(&key) < Self::MIN_BITS {
            return Err(CliError::Other(format!(
                "the stored RSA key is under {} bits",
                Self::MIN_BITS
            )));
        }
        Ok(Self::from_key(key))
    }

    /// The key a registered client keeps ([`DynamicClient::key`]): the
    /// signer a [`crate::Renewer`] makes for it.
    pub fn for_client(client: &DynamicClient) -> Result<Self, CliError> {
        Self::from_pkcs8(&client.key())
    }

    /// PKCS#8 DER in base64, the form [`crate::DynamicClient`] keeps.
    pub fn to_pkcs8(&self) -> Result<Secret, CliError> {
        let der = self
            .key
            .to_pkcs8_der()
            .map_err(|e| CliError::Other(format!("encoding the RSA key: {e}")))?;
        Ok(Secret::new(
            base64::engine::general_purpose::STANDARD.encode(der.as_bytes()),
        ))
    }

    /// The RFC 7638 thumbprint of the public key: the `kid`.
    pub fn kid(&self) -> &str {
        &self.kid
    }

    /// The public key as a JWK Set (`kty`, `n`, `e`, `alg`, `use`, `kid`), for
    /// the registration.
    pub fn jwks(&self) -> Value {
        let (n, e) = public_parts(&self.key);
        json!({ "keys": [{
            "kty": "RSA",
            "n": n,
            "e": e,
            "alg": RS384,
            "use": "sig",
            "kid": self.kid,
        }]})
    }

    fn from_key(key: PrivateKey) -> Self {
        let (n, e) = public_parts(&key);
        // RFC 7638 §3.2: the required members, in lexicographic order, no whitespace.
        let canonical = format!(r#"{{"e":"{e}","kty":"RSA","n":"{n}"}}"#);
        let kid = b64url(&Sha256::digest(canonical.as_bytes()));
        RsaKey { key, kid }
    }
}

impl JwtSigner for RsaKey {
    fn alg(&self) -> &str {
        RS384
    }

    fn kid(&self) -> Option<String> {
        Some(self.kid.clone())
    }

    fn sign(&self, signing_input: &[u8]) -> Result<Vec<u8>, CliError> {
        self.key
            .pkcs1v15_sign::<Sha384>(signing_input)
            .map_err(|e| CliError::Other(format!("signing with the RSA key: {e}")))
    }
}

impl std::fmt::Debug for RsaKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RsaKey")
            .field("kid", &self.kid)
            .finish_non_exhaustive()
    }
}

/// The modulus's size in bits, from its big-endian bytes.
fn modulus_bits(key: &PrivateKey) -> usize {
    let n = key.n_bytes();
    let first = n.iter().position(|&b| b != 0).unwrap_or(n.len());
    match n.get(first) {
        Some(top) => (n.len() - first) * 8 - top.leading_zeros() as usize,
        None => 0,
    }
}

fn public_parts(key: &PrivateKey) -> (String, String) {
    (b64url(&key.n_bytes()), b64url(&key.e_bytes()))
}

fn b64url(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_signs_rs384_and_survives_the_keychain_form() {
        let key = RsaKey::generate(2048).unwrap();
        let jwks = key.jwks();
        let jwk = &jwks["keys"][0];
        assert_eq!(
            (jwk["kty"].as_str(), jwk["alg"].as_str()),
            (Some("RSA"), Some("RS384"))
        );
        assert_eq!(jwk["e"], "AQAB");
        assert_eq!(jwk["kid"], key.kid());
        assert!(jwk.get("d").is_none(), "the private exponent stays here");
        assert!(!format!("{key:?}").contains(jwk["n"].as_str().unwrap()));

        let stored = key.to_pkcs8().unwrap();
        let again = RsaKey::from_pkcs8(&stored).unwrap();
        assert_eq!(again.kid(), key.kid());

        let signature = key.sign(b"header.claims").unwrap();
        assert_eq!(signature.len(), 256);
        key.key
            .to_public_key()
            .pkcs1v15_verify::<Sha384>(b"header.claims", &signature)
            .unwrap();
        assert_eq!(
            again.sign(b"header.claims").unwrap(),
            signature,
            "PKCS#1 v1.5 is deterministic"
        );

        assert_eq!(RsaKey::generate(1024).unwrap_err().exit_code(), 2);
        assert!(RsaKey::from_pkcs8(&Secret::new("not base64!")).is_err());
        assert_eq!(modulus_bits(&key.key), 2048);

        let client = DynamicClient::new("c", RS384, Some(key.kid().to_string()), stored);
        assert_eq!(RsaKey::for_client(&client).unwrap().kid(), key.kid());
    }

    #[test]
    fn a_stored_key_under_2048_bits_is_refused() {
        let small = PrivateKey::new(&mut getrandom::SysRng, 1024).unwrap();
        let der = small.to_pkcs8_der().unwrap();
        let stored = Secret::new(base64::engine::general_purpose::STANDARD.encode(der.as_bytes()));
        let err = RsaKey::from_pkcs8(&stored).unwrap_err();
        assert!(err.to_string().contains("2048"), "{err}");
    }
}
