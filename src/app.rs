use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
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
    time::{calculate_duration_minutes, human_duration, round_to_nearest_fifteen_minutes, Week},
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

    pub fn color(&self) -> Color {
        if self.message.is_none() {
            return Color::DarkGray;
        }

        if let Some(project_id) = &self.project {
            let mut hasher = DefaultHasher::new();
            project_id.hash(&mut hasher);
            let hash = hasher.finish();
            Color::Indexed((hash % 216) as u8 + 16)
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
    mondays: Vec<NaiveDate>,
    selected_mon_idx: usize,
    week: Week,
    auth_config: AuthConfig,
    tasks: Vec<PbsTask>,
    show_task_popup: bool,
    show_task_url: bool,
    task_popup_state: ListState,
    task_url_prefix: Option<String>,
}

impl App {
    /// Construct a new instance of [`App`].
    pub fn new(
        db: FirestoreDb,
        mondays: Vec<NaiveDate>,
        auth_config: AuthConfig,
        task_url_prefix: Option<String>,
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
                .map(|(ch, minutes)| {
                    Line::from(vec![
                        Span::from(ch.time.format("%d.%m %H:%M ").to_string()),
                        Span::from(ch.project.as_deref().unwrap_or("-").to_string()).bold(),
                        Span::from(" "),
                        Span::from(format!("({}) ", human_duration(*minutes))).fg(Color::Yellow),
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

        let [checkpoint_area] = Layout::vertical(vec![Constraint::Length(4)]).areas(fill_area);

        let [mon_area, tue_area, wed_area, thu_area, fri_area] =
            Layout::vertical(vec![Constraint::Length(3); 5])
                .spacing(1)
                .areas(timeline_area);

        let mon_w = Timeline {
            checkpoints: &self.week.mon,
            selected_checkpoint_idx: if self.week.selected_weekday == Weekday::Mon {
                Some(self.week.selected_checkpoint_idx)
            } else {
                None
            },
        };
        let tue_w = Timeline {
            checkpoints: &self.week.tue,
            selected_checkpoint_idx: if self.week.selected_weekday == Weekday::Tue {
                Some(self.week.selected_checkpoint_idx)
            } else {
                None
            },
        };
        let wed_w = Timeline {
            checkpoints: &self.week.wed,
            selected_checkpoint_idx: if self.week.selected_weekday == Weekday::Wed {
                Some(self.week.selected_checkpoint_idx)
            } else {
                None
            },
        };
        let thu_w = Timeline {
            checkpoints: &self.week.thu,
            selected_checkpoint_idx: if self.week.selected_weekday == Weekday::Thu {
                Some(self.week.selected_checkpoint_idx)
            } else {
                None
            },
        };
        let fri_w = Timeline {
            checkpoints: &self.week.fri,
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

            if let Some(prefix) = &self.task_url_prefix {
                lines.push(Line::from(vec![
                    Span::from(" Project: ").fg(Color::Gray),
                    Span::from(prefix).fg(Color::Gray),
                    Span::from(selected_ch.project.as_deref().unwrap_or("")),
                ]));
            } else {
                lines.push(Line::from(vec![
                    Span::from(" Project: ").fg(Color::Gray),
                    Span::from(selected_ch.project.as_deref().unwrap_or("")),
                ]));
            }

            frame.render_widget(Paragraph::new(lines), checkpoint_area);
        }

        self.render_input(frame, input_area);

        if self.show_task_popup {
            let area = centered_rect(60, 80, frame.area());
            frame.render_widget(Clear, area);
            let items: Vec<ListItem> = self
                .tasks
                .iter()
                .map(|t| {
                    let mut header_spans = vec![Span::from(format!("{} - {}", t.id, t.name))];

                    match (&t.time_spent, &t.time_total) {
                        (Some(s), Some(total)) => {
                            header_spans.push(Span::from(" ["));
                            header_spans.push(Span::from(s.to_string()).fg(Color::Green));
                            header_spans.push(Span::from(" / "));
                            header_spans.push(Span::from(total.to_string()).fg(Color::Blue));
                            header_spans.push(Span::from("]"));
                        }
                        (Some(s), None) => {
                            header_spans.push(Span::from(" ["));
                            header_spans.push(Span::from(s.to_string()).fg(Color::Green));
                            header_spans.push(Span::from("]"));
                        }
                        _ => {}
                    }

                    let header = Line::from(header_spans);

                    if self.show_task_url {
                        if let Some(prefix) = &self.task_url_prefix {
                            let url = format!("{}{}", prefix, t.id);
                            let lines = vec![header, Line::from(Span::from(url).fg(Color::Blue))];
                            ListItem::new(lines)
                        } else {
                            ListItem::new(header)
                        }
                    } else {
                        ListItem::new(header)
                    }
                })
                .collect();
            let list = List::new(items)
                .block(Block::bordered().title("Select Task"))
                .highlight_style(Style::default().fg(Color::Yellow))
                .highlight_symbol("▶ ");

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
                KeyCode::Right => {
                    self.show_task_url = !self.show_task_url;
                }
                KeyCode::Enter => {
                    self.assign_selected_task().await;
                    self.show_task_popup = false;
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

    async fn assign_selected_task(&mut self) {
        let task_id = if let Some(selected_idx) = self.task_popup_state.selected() {
            self.tasks.get(selected_idx).map(|t| t.id.to_string())
        } else {
            None
        };

        if let Some(id) = task_id {
            // Update local state
            {
                if let Some(selected_checkpoint) = self.week.selected_checkpoint_mut() {
                    selected_checkpoint.project = Some(id);
                }
            }

            // Update remote state
            if let Some(selected_checkpoint) = self.week.selected_checkpoint() {
                if let Err(err) = update_checkpoint(&self.db, selected_checkpoint).await {
                    eprintln!("{}", err);
                }
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
            if let Err(err) = delete_checkpoint(&self.db, selected).await {
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

        let mut unregistered: Vec<(Checkpoint, u32)> = vec![];

        // Iterate through each day's checkpoints and collect unregistered ones, excluding the last checkpoint of each day
        for day_checkpoints in [&mon, &tue, &wed, &thu, &fri] {
            if day_checkpoints.is_empty() {
                continue;
            }
            let last_idx = day_checkpoints.len() - 1;
            for (idx, checkpoint) in day_checkpoints.iter().enumerate() {
                if !checkpoint.registered && idx != last_idx {
                    let start_time = checkpoint.time;
                    let end_time = day_checkpoints[idx + 1].time;

                    let minutes = calculate_duration_minutes(start_time, end_time);

                    unregistered.push((checkpoint.clone(), minutes));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_color_generation() {
        let mut checkpoint = Checkpoint::new();
        checkpoint.message = Some("message".to_string());

        // Test with a task ID that should generate a color
        checkpoint.project = Some("12345".to_string());
        let color1 = checkpoint.color();

        // Test with another task ID
        checkpoint.project = Some("67890".to_string());
        let color2 = checkpoint.color();

        // Colors should be different (highly likely, but collisions are possible, so maybe test multiple)
        // With only 2, collision is possible but unlikely if hash is good.
        // Let's verify they are not White or Red

        if let Color::Indexed(c) = color1 {
            assert!(c >= 16 && c <= 231, "Color {} is out of range 16-231", c);
        } else {
            panic!("Expected Color::Indexed, got {:?}", color1);
        }

        if let Color::Indexed(c) = color2 {
            assert!(c >= 16 && c <= 231, "Color {} is out of range 16-231", c);
        } else {
            panic!("Expected Color::Indexed, got {:?}", color2);
        }

        assert_ne!(color1, Color::White);
        assert_ne!(color2, Color::White);
        assert_ne!(color1, Color::Red);
        assert_ne!(color2, Color::Red);

        // Test with no message -> Red
        checkpoint.message = None;
        let color_no_msg = checkpoint.color();
        assert_eq!(color_no_msg, Color::Red);
    }
}
