use ratatui::{text::Line, widgets::Widget};

use crate::app::Checkpoint;

pub struct Timeline<'a> {
    pub checkpoints: &'a Vec<Checkpoint>,
}

impl<'a> Widget for Timeline<'a> {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let l = Line::from(self.checkpoints.len().to_string());
        l.render(area, buf);
    }
}
