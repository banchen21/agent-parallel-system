use chrono::{Duration, Local};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

use crate::core::config::CONFIG;

pub const CONSOLE_SECRET_HEADER: &str = "X-Console-Secret";

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,        // username
    pub exp: usize,         // expiration time
    pub iat: usize,         // issued at
    pub token_type: String, // "access" 或 "refresh"
}

fn secret_bytes() -> &'static [u8] {
    CONFIG.security.super_secret_key.as_bytes()
}

pub fn validate_console_secret(secret: &str) -> bool {
    !secret.is_empty() && secret == CONFIG.security.super_secret_key
}

pub fn generate_tokens(username: &str) -> (String, String) {
    let now = Local::now();

    // Access Token
    let access_claims = Claims {
        sub: username.to_string(),
        exp: (now + Duration::minutes(15)).timestamp() as usize,
        iat: now.timestamp() as usize,
        token_type: "access".to_string(),
    };

    // Refresh Token
    let refresh_claims = Claims {
        sub: username.to_string(),
        exp: (now + Duration::days(7)).timestamp() as usize,
        iat: now.timestamp() as usize,
        token_type: "refresh".to_string(),
    };
    let access_token = encode(
        &Header::default(),
        &access_claims,
        &EncodingKey::from_secret(secret_bytes()),
    )
    .unwrap();

    let refresh_token = encode(
        &Header::default(),
        &refresh_claims,
        &EncodingKey::from_secret(secret_bytes()),
    )
    .unwrap();

    (access_token, refresh_token)
}

pub fn validate_token(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
}
