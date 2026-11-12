//! A client registered for one sign-in, as Epic's patient apps do: the code
//! exchange's access token is the one-time initial access token, the
//! registration carries the client's public key, and every later access token
//! comes from the JWT-bearer grant the client signs. No refresh token.

use pk_cli_auth::oauth::{ClientAuth, ClientMetadata, OAuthClient, TokenSet};
use pk_cli_core::transport::Transport;
use pk_cli_core::CliError;
use serde_json::Value;

use crate::assertion::{jwt_bearer_grant, signed_assertion, JwtSigner, MAX_LIFETIME_SECS};
use crate::discovery::SmartConfiguration;
use crate::session::{DynamicClient, SmartSession};

/// Register a client for `session` with the public JWK Set `jwks` and
/// `software_id` (Epic: the app's own client id), then get its first access
/// token with the JWT-bearer grant. `key` is the private key as the signer
/// reads it. The session comes back holding the client and the new tokens;
/// store it.
#[allow(clippy::too_many_arguments)]
pub async fn register_dynamic_client<T: Transport>(
    oauth: &OAuthClient<T>,
    config: &SmartConfiguration,
    mut session: SmartSession,
    software_id: &str,
    jwks: Value,
    signer: &dyn JwtSigner,
    key: pk_cli_secrets::Secret,
    now: u64,
) -> Result<SmartSession, CliError> {
    let endpoint = config
        .registration_endpoint
        .as_deref()
        .ok_or_else(|| CliError::Upstream("the server offers no registration endpoint".into()))?;
    let mut metadata = ClientMetadata::native_public(&session.client_id, Vec::new()).jwks(jwks);
    metadata.software_id = Some(software_id.to_string());
    metadata.token_endpoint_auth_method = "private_key_jwt".into();
    metadata.grant_types = vec![crate::assertion::JWT_BEARER.into()];
    metadata.response_types = Vec::new();
    metadata.client_name = None;
    let registered = oauth
        .register(endpoint, &metadata, Some(&session.bearer()))
        .await?;
    let client = DynamicClient::new(
        registered.client_id.clone(),
        signer.alg(),
        signer.kid(),
        key,
    );
    session.tokens = jwt_bearer_tokens(oauth, &session, &client, signer, now).await?;
    session.dynamic_client = Some(client);
    Ok(session)
}

/// A fresh token set for a registered client: a signed assertion, the
/// JWT-bearer grant, and the session's other members carried over.
pub async fn jwt_bearer_tokens<T: Transport>(
    oauth: &OAuthClient<T>,
    session: &SmartSession,
    client: &DynamicClient,
    signer: &dyn JwtSigner,
    now: u64,
) -> Result<TokenSet, CliError> {
    let assertion = signed_assertion(
        signer,
        &client.client_id,
        &session.token_endpoint,
        now,
        MAX_LIFETIME_SECS,
    )?;
    let mut grant = jwt_bearer_grant(&assertion);
    if !session.scope.is_empty() {
        grant = grant.scope(&session.scope);
    }
    let response = oauth
        .token(
            &session.token_endpoint,
            &ClientAuth::public(&client.client_id),
            &grant,
        )
        .await?;
    Ok(TokenSet::from_response(
        response,
        now,
        Some(&session.tokens),
    ))
}
