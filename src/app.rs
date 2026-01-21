use std::{fmt::Display, vec};
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

use chrono::{DateTime, Datelike, Days, Local, NaiveDate, TimeDelta, Weekday};
use color_eyre::Result;
use firestore::FirestoreDb;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Paragraph},
    DefaultTerminal, Frame,
};
use serde::{Deserialize, Serialize};

use crate::{
    firestore::{delete_checkpoint, find_checkpoints, insert_checkpoint, update_checkpoint},
    pbs::{fetch_tasks, AuthConfig, PbsTask},
    projects::{find_by_id, Project},
    time::{round_to_nearest_fifteen_minutes, Week},
    timeline_widget::Timeline,
    widgets::HelpLine,
};

use ratatui::widgets::{Clear, List, ListItem, ListState};

#[derive(Default)]
pub struct TimeSpan {
    units: u16,
}

impl Display for TimeSpan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.units)
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
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

    pub fn color(&self, projects: &[Project]) -> Color {
        if self.message.is_none() {
            return Color::Red;
        }

        if let Some(project_id) = &self.project {
            match find_by_id(projects, project_id) {
                Some(p) => Color::Indexed(p.color),
                None => Color::White,
            }
        } else {
            Color::White
        }
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
    mondays: Vec<NaiveDate>,
    selected_mon_idx: usize,
    week: Week,
    auth_config: AuthConfig,
    tasks: Vec<PbsTask>,
    show_task_popup: bool,
    show_task_url: bool,
    task_popup_state: ListState,
    task_url_prefix: String,
}

impl App {
    /// Construct a new instance of [`App`].
    pub fn new(
        db: FirestoreDb,
        projects: Vec<Project>,
        mondays: Vec<NaiveDate>,
        auth_config: AuthConfig,
        task_url_prefix: String,
    ) -> Self {
        let today = Local::now().date_naive();
        let current_monday = today - TimeDelta::days(today.weekday().num_days_from_monday() as i64);
        let selected_mon_idx = mondays
            .iter()
            .position(|&m| m == current_monday)
            .unwrap_or(0);

        Self {
            running: true,
            input: Input::default(),
            input_mode: InputMode::default(),
            db,
            projects,
            mondays,
            selected_mon_idx,
            week: Week::new(),
            auth_config,
            tasks: vec![],
            show_task_popup: false,
            show_task_url: false,
            task_popup_state: ListState::default(),
            task_url_prefix,
        }
    }

    /// Run the application's main loop.
    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.running = true;

        self.load_week().await;

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
        let unregistered_count = self.week.unregistered_checkpoints.len();
        let unregistered_height = if unregistered_count > 0 {
            // Add 2 for the bottom and top border of the block
            unregistered_count as u16 + 2
        } else {
            0
        };

        let mut constraints = Vec::new();
        if unregistered_height > 0 {
            constraints.push(Constraint::Length(unregistered_height));
        }
        constraints.extend(vec![
            Constraint::Length(1),         // days
            Constraint::Length(1),         // spacer
            Constraint::Length(3 * 5 + 4), // timeline
            Constraint::Length(1),         // spacer
            Constraint::Fill(1),
            Constraint::Length(3), // input
            Constraint::Length(1), // controls
        ]);

        let areas = Layout::vertical(constraints).split(frame.area());

        let mut area_index = 0;
        if unregistered_height > 0 {
            let unregistered_area = areas[area_index];
            let lines: Vec<Line> = self
                .week
                .unregistered_checkpoints
                .iter()
                .map(|ch| {
                    Line::from(vec![
                        Span::from(ch.time.format("%d.%m %H:%M ").to_string()),
                        Span::from(ch.project.as_deref().unwrap_or("-").to_string()).bold(),
                        Span::from(" "),
                        Span::from(ch.message.as_deref().unwrap_or("")),
                    ])
                })
                .collect();
            let paragraph =
                Paragraph::new(lines).block(Block::bordered().title("Unregistered Checkpoints"));
            frame.render_widget(paragraph, unregistered_area);
            area_index += 1;
        }

        let weeks_area = areas[area_index];
        let timeline_area = areas[area_index + 2];
        let fill_area = areas[area_index + 4];
        let input_area = areas[area_index + 5];
        let controls_area = areas[area_index + 6];

        frame.render_widget(HelpLine::default(), controls_area);

        let days_layout = Layout::horizontal(vec![Constraint::Length(5); self.mondays.len()])
            .spacing(1)
            .split(weeks_area);

        for (i, day) in self.mondays.iter().enumerate() {
            let mut p = Paragraph::new(day.format("%d.%m").to_string());
            if self.selected_mon_idx == i {
                p = p.underlined();
            }
            frame.render_widget(p, days_layout[i]);
        }

        let [checkpoint_area, projects_area] =
            Layout::vertical(vec![Constraint::Length(4), Constraint::Fill(1)])
                .spacing(1)
                .areas(fill_area);

        let [mon_area, tue_area, wed_area, thu_area, fri_area] =
            Layout::vertical(vec![Constraint::Length(3); 5])
                .spacing(1)
                .areas(timeline_area);

        let mon_w = Timeline {
            checkpoints: &self.week.mon,
            projects: &self.projects,
            selected_checkpoint_idx: if self.week.selected_weekday == Weekday::Mon {
                Some(self.week.selected_checkpoint_idx)
            } else {
                None
            },
        };
        let tue_w = Timeline {
            checkpoints: &self.week.tue,
            projects: &self.projects,
            selected_checkpoint_idx: if self.week.selected_weekday == Weekday::Tue {
                Some(self.week.selected_checkpoint_idx)
            } else {
                None
            },
        };
        let wed_w = Timeline {
            checkpoints: &self.week.wed,
            projects: &self.projects,
            selected_checkpoint_idx: if self.week.selected_weekday == Weekday::Wed {
                Some(self.week.selected_checkpoint_idx)
            } else {
                None
            },
        };
        let thu_w = Timeline {
            checkpoints: &self.week.thu,
            projects: &self.projects,
            selected_checkpoint_idx: if self.week.selected_weekday == Weekday::Thu {
                Some(self.week.selected_checkpoint_idx)
            } else {
                None
            },
        };
        let fri_w = Timeline {
            checkpoints: &self.week.fri,
            projects: &self.projects,
            selected_checkpoint_idx: if self.week.selected_weekday == Weekday::Fri {
                Some(self.week.selected_checkpoint_idx)
            } else {
                None
            },
        };
        frame.render_widget(mon_w, mon_area);
        frame.render_widget(tue_w, tue_area);
        frame.render_widget(wed_w, wed_area);
        frame.render_widget(thu_w, thu_area);
        frame.render_widget(fri_w, fri_area);

        if let Some(selected_ch) = self.week.selected_checkpoint() {
            let next_ch = self.week.next_checkpoint();

            let rounded_start = selected_ch.rounded_time();

            let mut lines = vec![Line::from(vec![
                Span::from(" Started: ").fg(Color::Gray),
                Span::from(selected_ch.time.format("%H:%M").to_string()),
                Span::from(" ("),
                Span::from(rounded_start.format("%H:%M").to_string()),
                Span::from(")"),
            ])];

            if let Some(next_ch) = next_ch {
                let rounded_end = next_ch.rounded_time();
                lines.push(Line::from(vec![
                    Span::from("Finished: ").fg(Color::Gray),
                    Span::from(next_ch.time.format("%H:%M").to_string()),
                    Span::from(" ("),
                    Span::from(rounded_end.format("%H:%M").to_string()),
                    Span::from(")"),
                ]));
            }

            lines.push(Line::from(vec![
                Span::from(" Comment: ").fg(Color::Gray),
                Span::from(selected_ch.message.as_deref().unwrap_or("")).fg(Color::Green),
            ]));

            lines.push(Line::from(vec![
                Span::from(" Project: ").fg(Color::Gray),
                Span::from(&self.task_url_prefix).fg(Color::Gray),
                Span::from(selected_ch.project.as_deref().unwrap_or("")),
            ]));

            frame.render_widget(Paragraph::new(lines), checkpoint_area);
        }

        let mut project_lines: Vec<Line> = vec![];

        for (i, p) in self.projects.iter().enumerate() {
            project_lines.append(&mut vec![Line::from(vec![
                Span::from(format!("{}", i + 1)).bg(Color::Indexed(p.color)),
                " ".into(),
                Span::from(p.id.as_str()).fg(Color::Gray),
                " ".into(),
                Span::from(&p.name).fg(Color::Gray),
            ])]);

            let task_lines = &mut p
                .tasks
                .iter()
                .map(|t| Line::from(vec![Span::from("  - "), Span::from(t)]))
                .collect::<Vec<Line>>();

            project_lines.append(task_lines);
        }
        frame.render_widget(Paragraph::new(project_lines), projects_area);

        self.render_input(frame, input_area);

        if self.show_task_popup {
            let area = centered_rect(60, 50, frame.area());
            frame.render_widget(Clear, area);
            let items: Vec<ListItem> = self
                .tasks
                .iter()
                .map(|t| {
                    if self.show_task_url {
                        let url = format!("{}{}", self.task_url_prefix, t.id);
                        let lines = vec![
                            Line::from(format!("{} - {}", t.id, t.name)),
                            Line::from(Span::from(url).fg(Color::Blue)),
                        ];
                        ListItem::new(lines)
                    } else {
                        ListItem::new(format!("{} - {}", t.id, t.name))
                    }
                })
                .collect();
            let list = List::new(items)
                .block(Block::bordered().title("Select Task"))
                .highlight_style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(ratatui::style::Modifier::BOLD),
                )
                .highlight_symbol(">> ");

            frame.render_stateful_widget(list, area, &mut self.task_popup_state);
        }
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
                    KeyCode::Enter => {
                        self.push_message().await;
                        self.stop_editing();
                    }
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
        if self.show_task_popup {
            match key.code {
                KeyCode::Esc => self.show_task_popup = false,
                KeyCode::Down => {
                    self.task_popup_state.select_next();
                }
                KeyCode::Up => {
                    self.task_popup_state.select_previous();
                }
                KeyCode::Left => {
                    self.show_task_url = !self.show_task_url;
                }
                _ => {}
            }
            return;
        }

        match (key.modifiers, key.code) {
            (_, KeyCode::Esc | KeyCode::Char('q'))
            | (KeyModifiers::CONTROL, KeyCode::Char('c') | KeyCode::Char('C')) => self.quit().await,
            // Add other key handlers here.
            (_, KeyCode::Char('m')) => self.start_editing(),
            (_, KeyCode::Char('p')) => self.fetch_tasks().await,
            (_, KeyCode::Char(' ')) => self.append_checkpoint().await,
            (_, KeyCode::Char('s')) => self.split_checkpoint().await,
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
            (_, KeyCode::Up) => self.move_up().await,
            (_, KeyCode::Down) => self.move_down().await,
            (_, KeyCode::Tab) => self.cycle_weeks().await,
            (_, KeyCode::Char('r')) => self.mark_registered().await,
            _ => {}
        }
    }

    async fn fetch_tasks(&mut self) {
        match fetch_tasks(&self.auth_config).await {
            Ok(tasks) => {
                self.tasks = tasks;
                self.show_task_popup = true;
                self.task_popup_state.select(Some(0));
            }
            Err(err) => {
                eprintln!("Failed to fetch tasks: {}", err);
            }
        }
    }

    /// Set running to false to quit the application.
    async fn quit(&mut self) {
        self.running = false;
    }

    /// Append new checkpoint with the current time
    async fn append_checkpoint(&mut self) {
        // Create a new checkpoint with the current time
        match insert_checkpoint(&self.db, Checkpoint::new()).await {
            Ok(checkpoint) => self.week.append_checkpoint(checkpoint),
            Err(err) => eprintln!("{}", err),
        };
        self.load_week().await;
    }

    async fn split_checkpoint(&mut self) {
        let (start_time, end_time) = {
            let selected = self.week.selected_checkpoint();
            let next = self.week.next_checkpoint();
            match (selected, next) {
                (Some(s), Some(n)) => (s.time, n.time),
                _ => return,
            }
        };

        let duration = end_time - start_time;
        // Check if duration is positive
        if duration <= TimeDelta::zero() {
            return;
        }

        let half_duration = duration / 2;
        let mid_time = start_time + half_duration;

        let mut new_checkpoint = Checkpoint::new();
        new_checkpoint.time = mid_time;

        if let Err(err) = insert_checkpoint(&self.db, new_checkpoint).await {
            eprintln!("{}", err);
        }
        self.load_week().await;
    }

    async fn delete_checkpoint(&mut self) {
        if let Some(selected) = self.week.selected_checkpoint() {
            if let Err(err) =
                delete_checkpoint(&self.db, self.week.next_checkpoint().unwrap_or(selected)).await
            {
                eprintln!("{}", err);
            }
            self.load_week().await;
        }
    }

    async fn load_checkpoints(&mut self, day: NaiveDate) -> Vec<Checkpoint> {
        match find_checkpoints(&self.db, &day).await {
            Ok(checkpoints) => checkpoints,
            Err(err) => {
                eprintln!("{}", err);
                vec![]
            }
        }
    }

    async fn load_week(&mut self) {
        let first_mon = self.mondays[self.selected_mon_idx]; // must be mondays in a month
        let mon = self.load_checkpoints(first_mon).await;
        let tue = self.load_checkpoints(first_mon + Days::new(1)).await;
        let wed = self.load_checkpoints(first_mon + Days::new(2)).await;
        let thu = self.load_checkpoints(first_mon + Days::new(3)).await;
        let fri = self.load_checkpoints(first_mon + Days::new(4)).await;

        let mut unregistered = vec![];

        // Iterate through each day's checkpoints and collect unregistered ones, excluding the last checkpoint of each day
        for day_checkpoints in [&mon, &tue, &wed, &thu, &fri] {
            if day_checkpoints.is_empty() {
                continue;
            }
            let last_idx = day_checkpoints.len() - 1;
            for (idx, checkpoint) in day_checkpoints.iter().enumerate() {
                if !checkpoint.registered && idx != last_idx {
                    unregistered.push(checkpoint.clone());
                }
            }
        }

        self.week = Week {
            mon,
            tue,
            wed,
            thu,
            fri,
            unregistered_checkpoints: unregistered,
            selected_weekday: chrono::Weekday::Mon,
            selected_checkpoint_idx: 0,
        };
    }

    async fn lenghten_r(&mut self) {
        if let Some(selected) = self.week.selected_checkpoint_mut() {
            if let Some(t) = selected.time.checked_add_signed(TimeDelta::minutes(15)) {
                selected.time = t;

                if let Err(err) = update_checkpoint(&self.db, selected).await {
                    eprintln!("{}", err);
                }
            }
        }
    }

    async fn lenghten_ctrl_r(&mut self) {
        if let Some(next) = self.week.next_checkpoint_mut() {
            if let Some(t) = next.time.checked_add_signed(TimeDelta::minutes(15)) {
                next.time = t;

                if let Err(err) = update_checkpoint(&self.db, next).await {
                    eprintln!("{}", err);
                }
            }
        }
    }

    async fn lenghten_l(&mut self) {
        if let Some(selected) = self.week.selected_checkpoint_mut() {
            if let Some(t) = selected.time.checked_add_signed(TimeDelta::minutes(-15)) {
                selected.time = t;

                if let Err(err) = update_checkpoint(&self.db, selected).await {
                    eprintln!("{}", err);
                }
            }
        }
    }

    async fn lenghten_ctrl_l(&mut self) {
        if let Some(next) = self.week.next_checkpoint_mut() {
            if let Some(t) = next.time.checked_add_signed(TimeDelta::minutes(-15)) {
                next.time = t;

                if let Err(err) = update_checkpoint(&self.db, next).await {
                    eprintln!("{}", err);
                }
            }
        }
    }

    async fn move_right(&mut self) {
        self.week.select_next_checkpoint();
    }

    async fn move_left(&mut self) {
        self.week.select_prev_checkpoint();
    }

    async fn move_up(&mut self) {
        self.week.select_prev_day();
    }

    async fn move_down(&mut self) {
        self.week.select_next_day();
    }

    async fn cycle_weeks(&mut self) {
        self.selected_mon_idx = if self.mondays.len() > self.selected_mon_idx + 1 {
            self.selected_mon_idx + 1
        } else {
            0
        };
        self.load_week().await;
    }

    async fn assign_project(&mut self, num: usize) {
        if let Some(selected) = self.week.selected_checkpoint_mut() {
            let project_id = self.projects[num].id.clone();
            if let Some(current_project_id) = &selected.project {
                if current_project_id == &project_id {
                    selected.project = None;
                } else {
                    selected.project = Some(project_id);
                }
            } else {
                selected.project = Some(project_id);
            }

            if let Err(err) = update_checkpoint(&self.db, selected).await {
                eprintln!("{}", err);
            }
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
        if let Some(selected) = self.week.selected_checkpoint_mut() {
            selected.message = Some(self.input.value_and_reset());

            if let Err(err) = update_checkpoint(&self.db, selected).await {
                eprintln!("{}", err);
            }
        };
    }

    async fn mark_registered(&mut self) {
        if let Some(selected) = self.week.selected_checkpoint_mut() {
            selected.registered = !selected.registered;

            if let Err(err) = update_checkpoint(&self.db, selected).await {
                eprintln!("{}", err);
            }
        };
    }

    // async fn migrate(&mut self) {
    //     for ch in self.checkpoints.iter_mut() {
    //         ch.registered = false;
    //
    //         if let Err(err) = update_checkpoint(&self.db, ch).await {
    //             eprintln!("{}", err);
    //         }
    //     }
    // }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}
