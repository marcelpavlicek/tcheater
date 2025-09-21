use std::fmt::Display;
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

use chrono::{DateTime, Duration, Local, NaiveDate, TimeDelta, TimeZone, Timelike};
use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use firestore::{FirestoreDb, FirestoreResult};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget},
    DefaultTerminal, Frame,
};
use serde::{Deserialize, Serialize};

use crate::{
    firestore::{
        delete_checkpoint, get_distinct_dates, insert_checkpoint, load_checkpoints,
        update_checkpoint,
    },
    projects::{find_by_id, Project},
    timeline_widget::Timeline,
    widgets::HelpLine,
};

const UNIT: u32 = 15;

#[derive(Default)]
pub struct TimeSpan {
    units: u16,
}

impl Display for TimeSpan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.units)
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Checkpoint {
    #[serde(alias = "_firestore_id")]
    pub id: Option<String>,
    pub time: DateTime<Local>,
    pub project: Option<String>,
    pub message: Option<String>,
    pub registered: bool,
}

impl Checkpoint {
    pub fn new() -> Self {
        Self {
            id: None,
            time: Local::now(),
            project: None,
            message: None,
            registered: false,
        }
    }

    pub fn rounded_time(&self) -> DateTime<Local> {
        round_to_nearest_fifteen_minutes(self.time)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum InputMode {
    #[default]
    Normal,
    Editing,
}

pub struct App {
    /// Is the application running?
    running: bool,
    input: Input,
    input_mode: InputMode,
    db: FirestoreDb,
    projects: Vec<Project>,
    checkpoints: Vec<Checkpoint>,
    selected_checkpoint: Option<usize>,
    dates: Vec<NaiveDate>,
    selected_date: Option<usize>,
}

pub fn round_to_nearest_fifteen_minutes<Tz: TimeZone>(dt: DateTime<Tz>) -> DateTime<Tz> {
    let minute = dt.minute();
    let remainder = minute % 15;

    let rounded_dt = if remainder >= 8 {
        // Round up
        let minutes_to_add = 15 - remainder;
        dt + Duration::minutes(minutes_to_add as i64)
    } else {
        // Round down
        let minutes_to_subtract = remainder;
        dt - Duration::minutes(minutes_to_subtract as i64)
    };

    // Zero out seconds and microseconds
    rounded_dt
        .with_second(0)
        .unwrap()
        .with_nanosecond(0)
        .unwrap()
    /*
        // Get time components
        let minute = dt.minute();
        let second = dt.second();

        // Calculate total seconds and nanos into the current hour
        let total_secs = minute * 60 + second;

        // Duration of 15 minutes in seconds
        let fifteen_mins_secs = UNIT * 60;

        // Calculate the nearest 15-minute mark
        let rounded_secs =
            ((total_secs as f64 / fifteen_mins_secs as f64).round() * fifteen_mins_secs as f64) as i64;

        // Create a duration from the start of the hour
        let duration_from_hour_start = Duration::seconds(rounded_secs);

        // Start of the current hour
        let hour_start = dt.with_minute(0).unwrap().with_second(0).unwrap();

        // Add the rounded duration to the start of the hour
        hour_start + duration_from_hour_start
    */
}

/// Calculates the number of 15-minute intervals between two DateTime objects.
///
/// This function assumes that both DateTime objects are already rounded to 15-minute intervals.
/// If they are not, the result may not be accurate.
///
/// # Arguments
///
/// * `start` - The starting DateTime, assumed to be rounded to 15 minutes
/// * `end` - The ending DateTime, assumed to be rounded to 15 minutes
///
/// # Returns
///
/// The number of 15-minute intervals between the two DateTimes.
/// Returns a positive number if `end` is after `start`, or a negative number if `end` is before `start`.
pub fn count_fifteen_minute_intervals<Tz: TimeZone>(start: DateTime<Tz>, end: DateTime<Tz>) -> i64 {
    // Calculate the duration between the two DateTimes
    let duration = end.signed_duration_since(start);

    // Convert the duration to minutes
    let minutes = duration.num_minutes();

    // Divide by 15 to get the number of 15-minute intervals
    minutes / UNIT as i64
}

/// Converts minutes to human readable string
///
/// # Arguments
///
/// * `minutes` - The number of minutes to convert
///
/// # Returns
///
/// A human-readable string representation of the duration (e.g., "2h 30m", "45m", "1h")
pub fn human_duration(minutes: u32) -> String {
    if minutes == 0 {
        return "0m".to_string();
    }

    let hours = minutes / 60;
    let remaining_minutes = minutes % 60;

    match (hours, remaining_minutes) {
        (0, m) => format!("{}m", m),
        (h, 0) => format!("{}h", h),
        (h, m) => format!("{}h{}m", h, m),
    }
}

impl App {
    /// Construct a new instance of [`App`].
    pub fn new(db: FirestoreDb, projects: Vec<Project>) -> Self {
        Self {
            running: true,
            input: Input::default(),
            input_mode: InputMode::default(),
            db,
            projects,
            checkpoints: vec![],
            selected_checkpoint: None,
            dates: vec![],
            selected_date: None,
        }
    }

    /// Run the application's main loop.
    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.running = true;
        if let Err(err) = self.load_dates().await {
            eprintln!("{}", err);
        }
        self.load_checkpoints().await;
        while self.running {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_crossterm_events().await?;
        }
        Ok(())
    }

    /// Renders the user interface.
    ///
    /// This is where you add new widgets. See the following resources for more information:
    /// - <https://docs.rs/ratatui/latest/ratatui/widgets/index.html>
    /// - <https://github.com/ratatui/ratatui/tree/master/examples>
    fn draw(&mut self, frame: &mut Frame) {
        let [days_area, _, timeline_area, _, fill_area, input_area, controls_area] =
            Layout::vertical(vec![
                Constraint::Length(1), // days
                Constraint::Length(1), // spacer
                Constraint::Length(3), // timeline
                Constraint::Length(1), // spacer
                Constraint::Fill(1),
                Constraint::Length(3), // input
                Constraint::Length(1), // controls
            ])
            .areas(frame.area());

        frame.render_widget(
            // Paragraph::new(help_line()).block(Block::new().padding(Padding::horizontal(1))),
            HelpLine::default(),
            controls_area,
        );

        let days_constraints = vec![Constraint::Length(8); self.dates.len()];

        let days_layout = Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints(days_constraints)
            .spacing(1)
            .split(days_area);

        let spans = self.time_spans();

        let timeline_constraint = spans
            .iter()
            .map(|f| Constraint::Length((f.units * 2) + 2)) // border
            .collect::<Vec<Constraint>>();

        let timeline_layout = Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints(timeline_constraint)
            .split(timeline_area);

        for (i, day) in self.dates.iter().enumerate() {
            let mut p = Paragraph::new(day.format("%d.%m.%y").to_string());
            if let Some(j) = self.selected_date {
                if j == i {
                    p = p.underlined();
                }
            }
            frame.render_widget(p, days_layout[i]);
        }

        let fill_layout = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints(vec![Constraint::Length(4), Constraint::Fill(1)])
            .spacing(1)
            .split(fill_area);

        // TIMELINE

        for (i, span) in spans.iter().enumerate() {
            let current_ch = &self.checkpoints[i];

            let project = if let Some(project_id) = &current_ch.project {
                find_by_id(&self.projects, project_id)
            } else {
                None
            };

            let mut title_top = Line::from(human_duration(span.units as u32 * UNIT)).centered();
            let mut title_bottom = Line::from(current_ch.time.format("%H:%M").to_string()).gray();

            let mut text1 = "──".to_string().repeat(span.units as usize);

            let mut timeline_style = Style::new();

            if let Some(project) = project {
                timeline_style = timeline_style.fg(Color::Indexed(project.color));
            } else {
                text1 = "  ".to_string().repeat(span.units as usize);
            }

            if current_ch.message.is_none() {
                timeline_style = timeline_style.red();
            }

            if !current_ch.registered {
                title_bottom = title_bottom.bg(Color::Red);
            }

            if let Some(si) = self.selected_checkpoint {
                if i == si {
                    timeline_style = timeline_style.bold();
                    title_top = title_top.bold().underlined();
                }
            }

            frame.render_widget(
                // Paragraph::new(if i % 2 == 0 { text1 } else { text2 })
                Paragraph::new(Line::from(vec!["├".into(), text1.into(), "┤".into()]))
                    .style(timeline_style)
                    .block(Block::new().title(title_top).title_bottom(title_bottom))
                    .centered(),
                timeline_layout[i],
            );
        }

        if let Some(si) = self.selected_checkpoint {
            let selected_ch = &self.checkpoints[si];

            let rounded_start = selected_ch.rounded_time();
            let rounded_end = if self.checkpoints.len() > 1 {
                Some(self.checkpoints[si + 1].rounded_time())
            } else {
                None
            };

            let mut lines = vec![Line::from(vec![
                Span::from(" Started: "),
                Span::from(selected_ch.time.format("%H:%M").to_string()),
                Span::from(" ("),
                Span::from(rounded_start.format("%H:%M").to_string()),
                Span::from(")"),
            ])];

            if let Some(rounded_end) = rounded_end {
                lines.push(Line::from(vec![
                    Span::from("Finished: "),
                    Span::from(self.checkpoints[si + 1].time.format("%H:%M").to_string()),
                    Span::from(" ("),
                    Span::from(rounded_end.format("%H:%M").to_string()),
                    Span::from(")"),
                ]));
            }

            lines.push(Line::from(vec![
                Span::from(" Comment: "),
                Span::from(selected_ch.message.as_deref().unwrap_or("")).bg(Color::Indexed(28)),
            ]));

            lines.push(Line::from(vec![
                Span::from(
                    " Project: https://pbs2.praguebest.cz/main.php?pageid=110&action=detail&id=",
                ),
                Span::from(selected_ch.project.as_deref().unwrap_or("")),
            ]));

            frame.render_widget(Paragraph::new(lines), fill_layout[0]);
        }
        let projs = self
            .projects
            .iter()
            .enumerate()
            .map(|(i, p)| {
                Line::from(vec![
                    Span::from(format!("{}", i + 1)).bg(Color::Indexed(p.color)),
                    " ".into(),
                    Span::from(p.id.as_str()),
                    " ".into(),
                    p.name.as_str().into(),
                ])
            })
            .collect::<Vec<Line>>();
        frame.render_widget(Paragraph::new(projs), fill_layout[1]);
        self.render_input(frame, input_area);

        let xxx = Timeline {
            checkpoints: &self.checkpoints,
        };
        frame.render_widget(xxx, fill_layout[1]);
    }

    /// Reads the crossterm events and updates the state of [`App`].
    ///
    /// If your application needs to perform work in between handling events, you can use the
    /// [`event::poll`] function to check if there are any events available with a timeout.
    async fn handle_crossterm_events(&mut self) -> Result<()> {
        let event = event::read()?;
        match event {
            // it's important to check KeyEventKind::Press to avoid handling key release events
            Event::Key(key) if key.kind == KeyEventKind::Press => match self.input_mode {
                InputMode::Normal => self.on_key_event(key).await,
                InputMode::Editing => match key.code {
                    KeyCode::Enter => self.push_message().await,
                    KeyCode::Esc => self.stop_editing(),
                    _ => {
                        self.input.handle_event(&event);
                    }
                },
            },
            Event::Mouse(_) => {}
            Event::Resize(_, _) => {}
            _ => {}
        }
        Ok(())
    }

    /// Handles the key events and updates the state of [`App`].
    async fn on_key_event(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (_, KeyCode::Esc | KeyCode::Char('q'))
            | (KeyModifiers::CONTROL, KeyCode::Char('c') | KeyCode::Char('C')) => self.quit().await,
            // Add other key handlers here.
            (_, KeyCode::Char('e')) => self.start_editing(),
            (_, KeyCode::Char(' ')) => self.append_checkpoint().await,
            (_, KeyCode::Char('d')) => self.delete_checkpoint().await,
            (_, KeyCode::Char('1')) => self.assign_project(0).await,
            (_, KeyCode::Char('2')) => self.assign_project(1).await,
            (_, KeyCode::Char('3')) => self.assign_project(2).await,
            (_, KeyCode::Char('4')) => self.assign_project(3).await,
            (_, KeyCode::Char('5')) => self.assign_project(4).await,
            (_, KeyCode::Char('6')) => self.assign_project(5).await,
            (_, KeyCode::Char('7')) => self.assign_project(6).await,
            (_, KeyCode::Char('8')) => self.assign_project(7).await,
            (_, KeyCode::Char('9')) => self.assign_project(8).await,
            (KeyModifiers::CONTROL, KeyCode::Char('l')) => self.lenghten_ctrl_r().await,
            (_, KeyCode::Char('l')) => self.lenghten_r().await,
            (KeyModifiers::CONTROL, KeyCode::Char('h')) => self.lenghten_ctrl_l().await,
            (_, KeyCode::Char('h')) => self.lenghten_l().await,
            (_, KeyCode::Right) => self.move_right().await,
            (_, KeyCode::Left) => self.move_left().await,
            (_, KeyCode::Tab) => self.cycle_days().await,
            (_, KeyCode::Char('r')) => self.mark_registered().await,
            _ => {}
        }
    }

    /// For every two consecutive checkpoints count time span containing number of 15-minutes.
    /// Each TimeSpan represents the number of 15-minute intervals between two consecutive checkpoints.
    pub fn time_spans(&self) -> Vec<TimeSpan> {
        // If we have fewer than 2 checkpoints, we can't calculate any time spans
        if self.checkpoints.len() < 2 {
            return Vec::new();
        }

        let mut spans = Vec::new();

        // Iterate through consecutive pairs of checkpoints
        for i in 0..self.checkpoints.len() - 1 {
            let start_time = self.checkpoints[i].time;
            let end_time = self.checkpoints[i + 1].time;

            // Round both times to the nearest 15 minutes
            let rounded_start = round_to_nearest_fifteen_minutes(start_time);
            let rounded_end = round_to_nearest_fifteen_minutes(end_time);

            // Calculate the number of 15-minute intervals
            let intervals = count_fifteen_minute_intervals(rounded_start, rounded_end);

            // Create a TimeSpan with the calculated number of intervals
            // Convert to u32 since we expect positive intervals between consecutive checkpoints
            let time_span = TimeSpan {
                units: intervals.max(0) as u16,
            };

            spans.push(time_span);
        }
        spans
    }

    /// Set running to false to quit the application.
    async fn quit(&mut self) {
        self.running = false;
    }

    /// Append new checkpoint with the current time
    async fn append_checkpoint(&mut self) {
        // Create a new checkpoint with the current time
        match insert_checkpoint(&self.db).await {
            Ok(checkpoint) => self.checkpoints.push(checkpoint),
            Err(err) => eprintln!("{}", err),
        };
    }

    async fn delete_checkpoint(&mut self) {
        if let Some(i) = self.selected_checkpoint {
            if let Err(err) = delete_checkpoint(
                &self.db,
                &self.checkpoints[if self.checkpoints.len() == 1 {
                    0
                } else {
                    i + 1
                }],
            )
            .await
            {
                eprintln!("{}", err);
            }
            self.load_checkpoints().await;
        }
    }

    async fn load_checkpoints(&mut self) {
        if let Some(i) = self.selected_date {
            match load_checkpoints(&self.db, &self.dates[i]).await {
                Ok(checkpoints) => {
                    self.checkpoints = checkpoints;
                    self.selected_checkpoint = if self.checkpoints.is_empty() {
                        None
                    } else {
                        Some(0)
                    };
                }
                Err(err) => eprintln!("{}", err),
            }
        };
    }

    async fn load_dates(&mut self) -> FirestoreResult<()> {
        self.dates = get_distinct_dates(&self.db).await?;
        if !self.dates.is_empty() {
            self.selected_date = Some(self.dates.len() - 1);
        };
        Ok(())
    }

    async fn lenghten_r(&mut self) {
        if let Some(i) = self.selected_checkpoint {
            let selected_checkpoint = &mut self.checkpoints[i];
            if let Some(t) = selected_checkpoint
                .time
                .checked_add_signed(TimeDelta::minutes(15))
            {
                selected_checkpoint.time = t;
                if let Err(err) = update_checkpoint(&self.db, selected_checkpoint).await {
                    eprintln!("{}", err);
                }
            }
        }
    }

    async fn lenghten_ctrl_r(&mut self) {
        if let Some(i) = self.selected_checkpoint {
            if self.checkpoints.len() > i + 1 {
                let selected_checkpoint = &mut self.checkpoints[i + 1];
                if let Some(t) = selected_checkpoint
                    .time
                    .checked_add_signed(TimeDelta::minutes(15))
                {
                    selected_checkpoint.time = t;
                    if let Err(err) = update_checkpoint(&self.db, selected_checkpoint).await {
                        eprintln!("{}", err);
                    }
                }
            }
        }
    }

    async fn lenghten_l(&mut self) {
        if let Some(i) = self.selected_checkpoint {
            let selected_checkpoint = &mut self.checkpoints[i];
            if let Some(t) = selected_checkpoint
                .time
                .checked_add_signed(TimeDelta::minutes(-15))
            {
                selected_checkpoint.time = t;
                if let Err(err) = update_checkpoint(&self.db, selected_checkpoint).await {
                    eprintln!("{}", err);
                }
            }
        }
    }

    async fn lenghten_ctrl_l(&mut self) {
        if let Some(i) = self.selected_checkpoint {
            if self.checkpoints.len() > i + 1 {
                let selected_checkpoint = &mut self.checkpoints[i + 1];
                if let Some(t) = selected_checkpoint
                    .time
                    .checked_add_signed(TimeDelta::minutes(-15))
                {
                    selected_checkpoint.time = t;
                    if let Err(err) = update_checkpoint(&self.db, selected_checkpoint).await {
                        eprintln!("{}", err);
                    }
                }
            }
        }
    }

    async fn move_right(&mut self) {
        if let Some(i) = self.selected_checkpoint {
            if self.checkpoints.len() > i + 2 {
                self.selected_checkpoint = Some(i + 1);
            }
        }
    }

    async fn move_left(&mut self) {
        if let Some(i) = self.selected_checkpoint {
            self.selected_checkpoint = Some(i.max(1) - 1);
        }
    }

    async fn cycle_days(&mut self) {
        if let Some(i) = self.selected_date {
            let j = if self.dates.len() > i + 1 { i + 1 } else { 0 };
            self.selected_date = Some(j);
            self.load_checkpoints().await;
        }
    }

    async fn assign_project(&mut self, num: usize) {
        if let Some(i) = self.selected_checkpoint {
            let ch = &mut self.checkpoints[i];
            ch.project = Some(self.projects[num].id.clone());

            if let Err(err) = update_checkpoint(&self.db, ch).await {
                eprintln!("{}", err);
            }
            self.load_checkpoints().await;
        }
    }

    fn render_input(&self, frame: &mut Frame, area: Rect) {
        // keep 2 for borders and 1 for cursor
        let width = area.width.max(3) - 3;
        let scroll = self.input.visual_scroll(width as usize);
        let style = match self.input_mode {
            InputMode::Normal => Style::default().gray(),
            InputMode::Editing => Color::Yellow.into(),
        };
        let input = Paragraph::new(self.input.value())
            .style(style)
            .scroll((0, scroll as u16))
            .block(Block::bordered().title("Input"));
        frame.render_widget(input, area);

        if self.input_mode == InputMode::Editing {
            // Ratatui hides the cursor unless it's explicitly set. Position the  cursor past the
            // end of the input text and one line down from the border to the input line
            let x = self.input.visual_cursor().max(scroll) - scroll + 1;
            frame.set_cursor_position((area.x + x as u16, area.y + 1))
        }
    }

    fn start_editing(&mut self) {
        self.input_mode = InputMode::Editing
    }

    fn stop_editing(&mut self) {
        self.input_mode = InputMode::Normal
    }

    async fn push_message(&mut self) {
        if let Some(i) = self.selected_checkpoint {
            let ch = &mut self.checkpoints[i];
            ch.message = Some(self.input.value_and_reset());

            if let Err(err) = update_checkpoint(&self.db, ch).await {
                eprintln!("{}", err);
            }
            self.load_checkpoints().await;
        };
    }

    async fn mark_registered(&mut self) {
        if let Some(i) = self.selected_checkpoint {
            let ch = &mut self.checkpoints[i];
            ch.registered = true;
            if let Err(err) = update_checkpoint(&self.db, ch).await {
                eprintln!("{}", err);
            }
            self.load_checkpoints().await;
        };
    }

    async fn migrate(&mut self) {
        for ch in self.checkpoints.iter_mut() {
            ch.registered = false;

            if let Err(err) = update_checkpoint(&self.db, ch).await {
                eprintln!("{}", err);
            }
        }
    }
}
