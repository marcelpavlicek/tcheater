use std::process::exit;

pub use app::App;
use chrono::{Datelike, Local};
use time::get_mondays_in_month;

pub mod app;
pub mod firestore;
pub mod projects;
pub mod time;
pub mod timeline_widget;
pub mod widgets;

#[tokio::main]
async fn main() {
    let db = match firestore::connect().await {
        Ok(db) => db,
        Err(err) => {
            eprint!("{}", err);
            exit(1)
        }
    };

    let projects = projects::Project::from_toml_file("./projects.toml").unwrap();
    let mondays = get_mondays_in_month(Local::now().month());

    color_eyre::install().unwrap();
    let terminal = ratatui::init();
    if let Err(err) = App::new(db, projects, mondays).run(terminal).await {
        eprintln!("{}", err);
    }
    ratatui::restore();
}
