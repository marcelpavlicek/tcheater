use crate::{app::Checkpoint, time::time_spans};
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{Block, Paragraph, Widget},
};

const FIFTEEN_LEN: u16 = 4;

pub struct Timeline<'a> {
    pub checkpoints: &'a Vec<Checkpoint>,
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
            .map(|s| Constraint::Length(s.units * FIFTEEN_LEN + 2)) // border
            .collect::<Vec<Constraint>>();

        let areas = Layout::horizontal(timeline_constraint).split(main_area);

        for (i, span) in spans.iter().enumerate() {
            let current_ch = &self.checkpoints[i];

            let title_top = Line::from(span.human_time()).centered();
            let mut title_bottom = Line::from(current_ch.time.format("%H:%M").to_string());
            let timeline_style = Style::new().fg(current_ch.color());

            let mut fill_char = "─";

            if current_ch.project.is_none() {
                if current_ch.message.as_deref().unwrap_or("").is_empty() {
                    fill_char = " ";
                } else {
                    fill_char = "╶";
                }
            }

            let text = fill_char
                .repeat(FIFTEEN_LEN.into())
                .repeat(span.units as usize);

            if !current_ch.registered {
                title_bottom = title_bottom.bg(Color::Red).fg(Color::White);
            }

            let text_span = ratatui::text::Span::from(text);
            let mut left_marker = if i == 0 {
                ratatui::text::Span::from("├")
            } else {
                ratatui::text::Span::from("┼")
            };
            let mut right_marker = if i + 1 == spans.len() {
                ratatui::text::Span::from("┤")
            } else {
                ratatui::text::Span::from(fill_char)
            };

            if let Some(j) = self.selected_checkpoint_idx {
                if i == j {
                    left_marker = left_marker.bg(Color::DarkGray);
                }
                if i + 1 == j && i + 1 == spans.len() {
                    right_marker = right_marker.bg(Color::DarkGray);
                }
            }

            let p = Paragraph::new(Line::from(vec![left_marker, text_span, right_marker]))
                .style(timeline_style)
                .block(Block::new().title(title_top).title_bottom(title_bottom))
                .centered();
            p.render(areas[i], buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Local};
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn test_render_spaces_for_empty_checkpoint() {
        let backend = TestBackend::new(40, 5);
        let mut terminal = Terminal::new(backend).unwrap();

        let start_time = Local::now();
        // Create two checkpoints 15 minutes apart to get 1 unit span
        let checkpoints = vec![
            Checkpoint {
                time: start_time,
                project: None,
                message: None,
                ..Checkpoint::new()
            },
            Checkpoint {
                time: start_time + Duration::minutes(15),
                project: None,
                message: None,
                ..Checkpoint::new()
            },
        ];

        let widget = Timeline {
            checkpoints: &checkpoints,
            selected_checkpoint_idx: None,
        };

        terminal
            .draw(|f| {
                f.render_widget(widget, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();

        // We expect "├    ┤" somewhere.
        // We iterate over lines and check content.
        let mut found = false;
        for y in 0..5 {
            let line_text: String = (0..40).map(|x| buffer[(x, y)].symbol()).collect();
            // println!("Line {}: {}", y, line_text); // Cannot print in test without capturing?
            if line_text.contains("├    ┤") {
                found = true;
                break;
            }
        }
        assert!(found, "Did not find space line '├    ┤' in buffer");
    }

    #[test]
    fn test_render_line_for_empty_project_with_message() {
        let backend = TestBackend::new(40, 5);
        let mut terminal = Terminal::new(backend).unwrap();

        let start_time = Local::now();
        let checkpoints = vec![
            Checkpoint {
                time: start_time,
                project: None,
                message: Some("Break".to_string()),
                ..Checkpoint::new()
            },
            Checkpoint {
                time: start_time + Duration::minutes(15),
                project: None,
                message: None,
                ..Checkpoint::new()
            },
        ];

        let widget = Timeline {
            checkpoints: &checkpoints,
            selected_checkpoint_idx: None,
        };

        terminal
            .draw(|f| {
                f.render_widget(widget, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();

        // We expect "├╶╶╶╶┤" somewhere.
        let mut found = false;
        for y in 0..5 {
            let line_text: String = (0..40).map(|x| buffer[(x, y)].symbol()).collect();
            if line_text.contains("├╶╶╶╶┤") {
                found = true;
                break;
            }
        }
        assert!(found, "Did not find line '├╶╶╶╶┤' in buffer");
    }

    #[test]
    fn test_highlight_selected_checkpoint() {
        let backend = TestBackend::new(40, 5);
        let mut terminal = Terminal::new(backend).unwrap();

        let start_time = Local::now();
        // Create two checkpoints 15 minutes apart to get 1 unit span
        let checkpoints = vec![
            Checkpoint {
                time: start_time,
                project: None,
                message: None,
                ..Checkpoint::new()
            },
            Checkpoint {
                time: start_time + Duration::minutes(15),
                project: None,
                message: None,
                ..Checkpoint::new()
            },
        ];

        let widget = Timeline {
            checkpoints: &checkpoints,
            selected_checkpoint_idx: Some(0), // Select the first one
        };

        terminal
            .draw(|f| {
                f.render_widget(widget, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();

        // Check if the background of the first checkpoint is DarkGray
        // The layout is: 5 chars prelude, 1 char spacer, then the timeline.
        // So the timeline starts at x=6.

        let marker_cell = &buffer[(6, 1)]; // content is at y=1 because title is at y=0
        assert_eq!(marker_cell.symbol(), "├");
        assert_eq!(
            marker_cell.bg,
            Color::DarkGray,
            "Background color should be DarkGray for the left marker of the selected checkpoint"
        );

        // The span itself (e.g. x=8) should NOT be highlighted
        let content_cell = &buffer[(8, 1)];
        assert_ne!(
            content_cell.bg,
            Color::DarkGray,
            "Background color should NOT be DarkGray for the content of the span"
        );
    }

    #[test]
    fn test_highlight_last_checkpoint() {
        let backend = TestBackend::new(40, 5);
        let mut terminal = Terminal::new(backend).unwrap();

        let start_time = Local::now();
        // Create two checkpoints 15 minutes apart
        let checkpoints = vec![
            Checkpoint {
                time: start_time,
                project: None,
                message: None,
                ..Checkpoint::new()
            },
            Checkpoint {
                time: start_time + Duration::minutes(15),
                project: None,
                message: None,
                ..Checkpoint::new()
            },
        ];

        // Select the last checkpoint (index 1)
        let widget = Timeline {
            checkpoints: &checkpoints,
            selected_checkpoint_idx: Some(1),
        };

        terminal
            .draw(|f| {
                f.render_widget(widget, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();

        // The first (and only) span represents the interval between Ch0 and Ch1.
        // Ch1 is the end of this span.
        // Span width = 6. Starts at x=6.
        // Ends at x=11.
        // The right marker "┤" is at x=11.
        // Content at y=1.

        let marker_cell = &buffer[(11, 1)];
        assert_eq!(marker_cell.symbol(), "┤");
        assert_eq!(
            marker_cell.bg,
            Color::DarkGray,
            "Background color should be DarkGray for the right marker (last checkpoint)"
        );
    }
}
