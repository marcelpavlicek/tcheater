use std::env;
use std::process::exit;

pub use app::App;
use chrono::{Datelike, Local};
use directories::UserDirs;
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

    let home_dir = match UserDirs::new() {
        Some(user_dirs) => user_dirs.home_dir().to_path_buf(),
        None => exit(1),
    };

    let projects = projects::Project::from_toml_file(home_dir.join("projects.toml")).unwrap();

    // Get month from command line argument or use current month
    let month = env::args()
        .nth(1)
        .and_then(|arg| arg.parse::<u32>().ok())
        .filter(|&m| (1..=12).contains(&m))
        .unwrap_or_else(|| Local::now().month());

    let mondays = get_mondays_in_month(month);

    color_eyre::install().unwrap();
    let terminal = ratatui::init();
    if let Err(err) = App::new(db, projects, mondays).run(terminal).await {
        eprintln!("{}", err);
    }
    ratatui::restore();
}
