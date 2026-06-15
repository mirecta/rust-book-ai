// k05_traits.rs — Trait Visualizer: Monomorphization vs Dynamic Dispatch
//
// Cieľové publikum: C/C++ programátori učiaci sa Rust
// Ukazuje:
//   Ľavá kolónka  — impl Trait / Generics → Monomorphization (ako C++ templates)
//   Pravá kolónka — dyn Trait → Dynamic dispatch cez vtable (ako C++ virtual)
//
// Ovládanie:
//   TAB   — prepni zvýraznenú kolónku (ľavá ↔ pravá)
//   SPACE — spusti / pokračuj animáciu
//   Q / ESC — ukončiť

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use std::io::stdout;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Dátový model animácie
// ---------------------------------------------------------------------------

/// Ktorá kolónka je aktívna ("kurzor")
#[derive(PartialEq, Clone, Copy)]
enum Focus {
    Left,
    Right,
}

/// Fázy animácie ľavej kolónky (monomorphization)
#[derive(Clone, Copy, PartialEq)]
enum MonoStep {
    Idle,
    Circle,
    Square,
    Triangle,
    Done,
}

/// Fázy animácie pravej kolónky (vtable lookup)
#[derive(Clone, Copy, PartialEq)]
enum VtableStep {
    Idle,
    DataPtr,
    VtablePtr,
    AreaLookup,
    PerimeterLookup,
    Done,
}

struct App {
    focus: Focus,
    mono_step: MonoStep,
    vtable_step: VtableStep,
    last_tick: Instant,
    /// Blikač — striedame true/false každých ~400 ms pre cursor efekt
    blink: bool,
    last_blink: Instant,
}

impl App {
    fn new() -> Self {
        Self {
            focus: Focus::Left,
            mono_step: MonoStep::Idle,
            vtable_step: VtableStep::Idle,
            last_tick: Instant::now(),
            blink: true,
            last_blink: Instant::now(),
        }
    }

    fn advance_animation(&mut self) {
        match self.focus {
            Focus::Left => {
                self.mono_step = match self.mono_step {
                    MonoStep::Idle => MonoStep::Circle,
                    MonoStep::Circle => MonoStep::Square,
                    MonoStep::Square => MonoStep::Triangle,
                    MonoStep::Triangle => MonoStep::Done,
                    MonoStep::Done => {
                        // Reset po dokončení
                        MonoStep::Idle
                    }
                };
            }
            Focus::Right => {
                self.vtable_step = match self.vtable_step {
                    VtableStep::Idle => VtableStep::DataPtr,
                    VtableStep::DataPtr => VtableStep::VtablePtr,
                    VtableStep::VtablePtr => VtableStep::AreaLookup,
                    VtableStep::AreaLookup => VtableStep::PerimeterLookup,
                    VtableStep::PerimeterLookup => VtableStep::Done,
                    VtableStep::Done => VtableStep::Idle,
                };
            }
        }
        self.last_tick = Instant::now();
    }
}

// ---------------------------------------------------------------------------
// Pomocné funkcie pre štýlovanie
// ---------------------------------------------------------------------------

fn active_border_style(is_focused: bool) -> Style {
    if is_focused {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

// ---------------------------------------------------------------------------
// Renderovanie ľavej kolónky — Monomorphization
// ---------------------------------------------------------------------------

fn render_left(f: &mut ratatui::Frame, area: ratatui::layout::Rect, app: &App) {
    let is_focused = app.focus == Focus::Left;
    let step = app.mono_step;

    // Pomocná funkcia: zvýrazni riadok ak je aktívny krok
    let fn_style = |active_step: MonoStep| -> Style {
        if step == active_step {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else if step == MonoStep::Done {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Gray)
        }
    };

    let status_line = match step {
        MonoStep::Idle => Line::from(Span::styled(
            "  [SPACE] spusti animáciu kompilácie",
            Style::default().fg(Color::DarkGray),
        )),
        MonoStep::Circle => Line::from(vec![
            Span::styled("  ⚙ ", Style::default().fg(Color::Yellow)),
            Span::styled(
                "Kompilujem pre Circle...",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        MonoStep::Square => Line::from(vec![
            Span::styled("  ⚙ ", Style::default().fg(Color::Yellow)),
            Span::styled(
                "Kompilujem pre Square...",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        MonoStep::Triangle => Line::from(vec![
            Span::styled("  ⚙ ", Style::default().fg(Color::Yellow)),
            Span::styled(
                "Kompilujem pre Triangle...",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        MonoStep::Done => Line::from(Span::styled(
            "  Kompilácia hotová — 3 špecializované funkcie",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )),
    };

    let lines: Vec<Line> = vec![
        Line::from(Span::styled(
            "  fn print_area<T: Shape>(s: T) {",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "      println!(\"{}\", s.area());",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(
            "  }",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "      ↓ kompilátor generuje:",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::ITALIC),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                "fn print_area_Circle(s: Circle)",
                fn_style(MonoStep::Circle),
            ),
            if step == MonoStep::Circle {
                Span::styled(
                    "  ← aktívne",
                    Style::default().fg(Color::Yellow),
                )
            } else {
                Span::raw("")
            },
        ]),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                "fn print_area_Square(s: Square)",
                fn_style(MonoStep::Square),
            ),
            if step == MonoStep::Square {
                Span::styled(
                    "  ← aktívne",
                    Style::default().fg(Color::Yellow),
                )
            } else {
                Span::raw("")
            },
        ]),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                "fn print_area_Triangle(s: Triangle)",
                fn_style(MonoStep::Triangle),
            ),
            if step == MonoStep::Triangle {
                Span::styled(
                    "  ← aktívne",
                    Style::default().fg(Color::Yellow),
                )
            } else {
                Span::raw("")
            },
        ]),
        Line::from(""),
        status_line,
        Line::from(""),
        Line::from(Span::styled(
            "  ✓ Zero overhead — priame volanie funkcie",
            Style::default().fg(Color::Green),
        )),
        Line::from(Span::styled(
            "  ✓ Inlining možný (kompilátor vidí konkrétny typ)",
            Style::default().fg(Color::Green),
        )),
        Line::from(Span::styled(
            "  ✓ Ako C++ templates — ale bez header-only problémov",
            Style::default().fg(Color::Green),
        )),
        Line::from(Span::styled(
            "  ✗ Väčší binárny súbor (každý typ = nová kópia)",
            Style::default().fg(Color::Red),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  C++ ekvivalent:  template<typename T> void print_area(T s)",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )),
    ];

    let title_style = active_border_style(is_focused);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(title_style)
        .title(Span::styled(
            " impl Trait / Generics → Monomorphization ",
            title_style,
        ));

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, area);
}

// ---------------------------------------------------------------------------
// Renderovanie pravej kolónky — Dynamic Dispatch / vtable
// ---------------------------------------------------------------------------

fn render_right(f: &mut ratatui::Frame, area: ratatui::layout::Rect, app: &App) {
    let is_focused = app.focus == Focus::Right;
    let step = app.vtable_step;

    // Zvýrazni konkrétny riadok vtable diagramu
    let ptr_style = |active: bool| -> Style {
        if active {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Magenta)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        }
    };

    let data_active = matches!(step, VtableStep::DataPtr);
    let vtable_active = matches!(step, VtableStep::VtablePtr);
    let area_active = matches!(step, VtableStep::AreaLookup);
    let perim_active = matches!(step, VtableStep::PerimeterLookup);

    let status_line = match step {
        VtableStep::Idle => Line::from(Span::styled(
            "  [SPACE] animuj runtime lookup",
            Style::default().fg(Color::DarkGray),
        )),
        VtableStep::DataPtr => Line::from(vec![
            Span::styled("  → ", Style::default().fg(Color::Magenta)),
            Span::styled(
                "Runtime: čítam data pointer...",
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            ),
        ]),
        VtableStep::VtablePtr => Line::from(vec![
            Span::styled("  → ", Style::default().fg(Color::Magenta)),
            Span::styled(
                "Runtime: nasledujem vtable pointer...",
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            ),
        ]),
        VtableStep::AreaLookup => Line::from(vec![
            Span::styled("  → ", Style::default().fg(Color::Magenta)),
            Span::styled(
                "Runtime: hľadám area() vo vtable...",
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            ),
        ]),
        VtableStep::PerimeterLookup => Line::from(vec![
            Span::styled("  → ", Style::default().fg(Color::Magenta)),
            Span::styled(
                "Runtime: hľadám perimeter() vo vtable...",
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            ),
        ]),
        VtableStep::Done => Line::from(Span::styled(
            "  Lookup hotový — 2 indirect cally vykonané",
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )),
    };

    let lines: Vec<Line> = vec![
        Line::from(Span::styled(
            "  fn print_area(s: &dyn Shape) {",
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "      println!(\"{}\", s.area());",
            Style::default().fg(Color::Magenta),
        )),
        Line::from(Span::styled(
            "  }",
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  dyn Shape — fat pointer (2 × usize):",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "┌─────────────────┐",
                Style::default().fg(Color::Gray),
            ),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("│ ", Style::default().fg(Color::Gray)),
            Span::styled("data ptr   ", ptr_style(data_active)),
            Span::styled(
                if data_active { "──→ Circle { r: 5.0 } ←" } else { "──→ Circle { r: 5.0 }  " },
                if data_active {
                    Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                },
            ),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("│ ", Style::default().fg(Color::Gray)),
            Span::styled("vtable ptr ", ptr_style(vtable_active)),
            Span::styled(
                "──→ ┌────────────────────┐",
                if vtable_active {
                    Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                },
            ),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "└─────────────────┘    │ ",
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                "drop_in_place    │",
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(vec![
            Span::raw("                       │ "),
            Span::styled("size  = 16       │", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("                       │ "),
            Span::styled("align = 8        │", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw("                       │ "),
            Span::styled("area()    ", ptr_style(area_active)),
            Span::styled(
                if area_active { "──→ Circle::area  ←" } else { "──→ Circle::area   " },
                if area_active {
                    Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                },
            ),
        ]),
        Line::from(vec![
            Span::raw("                       │ "),
            Span::styled("perimeter()", ptr_style(perim_active)),
            Span::styled(
                if perim_active { "──→ Circle::perimeter ←" } else { "──→ Circle::perimeter  " },
                if perim_active {
                    Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                },
            ),
        ]),
        Line::from(vec![
            Span::raw("                       "),
            Span::styled("└────────────────────┘", Style::default().fg(Color::Gray)),
        ]),
        Line::from(""),
        status_line,
        Line::from(""),
        Line::from(Span::styled(
            "  ✓ Menší binárny súbor (jedna funkcia pre všetky typy)",
            Style::default().fg(Color::Green),
        )),
        Line::from(Span::styled(
            "  ✓ Heterogénna kolekcia: Vec<Box<dyn Shape>>",
            Style::default().fg(Color::Green),
        )),
        Line::from(Span::styled(
            "  ✗ Indirect call — možný cache miss",
            Style::default().fg(Color::Red),
        )),
        Line::from(Span::styled(
            "  ✗ Inlining nemožný (typ neznámy v čase kompilácie)",
            Style::default().fg(Color::Red),
        )),
        Line::from(Span::styled(
            "  C++ ekvivalent: virtual funkcie + vptr v každom objekte",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )),
    ];

    let title_style = active_border_style(is_focused);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(title_style)
        .title(Span::styled(
            " dyn Trait → Dynamic Dispatch (vtable) ",
            title_style,
        ));

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, area);
}

// ---------------------------------------------------------------------------
// Renderovanie celého UI
// ---------------------------------------------------------------------------

fn ui(f: &mut ratatui::Frame, app: &App) {
    let area = f.area();

    // Vertikálny layout: titulok | hlavný obsah | footer
    let outer_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(area);

    // ── Titulok ─────────────────────────────────────────────────────────────
    let title_text = vec![Line::from(vec![
        Span::styled("Rust Traits: ", Style::default().fg(Color::White)),
        Span::styled(
            "Monomorphization",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" vs ", Style::default().fg(Color::Gray)),
        Span::styled(
            "Dynamic Dispatch",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " — Kapitola 5",
            Style::default().fg(Color::DarkGray),
        ),
    ])];

    let title_widget = Paragraph::new(title_text)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default());
    f.render_widget(title_widget, outer_chunks[0]);

    // ── Dve kolónky vedľa seba ──────────────────────────────────────────────
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(outer_chunks[1]);

    render_left(f, columns[0], app);
    render_right(f, columns[1], app);

    // ── Footer — klávesové skratky ─────────────────────────────────────────
    let focused_label = match app.focus {
        Focus::Left => "ľavá (Mono)",
        Focus::Right => "pravá (vtable)",
    };

    let footer_line = Line::from(vec![
        Span::styled(" TAB ", Style::default().fg(Color::Black).bg(Color::Yellow)),
        Span::styled(
            format!(" prepni kolónku (aktívna: {})   ", focused_label),
            Style::default().fg(Color::Gray),
        ),
        Span::styled(" SPACE ", Style::default().fg(Color::Black).bg(Color::Cyan)),
        Span::styled(" ďalší krok animácie   ", Style::default().fg(Color::Gray)),
        Span::styled(" Q / ESC ", Style::default().fg(Color::Black).bg(Color::Red)),
        Span::styled(" ukončiť", Style::default().fg(Color::Gray)),
    ]);

    let footer = Paragraph::new(footer_line)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, outer_chunks[2]);
}

// ---------------------------------------------------------------------------
// Hlavná slučka
// ---------------------------------------------------------------------------

fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    // Tick interval — 200 ms pre blikanie
    const TICK: Duration = Duration::from_millis(200);

    loop {
        terminal.draw(|f| ui(f, &app))?;

        // Aktualizuj blikač
        if app.last_blink.elapsed() >= Duration::from_millis(400) {
            app.blink = !app.blink;
            app.last_blink = Instant::now();
        }

        if event::poll(TICK)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    // Ukončiť
                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => break,

                    // TAB — prepni fokus
                    KeyCode::Tab => {
                        app.focus = match app.focus {
                            Focus::Left => Focus::Right,
                            Focus::Right => Focus::Left,
                        };
                        // Reset animácie pri prepnutí
                        app.mono_step = MonoStep::Idle;
                        app.vtable_step = VtableStep::Idle;
                    }

                    // SPACE — ďalší krok animácie
                    KeyCode::Char(' ') => {
                        app.advance_animation();
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
