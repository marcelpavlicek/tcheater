use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
    widgets::Widget,
};

#[derive(Default)]
pub struct HelpLine {}

impl Widget for HelpLine {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let help_style = Style::new().fg(Color::Gray);
        let line = Line::from(vec![
            Span::styled("Add: ", help_style),
            Span::raw("<space>"),
            Span::styled(" | Del: ", help_style),
            Span::raw("d"),
            Span::styled(" | Message: ", help_style),
            Span::raw("m"),
            Span::styled(" | Lenghten: ", help_style),
            Span::raw("<ctrl> h"),
            Span::styled("/", help_style),
            Span::raw("l"),
            Span::styled(" | Next: ", help_style),
            Span::raw("\u{003e}"),
            Span::styled(" | Prev: ", help_style),
            Span::raw("\u{003c}"),
            Span::styled(" | Cycle Days: ", help_style),
            Span::raw("<tab>"),
            Span::styled(" | Registered: ", help_style),
            Span::raw("r"),
            Span::styled(" | Assign: ", help_style),
            Span::raw("1-9"),
            Span::styled(" | Quit: ", help_style),
            Span::raw("q"),
        ]);
        buf.set_line(area.left() + 1, area.top(), &line, area.width);
    }
}
