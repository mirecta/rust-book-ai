/// K02 — Ownership Visualizer
///
/// TUI demo pre C/C++ programátorov učiacich sa Rust.
/// Vizualizuje ownership a move semantics krok po kroku.
///
/// Ovládanie: SPACE = ďalší krok, Q / ESC = ukončiť
use std::io::stdout;

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

// ---------------------------------------------------------------------------
// Dátový model
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum ColState {
    Stack,
    Dropped,
}

struct Var {
    name: &'static str,
    details: &'static str, // ptr / len / cap riadok
    col: ColState,
}

struct AppState {
    step: usize,
    max_steps: usize,
}

impl AppState {
    fn new() -> Self {
        Self { step: 0, max_steps: 4 }
    }

    fn next_step(&mut self) {
        if self.step < self.max_steps {
            self.step += 1;
        }
    }

    /// Vráti zoznam premenných pre aktuálny krok.
    fn vars(&self) -> Vec<Var> {
        match self.step {
            // Krok 0: prázdny stav
            0 => vec![],

            // Krok 1: let s1 = String::from("hello")
            1 => vec![Var {
                name: "s1",
                details: "  ptr=0x7f4a  len=5  cap=8",
                col: ColState::Stack,
            }],

            // Krok 2: let s2 = s1  →  s1 sa presunie do DROPPED
            2 => vec![
                Var {
                    name: "s1",
                    details: "  [presunutá — moved]",
                    col: ColState::Dropped,
                },
                Var {
                    name: "s2",
                    details: "  ptr=0x7f4a  len=5  cap=8",
                    col: ColState::Stack,
                },
            ],

            // Krok 3: pokus o s1.len() — chyba kompilácie
            3 => vec![
                Var {
                    name: "s1",
                    details: "  [USE AFTER MOVE!]",
                    col: ColState::Dropped,
                },
                Var {
                    name: "s2",
                    details: "  ptr=0x7f4a  len=5  cap=8",
                    col: ColState::Stack,
                },
            ],

            // Krok 4: drop(s2)
            _ => vec![
                Var {
                    name: "s1",
                    details: "  [presunutá — moved]",
                    col: ColState::Dropped,
                },
                Var {
                    name: "s2",
                    details: "  [uvoľnená — dropped]",
                    col: ColState::Dropped,
                },
            ],
        }
    }

    fn heap_items(&self) -> Vec<(&'static str, bool)> {
        // (text, is_dropped)
        match self.step {
            1 | 2 | 3 => vec![
                ("0x7f4a: 'h'", false),
                ("0x7f4b: 'e'", false),
                ("0x7f4c: 'l'", false),
                ("0x7f4d: 'l'", false),
                ("0x7f4e: 'o'", false),
                ("0x7f4f: '\\0'", false),
                ("0x7f50: [pad]", false),
                ("0x7f51: [pad]", false),
            ],
            4 => vec![
                ("0x7f4a: 'h'", true),
                ("0x7f4b: 'e'", true),
                ("0x7f4c: 'l'", true),
                ("0x7f4d: 'l'", true),
                ("0x7f4e: 'o'", true),
                ("0x7f4f: '\\0'", true),
                ("0x7f50: [pad]", true),
                ("0x7f51: [pad]", true),
            ],
            _ => vec![],
        }
    }

    fn step_code(&self) -> &'static str {
        match self.step {
            0 => "// Začíname — zásobník a halda sú prázdne",
            1 => "let s1 = String::from(\"hello\");",
            2 => "let s2 = s1;   // s1 je MOVED do s2",
            3 => "// s1.len()   <-- CHYBA: value used here after move",
            4 => "drop(s2);   // explicitný drop (alebo koniec scope)",
            _ => "",
        }
    }

    fn step_desc(&self) -> &'static str {
        match self.step {
            0 => "Zásobník (stack) uchováva lokálne premenné s pevnou veľkosťou. \
                  Halda (heap) uchováva dynamické dáta.",
            1 => "String::from() alokuje pamäť na halde. Na zásobníku vznikne \"fat pointer\": \
                  ptr (adresa), len (dĺžka), cap (kapacita).",
            2 => "Move semantics: hodnotový typ sa PRESUNIE. s1 stráca vlastníctvo. \
                  V C++ by sa tu vytvorila KÓPIA — Rust to robí efektívnejšie.",
            3 => "Rust ODMIETNE skompilovať tento kód. Žiadny runtime crash — \
                  chyba je odhalená počas kompilácie. V C++ by to bol undefined behaviour.",
            4 => "drop() uvoľní heap pamäť. Rust to robí deterministicky \
                  (na rozdiel od GC). Žiadny double-free, pretože vlastník je jediný.",
            _ => "",
        }
    }

    fn step_title(&self) -> &'static str {
        match self.step {
            0 => "Krok 0/4 — Počiatočný stav",
            1 => "Krok 1/4 — Alokácia na halde",
            2 => "Krok 2/4 — Move semantics",
            3 => "Krok 3/4 — Chyba kompilácie (use after move)",
            4 => "Krok 4/4 — Drop a uvoľnenie pamäte",
            _ => "",
        }
    }
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

fn draw(f: &mut Frame, state: &AppState) {
    let area = f.area();

    // Vonkajší layout: header | main | footer
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(0),    // main
            Constraint::Length(5), // footer
        ])
        .split(area);

    // --- HEADER ---
    let header_text = Text::from(vec![Line::from(vec![
        Span::styled(
            "  Ownership Visualizer — K02  ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
        Span::styled("SPACE", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" = ďalší krok    "),
        Span::styled("Q / ESC", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" = ukončiť"),
    ])]);
    let header =
        Paragraph::new(header_text).block(Block::default().borders(Borders::ALL).title(
            Span::styled(state.step_title(), Style::default().fg(Color::Cyan)),
        ));
    f.render_widget(header, outer[0]);

    // --- MAIN: 3 stĺpce ---
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(35), // STACK
            Constraint::Percentage(35), // HEAP
            Constraint::Percentage(30), // DROPPED
        ])
        .split(outer[1]);

    render_stack(f, state, cols[0]);
    render_heap(f, state, cols[1]);
    render_dropped(f, state, cols[2]);

    // --- FOOTER ---
    render_footer(f, state, outer[2]);
}

fn render_stack(f: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {
    let vars = state.vars();
    let stack_vars: Vec<&Var> = vars.iter().filter(|v| v.col == ColState::Stack).collect();

    let mut items: Vec<ListItem> = vec![
        ListItem::new(Line::from(Span::styled(
            "  [zásobník — stack]",
            Style::default().fg(Color::DarkGray),
        ))),
        ListItem::new(Line::from(Span::raw(""))),
    ];

    for v in &stack_vars {
        items.push(ListItem::new(Line::from(vec![
            Span::styled(
                format!("  {} ", v.name),
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ),
            Span::styled("┐", Style::default().fg(Color::Green)),
        ])));
        items.push(ListItem::new(Line::from(Span::styled(
            v.details,
            Style::default().fg(Color::Green),
        ))));
        items.push(ListItem::new(Line::from(Span::styled(
            "  └─────────────────",
            Style::default().fg(Color::DarkGray),
        ))));
        items.push(ListItem::new(Line::from(Span::raw(""))));
    }

    // Šípka smerom na haldu
    if !stack_vars.is_empty() && state.step < 4 {
        items.push(ListItem::new(Line::from(Span::styled(
            "    ptr ──────────────►",
            Style::default().fg(Color::Cyan),
        ))));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled("  STACK  ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)));
    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn render_heap(f: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {
    let heap = state.heap_items();

    let mut items: Vec<ListItem> = vec![
        ListItem::new(Line::from(Span::styled(
            "  [halda — heap]",
            Style::default().fg(Color::DarkGray),
        ))),
        ListItem::new(Line::from(Span::raw(""))),
    ];

    for (text, dropped) in &heap {
        let style = if *dropped {
            Style::default().fg(Color::Red).add_modifier(Modifier::DIM)
        } else {
            Style::default().fg(Color::Blue)
        };
        items.push(ListItem::new(Line::from(Span::styled(
            format!("  {text}"),
            style,
        ))));
    }

    if heap.is_empty() {
        items.push(ListItem::new(Line::from(Span::styled(
            "  (prázdna)",
            Style::default().fg(Color::DarkGray),
        ))));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled("  HEAP  ", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)));
    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn render_dropped(f: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {
    let vars = state.vars();
    let dropped_vars: Vec<&Var> = vars.iter().filter(|v| v.col == ColState::Dropped).collect();

    let blink_style = if state.step == 3 {
        Style::default()
            .fg(Color::Red)
            .add_modifier(Modifier::BOLD)
            .add_modifier(Modifier::RAPID_BLINK)
    } else {
        Style::default().fg(Color::Red)
    };

    let mut items: Vec<ListItem> = vec![
        ListItem::new(Line::from(Span::styled(
            "  [nedostupné / dropped]",
            Style::default().fg(Color::DarkGray),
        ))),
        ListItem::new(Line::from(Span::raw(""))),
    ];

    for v in &dropped_vars {
        items.push(ListItem::new(Line::from(vec![
            Span::styled(format!("  {} ", v.name), blink_style),
            Span::styled("✗", blink_style),
        ])));
        items.push(ListItem::new(Line::from(Span::styled(
            v.details,
            Style::default().fg(Color::Red).add_modifier(Modifier::DIM),
        ))));
        items.push(ListItem::new(Line::from(Span::raw(""))));
    }

    if dropped_vars.is_empty() {
        items.push(ListItem::new(Line::from(Span::styled(
            "  (nič)",
            Style::default().fg(Color::DarkGray),
        ))));
    }

    let title_style = if state.step == 3 {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD).add_modifier(Modifier::RAPID_BLINK)
    } else {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled("  DROPPED  ", title_style));
    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn render_footer(f: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {
    let code_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::White);

    let final_note = if state.step == state.max_steps {
        Line::from(Span::styled(
            "  ✓ Rust zaručuje: žiadny double-free, žiadny use-after-move, žiadny memory leak",
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        ))
    } else {
        Line::from(Span::raw(""))
    };

    let text = Text::from(vec![
        Line::from(vec![
            Span::raw("  "),
            Span::styled("Kód: ", Style::default().fg(Color::DarkGray)),
            Span::styled(state.step_code(), code_style),
        ]),
        Line::from(Span::raw("")),
        Line::from(Span::styled(
            format!("  {}", state.step_desc()),
            desc_style,
        )),
        final_note,
    ]);

    let para = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(Span::styled(
            "  Vysvetlenie  ",
            Style::default().fg(Color::White),
        )))
        .wrap(ratatui::widgets::Wrap { trim: false });
    f.render_widget(para, area);
}

// ---------------------------------------------------------------------------
// Hlavný cyklus
// ---------------------------------------------------------------------------

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Inicializácia terminálu
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
                    KeyCode::Char(' ') => {
                        state.next_step();
                    }
                    _ => {}
                }
            }
        }
    }

    // Cleanup
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;
    Ok(())
}
