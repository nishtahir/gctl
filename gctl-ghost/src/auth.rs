use crate::error::GhostError;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::debug;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Claims {
    exp: usize,
    iat: usize,
    aud: String,
}

pub fn generate_token(id: &str, secret: &str) -> Result<String, GhostError> {
    let mut header = Header::new(Algorithm::HS256);
    header.typ = Some("JWT".to_string());
    header.kid = Some(id.to_string());

    let now = SystemTime::now();
    let timestamp = now.duration_since(UNIX_EPOCH).expect("Time went backwards");

    let claims = Claims {
        iat: timestamp.as_secs() as usize,
        exp: timestamp.as_secs() as usize + 300,
        aud: "/admin/".to_string(),
    };

    let bytes = hex::decode(secret)
        .map_err(|e| GhostError::ParseError(e.to_string()))?;
    let key = EncodingKey::from_secret(&bytes);
    debug!("Generated JWT token");
    Ok(encode(&header, &claims, &key)?)
} 