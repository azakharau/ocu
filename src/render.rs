use std::io::{self, Write};

use anyhow::Result;
use ratatui::buffer::{Buffer, Cell as BufferCell};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Widget};

use crate::snapshot::{UsageBucket, UsageSnapshot, WindowUsage};

const VIEW_WIDTH: u16 = 86;
const SECTION_HEIGHT: u16 = CARD_HEIGHT;
const SECTION_GAP: u16 = 2;
const CARD_HEIGHT: u16 = 7;
const LIMIT_ROW_HEIGHT: u16 = 2;
const LEFT_CELL_WIDTH: u16 = 30;
const BAR_CELL_WIDTH: u16 = 36;
const BAR_WIDTH: u16 = 18;
const VALUE_WIDTH: u16 = 9;

const MUTED_FG: Color = Color::Rgb(175, 175, 180);
const BORDER: Color = Color::Rgb(137, 149, 184);
const BAR_EMPTY: Color = Color::Rgb(64, 64, 64);

pub(crate) fn render_snapshot(snapshot: &UsageSnapshot) -> Result<()> {
    let area = Rect::new(0, 0, VIEW_WIDTH, output_height(snapshot));
    let mut buffer = Buffer::empty(area);

    render_buffer(area, &mut buffer, snapshot);
    print_buffer(&buffer)?;
    Ok(())
}

fn render_buffer(area: Rect, buffer: &mut Buffer, snapshot: &UsageSnapshot) {
    let content_area = Rect {
        x: area.x,
        y: area.y,
        width: VIEW_WIDTH.min(area.width),
        height: output_height(snapshot).min(area.height),
    };
    let mut y = content_area.y;

    for (bucket_index, bucket) in snapshot.buckets.iter().enumerate() {
        render_bucket(
            Rect::new(content_area.x, y, content_area.width, SECTION_HEIGHT),
            buffer,
            bucket,
        );
        y = y.saturating_add(SECTION_HEIGHT);
        if bucket_index + 1 < snapshot.buckets.len() {
            y = y.saturating_add(SECTION_GAP);
        }
    }
}

fn render_bucket(area: Rect, buffer: &mut Buffer, bucket: &UsageBucket) {
    let card = Rect::new(area.x, area.y, area.width, CARD_HEIGHT);
    let card_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .title(section_title(bucket.title))
        .title_style(Style::default().add_modifier(Modifier::BOLD));
    let card_inner = card_block.inner(card);
    let [first_row, divider, second_row] = Layout::vertical([
        Constraint::Length(LIMIT_ROW_HEIGHT),
        Constraint::Length(1),
        Constraint::Length(LIMIT_ROW_HEIGHT),
    ])
    .areas(card_inner);

    Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(BORDER))
        .render(
            Rect::new(
                card.x,
                card.y,
                card.width,
                divider.y.saturating_sub(card.y).saturating_add(1),
            ),
            buffer,
        );
    card_block.render(card, buffer);
    draw_limit_row(buffer, first_row, &bucket.windows[0]);
    draw_limit_row(buffer, second_row, &bucket.windows[1]);
}

fn draw_limit_row(buffer: &mut Buffer, row: Rect, window: &WindowUsage) {
    let [title_line, reset_line] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(row);
    let title_cells = line_cells(title_line);
    let reset_cells = line_cells(reset_line);
    let [_bar_left, bar_middle, _bar_right] = Layout::horizontal([
        Constraint::Percentage(25),
        Constraint::Percentage(50),
        Constraint::Percentage(25),
    ])
    .areas(reset_cells.bar);
    let [_value_left, value_middle, _value_right] = Layout::horizontal([
        Constraint::Percentage(25),
        Constraint::Percentage(50),
        Constraint::Percentage(25),
    ])
    .areas(reset_cells.value);
    let remaining = remaining_percent(window.used_percent);
    let value = format!("{remaining:>3}% left");
    let fill_color = remaining_color(remaining);

    Paragraph::new(limit_title(window.label))
        .style(Style::default().add_modifier(Modifier::BOLD))
        .render(title_cells.left, buffer);
    Paragraph::new(format!("Resets {}", window.reset_at_text()))
        .style(Style::default().fg(MUTED_FG))
        .render(reset_cells.left, buffer);
    Paragraph::new(rounded_bar(remaining, BAR_WIDTH, fill_color)).render(
        bar_middle.centered(Constraint::Length(BAR_WIDTH), Constraint::Length(1)),
        buffer,
    );
    Paragraph::new(value)
        .style(Style::default().fg(Color::White))
        .centered()
        .render(
            value_middle.centered(Constraint::Length(VALUE_WIDTH), Constraint::Length(1)),
            buffer,
        );
}

fn rounded_bar(percent: u16, width: u16, fill_color: Color) -> Line<'static> {
    let inner_width = width.saturating_sub(2);
    let filled_width = inner_width.saturating_mul(percent).saturating_add(99) / 100;
    let empty_width = inner_width.saturating_sub(filled_width);
    let left_cap = if percent == 0 { BAR_EMPTY } else { fill_color };
    let right_cap = if percent == 100 {
        fill_color
    } else {
        BAR_EMPTY
    };

    Line::from(vec![
        Span::styled("", Style::default().fg(left_cap)),
        Span::styled(
            " ".repeat(filled_width as usize),
            Style::default().fg(fill_color).bg(fill_color),
        ),
        Span::styled(
            " ".repeat(empty_width as usize),
            Style::default().fg(BAR_EMPTY).bg(BAR_EMPTY),
        ),
        Span::styled("", Style::default().fg(right_cap)),
    ])
}

fn line_cells(row: Rect) -> RowCells {
    let [left, bar, value] = Layout::horizontal([
        Constraint::Length(LEFT_CELL_WIDTH),
        Constraint::Length(BAR_CELL_WIDTH),
        Constraint::Fill(1),
    ])
    .areas(row);

    RowCells { left, bar, value }
}

#[derive(Debug, Clone, Copy)]
struct RowCells {
    left: Rect,
    bar: Rect,
    value: Rect,
}

fn section_title(title: &'static str) -> &'static str {
    match title {
        "Main Codex bucket" => "General usage limits",
        "Codex 5.3 Spark" => "GPT-5.3-Codex-Spark usage limits",
        _ => title,
    }
}

fn limit_title(label: &'static str) -> &'static str {
    match label {
        "5h" => "5 hour usage limit",
        "weekly" => "Weekly usage limit",
        _ => label,
    }
}

fn print_buffer(buffer: &Buffer) -> Result<()> {
    let area = buffer.area;
    let mut stdout = io::stdout().lock();

    for y in area.y..area.bottom() {
        let mut current_style = CellStyle::default();
        let line_end = line_end(buffer, y);

        for x in area.x..line_end {
            let cell = &buffer[(x, y)];
            let style = CellStyle::from_cell(cell);

            if style != current_style {
                write_ansi_style(&mut stdout, style)?;
                current_style = style;
            }
            write!(stdout, "{}", cell.symbol())?;
        }

        if current_style != CellStyle::default() {
            write!(stdout, "\x1b[0m")?;
        }
        writeln!(stdout)?;
    }

    stdout.flush()?;
    Ok(())
}

fn line_end(buffer: &Buffer, y: u16) -> u16 {
    let area = buffer.area;
    (area.x..area.right())
        .rev()
        .find(|x| !cell_is_blank_default(&buffer[(*x, y)]))
        .map_or(area.x, |x| x + 1)
}

fn cell_is_blank_default(cell: &BufferCell) -> bool {
    cell.symbol() == " " && CellStyle::from_cell(cell) == CellStyle::default()
}

fn remaining_percent(used_percent: u16) -> u16 {
    100u16.saturating_sub(used_percent.min(100))
}

fn remaining_color(remaining_percent: u16) -> Color {
    match remaining_percent {
        0..=10 => Color::Rgb(243, 139, 168),
        11..=30 => Color::Rgb(249, 226, 175),
        _ => Color::Rgb(166, 227, 161),
    }
}

fn output_height(snapshot: &UsageSnapshot) -> u16 {
    let section_count = u16::try_from(snapshot.buckets.len()).unwrap_or(u16::MAX);
    let gap_count = section_count.saturating_sub(1);
    section_count
        .saturating_mul(SECTION_HEIGHT)
        .saturating_add(gap_count.saturating_mul(SECTION_GAP))
        .max(1)
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
struct CellStyle {
    fg: Color,
    bg: Color,
    modifier: Modifier,
}

impl CellStyle {
    fn from_cell(cell: &BufferCell) -> Self {
        Self {
            fg: cell.fg,
            bg: cell.bg,
            modifier: cell.modifier,
        }
    }
}

fn write_ansi_style(writer: &mut impl Write, style: CellStyle) -> io::Result<()> {
    write!(writer, "\x1b[0m")?;

    if style.modifier.contains(Modifier::BOLD) {
        write!(writer, "\x1b[1m")?;
    }
    write_ansi_color(writer, 38, style.fg)?;
    write_ansi_color(writer, 48, style.bg)?;
    Ok(())
}

fn write_ansi_color(writer: &mut impl Write, prefix: u8, color: Color) -> io::Result<()> {
    match color {
        Color::Reset => Ok(()),
        Color::Black => write!(writer, "\x1b[{prefix};5;0m"),
        Color::Red => write!(writer, "\x1b[{prefix};5;1m"),
        Color::Green => write!(writer, "\x1b[{prefix};5;2m"),
        Color::Yellow => write!(writer, "\x1b[{prefix};5;3m"),
        Color::Blue => write!(writer, "\x1b[{prefix};5;4m"),
        Color::Magenta => write!(writer, "\x1b[{prefix};5;5m"),
        Color::Cyan => write!(writer, "\x1b[{prefix};5;6m"),
        Color::Gray => write!(writer, "\x1b[{prefix};5;7m"),
        Color::DarkGray => write!(writer, "\x1b[{prefix};5;8m"),
        Color::LightRed => write!(writer, "\x1b[{prefix};5;9m"),
        Color::LightGreen => write!(writer, "\x1b[{prefix};5;10m"),
        Color::LightYellow => write!(writer, "\x1b[{prefix};5;11m"),
        Color::LightBlue => write!(writer, "\x1b[{prefix};5;12m"),
        Color::LightMagenta => write!(writer, "\x1b[{prefix};5;13m"),
        Color::LightCyan => write!(writer, "\x1b[{prefix};5;14m"),
        Color::White => write!(writer, "\x1b[{prefix};5;15m"),
        Color::Rgb(red, green, blue) => {
            write!(writer, "\x1b[{prefix};2;{red};{green};{blue}m")
        }
        Color::Indexed(index) => write!(writer, "\x1b[{prefix};5;{index}m"),
    }
}
