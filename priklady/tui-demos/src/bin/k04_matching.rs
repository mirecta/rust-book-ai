// k04_matching.rs — Interaktívny Pattern Match Visualizer
//
// Cieľové publikum: C/C++ programátori učiaci sa Rust
// Ukazuje: match expression s guard podmienkami, Result<T,E> typ
//
// Ovládanie:
//   Písanie  — zadaj číslo (napr. 7) alebo "err:správa" (napr. err:boom)
//   Enter    — vyhodnoť match a zvýrazni príslušný arm
//   Backspace — vymaž posledný znak
//   ESC / q  — ukončiť

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
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};
use std::io::stdout;

// ---------------------------------------------------------------------------
// Dátový model
// ---------------------------------------------------------------------------

/// Jeden arm match výrazu — text kódu + index (0-based)
struct MatchArm {
    code: &'static str,
    description: &'static str,
}

const ARMS: &[MatchArm] = &[
    MatchArm {
        code: "Ok(n) if n < 0  => ...",
        description: "záporné celé číslo (napr. -3)",
    },
    MatchArm {
        code: "Ok(0)           => ...",
        description: "presne nula",
    },
    MatchArm {
        code: "Ok(n) if n < 10 => ...",
        description: "malé kladné (1 – 9)",
    },
    MatchArm {
        code: "Ok(n)           => ...",
        description: "veľké kladné (≥ 10)",
    },
    MatchArm {
        code: "Err(e)          => ...",
        description: "chybová hodnota",
    },
];

/// Výsledok vyhodnotenia match
#[derive(Clone)]
struct MatchResult {
    matched_arm: usize,
    explanation: String,
}

// ---------------------------------------------------------------------------
// Logika vyhodnotenia (napodobuje match expression v Ruste)
// ---------------------------------------------------------------------------

fn evaluate(input: &str) -> Option<MatchResult> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Formát "err:správa" → Err("správa")
    if let Some(msg) = trimmed.strip_prefix("err:") {
        return Some(MatchResult {
            matched_arm: 4,
            explanation: format!(
                "✓ Arm 5 matchol: Err(e) — e = \"{}\"\n\
                 Rust skontroloval ramená 1–4, žiadne nepasovalo.\n\
                 Err variant zachytí každú chybovú hodnotu.",
                msg
            ),
        });
    }

    // Pokus o parsovanie čísla
    match trimmed.parse::<i32>() {
        Ok(n) if n < 0 => Some(MatchResult {
            matched_arm: 0,
            explanation: format!(
                "✓ Arm 1 matchol: Ok(n) if n < 0\n\
                 n = {} spĺňa guard podmienku n < 0.\n\
                 Guard (if n < 0) je extra podmienka za vzorom.",
                n
            ),
        }),
        Ok(0) => Some(MatchResult {
            matched_arm: 1,
            explanation: "✓ Arm 2 matchol: Ok(0)\n\
                 Literálny vzor — pasuje iba ak n == 0.\n\
                 Rust porovnáva vzory zhora nadol, guard z arm 1 neprešiel."
                .to_string(),
        }),
        Ok(n) if n < 10 => Some(MatchResult {
            matched_arm: 2,
            explanation: format!(
                "✓ Arm 3 matchol: Ok(n) if n < 10\n\
                 n = {} spĺňa podmienku n < 10 (a n > 0 z predchádzajúcich armen).\n\
                 Arms 1 a 2 zlyhali: {} nie je záporné ani nula.",
                n, n
            ),
        }),
        Ok(n) => Some(MatchResult {
            matched_arm: 3,
            explanation: format!(
                "✓ Arm 4 matchol: Ok(n) — catch-all pre Ok\n\
                 n = {} ≥ 10, žiaden predchádzajúci arm s guardom neprešiel.\n\
                 Tento arm bez guardu zachytí všetky zostávajúce Ok hodnoty.",
                n
            ),
        }),
        Err(_) => None, // Nevalidný vstup — nič nevyhodnocujeme
    }
}

// ---------------------------------------------------------------------------
// Stav aplikácie
// ---------------------------------------------------------------------------

struct App {
    input: String,
    result: Option<MatchResult>,
    list_state: ListState,
}

impl App {
    fn new() -> Self {
        Self {
            input: String::new(),
            result: None,
            list_state: ListState::default(),
        }
    }

    fn submit(&mut self) {
        self.result = evaluate(&self.input);
        if let Some(ref r) = self.result {
            self.list_state.select(Some(r.matched_arm));
        } else {
            self.list_state.select(None);
        }
    }
}

// ---------------------------------------------------------------------------
// Renderovanie
// ---------------------------------------------------------------------------

fn ui(f: &mut ratatui::Frame, app: &App) {
    let area = f.area();

    // Vertikálny layout: kód (40%) | vstup (20%) | vysvetlenie (40%)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(20),
            Constraint::Percentage(40),
        ])
        .split(area);

    // ── Horný panel: match arms ako List ──────────────────────────────────
    let matched_arm = app.result.as_ref().map(|r| r.matched_arm);

    let items: Vec<ListItem> = ARMS
        .iter()
        .enumerate()
        .map(|(i, arm)| {
            let is_matched = matched_arm == Some(i);
            let is_before_match = matched_arm.map(|m| i < m).unwrap_or(false);

            let arrow = if is_matched { "→ " } else { "  " };

            let (code_style, desc_style) = if is_matched {
                // Zvýraznený arm — zelená + tučné
                (
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                    Style::default().fg(Color::Green),
                )
            } else if is_before_match {
                // Arms pred matchnutým — stmavené (signalizujú "vyhodnotené a preskočené")
                (
                    Style::default().fg(Color::DarkGray),
                    Style::default().fg(Color::DarkGray),
                )
            } else {
                // Ostatné arms — normálna farba
                (
                    Style::default().fg(Color::Cyan),
                    Style::default().fg(Color::Gray),
                )
            };

            let arm_num = format!("  {}  ", i + 1);
            let line = Line::from(vec![
                Span::styled(arm_num, Style::default().fg(Color::DarkGray)),
                Span::styled(arrow, Style::default().fg(Color::Yellow)),
                Span::styled(arm.code, code_style),
                Span::raw("   "),
                Span::styled(format!("// {}", arm.description), desc_style),
            ]);
            ListItem::new(line)
        })
        .collect();

    let match_block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " match value { ... } — Pattern Matching v Ruste ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));

    // Použijeme List s highlight — ListState udržuje vybraný riadok
    let list = List::new(items)
        .block(match_block)
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    // render_stateful_widget potrebuje mutable referenciu na state
    let mut state = app.list_state.clone();
    f.render_stateful_widget(list, chunks[0], &mut state);

    // ── Stredný panel: vstupné pole ────────────────────────────────────────
    let hint = if app.input.is_empty() {
        " zadaj číslo (napr. 7, -3, 0, 42) alebo err:správa "
    } else {
        ""
    };

    let input_display = format!("  > {}{}█", app.input, hint);
    let input_style = if app.result.is_none() && !app.input.is_empty() {
        Style::default().fg(Color::Red) // nevalidný vstup
    } else {
        Style::default().fg(Color::White)
    };

    let input_widget = Paragraph::new(Line::from(vec![Span::styled(input_display, input_style)]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(
                    " Vstup — stlač Enter pre vyhodnotenie ",
                    Style::default().fg(Color::Blue),
                )),
        );
    f.render_widget(input_widget, chunks[1]);

    // ── Dolný panel: vysvetlenie ───────────────────────────────────────────
    let explanation_text = if let Some(ref res) = app.result {
        let lines: Vec<Line> = res
            .explanation
            .lines()
            .enumerate()
            .map(|(i, line)| {
                let style = if i == 0 {
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                };
                Line::from(Span::styled(format!("  {}", line), style))
            })
            .collect();
        lines
    } else if app.input.is_empty() {
        vec![
            Line::from(Span::styled(
                "  Napíš hodnotu do vstupného poľa a stlač Enter.",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Príklady: 7   -3   0   42   err:nieco",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  V C++ by si použil if/else if reťaz.",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                "  Rust match je exhaustívny — kompilátor ti nedovolí",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                "  zabudnúť na žiadny prípad.",
                Style::default().fg(Color::DarkGray),
            )),
        ]
    } else {
        vec![Line::from(Span::styled(
            "  ⚠ Nevalidný vstup — zadaj celé číslo alebo err:správa",
            Style::default().fg(Color::Red),
        ))]
    };

    let explanation_widget = Paragraph::new(explanation_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(
                " Vysvetlenie — prečo tento arm? ",
                Style::default().fg(Color::Magenta),
            )),
    );
    f.render_widget(explanation_widget, chunks[2]);
}

// ---------------------------------------------------------------------------
// Hlavná slučka
// ---------------------------------------------------------------------------

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Inicializácia terminálu (crossterm raw mode + alternate screen)
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    loop {
        terminal.draw(|f| ui(f, &app))?;

        // Čítame udalosti — blokujeme max 200 ms
        if event::poll(std::time::Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    // Ukončiť
                    KeyCode::Esc | KeyCode::Char('q') => break,

                    // Enter — vyhodnoť match
                    KeyCode::Enter => {
                        app.submit();
                    }

                    // Backspace — vymaž znak
                    KeyCode::Backspace => {
                        app.input.pop();
                        // Reset výsledku pri editácii
                        app.result = None;
                        app.list_state.select(None);
                    }

                    // Bežný znak — pridaj do bufferu
                    KeyCode::Char(c) => {
                        app.input.push(c);
                        // Reset výsledku pri editácii
                        app.result = None;
                        app.list_state.select(None);
                    }

                    _ => {}
                }
            }
        }
    }

    // Cleanup — vráť terminál do normálneho stavu
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;

    Ok(())
}
