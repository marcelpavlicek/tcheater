# Agent Guidelines

## Commands
- Build: `cargo build`
- Run: `cargo run`
- Test: `cargo test`
- Single test: `cargo test <module_name>::tests::<test_name>`
- Lint: `cargo clippy`
- Format: `cargo fmt`

## Code Style
- **Rust Edition:** 2021
- **Imports:** Standard Rust grouping: `std`, then external crates, then `crate::`.
- **Formatting:** Use default `rustfmt` via `cargo fmt`.
- **Naming:** 
  - `snake_case` for functions, variables, and modules.
  - `PascalCase` for structs, enums, and traits.
  - `SCREAMING_SNAKE_CASE` for constants.
- **Error Handling:** 
  - Prefer `color_eyre::Result` for application-level results.
  - Use `match` or `if let` for local error handling.
  - Avoid `unwrap()` in production code unless in tests or where panic is intended.
- **TUI:** Uses `ratatui`. State management is primarily in the `App` struct.
- **Tests:** Place unit tests in a `mod tests` block at the end of the file with `#[cfg(test)]`.

## Architecture
**tcheater** is a TUI time-tracking application built with Rust and `ratatui`. It interfaces with Google Firestore for data persistence and an external system (PBS) for task retrieval.

### Core Components
- **App (`src/app.rs`):** The central state container and event loop. It manages the application lifecycle, input handling, and coordinates data between the UI and backend services.
- **UI Rendering:** The UI is drawn in the `App::draw` method, utilizing a layout-based approach with `ratatui` widgets. Custom widgets like `Timeline` (`src/timeline_widget.rs`) are used for specific visualizations.
- **Data Persistence (`src/firestore.rs`):** Handles all interactions with Firestore, including CRUD operations for `Checkpoint` data.
- **External Integration (`src/pbs.rs`):** Manages authentication and data fetching from the PBS system.

## Key Modules
- `src/main.rs`: Entry point. Sets up connections (Firestore), loads config, and initializes the `App`.
- `src/app.rs`: Main application logic, input handling (Crossterm), and UI layout definition.
- `src/timeline_widget.rs`: Custom widget for displaying time entries (checkpoints) in a vertical timeline.
- `src/firestore.rs`: Firestore client wrapper and database operations.
- `src/projects.rs`: Manages project definitions loaded from `projects.toml`.
- `src/config.rs`: Handles application configuration from `config.toml`.
- `src/time.rs`: Time manipulation utilities (rounding, week calculations).

## Configuration
The application relies on two TOML configuration files located in the user's home directory:
- `config.toml`: General application settings and authentication details.
- `projects.toml`: Definitions of projects and their associated tasks/metadata.