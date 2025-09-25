use crate::{
    app::Checkpoint,
    main,
    projects::Project,
    time::{human_duration, time_spans, UNIT},
};
use color_eyre::owo_colors::OwoColorize;
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{Block, Paragraph, Widget},
};

pub struct Timeline<'a> {
    pub checkpoints: &'a Vec<Checkpoint>,
    pub projects: &'a Vec<Project>,
    pub selected_checkpoint_idx: Option<usize>,
}

impl<'a> Widget for Timeline<'a> {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let [pre_area, main_area] =
            Layout::horizontal(vec![Constraint::Length(5), Constraint::Fill(1)])
                .spacing(1)
                .areas(area);

        let mut prelude_p = Paragraph::default();

        if let Some(ch) = self.checkpoints.first() {
            prelude_p = Paragraph::new(vec![
                Line::from(ch.time.format("%a").to_string()),
                Line::from(ch.time.format("%d.").to_string()),
            ])
            .centered();
            if self.selected_checkpoint_idx.is_some() {
                prelude_p = prelude_p.bg(Color::Gray).fg(Color::Black).bold();
            }
        }
        prelude_p.render(pre_area, buf);

        let spans = time_spans(self.checkpoints);

        let timeline_constraint = spans
            .iter()
            .map(|s| Constraint::Length((s.units * 2) + 2)) // border
            .collect::<Vec<Constraint>>();

        let areas = Layout::horizontal(timeline_constraint).split(main_area);

        for (i, span) in spans.iter().enumerate() {
            let current_ch = &self.checkpoints[i];

            let mut title_top = Line::from(human_duration(span.units as u32 * UNIT)).centered();
            let mut title_bottom = Line::from(current_ch.time.format("%H:%M").to_string());
            let mut text = "──".to_string().repeat(span.units as usize);
            let timeline_style = Style::new().fg(current_ch.color(self.projects));

            if current_ch.project.is_none() {
                text = "  ".to_string().repeat(span.units as usize);
            }

            if !current_ch.registered {
                title_bottom = title_bottom.bg(Color::Red);
            }

            if let Some(j) = self.selected_checkpoint_idx {
                if i == j {
                    title_top = title_top.bold().underlined();
                }
            }

            let p = Paragraph::new(Line::from(vec!["├".into(), text.into(), "┤".into()]))
                .style(timeline_style)
                .block(Block::new().title(title_top).title_bottom(title_bottom))
                .centered();
            p.render(areas[i], buf);
        }
    }
}
