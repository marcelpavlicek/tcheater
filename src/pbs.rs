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
            .map(|row| PbsTask {
                id: row.get_attribute("data-id").unwrap().parse().unwrap(),
                name: row.get_child_elements().get(5).unwrap().get_content(),
            })
            .collect();
        parsed_tasks.sort_by(|a, b| b.id.cmp(&a.id));
        return Ok(parsed_tasks);
    }
    Ok(vec![])
}
