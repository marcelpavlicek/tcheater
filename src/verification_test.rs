#[cfg(test)]
mod verification_test {
    use ansi_to_tui::IntoText;

    #[test]
    fn test_ansi_hyperlink_parsing() {
        let url = "http://example.com";
        let text_content = "Link";
        let ansi = format!("\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\", url, text_content);

        let text = ansi.as_bytes().into_text().unwrap();
        println!("Parsed Text: {:?}", text);

        // Inspect the text lines and spans
        for line in text.lines {
            for span in line.spans {
                println!("Span content: {:?}", span.content);
                println!("Span style: {:?}", span.style);
            }
        }
    }
}
