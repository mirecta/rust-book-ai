/// K03 — Type System Visualizer
///
/// TUI demo pre C/C++ programátorov učiacich sa Rust.
/// Ľavá polovica: tabuľka primitívnych typov.
/// Pravá polovica: pamäťový diagram Tagged Union (enum IpAddr).
///
/// Ovládanie: TAB = prepnúť variant V4/V6, Q / ESC = ukončiť
use std::io::stdout;

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

// ---------------------------------------------------------------------------
// Dátový model
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum EnumVariant {
    V4,
    V6,
}

struct AppState {
    variant: EnumVariant,
}

impl AppState {
    fn new() -> Self {
        Self { variant: EnumVariant::V4 }
    }

    fn toggle(&mut self) {
        self.variant = match self.variant {
            EnumVariant::V4 => EnumVariant::V6,
            EnumVariant::V6 => EnumVariant::V4,
        };
    }
}

// ---------------------------------------------------------------------------
// Tabuľka typov
// ---------------------------------------------------------------------------

struct TypeRow {
    name: &'static str,
    bytes: &'static str,
    min: &'static str,
    max: &'static str,
    group: u8, // 0=unsigned, 1=signed, 2=float, 3=other
}

fn type_table() -> Vec<TypeRow> {
    vec![
        TypeRow { name: "u8",   bytes: "1", min: "0",              max: "255",                    group: 0 },
        TypeRow { name: "u16",  bytes: "2", min: "0",              max: "65 535",                 group: 0 },
        TypeRow { name: "u32",  bytes: "4", min: "0",              max: "4 294 967 295",          group: 0 },
        TypeRow { name: "u64",  bytes: "8", min: "0",              max: "1.8e19",                 group: 0 },
        TypeRow { name: "i8",   bytes: "1", min: "-128",           max: "127",                    group: 1 },
        TypeRow { name: "i16",  bytes: "2", min: "-32 768",        max: "32 767",                 group: 1 },
        TypeRow { name: "i32",  bytes: "4", min: "-2 147 483 648", max: "2 147 483 647",          group: 1 },
        TypeRow { name: "i64",  bytes: "8", min: "-9.2e18",        max: "9.2e18",                 group: 1 },
        TypeRow { name: "f32",  bytes: "4", min: "~1.4e-45",       max: "~3.4e38",                group: 2 },
        TypeRow { name: "f64",  bytes: "8", min: "~5e-324",        max: "~1.8e308",               group: 2 },
        TypeRow { name: "bool", bytes: "1", min: "false",          max: "true",                   group: 3 },
        TypeRow { name: "char", bytes: "4", min: "U+0000",         max: "U+10FFFF",               group: 3 },
    ]
}

fn group_color(group: u8) -> Color {
    match group {
        0 => Color::Green,
        1 => Color::Yellow,
        2 => Color::Cyan,
        _ => Color::White,
    }
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

fn draw(f: &mut Frame, state: &AppState) {
    let area = f.area();

    // Vonkajší layout: header | main
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(0),    // main
        ])
        .split(area);

    render_header(f, state, outer[0]);

    // Hlavná plocha: ľavá | pravá
    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(55), // tabuľka typov
            Constraint::Percentage(45), // tagged union
        ])
        .split(outer[1]);

    render_type_table(f, main[0]);
    render_tagged_union(f, state, main[1]);
}

fn render_header(f: &mut Frame, state: &AppState, area: Rect) {
    let variant_str = match state.variant {
        EnumVariant::V4 => "IpAddr::V4(192, 168, 1, 1)",
        EnumVariant::V6 => "IpAddr::V6(\"2001:db8::1\")",
    };

    let text = Text::from(vec![Line::from(vec![
        Span::styled(
            "  Typový systém Rust — K03  ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("   Aktívny variant: "),
        Span::styled(variant_str, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("   "),
        Span::styled("TAB", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" = prepnúť variant    "),
        Span::styled("Q / ESC", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" = ukončiť"),
    ])]);

    let para = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(Span::styled(
            "  Primitívne typy & Tagged Union  ",
            Style::default().fg(Color::Magenta),
        )));
    f.render_widget(para, area);
}

fn render_type_table(f: &mut Frame, area: Rect) {
    let table = type_table();

    // Legenda hlavičky
    let header_style = Style::default().fg(Color::White).add_modifier(Modifier::BOLD).add_modifier(Modifier::UNDERLINED);
    let sep = "─".repeat(50);

    let mut items: Vec<ListItem> = vec![
        ListItem::new(Line::from(Span::styled(
            format!("  {:<6} {:>5}   {:<18} {}", "Typ", "Bajty", "Min", "Max"),
            header_style,
        ))),
        ListItem::new(Line::from(Span::styled(
            format!("  {sep}"),
            Style::default().fg(Color::DarkGray),
        ))),
        // Legenda farieb
        ListItem::new(Line::from(vec![
            Span::raw("  "),
            Span::styled("■ unsigned  ", Style::default().fg(Color::Green)),
            Span::styled("■ signed  ", Style::default().fg(Color::Yellow)),
            Span::styled("■ float  ", Style::default().fg(Color::Cyan)),
            Span::styled("■ iné", Style::default().fg(Color::White)),
        ])),
        ListItem::new(Line::from(Span::styled(
            format!("  {sep}"),
            Style::default().fg(Color::DarkGray),
        ))),
    ];

    for row in &table {
        let color = group_color(row.group);
        let style = Style::default().fg(color);
        items.push(ListItem::new(Line::from(Span::styled(
            format!("  {:<6} {:>5}   {:<18} {}", row.name, row.bytes, row.min, row.max),
            style,
        ))));
    }

    // Poznámky pre C++ programátorov
    items.push(ListItem::new(Line::from(Span::raw(""))));
    items.push(ListItem::new(Line::from(Span::styled(
        "  {sep}",
        Style::default().fg(Color::DarkGray),
    ))));
    items.push(ListItem::new(Line::from(Span::styled(
        "  C++ vs Rust:",
        Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
    ))));
    items.push(ListItem::new(Line::from(Span::styled(
        "  int → i32 (vždy 32b, nie \"aspoň 16b\")",
        Style::default().fg(Color::DarkGray),
    ))));
    items.push(ListItem::new(Line::from(Span::styled(
        "  unsigned int → u32",
        Style::default().fg(Color::DarkGray),
    ))));
    items.push(ListItem::new(Line::from(Span::styled(
        "  long long → i64 / u64",
        Style::default().fg(Color::DarkGray),
    ))));
    items.push(ListItem::new(Line::from(Span::styled(
        "  char (4B Unicode) ≠ C char (1B ASCII)",
        Style::default().fg(Color::DarkGray),
    ))));

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            "  Primitívne typy  ",
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        ));
    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn render_tagged_union(f: &mut Frame, state: &AppState, area: Rect) {
    // Rozdelenie: diagram | popis
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // pamäťový diagram
            Constraint::Length(8), // popis
        ])
        .split(area);

    render_memory_diagram(f, state, inner[0]);
    render_union_desc(f, state, inner[1]);
}

fn render_memory_diagram(f: &mut Frame, state: &AppState, area: Rect) {
    // enum IpAddr má veľkosť ~ 32 B (tag 8B + najväčší variant String = ptr8+len8+cap8 = 24B)
    // Zobrazíme 32 "buniek" po 1 bajte (zjednodušenie pre vizualizáciu)

    // Definujeme farby pre každú bunku (index 0..31)
    // Bajty: [0..7]=tag, [8..31]=payload
    //   V4: tag=1B(červená)+7B(šedá padding), payload: 4×u8(zelená)+20×šedá
    //   V6: tag=1B(červená)+7B(šedá), payload: ptr8(modrá)+len8(modrá)+cap8(modrá)

    let num_cells = 32usize;

    struct Cell {
        label: String,
        color: Color,
    }

    let cells: Vec<Cell> = (0..num_cells)
        .map(|i| match state.variant {
            EnumVariant::V4 => match i {
                0     => Cell { label: "00".to_string(), color: Color::Red },
                1..=7 => Cell { label: "░░".to_string(), color: Color::DarkGray },
                8     => Cell { label: "C0".to_string(), color: Color::Green },
                9     => Cell { label: "A8".to_string(), color: Color::Green },
                10    => Cell { label: "01".to_string(), color: Color::Green },
                11    => Cell { label: "01".to_string(), color: Color::Green },
                _     => Cell { label: "░░".to_string(), color: Color::DarkGray },
            },
            EnumVariant::V6 => match i {
                0     => Cell { label: "01".to_string(), color: Color::Red },
                1..=7 => Cell { label: "░░".to_string(), color: Color::DarkGray },
                8..=15 => Cell {
                    label: format!("p{}", i - 8),
                    color: Color::Blue,
                },
                16..=23 => Cell {
                    label: format!("l{}", i - 16),
                    color: Color::Cyan,
                },
                24..=31 => Cell {
                    label: format!("c{}", i - 24),
                    color: Color::Magenta,
                },
                _ => Cell { label: "░░".to_string(), color: Color::DarkGray },
            },
        })
        .collect();

    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(
            "  Pamäť enum IpAddr (32 bajtov, offset→)",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::raw("")),
    ];

    // Riadok offsetov
    let mut offset_spans: Vec<Span> = vec![Span::raw("  ")];
    for i in 0..num_cells {
        offset_spans.push(Span::styled(
            format!("{:02} ", i),
            Style::default().fg(Color::DarkGray),
        ));
    }
    lines.push(Line::from(offset_spans));

    // Riadok buniek — rozdelíme na 2 riadky po 16
    for chunk_start in [0usize, 16usize] {
        let mut cell_spans: Vec<Span> = vec![Span::raw("  ")];
        for i in chunk_start..(chunk_start + 16).min(num_cells) {
            let c = &cells[i];
            cell_spans.push(Span::styled(
                format!("[{}]", c.label),
                Style::default().fg(c.color).add_modifier(Modifier::BOLD),
            ));
        }
        lines.push(Line::from(cell_spans));
    }

    lines.push(Line::from(Span::raw("")));

    // Legenda pre aktuálny variant
    match state.variant {
        EnumVariant::V4 => {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("[00]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::raw(" = tag (variant discriminant)  "),
                Span::styled("[░░]", Style::default().fg(Color::DarkGray)),
                Span::raw(" = padding"),
            ]));
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("[C0][A8][01][01]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw(" = 4×u8 (192.168.1.1)"),
            ]));
        }
        EnumVariant::V6 => {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("[01]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::raw(" = tag  "),
                Span::styled("[p0..p7]", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
                Span::raw(" = ptr (8B)"),
            ]));
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("[l0..l7]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw(" = len (8B)  "),
                Span::styled("[c0..c7]", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                Span::raw(" = cap (8B)"),
            ]));
        }
    }

    let text = Text::from(lines);
    let para = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(Span::styled(
            "  Pamäťový diagram — Tagged Union  ",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )));
    f.render_widget(para, area);
}

fn render_union_desc(f: &mut Frame, state: &AppState, area: Rect) {
    let enum_def = "enum IpAddr { V4(u8,u8,u8,u8), V6(String) }";

    let (variant_name, variant_detail, comparison) = match state.variant {
        EnumVariant::V4 => (
            "V4(192, 168, 1, 1)",
            "tag(8B) + 4×u8(4B) + padding(20B) = 32 B",
            "C++ union + int tag — ale Rust to robí bezpečne!",
        ),
        EnumVariant::V6 => (
            "V6(\"2001:db8::1\")",
            "tag(8B) + String{ptr,len,cap}(24B) = 32 B",
            "Najväčší variant určuje veľkosť celého enum!",
        ),
    };

    let items: Vec<ListItem> = vec![
        ListItem::new(Line::from(vec![
            Span::raw("  "),
            Span::styled(enum_def, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ])),
        ListItem::new(Line::from(vec![
            Span::raw("  Aktívny: "),
            Span::styled(variant_name, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        ])),
        ListItem::new(Line::from(vec![
            Span::raw("  Rozloženie: "),
            Span::styled(variant_detail, Style::default().fg(Color::Cyan)),
        ])),
        ListItem::new(Line::from(Span::raw(""))),
        ListItem::new(Line::from(vec![
            Span::styled("  Veľkosť enum: ", Style::default().fg(Color::White)),
            Span::styled("32 B", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw("  (= std::mem::size_of::<IpAddr>())"),
        ])),
        ListItem::new(Line::from(Span::styled(
            format!("  {comparison}"),
            Style::default().fg(Color::DarkGray),
        ))),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            "  Enum v pamäti  ",
            Style::default().fg(Color::Yellow),
        ));
    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

// ---------------------------------------------------------------------------
// Hlavný cyklus
// ---------------------------------------------------------------------------

fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut state = AppState::new();
    let mut running = true;

    while running {
        terminal.draw(|f| draw(f, &state))?;

        if event::poll(std::time::Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                        running = false;
                    }
                    KeyCode::Tab => {
                        state.toggle();
                    }
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;
    Ok(())
}
