use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub login_url: String,
    pub username: String,
    pub password: String,
}

use reqwest::Client;
use std::collections::HashMap;

pub async fn login(config: &AuthConfig) -> Result<String, Box<dyn std::error::Error>> {
    let client = Client::builder().cookie_store(true).build()?;

    let mut params = HashMap::new();
    params.insert("action", "login");
    params.insert("taskID", "0");
    params.insert("username", &config.username);
    params.insert("password", &config.password);

    let response = client.post(&config.login_url).form(&params).send().await?;

    for cookie in response.cookies() {
        if cookie.name() == "LoginCookie" {
            return Ok(cookie.value().to_string());
        }
    }

    Err("LoginCookie not found in response".into())
}
