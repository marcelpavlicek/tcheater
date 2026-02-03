use libxml::parser::Parser;
use libxml::xpath::Context;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub login_url: String,
    pub username: String,
    pub password: String,
}

pub struct PbsTask {
    pub id: i32,
    pub name: String,
    pub time_spent: Option<String>,
    pub time_total: Option<String>,
}

use reqwest::{redirect::Policy, Client};
use std::collections::HashMap;

async fn login(config: &AuthConfig) -> Result<Client, Box<dyn std::error::Error>> {
    let client = Client::builder()
        .redirect(Policy::none())
        .cookie_store(true)
        .build()?;

    let mut params = HashMap::new();
    params.insert("action", "login");
    params.insert("taskID", "0");
    params.insert("username", &config.username);
    params.insert("password", &config.password);

    let response = client.post(&config.login_url).form(&params).send().await?;

    for cookie in response.cookies() {
        if cookie.name() == "LoginCookie" {
            return Ok(client);
        }
    }

    Err("LoginCookie not found in response".into())
}

pub async fn fetch_tasks(config: &AuthConfig) -> Result<Vec<PbsTask>, Box<dyn std::error::Error>> {
    let client = login(config).await?;

    let res = client
        .get("https://pbs2.praguebest.cz/main.php?pageid=110&action=list&perpage=100")
        .send()
        .await?;

    let html = res.text().await?;

    let parser = Parser::default_html();
    let doc = parser.parse_string(html)?;
    if let Ok(context) = Context::new(&doc) {
        let result = context
            .evaluate("//div[@class=\"TaskList\"]/table/tbody/tr")
            .unwrap();
        let task_list = result.get_nodes_as_vec();
        let mut parsed_tasks: Vec<PbsTask> = task_list
            .iter()
            .map(|row| {
                let children = row.get_child_elements();
                let mut time_spent = None;
                let mut time_total = None;

                if let Ok(spans) = row.findnodes("//span[contains(@class, 'hour')]") {
                    if let Some(span) = spans.first() {
                        let content = span.get_content().replace('\u{a0}', "");
                        let parts: Vec<&str> = content.split('/').collect();
                        if parts.len() == 2 {
                            time_spent = Some(parts[0].trim().to_string());
                            time_total = Some(parts[1].trim().to_string());
                        } else if !parts.is_empty() {
                            time_spent = Some(parts[0].trim().to_string());
                        }
                    }
                }

                PbsTask {
                    id: row.get_attribute("data-id").unwrap().parse().unwrap(),
                    name: children.get(5).unwrap().get_content(),
                    time_spent,
                    time_total,
                }
            })
            .collect();
        parsed_tasks.sort_by(|a, b| b.id.cmp(&a.id));
        return Ok(parsed_tasks);
    }
    Ok(vec![])
}

pub fn rescale(val: f64, old_min: f64, old_max: f64, new_min: f64, new_max: f64) -> f64 {
    if old_max == old_min {
        return new_min;
    }
    ((val - old_min) / (old_max - old_min)) * (new_max - new_min) + new_min
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rescale() {
        assert_eq!(rescale(5.0, 0.0, 10.0, 0.0, 100.0), 50.0);
        assert_eq!(rescale(0.0, 0.0, 10.0, 0.0, 100.0), 0.0);
        assert_eq!(rescale(10.0, 0.0, 10.0, 0.0, 100.0), 100.0);
        assert_eq!(rescale(2.5, 0.0, 5.0, 0.0, 10.0), 5.0);
    }

    #[test]
    fn test_rescale_inverse() {
        assert_eq!(rescale(5.0, 0.0, 10.0, 100.0, 0.0), 50.0);
    }

    #[test]
    fn test_rescale_zero_range() {
        assert_eq!(rescale(5.0, 10.0, 10.0, 0.0, 100.0), 0.0);
    }
}
