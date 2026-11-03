//! Bounded, native Markdown presentation. Model text never becomes HTML, a
//! resource request, a hyperlink action or a command. Source text stays intact.
use egui::{FontId, RichText, TextStyle, text::LayoutJob};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

const MAX_EVENTS: usize = 32_768;
const MAX_BLOCKS: usize = 2048;
const MAX_DEPTH: usize = 32;
const MAX_TABLE_CELLS: usize = 2048;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Style {
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    code: bool,
}

#[derive(Clone, Debug)]
struct Span {
    text: String,
    style: Style,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Role {
    #[default]
    Body,
    Heading(u8),
    Code,
}

#[derive(Clone, Debug, Default)]
struct Text {
    role: Role,
    prefix: String,
    indent: usize,
    quote: bool,
    spans: Vec<Span>,
}

#[derive(Clone, Debug)]
enum Block {
    Text(Text),
    Rule,
    Table(Vec<Vec<Vec<Span>>>),
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Document {
    blocks: Vec<Block>,
}

#[derive(Default)]
struct Builder {
    doc: Document,
    text: Text,
    lists: Vec<Option<u64>>,
    quote: usize,
    bold: usize,
    italic: usize,
    underline: usize,
    strike: usize,
    table: Option<Vec<Vec<Vec<Span>>>>,
    row: Vec<Vec<Span>>,
    cell: Option<Vec<Span>>,
    cell_count: usize,
}

impl Builder {
    fn flush(&mut self) {
        if !self.text.spans.is_empty() || !self.text.prefix.is_empty() {
            self.doc.blocks.push(Block::Text(std::mem::take(&mut self.text)));
        }
        self.text.indent = self.lists.len().saturating_sub(1);
        self.text.quote = self.quote > 0;
    }

    fn push(&mut self, text: &str, code: bool) {
        if text.is_empty() {
            return;
        }
        let style = Style {
            bold: self.bold > 0,
            italic: self.italic > 0,
            underline: self.underline > 0,
            strike: self.strike > 0,
            code: code || self.text.role == Role::Code,
        };
        let spans = self.cell.as_mut().unwrap_or(&mut self.text.spans);
        if let Some(last) = spans.last_mut()
            && last.style == style
        {
            last.text.push_str(text);
        } else {
            spans.push(Span { text: text.into(), style });
        }
    }

    fn finish_row(&mut self) {
        if !self.row.is_empty()
            && let Some(table) = &mut self.table
        {
            table.push(std::mem::take(&mut self.row));
        }
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph if !self.text.spans.is_empty() => {
                    self.flush();
                }
                Tag::Heading { level, .. } => {
                    self.flush();
                    self.text.role = Role::Heading(level as u8);
                }
                Tag::CodeBlock(_) => {
                    self.flush();
                    self.text.role = Role::Code;
                }
                Tag::BlockQuote(_) => {
                    self.flush();
                    self.quote += 1;
                    self.text.quote = true;
                }
                Tag::List(start) => {
                    self.flush();
                    self.lists.push(start);
                }
                Tag::Item => {
                    self.flush();
                    self.text.indent = self.lists.len().saturating_sub(1);
                    self.text.prefix = match self.lists.last_mut() {
                        Some(Some(next)) => {
                            let marker = format!("{next}.");
                            *next = next.saturating_add(1);
                            marker
                        }
                        _ => "•".into(),
                    };
                }
                Tag::Emphasis => self.italic += 1,
                Tag::Strong => self.bold += 1,
                Tag::Strikethrough => self.strike += 1,
                Tag::Image { .. } => self.push("[Image: ", false),
                Tag::Table(_) => {
                    self.flush();
                    self.table = Some(Vec::new());
                }
                Tag::TableHead | Tag::TableRow => self.finish_row(),
                Tag::TableCell => self.cell = Some(Vec::new()),
                // Link destinations and HTML attributes have no executable route.
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::CodeBlock | TagEnd::Item => self.flush(),
                TagEnd::BlockQuote(_) => {
                    self.flush();
                    self.quote = self.quote.saturating_sub(1);
                    self.text.quote = self.quote > 0;
                }
                TagEnd::List(_) => {
                    self.flush();
                    self.lists.pop();
                }
                TagEnd::Emphasis => self.italic = self.italic.saturating_sub(1),
                TagEnd::Strong => self.bold = self.bold.saturating_sub(1),
                TagEnd::Strikethrough => self.strike = self.strike.saturating_sub(1),
                TagEnd::Image => self.push("]", false),
                TagEnd::TableCell => {
                    if let Some(cell) = self.cell.take() {
                        self.row.push(cell);
                        self.cell_count += 1;
                    }
                }
                TagEnd::TableHead | TagEnd::TableRow => self.finish_row(),
                TagEnd::Table => {
                    self.finish_row();
                    if let Some(table) = self.table.take() {
                        self.doc.blocks.push(Block::Table(table));
                    }
                }
                _ => {}
            },
            Event::Text(text) => self.push(&text, false),
            Event::Code(text) => self.push(&text, true),
            Event::SoftBreak => self.push(" ", false),
            Event::HardBreak => self.push("\n", false),
            Event::Rule => {
                self.flush();
                self.doc.blocks.push(Block::Rule);
            }
            Event::TaskListMarker(checked) => self.text.prefix = if checked { "☑" } else { "☐" }.into(),
            Event::InlineHtml(html) if html.eq_ignore_ascii_case("<u>") => self.underline = (self.underline + 1).min(MAX_DEPTH),
            Event::InlineHtml(html) if html.eq_ignore_ascii_case("</u>") => self.underline = self.underline.saturating_sub(1),
            Event::Html(html) | Event::InlineHtml(html) => self.push(&html, false),
            _ => {}
        }
    }
}

impl Document {
    pub(crate) fn parse(source: &str) -> Self {
        let source = crate::prefix(source, crate::ANSWER_BYTES);
        let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
        let mut builder = Builder::default();
        let mut depth = 0usize;
        for (count, event) in Parser::new_ext(source, options).enumerate() {
            match &event {
                Event::Start(_) => depth = depth.saturating_add(1),
                Event::End(_) => depth = depth.saturating_sub(1),
                _ => {}
            }
            if count >= MAX_EVENTS || depth > MAX_DEPTH || builder.doc.blocks.len() >= MAX_BLOCKS || builder.cell_count >= MAX_TABLE_CELLS {
                return Self {
                    blocks: vec![Block::Text(Text { spans: vec![Span { text: source.into(), style: Style::default() }], ..Default::default() })],
                };
            }
            builder.event(event);
        }
        builder.flush();
        builder.doc
    }

    pub(crate) fn show(&self, ui: &mut egui::Ui) {
        for (index, block) in self.blocks.iter().enumerate() {
            match block {
                Block::Rule => {
                    ui.separator();
                }
                Block::Text(text) => {
                    if matches!(text.role, Role::Heading(_)) {
                        ui.add_space(5.0);
                    }
                    let show = |ui: &mut egui::Ui| {
                        ui.horizontal_top(|ui| {
                            ui.add_space((text.indent.min(MAX_DEPTH) * 16) as f32);
                            if text.quote {
                                ui.colored_label(ui.visuals().selection.stroke.color, "│");
                            }
                            if !text.prefix.is_empty() {
                                ui.label(&text.prefix);
                            }
                            let job = layout(ui.style(), &text.spans, text.role, false);
                            ui.add(egui::Label::new(job).selectable(true).wrap());
                        });
                    };
                    if text.role == Role::Code {
                        egui::Frame::new().fill(ui.visuals().code_bg_color).corner_radius(6).inner_margin(8).show(ui, show);
                    } else {
                        show(ui);
                    }
                    ui.add_space(4.0);
                }
                Block::Table(rows) => {
                    let columns = rows.iter().map(Vec::len).max().unwrap_or(1).clamp(1, 64);
                    let width = (ui.available_width() / columns as f32).clamp(80.0, 240.0);
                    egui::ScrollArea::horizontal().id_salt(("ai-answer-table", index)).show(ui, |ui| {
                        egui::Grid::new(("ai-answer-grid", index)).num_columns(columns).min_col_width(80.0).max_col_width(width).striped(true).show(
                            ui,
                            |ui| {
                                for (row_index, row) in rows.iter().enumerate() {
                                    for cell in row {
                                        ui.add(egui::Label::new(layout(ui.style(), cell, Role::Body, row_index == 0)).selectable(true).wrap());
                                    }
                                    ui.end_row();
                                }
                            },
                        );
                    });
                    ui.add_space(6.0);
                }
            }
        }
    }
}

fn layout(style: &egui::Style, spans: &[Span], role: Role, header: bool) -> LayoutJob {
    let body = TextStyle::Body.resolve(style);
    let heading = TextStyle::Heading.resolve(style);
    let scale = match role {
        Role::Heading(1) => 1.55,
        Role::Heading(2) => 1.35,
        Role::Heading(3) => 1.18,
        _ => 1.0,
    };
    let mut job = LayoutJob::default();
    for span in spans {
        let bold = span.style.bold || header || matches!(role, Role::Heading(_));
        let font = if span.style.code {
            TextStyle::Monospace.resolve(style)
        } else {
            FontId::new(body.size * scale, if bold { heading.family.clone() } else { body.family.clone() })
        };
        let mut text = RichText::new(&span.text).font(font);
        if bold {
            text = text.strong();
        }
        if span.style.italic {
            text = text.italics();
        }
        if span.style.underline {
            text = text.underline();
        }
        if span.style.strike {
            text = text.strikethrough();
        }
        if span.style.code && role != Role::Code {
            text = text.background_color(style.visuals.code_bg_color);
        }
        text.append_to(&mut job, style, egui::FontSelection::Default, egui::Align::Center);
    }
    job
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visible(doc: &Document) -> String {
        let mut result = String::new();
        for block in &doc.blocks {
            match block {
                Block::Text(text) => {
                    result.push_str(&text.prefix);
                    for span in &text.spans {
                        result.push_str(&span.text);
                    }
                    result.push('\n');
                }
                Block::Table(rows) => {
                    for row in rows {
                        for cell in row {
                            for span in cell {
                                result.push_str(&span.text);
                            }
                            result.push('\t');
                        }
                        result.push('\n');
                    }
                }
                Block::Rule => result.push_str("---\n"),
            }
        }
        result
    }

    #[test]
    fn markdown_answer_formats_emphasis_and_keeps_unicode_and_citations() {
        let doc = Document::parse(
            "# Result ✅\n\n**Bold** and *italic* with <u>underline</u>, ~~old~~ and `x * y`. [S1] 😊\n\n- First\n  - Nested\n\n3. Third\n",
        );
        let text = visible(&doc);
        assert!(text.contains("Result ✅") && text.contains("Bold and italic with underline, old and x * y. [S1] 😊"));
        assert!(!text.contains("**") && !text.contains("<u>") && text.contains("•Nested") && text.contains("3.Third"));
        let Block::Text(body) = &doc.blocks[1] else { panic!("body paragraph") };
        let mut style = egui::Style::default();
        style.text_styles.insert(TextStyle::Heading, FontId::new(20.0, egui::FontFamily::Name("real-bold-face".into())));
        let job = layout(&style, &body.spans, Role::Body, false);
        assert!(job.sections.iter().any(|s| s.format.font_id.family == egui::FontFamily::Name("real-bold-face".into())));
        assert!(job.sections.iter().any(|s| s.format.italics));
        assert!(job.sections.iter().any(|s| !s.format.underline.is_empty()));
        assert!(job.sections.iter().any(|s| !s.format.strikethrough.is_empty()));
    }

    #[test]
    fn model_resources_and_html_stay_inert_and_code_stays_literal() {
        let source = "[Website](https://example.invalid/private) ![Secret](file:///C:/private.png)\n\n<script>alert('x')</script>\n\n```html\n**literal** <u>code</u>\n```\n\n\\*escaped\\*";
        let text = visible(&Document::parse(source));
        assert!(text.contains("Website") && text.contains("[Image: Secret]") && text.contains("<script>alert('x')</script>"));
        assert!(text.contains("**literal** <u>code</u>") && text.contains("*escaped*"));
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            Document::parse(source).show(ui);
        });
        // This headless check inspects commands without uploading font textures.
        output.textures_delta.clear();
        assert!(output.platform_output.commands.is_empty(), "No links, clipboard actions or host commands from model text");
        for viewport in output.viewport_output.values() {
            assert!(
                viewport.commands.iter().all(|command| matches!(command, egui::ViewportCommand::SetTheme(_))),
                "Unexpected viewport command: {:?}",
                viewport.commands
            );
        }
    }

    #[test]
    fn tables_task_lists_and_deep_input_have_bounded_native_output() {
        let doc = Document::parse("| Item | Value |\n|---|---|\n| **Budget** | 42 |\n\n- [x] Done\n- [ ] Next");
        assert!(matches!(doc.blocks.first(), Some(Block::Table(rows)) if rows.len()==2));
        let text = visible(&doc);
        assert!(text.contains("Budget\t42") && text.contains("☑Done") && text.contains("☐Next"));
        let nested = format!("{}data", "> ".repeat(500));
        let fallback = Document::parse(&nested);
        assert_eq!(fallback.blocks.len(), 1);
        assert_eq!(visible(&fallback), nested + "\n");
    }
}
