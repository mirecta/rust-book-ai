// K09 — Vizualizácia vlákien a Mutex
//
// Cieľ: ukázať C/C++ programátorom ako funguje Mutex v Ruste —
// "kto môže vstúpiť do miestnosti?"
//
// Ovládanie:
//   SPACE  — prepne normal / deadlock scenár
//   Q      — ukončí program

use macroquad::prelude::*;

// ─── Stavy jedného simulovaného vlákna ───────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
enum ThreadState {
    Waiting,   // čaká na kružnici
    Acquiring, // snaží sa získať lock (bliká)
    Locked,    // drží mutex (je vo vnútri boxu)
    Done,      // práve uvoľnil, odchádza naspäť
}

#[derive(Clone)]
struct SimThread {
    id: usize,
    color: Color,
    state: ThreadState,
    /// uhol na kružnici (radiány)
    angle: f32,
    /// čas keď naposledy zmenil stav
    state_since: f64,
    /// uhol kde sa thread "zakotvil" keď začal Acquiring
    acquire_start_angle: f32,
}

impl SimThread {
    fn new(id: usize, angle: f32, color: Color) -> Self {
        SimThread {
            id,
            color,
            state: ThreadState::Waiting,
            angle,
            state_since: 0.0,
            acquire_start_angle: angle,
        }
    }

    /// Farebný štítok "T0", "T1" …
    fn label(&self) -> String {
        format!("T{}", self.id)
    }
}

// ─── Scenár: normálny mutex vs. deadlock ─────────────────────────────────────

#[derive(PartialEq, Clone, Copy)]
enum Scene {
    Normal,
    Deadlock,
}

// ─── Hlavná funkcia ──────────────────────────────────────────────────────────

#[macroquad::main("K09 — Vlákna a Mutex")]
async fn main() {
    // Farby pre 6 vlákien — aproximácia dúhy
    let thread_colors = [
        Color::new(1.0, 0.3, 0.3, 1.0), // červená
        Color::new(1.0, 0.65, 0.0, 1.0), // oranžová
        Color::new(1.0, 1.0, 0.2, 1.0), // žltá
        Color::new(0.2, 0.9, 0.2, 1.0), // zelená
        Color::new(0.2, 0.5, 1.0, 1.0), // modrá
        Color::new(0.7, 0.2, 1.0, 1.0), // fialová
    ];

    // Inicializácia 6 vlákien rovnomerne na kružnici
    let mut threads: Vec<SimThread> = (0..6)
        .map(|i| {
            let angle = (i as f32) * std::f32::consts::TAU / 6.0;
            SimThread::new(i, angle, thread_colors[i])
        })
        .collect();

    // index vlákna ktoré momentálne drží mutex (None = nikto)
    let mut locked_by: Option<usize> = None;
    // čas posledného uvoľnenia (aby sme dali pauzu pred ďalším)
    let mut last_release: f64 = 0.0;

    let mut scene = Scene::Normal;

    // Deadlock stav — fixný, meniaci sa len s časom
    let mut deadlock_initialized = false;

    loop {
        let t = get_time();
        let sw = screen_width();
        let sh = screen_height();

        // ── Vstup ────────────────────────────────────────────────────────────
        if is_key_pressed(KeyCode::Q) {
            break;
        }
        if is_key_pressed(KeyCode::Space) {
            scene = match scene {
                Scene::Normal => Scene::Deadlock,
                Scene::Deadlock => Scene::Normal,
            };
            deadlock_initialized = false;
            // reset vlákien
            for (i, th) in threads.iter_mut().enumerate() {
                let angle = (i as f32) * std::f32::consts::TAU / 6.0;
                th.angle = angle;
                th.state = ThreadState::Waiting;
                th.state_since = t;
            }
            locked_by = None;
            last_release = t;
        }

        // ── Geometria ────────────────────────────────────────────────────────
        let cx = sw / 2.0;
        let cy = sh / 2.0;
        let orbit_r = 170.0_f32; // polomer kružnice vlákien
        let box_half = 100.0_f32; // box = 200×200

        clear_background(Color::new(0.08, 0.08, 0.12, 1.0));

        match scene {
            // ================================================================
            Scene::Normal => {
                draw_normal_scene(
                    &mut threads,
                    &mut locked_by,
                    &mut last_release,
                    t,
                    cx,
                    cy,
                    orbit_r,
                    box_half,
                );
            }
            // ================================================================
            Scene::Deadlock => {
                if !deadlock_initialized {
                    deadlock_initialized = true;
                }
                draw_deadlock_scene(t, cx, cy, box_half, &thread_colors);
            }
        }

        // ── Status bar ───────────────────────────────────────────────────────
        let bar_y = sh - 30.0;
        draw_rectangle(0.0, bar_y, sw, 30.0, Color::new(0.0, 0.0, 0.0, 0.7));

        let locked_label = locked_by
            .map(|i| format!("T{}", i))
            .unwrap_or_else(|| "nikto".to_string());

        let waiting_labels: Vec<String> = threads
            .iter()
            .filter(|th| th.state == ThreadState::Acquiring)
            .map(|th| th.label())
            .collect();
        let waiting_str = if waiting_labels.is_empty() {
            "—".to_string()
        } else {
            waiting_labels.join(", ")
        };

        let status = if scene == Scene::Normal {
            format!(
                "Q=quit | SPACE=deadlock demo | Teraz locked: {} | Čakajú: {}",
                locked_label, waiting_str
            )
        } else {
            "Q=quit | SPACE=normal demo | DEADLOCK scenár aktívny".to_string()
        };
        draw_text(&status, 10.0, bar_y + 19.0, 18.0, GRAY);

        next_frame().await;
    }
}

// ─── Normálny scenár: Mutex ───────────────────────────────────────────────────

fn draw_normal_scene(
    threads: &mut Vec<SimThread>,
    locked_by: &mut Option<usize>,
    last_release: &mut f64,
    t: f64,
    cx: f32,
    cy: f32,
    orbit_r: f32,
    box_half: f32,
) {
    // ── Titul ─────────────────────────────────────────────────────────────
    draw_text("Mutex<Data> — len jedno vlákno naraz", cx - 200.0, 36.0, 26.0, WHITE);
    draw_text(
        "Každé vlákno sa snaží zamknúť Mutex. Iba jedno uspeje.",
        cx - 230.0,
        62.0,
        17.0,
        GRAY,
    );

    // ── Logika simulácie ──────────────────────────────────────────────────

    // Každé vlákno rotuje s miernym ofsetom rýchlosti
    for th in threads.iter_mut() {
        let speed = 0.35 + th.id as f64 * 0.04; // rad/s, každé trochu iná
        match th.state {
            ThreadState::Waiting => {
                th.angle = (th.angle + (speed * 0.016) as f32) % std::f32::consts::TAU;

                // Každé vlákno sa pokúsi získať lock v inom rytme
                // Využijeme sinus fázu na "náhodné" zapnutie touhy po locku
                let phase = th.id as f64 * 1.3 + t * 0.4;
                if phase.sin() > 0.85 && *locked_by == None && t - *last_release > 0.3 {
                    th.state = ThreadState::Acquiring;
                    th.state_since = t;
                    th.acquire_start_angle = th.angle;
                }
            }
            ThreadState::Acquiring => {
                // pohyb k boxu — uhol zostáva, ale radius sa zmenšuje
                // (simulujeme len blikanie a pohyb bude v draw)
                th.angle = (th.angle + (speed * 0.008) as f32) % std::f32::consts::TAU;

                // Môžeme získať lock ak nikto iný ho nemá
                if *locked_by == None && t - *last_release > 0.3 {
                    *locked_by = Some(th.id);
                    th.state = ThreadState::Locked;
                    th.state_since = t;
                }
            }
            ThreadState::Locked => {
                // Drží lock 1.5 s
                if t - th.state_since > 1.5 {
                    th.state = ThreadState::Done;
                    th.state_since = t;
                    *locked_by = None;
                    *last_release = t;
                }
            }
            ThreadState::Done => {
                // 0.4 s odchádzania, potom Waiting
                if t - th.state_since > 0.4 {
                    th.state = ThreadState::Waiting;
                    th.state_since = t;
                }
                th.angle = (th.angle + (speed * 0.012) as f32) % std::f32::consts::TAU;
            }
        }
    }

    // ── Kresliť dráhu (kružnicu) ──────────────────────────────────────────
    draw_circle_lines(cx, cy, orbit_r, 1.0, Color::new(0.3, 0.3, 0.3, 1.0));

    // ── Box = Mutex ───────────────────────────────────────────────────────
    let box_x = cx - box_half;
    let box_y = cy - box_half;
    let box_size = box_half * 2.0;

    // Výplň podľa stavu
    let box_fill = if let Some(id) = *locked_by {
        let c = threads[id].color;
        Color::new(c.r * 0.25, c.g * 0.25, c.b * 0.25, 1.0)
    } else {
        Color::new(0.12, 0.12, 0.18, 1.0)
    };
    draw_rectangle(box_x, box_y, box_size, box_size, box_fill);

    // Okraj
    let box_border = if locked_by.is_some() {
        let id = locked_by.unwrap();
        threads[id].color
    } else {
        WHITE
    };
    draw_rectangle_lines(box_x, box_y, box_size, box_size, 3.0, box_border);

    // Popis
    draw_text("Mutex<Data>", cx - 52.0, cy - 10.0, 20.0, WHITE);
    if let Some(id) = *locked_by {
        let label = format!("LOCKED by T{}", id);
        draw_text(&label, cx - 55.0, cy + 16.0, 20.0, threads[id].color);
    } else {
        draw_text("unlocked", cx - 38.0, cy + 16.0, 18.0, GRAY);
    }

    // ── Vlákna ────────────────────────────────────────────────────────────
    for th in threads.iter() {
        let (px, py) = match th.state {
            ThreadState::Waiting | ThreadState::Done => {
                // na kružnici
                let x = cx + th.angle.cos() * orbit_r;
                let y = cy + th.angle.sin() * orbit_r;
                (x, y)
            }
            ThreadState::Acquiring => {
                // pohyb k okraju boxu — interpolujeme medzi kružnicou a okrajom boxu
                let progress = ((t - th.state_since) as f32 * 0.8).min(1.0);
                let ox = cx + th.angle.cos() * orbit_r;
                let oy = cy + th.angle.sin() * orbit_r;
                // cieľ: bod na okraji boxu smerom od středu
                let dir_x = th.angle.cos();
                let dir_y = th.angle.sin();
                let tx = cx + dir_x * (box_half + 15.0);
                let ty = cy + dir_y * (box_half + 15.0);
                (ox + (tx - ox) * progress, oy + (ty - oy) * progress)
            }
            ThreadState::Locked => {
                // vnútri boxu — pozícia podľa id
                let slot = th.id as f32;
                let ix = cx - 60.0 + (slot % 3.0) * 50.0;
                let iy = cy - 20.0 + (slot / 3.0).floor() * 40.0;
                (ix, iy)
            }
        };

        // Blikanie pre Acquiring
        let alpha = if th.state == ThreadState::Acquiring {
            let blink = ((t * 6.0).sin() * 0.5 + 0.5) as f32;
            0.5 + blink * 0.5
        } else {
            1.0
        };

        let c = th.color;
        let draw_color = Color::new(c.r, c.g, c.b, alpha);

        draw_circle(px, py, 18.0, draw_color);
        draw_circle_lines(px, py, 18.0, 2.0, WHITE);
        // Label "T0"
        draw_text(&th.label(), px - 9.0, py + 6.0, 18.0, BLACK);

        // Šípka pre Acquiring → box
        if th.state == ThreadState::Acquiring {
            let dir_x = th.angle.cos();
            let dir_y = th.angle.sin();
            let ax = px + dir_x * 22.0;
            let ay = py + dir_y * 22.0;
            let bx = cx + dir_x * (box_half + 2.0);
            let by_ = cy + dir_y * (box_half + 2.0);
            draw_line(ax, ay, bx, by_, 2.0, draw_color);
        }
    }

    // ── Legenda ───────────────────────────────────────────────────────────
    let lx = 20.0_f32;
    let mut ly = 100.0_f32;
    draw_text("Stavy:", lx, ly, 17.0, WHITE);
    ly += 24.0;
    let legend = [
        (WHITE, "Waiting — čaká na kružnici"),
        (YELLOW, "Acquiring — snaží sa o lock (bliká)"),
        (GREEN, "Locked — drží mutex (vo vnútri)"),
        (GRAY, "Done — uvoľnil, odchádza"),
    ];
    for (col, txt) in &legend {
        draw_circle(lx + 8.0, ly - 5.0, 7.0, *col);
        draw_text(txt, lx + 20.0, ly, 15.0, WHITE);
        ly += 22.0;
    }
}

// ─── Deadlock scenár ─────────────────────────────────────────────────────────

fn draw_deadlock_scene(
    t: f64,
    cx: f32,
    cy: f32,
    box_half: f32,
    thread_colors: &[Color],
) {
    // Pozície dvoch boxov
    let ax = cx - 200.0;
    let bx = cx + 200.0 - box_half * 2.0;
    let by_ = cy - box_half;
    let box_size = box_half * 2.0;

    // Blikanie červenou pre boxy
    let blink = ((t * 2.0).sin() * 0.5 + 0.5) as f32;
    let red_fill = Color::new(0.4 * blink, 0.05, 0.05, 1.0);

    // Titul
    draw_text("DEADLOCK scenár", cx - 130.0, 36.0, 28.0, RED);
    draw_text(
        "T0 drží Mutex A, čaká na B. T1 drží Mutex B, čaká na A.",
        cx - 280.0,
        62.0,
        17.0,
        GRAY,
    );

    // ── Box A ─────────────────────────────────────────────────────────────
    draw_rectangle(ax, by_, box_size, box_size, red_fill);
    draw_rectangle_lines(ax, by_, box_size, box_size, 3.0, thread_colors[0]);
    draw_text("Mutex A", ax + 28.0, cy - 10.0, 20.0, WHITE);
    draw_text("held by T0", ax + 18.0, cy + 14.0, 17.0, thread_colors[0]);

    // ── Box B ─────────────────────────────────────────────────────────────
    draw_rectangle(bx, by_, box_size, box_size, red_fill);
    draw_rectangle_lines(bx, by_, box_size, box_size, 3.0, thread_colors[1]);
    draw_text("Mutex B", bx + 28.0, cy - 10.0, 20.0, WHITE);
    draw_text("held by T1", bx + 18.0, cy + 14.0, 17.0, thread_colors[1]);

    // ── Vlákno T0 (ľavý stred medzi boxmi) ───────────────────────────────
    let t0x = cx - 60.0;
    let t0y = cy + 130.0;
    draw_circle(t0x, t0y, 22.0, thread_colors[0]);
    draw_circle_lines(t0x, t0y, 22.0, 2.0, WHITE);
    draw_text("T0", t0x - 11.0, t0y + 7.0, 20.0, BLACK);

    // ── Vlákno T1 (pravý stred) ───────────────────────────────────────────
    let t1x = cx + 60.0;
    let t1y = cy + 130.0;
    draw_circle(t1x, t1y, 22.0, thread_colors[1]);
    draw_circle_lines(t1x, t1y, 22.0, 2.0, WHITE);
    draw_text("T1", t1x - 11.0, t1y + 7.0, 20.0, BLACK);

    // ── Šípky "drží" (pevné čiary) ────────────────────────────────────────
    // T0 → drží A (zelená plná šípka)
    draw_line(t0x - 20.0, t0y - 16.0, ax + box_size / 2.0, by_ + box_size, 2.5, thread_colors[0]);
    // T1 → drží B
    draw_line(t1x + 20.0, t1y - 16.0, bx + box_size / 2.0, by_ + box_size, 2.5, thread_colors[1]);

    // ── Šípky "čaká na" (blikajúce červené) ─────────────────────────────
    let arrow_alpha = ((t * 3.0).sin() * 0.5 + 0.5) as f32;
    let wait_color = Color::new(1.0, 0.1, 0.1, arrow_alpha);

    // T0 čaká na B — šípka z T0 ku boxu B
    draw_line(t0x + 22.0, t0y, bx + box_size / 2.0, by_ + box_size, 3.0, wait_color);
    // T1 čaká na A — šípka z T1 ku boxu A
    draw_line(t1x - 22.0, t1y, ax + box_size / 2.0, by_ + box_size, 3.0, wait_color);

    // Popisy šípok
    draw_text("čaká na B →", t0x + 25.0, t0y - 15.0, 15.0, RED);
    draw_text("← čaká na A", t1x - 105.0, t1y - 15.0, 15.0, RED);
    draw_text("drží A", t0x - 70.0, (t0y + by_ + box_size) / 2.0, 14.0, thread_colors[0]);
    draw_text("drží B", t1x + 15.0, (t1y + by_ + box_size) / 2.0, 14.0, thread_colors[1]);

    // ── Blikajúci DEADLOCK text ───────────────────────────────────────────
    let text_alpha = ((t * 1.8).sin() * 0.5 + 0.5) as f32;
    let warn_color = Color::new(1.0, 0.2, 0.2, text_alpha);
    draw_text(
        "⚠  DEADLOCK — obe vlákna čakajú navždy!",
        cx - 240.0,
        cy + 210.0,
        24.0,
        warn_color,
    );
    draw_text(
        "Riešenie: vždy zamykaj mutexы v rovnakom poradí (A potom B)",
        cx - 270.0,
        cy + 240.0,
        17.0,
        YELLOW,
    );

    // ── Rust vs C++ poznámka ─────────────────────────────────────────────
    draw_text(
        "Rust: std::sync::Mutex + ownership — kompilátor ti pomôže, ale poradie zamykania musíš strážiť sám.",
        20.0,
        cy + 275.0,
        15.0,
        Color::new(0.6, 0.6, 0.6, 1.0),
    );
}
