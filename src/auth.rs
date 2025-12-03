use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub login_url: String,
    pub username: String,
    pub password: String,
}

use std::collections::HashMap;
use reqwest::{Client, Url};
use std::sync::Arc;
use reqwest::cookie::{CookieStore, Jar};

pub async fn login(config: &AuthConfig) -> Result<String, Box<dyn std::error::Error>> {
    let jar = Arc::new(Jar::default());
    let client = Client::builder()
        .cookie_provider(jar.clone())
        .build()?;

    let mut params = HashMap::new();
    params.insert("action", "login");
    params.insert("taskID", "0");
    params.insert("username", &config.username);
    params.insert("password", &config.password);

    let _ = client.post(&config.login_url)
        .form(&params)
        .send()
        .await?;

    let url = config.login_url.parse::<Url>()?;
    let cookie_header = jar.cookies(&url);

    if let Some(header_value) = cookie_header {
        if let Some(cookie) = header_value.to_str().ok().and_then(|c| {
            c.split(';').find_map(|s| {
                let mut parts = s.splitn(2, '=');
                if let (Some(name), Some(value)) = (parts.next(), parts.next()) {
                    if name.trim() == "LoginCookie" {
                        return Some(value.trim().to_string());
                    }
                }
                None
            })
        }) {
            return Ok(cookie);
        }
    }

    // Fallback: Check if the jar has the cookie directly (Cookie store implementation detail)
    // reqwest::cookie::Jar::cookies returns a HeaderValue which is a string "name=value; name2=value2"
    // So the parsing above is correct.

    Err("LoginCookie not found in response".into())
}
