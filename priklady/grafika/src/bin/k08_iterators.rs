// k08_iterators.rs — Animovaná vizualizácia Iterator pipeline
// Pre C/C++ programátorov: iterátory sú lazy a zero-cost
//
// Pipeline: (0..20).filter(|x| x%2==0).map(|x| x*x).take(5).collect()
//
// SPACE = restart | Q = koniec

use macroquad::prelude::*;

// ── Konštanty layoutu ────────────────────────────────────────────────
const BOX_W: f32 = 148.0;
const BOX_H: f32 = 76.0;
const BOX_GAP: f32 = 52.0;          // medzera medzi boxmi (šípky)
const DOT_R: f32 = 14.0;
const DOT_SPEED: f32 = 160.0;       // px/s pohyb bodky
const SPAWN_INTERVAL: f64 = 0.38;   // sekúnd medzi generovaním čísel

// Počet stĺpcov pipeline: SOURCE | filter | map | take | RESULT
const NUM_STAGES: usize = 5;

// ── Stavy bodky ──────────────────────────────────────────────────────
#[derive(Clone, Debug, PartialEq)]
enum DotState {
    Moving,            // ide doprava
    FilterFlash(f64),  // odmietnutá filtrom — červený flash, potom zmizne
    _Mapped,           // prešla cez map (zmenila farbu + hodnotu)
    TakeFlash(f64),    // odmietnutá take — šedý flash
    Collected,         // v RESULT boxe
}

#[derive(Clone, Debug)]
struct Dot {
    _original: i32,  // pôvodná hodnota zo source (pre debug)
    value: i32,      // aktuálna hodnota (môže byť x²)
    x: f32,
    y: f32,
    // ktorý stage práve obieha (0=source..4=result)
    stage: usize,
    state: DotState,
    color: Color,
    // či bola transformovaná mapom
    mapped: bool,
}

impl Dot {
    fn new(val: i32, start_x: f32, start_y: f32) -> Self {
        Dot {
            _original: val,
            value: val,
            x: start_x,
            y: start_y,
            stage: 0,
            state: DotState::Moving,
            color: WHITE,
            mapped: false,
        }
    }
}

// ── Stav simulácie ───────────────────────────────────────────────────
struct Sim {
    dots: Vec<Dot>,
    next_spawn_val: i32,        // ďalšie číslo na spawn
    last_spawn_time: f64,
    // štatistiky
    filter_passed: i32,
    filter_total: i32,
    map_count: i32,
    take_count: i32,
    result: Vec<i32>,
    done: bool,
}

impl Sim {
    fn new() -> Self {
        Sim {
            dots: Vec::new(),
            next_spawn_val: 0,
            last_spawn_time: -1.0,
            filter_passed: 0,
            filter_total: 0,
            map_count: 0,
            take_count: 0,
            result: Vec::new(),
            done: false,
        }
    }

    fn restart(&mut self) {
        *self = Sim::new();
    }
}

// ── Pomocné výpočty pozícií ──────────────────────────────────────────

/// Vracia ľavý okraj i-teho boxu (0 = SOURCE, 4 = RESULT)
fn box_x(i: usize, offset_x: f32) -> f32 {
    offset_x + i as f32 * (BOX_W + BOX_GAP)
}

/// Stred boxu horizontálne
fn box_cx(i: usize, offset_x: f32) -> f32 {
    box_x(i, offset_x) + BOX_W * 0.5
}

/// Stred pipeline vertikálne
fn pipeline_y(sh: f32) -> f32 {
    sh * 0.42
}

/// Šírka celej pipeline
fn pipeline_total_w() -> f32 {
    NUM_STAGES as f32 * BOX_W + (NUM_STAGES - 1) as f32 * BOX_GAP
}

/// Offset pre centrovanie pipeline horizontálne
fn pipeline_offset_x(sw: f32) -> f32 {
    (sw - pipeline_total_w()) * 0.5
}

// ── Vykresľovanie ────────────────────────────────────────────────────

fn draw_pipeline_boxes(offset_x: f32, sh: f32) {
    let cy = pipeline_y(sh);
    let by = cy - BOX_H * 0.5;

    let labels = ["SOURCE\n0..20", "filter\nx%2==0", "map\nx*x", "take\n5", "RESULT"];
    let colors: [Color; NUM_STAGES] = [
        Color { r: 0.15, g: 0.25, b: 0.45, a: 1.0 },  // SOURCE - modrá
        Color { r: 0.35, g: 0.15, b: 0.15, a: 1.0 },  // filter - tmavočervená
        Color { r: 0.30, g: 0.25, b: 0.05, a: 1.0 },  // map - tmavožltá
        Color { r: 0.15, g: 0.30, b: 0.15, a: 1.0 },  // take - tmavozelená
        Color { r: 0.10, g: 0.35, b: 0.10, a: 1.0 },  // RESULT - zelená
    ];
    let border_colors: [Color; NUM_STAGES] = [
        BLUE, RED, YELLOW, GREEN,
        Color { r: 0.0, g: 1.0, b: 0.3, a: 1.0 },
    ];

    for i in 0..NUM_STAGES {
        let bx = box_x(i, offset_x);

        // box
        draw_rectangle(bx, by, BOX_W, BOX_H, colors[i]);
        draw_rectangle_lines(bx, by, BOX_W, BOX_H, 2.5, border_colors[i]);

        // label — rozdelenie na 2 riadky podľa \n
        let parts: Vec<&str> = labels[i].split('\n').collect();
        let lx = bx + BOX_W * 0.5;
        if parts.len() == 2 {
            draw_text_center(parts[0], lx, by + BOX_H * 0.35, 17.0, border_colors[i]);
            draw_text_center(parts[1], lx, by + BOX_H * 0.70, 15.0, WHITE);
        } else {
            draw_text_center(parts[0], lx, by + BOX_H * 0.55, 17.0, border_colors[i]);
        }

        // šípka napravo (okrem posledného boxu)
        if i < NUM_STAGES - 1 {
            let ax0 = bx + BOX_W + 4.0;
            let ax1 = bx + BOX_W + BOX_GAP - 4.0;
            let ay = cy;
            draw_line(ax0, ay, ax1, ay, 2.5, DARKGRAY);
            // hrot
            draw_line(ax1 - 10.0, ay - 6.0, ax1, ay, 2.5, DARKGRAY);
            draw_line(ax1 - 10.0, ay + 6.0, ax1, ay, 2.5, DARKGRAY);
        }
    }
}

fn draw_text_center(text: &str, cx: f32, y: f32, size: f32, color: Color) {
    // aproximácia: každý znak je cca size*0.52 px
    let w = text.len() as f32 * size * 0.52;
    draw_text(text, cx - w * 0.5, y, size, color);
}

fn draw_dot(dot: &Dot) {
    let alpha = match &dot.state {
        DotState::FilterFlash(t) => {
            let elapsed = get_time() - t;
            (1.0 - (elapsed / 0.4) as f32).max(0.0)
        }
        DotState::TakeFlash(t) => {
            let elapsed = get_time() - t;
            (1.0 - (elapsed / 0.4) as f32).max(0.0)
        }
        _ => 1.0,
    };

    if alpha <= 0.0 {
        return;
    }

    let col = Color { r: dot.color.r, g: dot.color.g, b: dot.color.b, a: alpha };
    let border = match dot.state {
        DotState::FilterFlash(_) => Color { r: 1.0, g: 0.0, b: 0.0, a: alpha },
        DotState::TakeFlash(_) => Color { r: 0.5, g: 0.5, b: 0.5, a: alpha },
        _ => WHITE,
    };

    draw_circle(dot.x, dot.y, DOT_R, col);
    draw_circle_lines(dot.x, dot.y, DOT_R, 2.0, border);

    // číslo vnútri bodky
    let label = dot.value.to_string();
    let lx = dot.x - label.len() as f32 * 5.5;
    draw_text(&label, lx, dot.y + 5.0, 16.0, Color { r: 0.0, g: 0.0, b: 0.0, a: alpha });
}

fn draw_result_items(result: &[i32], offset_x: f32, sh: f32) {
    if result.is_empty() {
        return;
    }
    let cx = box_cx(4, offset_x);
    let by = pipeline_y(sh) - BOX_H * 0.5;

    // menší font, vypíš naakumulované hodnoty vertikálne pod RESULT boxom
    let label_y_start = by + BOX_H + 18.0;
    for (i, val) in result.iter().enumerate() {
        let txt = format!("{}", val);
        draw_text_center(&txt, cx, label_y_start + i as f32 * 22.0, 16.0,
            Color { r: 0.0, g: 1.0, b: 0.3, a: 1.0 });
    }
}

// ── Logika simulácie ─────────────────────────────────────────────────

fn update_sim(sim: &mut Sim, offset_x: f32, sh: f32, now: f64, dt: f32) {
    let cy = pipeline_y(sh);

    // Spawn novej bodky každých SPAWN_INTERVAL sekúnd (max 20 hodnôt)
    if sim.next_spawn_val < 20
        && !sim.done
        && now - sim.last_spawn_time >= SPAWN_INTERVAL
    {
        let sx = box_cx(0, offset_x);
        sim.dots.push(Dot::new(sim.next_spawn_val, sx, cy));
        sim.next_spawn_val += 1;
        sim.last_spawn_time = now;
    }

    // Uprav každú bodku
    let mut to_remove: Vec<usize> = Vec::new();

    for (idx, dot) in sim.dots.iter_mut().enumerate() {
        match dot.state.clone() {
            DotState::Moving => {
                // Cieľ: stred nasledujúceho boxu
                let target_stage = (dot.stage + 1).min(NUM_STAGES - 1);
                let target_x = if dot.stage == NUM_STAGES - 1 {
                    // v RESULT — prestane sa hýbať
                    dot.x
                } else {
                    box_cx(target_stage, offset_x)
                };

                let dx = target_x - dot.x;
                if dx.abs() < 2.0 {
                    // Dorazila do ďalšieho boxu
                    dot.stage = target_stage;
                    dot.x = target_x;

                    match dot.stage {
                        1 => {
                            // filter: x % 2 == 0?
                            sim.filter_total += 1;
                            if dot.value % 2 != 0 {
                                // odmietnutá
                                dot.color = RED;
                                dot.state = DotState::FilterFlash(now);
                            } else {
                                sim.filter_passed += 1;
                                // prechádza ďalej — pohybuje sa na map
                            }
                        }
                        2 => {
                            // map: x → x²
                            dot.value = dot.value * dot.value;
                            dot.color = YELLOW;
                            dot.mapped = true;
                            sim.map_count += 1;
                        }
                        3 => {
                            // take: max 5
                            if sim.take_count >= 5 {
                                dot.color = GRAY;
                                dot.state = DotState::TakeFlash(now);
                            } else {
                                sim.take_count += 1;
                            }
                        }
                        4 => {
                            // RESULT — zhromaždi
                            dot.state = DotState::Collected;
                            sim.result.push(dot.value);
                            if sim.result.len() >= 5 {
                                sim.done = true;
                            }
                        }
                        _ => {}
                    }
                } else {
                    // pohyb smerom k cieľu
                    let speed = DOT_SPEED * dt;
                    dot.x += dx.signum() * speed.min(dx.abs());
                }
            }

            DotState::FilterFlash(t) => {
                let elapsed = now - t;
                if elapsed > 0.45 {
                    to_remove.push(idx);
                }
            }

            DotState::TakeFlash(t) => {
                let elapsed = now - t;
                if elapsed > 0.45 {
                    to_remove.push(idx);
                }
            }

            DotState::Collected => {
                // nehýbe sa, zobrazená v result liste
            }

            DotState::_Mapped => {}
        }
    }

    // Odstráň zmiznuté bodky (odzadu)
    for &i in to_remove.iter().rev() {
        sim.dots.remove(i);
    }
}

// ── Vykresli kódovú anotáciu nad pipeline ────────────────────────────
fn draw_code_bar(sw: f32) {
    let code = "(0..20).filter(|x| x % 2 == 0).map(|x| x * x).take(5).collect::<Vec<_>>()";
    draw_text_center(code, sw * 0.5, 38.0, 17.0, Color { r: 0.6, g: 0.8, b: 1.0, a: 1.0 });
    draw_text_center(
        "Iterator pipeline je LAZY — prvky sa spracovávajú jeden po druhom, nie naraz",
        sw * 0.5, 62.0, 15.0, GRAY,
    );
}

// ── Stavový riadok dole ──────────────────────────────────────────────
fn draw_status(sim: &Sim, sw: f32, sh: f32) {
    draw_rectangle(0.0, sh - 52.0, sw, 52.0,
        Color { r: 0.08, g: 0.08, b: 0.08, a: 0.97 });

    let line1 = format!(
        "filter preslo: {}/{}    map transformovalo: {}    take vzal: {}/{}",
        sim.filter_passed, sim.filter_total,
        sim.map_count,
        sim.take_count, 5,
    );
    draw_text_center(&line1, sw * 0.5, sh - 30.0, 17.0, WHITE);

    let result_str = if sim.result.is_empty() {
        "[]".to_string()
    } else {
        let s: Vec<String> = sim.result.iter().map(|v| v.to_string()).collect();
        format!("[{}]", s.join(", "))
    };
    let line2 = format!("RESULT: {}    SPACE = restart | Q = quit", result_str);
    draw_text_center(&line2, sw * 0.5, sh - 10.0, 17.0,
        if sim.done {
            Color { r: 0.0, g: 1.0, b: 0.3, a: 1.0 }
        } else {
            DARKGRAY
        });
}

// ── Legenda farieb ───────────────────────────────────────────────────
fn draw_legend(sw: f32, sh: f32) {
    let y = sh - 100.0;
    let items: &[(&str, Color)] = &[
        ("• biele = nefiltrované (odd)", WHITE),
        ("• červené = odmietnuté filtrom", RED),
        ("• žlté = transformované mapom (x²)", YELLOW),
        ("• šedé = odmietnuté take", GRAY),
        ("• zelené = v RESULT", Color { r: 0.0, g: 1.0, b: 0.3, a: 1.0 }),
    ];
    let total_w: f32 = items.len() as f32 * 205.0;
    let start_x = (sw - total_w) * 0.5 + 10.0;
    for (i, (txt, col)) in items.iter().enumerate() {
        draw_text(txt, start_x + i as f32 * 205.0, y, 15.0, *col);
    }
}

// ── Hotový výsledok ──────────────────────────────────────────────────
fn draw_done_banner(sw: f32, sh: f32) {
    let msg = "Hotovo!  [0, 4, 16, 36, 64]  — SPACE = restart";
    draw_rectangle(sw * 0.5 - 310.0, sh * 0.5 - 28.0, 620.0, 56.0,
        Color { r: 0.0, g: 0.3, b: 0.1, a: 0.95 });
    draw_rectangle_lines(sw * 0.5 - 310.0, sh * 0.5 - 28.0, 620.0, 56.0,
        2.0, Color { r: 0.0, g: 1.0, b: 0.3, a: 1.0 });
    draw_text_center(msg, sw * 0.5, sh * 0.5 + 10.0, 22.0,
        Color { r: 0.0, g: 1.0, b: 0.3, a: 1.0 });
}

// ── Main ─────────────────────────────────────────────────────────────
#[macroquad::main("Iterátory — Rust pre C/C++ programátorov")]
async fn main() {
    let mut sim = Sim::new();
    let mut last_time = get_time();

    loop {
        let now = get_time();
        let dt = (now - last_time) as f32;
        last_time = now;

        if is_key_pressed(KeyCode::Q) {
            break;
        }
        if is_key_pressed(KeyCode::Space) {
            sim.restart();
            last_time = get_time();
        }

        let sw = screen_width();
        let sh = screen_height();
        let offset_x = pipeline_offset_x(sw);

        clear_background(Color { r: 0.05, g: 0.05, b: 0.08, a: 1.0 });

        // kód hore
        draw_code_bar(sw);

        // pipeline boxy
        draw_pipeline_boxes(offset_x, sh);

        // výsledky v RESULT boxe
        draw_result_items(&sim.result, offset_x, sh);

        // update simulácie
        if !sim.done || sim.dots.iter().any(|d| matches!(d.state, DotState::Moving)) {
            update_sim(&mut sim, offset_x, sh, now, dt);
        }

        // bodky - najprv zbieraj info, potom vykresli
        let dots_snapshot: Vec<Dot> = sim.dots.clone();
        for dot in &dots_snapshot {
            if !matches!(dot.state, DotState::Collected) {
                draw_dot(dot);
            }
        }

        // legenda
        draw_legend(sw, sh);

        // status
        draw_status(&sim, sw, sh);

        // banner po dokončení
        if sim.done
            && !sim.dots.iter().any(|d| matches!(d.state, DotState::Moving))
        {
            draw_done_banner(sw, sh);
        }

        next_frame().await;
    }
}
