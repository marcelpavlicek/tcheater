use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub color: u8,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProjectsConfig {
    projects: Vec<Project>,
}

impl Project {
    pub fn from_toml_file<P: AsRef<Path>>(
        path: P,
    ) -> Result<Vec<Self>, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let config: ProjectsConfig = toml::from_str(&content)?;
        Ok(config.projects)
    }
}

pub fn find_by_id<'a>(projects: &'a [Project], id: &str) -> Option<&'a Project> {
    projects.iter().find(|project| project.id == id)
}
