use std::env;
use std::process::exit;

pub use app::App;
use chrono::{Datelike, Local};
use directories::UserDirs;
use time::get_mondays_in_month;

pub mod app;
pub mod config;
pub mod firestore;
pub mod pbs;
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

    let config =
        config::Config::from_toml_file(home_dir.join("config.toml")).unwrap_or_else(|_| {
            eprintln!("Failed to load config.toml");
            exit(1);
        });

    // Get month and year from command line arguments or use current
    let now = Local::now();
    let month = env::args()
        .nth(1)
        .and_then(|arg| arg.parse::<u32>().ok())
        .filter(|&m| (1..=12).contains(&m))
        .unwrap_or_else(|| now.month());

    let year = env::args()
        .nth(2)
        .and_then(|arg| arg.parse::<i32>().ok())
        .unwrap_or_else(|| now.year());

    let mondays = get_mondays_in_month(year, month);

    color_eyre::install().unwrap();
    let terminal = ratatui::init();
    if let Err(err) = App::new(db, mondays, config.auth, config.task_url_prefix)
        .run(terminal)
        .await
    {
        eprintln!("{}", err);
    }
    ratatui::restore();
}
