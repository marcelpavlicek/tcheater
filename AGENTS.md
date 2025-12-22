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
