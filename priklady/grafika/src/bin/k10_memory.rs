// K10 — Vizualizácia raw pamäte — Hex Dump
//
// Cieľ: ukázať C/C++ programátorom ako vyzerá pamäť v Ruste —
// STACK, HEAP, UNMAPPED regióny, raw pointery, use-after-free.
//
// Ovládanie:
//   Šípky    — pohyb kurzora po bunkách
//   TAB      — prepína scenár (Normal / Raw pointer / Use-after-free)
//   Q        — ukončí program

use macroquad::prelude::*;

// ─── Konštanty layoutu ───────────────────────────────────────────────────────

const COLS: usize = 16; // počet stĺpcov (16 bajtov na riadok)
const ROWS: usize = 12; // počet riadkov
const CELL_W: f32 = 42.0;
const CELL_H: f32 = 28.0;
const ADDR_PANEL_W: f32 = 72.0; // ľavý panel pre adresy
const RIGHT_PANEL_W: f32 = 220.0; // pravý panel pre legenda/popis
const TOP_BAR_H: f32 = 56.0; // horná lišta
const GRID_TOP: f32 = TOP_BAR_H + 10.0;

// ─── Regióny pamäte ──────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
enum Region {
    Stack,
    Heap,
    Unmapped,
}

impl Region {
    fn color(self) -> Color {
        match self {
            Region::Stack => Color::new(0.1, 0.55, 0.15, 1.0),    // zelená
            Region::Heap => Color::new(0.1, 0.3, 0.7, 1.0),       // modrá
            Region::Unmapped => Color::new(0.35, 0.05, 0.05, 1.0), // tmavočervená
        }
    }

    fn label(self) -> &'static str {
        match self {
            Region::Stack => "STACK",
            Region::Heap => "HEAP",
            Region::Unmapped => "UNMAPPED",
        }
    }

    fn is_safe(self) -> bool {
        !matches!(self, Region::Unmapped)
    }
}

// ─── Scenáre ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
enum Scene {
    Normal,      // bežný prístup
    RawPointer,  // raw pointer zo STACK na HEAP
    UseAfterFree, // HEAP uvoľnený, pointer bliká červenou
}

impl Scene {
    fn next(self) -> Self {
        match self {
            Scene::Normal => Scene::RawPointer,
            Scene::RawPointer => Scene::UseAfterFree,
            Scene::UseAfterFree => Scene::Normal,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Scene::Normal => "1. Normálny prístup",
            Scene::RawPointer => "2. Raw pointer (*mut i32)",
            Scene::UseAfterFree => "3. Use-after-free!",
        }
    }
}

// ─── Bunka pamäte ────────────────────────────────────────────────────────────

struct Cell {
    region: Region,
    value: u8,
    description: &'static str, // čo je na tejto adrese
}

// ─── Zostavenie pamäťovej mapy ───────────────────────────────────────────────

fn build_memory_map() -> Vec<Cell> {
    // Celkovo ROWS*COLS = 192 buniek
    let mut cells = Vec::with_capacity(ROWS * COLS);

    // ── STACK (riadky 0–3, adresy 0x0000–0x003F) ──────────────────────────
    // let x: i32 = 42  →  0x2A 0x00 0x00 0x00 (little-endian)
    let x_bytes: [u8; 4] = 42_i32.to_le_bytes();
    // let y: f64 = 3.14 → IEEE 754 8 bajtov
    let y_bytes: [u8; 8] = 3.14_f64.to_le_bytes();
    // let ptr: *mut u8  → 8 bajtov (hodnota = adresa 0x0040 na HEAP)
    let ptr_value: u64 = 0x0040;
    let ptr_bytes: [u8; 8] = ptr_value.to_le_bytes();

    // Riadok 0 (0x0000): x (4B) + y prvé 4B
    for i in 0..4 {
        cells.push(Cell { region: Region::Stack, value: x_bytes[i], description: "let x: i32 = 42 (little-endian)" });
    }
    for i in 0..4 {
        cells.push(Cell { region: Region::Stack, value: y_bytes[i], description: "let y: f64 = 3.14 (IEEE 754, byte 0-3)" });
    }
    // Riadok 1 (0x0010): y zvyšné 4B + ptr prvé 4B
    for i in 4..8 {
        cells.push(Cell { region: Region::Stack, value: y_bytes[i], description: "let y: f64 = 3.14 (IEEE 754, byte 4-7)" });
    }
    for i in 0..4 {
        cells.push(Cell { region: Region::Stack, value: ptr_bytes[i], description: "let ptr: *mut u8 → ukazuje na HEAP 0x0040" });
    }
    // Riadok 2 (0x0020): ptr zvyšné 4B + padding
    for i in 4..8 {
        cells.push(Cell { region: Region::Stack, value: ptr_bytes[i], description: "let ptr: *mut u8 → (horné bajty adresy)" });
    }
    for _ in 0..8 {
        cells.push(Cell { region: Region::Stack, value: 0x00, description: "stack padding / alignment" });
    }
    // Riadok 3 (0x0030): stack frame zásobníka
    for _ in 0..8 {
        cells.push(Cell { region: Region::Stack, value: 0x00, description: "return address / saved regs" });
    }
    for _ in 0..8 {
        cells.push(Cell { region: Region::Stack, value: 0x00, description: "stack guard" });
    }

    // ── HEAP (riadky 4–7, adresy 0x0040–0x007F) ──────────────────────────
    // Vec<u8> obsahuje "Hello" = 48 65 6C 6C 6F + dĺžka + kapacita
    let hello = b"Hello, Rust!    ";
    for i in 0..COLS {
        let byte = if i < hello.len() { hello[i] } else { 0x00 };
        let desc = if i < 5 {
            "Vec<u8> data: \"Hello\""
        } else if i < 12 {
            "Vec<u8>: \", Rust!\""
        } else {
            "Vec<u8> capacity padding"
        };
        cells.push(Cell { region: Region::Heap, value: byte, description: desc });
    }
    // Riadok 5: Vec metadata (len, capacity, ptr)
    let vec_len: [u8; 8] = 12_u64.to_le_bytes();
    let vec_cap: [u8; 8] = 16_u64.to_le_bytes();
    for i in 0..8 {
        cells.push(Cell { region: Region::Heap, value: vec_len[i], description: "Vec.len = 12 (little-endian u64)" });
    }
    for i in 0..8 {
        cells.push(Cell { region: Region::Heap, value: vec_cap[i], description: "Vec.capacity = 16 (little-endian u64)" });
    }
    // Riadky 6–7: zvyšok heap
    for i in 0..(2 * COLS) {
        let v = ((i * 17 + 0xAB) & 0xFF) as u8;
        cells.push(Cell { region: Region::Heap, value: v, description: "heap dáta (iné alokácie)" });
    }

    // ── UNMAPPED (riadky 8–11) ────────────────────────────────────────────
    for _ in 0..(4 * COLS) {
        cells.push(Cell { region: Region::Unmapped, value: 0x00, description: "nemapovaná pamäť — prístup = crash" });
    }

    cells
}

// ─── Pomocná: adresa z indexu bunky ──────────────────────────────────────────

fn cell_addr(idx: usize) -> u16 {
    (idx * 1) as u16 // každá bunka = 1 bajt, adresa = index
}

// ─── Hlavná funkcia ──────────────────────────────────────────────────────────

#[macroquad::main("K10 — Vizualizácia pamäte")]
async fn main() {
    let cells = build_memory_map();

    let mut cursor_row: usize = 0;
    let mut cursor_col: usize = 0;
    let mut scene = Scene::Normal;

    loop {
        let t = get_time();
        let sw = screen_width();
        let sh = screen_height();

        // ── Vstup ────────────────────────────────────────────────────────
        if is_key_pressed(KeyCode::Q) {
            break;
        }
        if is_key_pressed(KeyCode::Tab) {
            scene = scene.next();
        }
        if is_key_pressed(KeyCode::Right) && cursor_col + 1 < COLS {
            cursor_col += 1;
        }
        if is_key_pressed(KeyCode::Left) && cursor_col > 0 {
            cursor_col -= 1;
        }
        if is_key_pressed(KeyCode::Down) && cursor_row + 1 < ROWS {
            cursor_row += 1;
        }
        if is_key_pressed(KeyCode::Up) && cursor_row > 0 {
            cursor_row -= 1;
        }

        clear_background(Color::new(0.07, 0.07, 0.10, 1.0));

        // ── Horná lišta ──────────────────────────────────────────────────
        draw_rectangle(0.0, 0.0, sw, TOP_BAR_H, Color::new(0.12, 0.12, 0.18, 1.0));
        draw_text("Memory Visualizer", 14.0, 28.0, 26.0, WHITE);
        draw_text("K10: Unsafe Rust — raw pamäť", 14.0, 46.0, 15.0, GRAY);
        draw_text(
            &format!("Scenár: {}", scene.label()),
            sw - RIGHT_PANEL_W - 10.0,
            32.0,
            18.0,
            YELLOW,
        );

        // ── Grid ─────────────────────────────────────────────────────────
        let grid_left = ADDR_PANEL_W;
        let grid_w = COLS as f32 * CELL_W;

        // Hlavička stĺpcov (hex offset)
        for col in 0..COLS {
            let hx = grid_left + col as f32 * CELL_W + CELL_W / 2.0 - 8.0;
            let hy = GRID_TOP - 4.0;
            draw_text(&format!("+{:X}", col), hx, hy, 13.0, GRAY);
        }

        for row in 0..ROWS {
            let gy = GRID_TOP + row as f32 * CELL_H;

            // Adresa riadku
            let row_addr = row * COLS;
            draw_text(
                &format!("0x{:04X}", row_addr),
                4.0,
                gy + CELL_H - 8.0,
                13.0,
                Color::new(0.6, 0.6, 0.6, 1.0),
            );

            for col in 0..COLS {
                let idx = row * COLS + col;
                let cell = &cells[idx];
                let cx = grid_left + col as f32 * CELL_W;

                // Farba pozadia podľa regiónu a scenára
                let bg_color = region_bg_color(cell.region, scene, idx, t);

                draw_rectangle(cx, gy, CELL_W - 1.0, CELL_H - 1.0, bg_color);

                // Cursor zvýraznenie
                let is_cursor = row == cursor_row && col == cursor_col;
                if is_cursor {
                    draw_rectangle_lines(cx, gy, CELL_W - 1.0, CELL_H - 1.0, 2.0, WHITE);
                }

                // Text hodnoty
                let txt = if cell.region == Region::Unmapped {
                    "??".to_string()
                } else {
                    format!("{:02X}", cell.value)
                };
                let txt_color = cell_text_color(cell.region, scene, idx, t);
                let tx = cx + CELL_W / 2.0 - 9.0;
                let ty = gy + CELL_H - 8.0;
                draw_text(&txt, tx, ty, 14.0, txt_color);
            }
        }

        // ── Raw pointer šípka (scenár 2 a 3) ─────────────────────────────
        if scene == Scene::RawPointer || scene == Scene::UseAfterFree {
            // Bunka ptr (index 24 = 0x0018, ptr_bytes prvý bajt)
            // ptr leží na 0x0018–0x001F (riadok 1, col 8-11 + riadok 2 col 0-3)
            // Vizuálne: ukáže šípku z bunky 0x0018 (riadok 1, col 8) na bunku 0x0040 (riadok 4, col 0)
            let ptr_row = 1_usize;
            let ptr_col = 8_usize;
            let heap_row = 4_usize;
            let heap_col = 0_usize;

            let arrow_alpha = if scene == Scene::UseAfterFree {
                ((t * 4.0).sin() * 0.5 + 0.5) as f32
            } else {
                0.85
            };
            let arrow_color = if scene == Scene::UseAfterFree {
                Color::new(1.0, 0.15, 0.15, arrow_alpha)
            } else {
                Color::new(0.9, 0.7, 0.1, arrow_alpha)
            };

            let ax = grid_left + ptr_col as f32 * CELL_W + CELL_W / 2.0;
            let ay = GRID_TOP + ptr_row as f32 * CELL_H + CELL_H / 2.0;
            let bx = grid_left + heap_col as f32 * CELL_W + CELL_W / 2.0;
            let by_ = GRID_TOP + heap_row as f32 * CELL_H + CELL_H / 2.0;

            // Zaoblená šípka cez stred
            let mid_x = ax + 12.0;
            let mid_y = (ay + by_) / 2.0;
            draw_line(ax, ay, mid_x, mid_y, 2.0, arrow_color);
            draw_line(mid_x, mid_y, bx, by_, 2.0, arrow_color);

            // Hrot šípky (malý trojuholník)
            draw_circle(bx, by_, 5.0, arrow_color);

            // Popis
            let label = if scene == Scene::UseAfterFree {
                "dangling ptr!"
            } else {
                "ptr: *mut u8"
            };
            draw_text(label, mid_x + 4.0, mid_y, 13.0, arrow_color);
        }

        // ── Pravý panel ───────────────────────────────────────────────────
        let rp_x = sw - RIGHT_PANEL_W;
        draw_rectangle(rp_x, 0.0, RIGHT_PANEL_W, sh, Color::new(0.10, 0.10, 0.16, 1.0));
        draw_line(rp_x, 0.0, rp_x, sh, 1.0, Color::new(0.3, 0.3, 0.3, 1.0));

        draw_right_panel(rp_x, &cells, cursor_row, cursor_col, scene, t, sw, sh, grid_left, grid_w);

        // ── Status bar ────────────────────────────────────────────────────
        let bar_y = sh - 26.0;
        draw_rectangle(0.0, bar_y, sw, 26.0, Color::new(0.0, 0.0, 0.0, 0.75));
        let cursor_idx = cursor_row * COLS + cursor_col;
        let cur_cell = &cells[cursor_idx];
        let status = format!(
            "Šípky=pohyb | TAB=scenár | Q=quit | Adresa: 0x{:04X} | Región: {}",
            cell_addr(cursor_idx),
            cur_cell.region.label()
        );
        draw_text(&status, 10.0, bar_y + 17.0, 15.0, GRAY);

        next_frame().await;
    }
}

// ─── Farba pozadia bunky ─────────────────────────────────────────────────────

fn region_bg_color(region: Region, scene: Scene, _idx: usize, t: f64) -> Color {
    match region {
        Region::Unmapped => Color::new(0.20, 0.04, 0.04, 1.0),
        Region::Stack => {
            let base = region.color();
            Color::new(base.r * 0.45, base.g * 0.45, base.b * 0.45, 1.0)
        }
        Region::Heap => {
            // Use-after-free: HEAP červenie
            if scene == Scene::UseAfterFree {
                let blink = ((t * 2.5).sin() * 0.5 + 0.5) as f32;
                Color::new(0.35 + 0.15 * blink, 0.04, 0.04, 1.0)
            } else {
                let base = region.color();
                Color::new(base.r * 0.45, base.g * 0.45, base.b * 0.45, 1.0)
            }
        }
    }
}

// ─── Farba textu bunky ───────────────────────────────────────────────────────

fn cell_text_color(region: Region, scene: Scene, _idx: usize, t: f64) -> Color {
    match region {
        Region::Unmapped => Color::new(0.5, 0.1, 0.1, 1.0),
        Region::Stack => Color::new(0.5, 1.0, 0.55, 1.0),
        Region::Heap => {
            if scene == Scene::UseAfterFree {
                let blink = ((t * 3.0).sin() * 0.5 + 0.5) as f32;
                Color::new(1.0, 0.3 + 0.3 * blink, 0.3 * blink, 1.0)
            } else {
                Color::new(0.55, 0.75, 1.0, 1.0)
            }
        }
    }
}

// ─── Pravý info panel ────────────────────────────────────────────────────────

fn draw_right_panel(
    rp_x: f32,
    cells: &[Cell],
    cursor_row: usize,
    cursor_col: usize,
    scene: Scene,
    t: f64,
    _sw: f32,
    _sh: f32,
    _grid_left: f32,
    _grid_w: f32,
) {
    let x = rp_x + 10.0;
    let mut y = TOP_BAR_H + 20.0;
    let line = 20.0_f32;

    // ── Legenda ───────────────────────────────────────────────────────────
    draw_text("Legenda:", x, y, 17.0, WHITE);
    y += line + 4.0;

    let regions = [Region::Stack, Region::Heap, Region::Unmapped];
    for region in &regions {
        let c = region.color();
        draw_rectangle(x, y - 12.0, 14.0, 14.0, c);
        draw_text(region.label(), x + 18.0, y, 15.0, WHITE);
        y += line;
    }
    y += 8.0;

    // ── Popis scenára ─────────────────────────────────────────────────────
    draw_line(x - 5.0, y, rp_x + 210.0, y, 1.0, GRAY);
    y += 12.0;
    draw_text("Scenár:", x, y, 16.0, YELLOW);
    y += line;
    let scene_lines: &[&str] = match scene {
        Scene::Normal => &[
            "Bežný prístup.",
            "STACK: lokálne premenné.",
            "HEAP: Vec, Box, String.",
            "UNMAPPED: zakázaná zóna.",
        ],
        Scene::RawPointer => &[
            "let p: *mut i32 =",
            "  0x0040 as *mut i32;",
            "Šípka ukazuje kde",
            "ptr smeruje.",
            "unsafe { *p = 99; }",
        ],
        Scene::UseAfterFree => &[
            "drop(vec);  // uvoľní HEAP",
            "// ptr stále existuje!",
            "unsafe { *ptr }",
            "→ undefined behavior!",
            "V C++: silent korupcia.",
            "Rust: borrow checker",
            "to zakáže v safe kóde.",
        ],
    };
    for line_txt in scene_lines {
        draw_text(line_txt, x, y, 14.0, Color::new(0.85, 0.85, 0.85, 1.0));
        y += 18.0;
    }
    y += 8.0;

    // ── Popis kurzora ─────────────────────────────────────────────────────
    draw_line(x - 5.0, y, rp_x + 210.0, y, 1.0, GRAY);
    y += 12.0;

    let idx = cursor_row * COLS + cursor_col;
    let cell = &cells[idx];
    let addr = cell_addr(idx);

    draw_text("Vybraná bunka:", x, y, 16.0, WHITE);
    y += line;
    draw_text(&format!("Adresa: 0x{:04X}", addr), x, y, 15.0, YELLOW);
    y += line;

    let val_str = if cell.region == Region::Unmapped {
        "??  (nečitateľné)".to_string()
    } else {
        format!("0x{:02X}  ({})", cell.value, cell.value)
    };
    draw_text(&format!("Hodnota: {}", val_str), x, y, 14.0, Color::new(0.8, 0.8, 0.8, 1.0));
    y += line;

    let region_color = cell.region.color();
    draw_text(
        &format!("Región: {}", cell.region.label()),
        x,
        y,
        15.0,
        region_color,
    );
    y += line;

    let safe_txt = if cell.region.is_safe() { "Bezpečný prístup" } else { "UNSAFE — STOP!" };
    let safe_color = if cell.region.is_safe() { GREEN } else { RED };
    draw_text(safe_txt, x, y, 15.0, safe_color);
    y += line + 4.0;

    // Popis obsahu
    draw_text("Obsah:", x, y, 14.0, GRAY);
    y += 17.0;
    // Zalomiť dlhý popis na kratšie riadky
    let desc = cell.description;
    let max_w = 26_usize; // znaky na riadok
    let words: Vec<&str> = desc.split_whitespace().collect();
    let mut current_line = String::new();
    for word in words {
        if current_line.len() + word.len() + 1 > max_w {
            draw_text(&current_line, x, y, 13.0, Color::new(0.7, 0.7, 0.7, 1.0));
            y += 16.0;
            current_line = word.to_string();
        } else {
            if !current_line.is_empty() { current_line.push(' '); }
            current_line.push_str(word);
        }
    }
    if !current_line.is_empty() {
        draw_text(&current_line, x, y, 13.0, Color::new(0.7, 0.7, 0.7, 1.0));
        y += 16.0;
    }

    // Varovanie pre UNMAPPED
    if cell.region == Region::Unmapped {
        y += 8.0;
        let warn_alpha = ((t * 2.5).sin() * 0.5 + 0.5) as f32;
        let warn_col = Color::new(1.0, 0.2, 0.2, warn_alpha);
        draw_text("⚠ Segfault!", x, y, 17.0, warn_col);
        y += 20.0;
        draw_text("Toto by crashlo v C.", x, y, 13.0, Color::new(0.9, 0.4, 0.4, 1.0));
        y += 17.0;
        draw_text("Rust: kompilátor to", x, y, 13.0, Color::new(0.7, 0.7, 0.7, 1.0));
        y += 16.0;
        draw_text("zachytí v safe kóde.", x, y, 13.0, Color::new(0.7, 0.7, 0.7, 1.0));
    }

    // Use-after-free varovanie
    if scene == Scene::UseAfterFree && cell.region == Region::Heap {
        y += 10.0;
        let blink = ((t * 3.0).sin() * 0.5 + 0.5) as f32;
        draw_text("FREED MEMORY!", x, y, 15.0, Color::new(1.0, 0.3, 0.3, blink));
        y += 18.0;
        draw_text("V C++: silent UB.", x, y, 13.0, Color::new(0.9, 0.5, 0.5, 1.0));
        y += 16.0;
        let _ = y; // potlač unused warning
    }
}
