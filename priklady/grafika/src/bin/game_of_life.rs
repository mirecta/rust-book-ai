// Conway's Game of Life — ukážka closures a iterátorov
// Ovládanie: SPACE = pauza/spusti, N = jeden krok, R = randomize, LMB = prepni bunku
use macroquad::prelude::*;

const COLS: usize = 80;
const ROWS: usize = 60;

struct Grid {
    cells: Vec<bool>, // row-major: index = row * COLS + col
}

impl Grid {
    fn new() -> Self {
        Grid {
            cells: vec![false; COLS * ROWS],
        }
    }

    fn get(&self, col: i32, row: i32) -> bool {
        if col < 0 || row < 0 || col >= COLS as i32 || row >= ROWS as i32 {
            return false;
        }
        self.cells[row as usize * COLS + col as usize]
    }

    fn set(&mut self, col: usize, row: usize, val: bool) {
        self.cells[row * COLS + col] = val;
    }

    fn count_neighbors(&self, col: i32, row: i32) -> u8 {
        [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)]
            .iter()
            .filter(|(dc, dr)| self.get(col + dc, row + dr))
            .count() as u8
    }

    fn step(&self) -> Grid {
        let cells = (0..ROWS)
            .flat_map(|row| {
                (0..COLS).map(move |col| {
                    let neighbors = self.count_neighbors(col as i32, row as i32);
                    let alive = self.get(col as i32, row as i32);
                    matches!((alive, neighbors), (true, 2) | (true, 3) | (false, 3))
                })
            })
            .collect();
        Grid { cells }
    }

    fn randomize(&mut self) {
        let mut seed = get_time().to_bits();
        for cell in self.cells.iter_mut() {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            *cell = (seed & 3) == 0; // ~25% živých
        }
    }

    fn count_alive(&self) -> usize {
        self.cells.iter().filter(|&&c| c).count()
    }
}

#[macroquad::main("Conway's Game of Life")]
async fn main() {
    let mut grid = Grid::new();
    grid.randomize();

    let mut paused = false;
    let mut generation: u64 = 0;
    let mut accumulated: f32 = 0.0;
    const STEP_INTERVAL: f32 = 0.1; // 100 ms

    loop {
        let dt = get_frame_time();
        clear_background(BLACK);

        // Vstup
        if is_key_pressed(KeyCode::Space) {
            paused = !paused;
        }
        if is_key_pressed(KeyCode::N) {
            grid = grid.step();
            generation += 1;
        }
        if is_key_pressed(KeyCode::R) {
            grid.randomize();
            generation = 0;
        }

        // Kliknutie myšou — prepni bunku
        if is_mouse_button_pressed(MouseButton::Left) {
            let (mx, my) = mouse_position();
            let cw = screen_width() / COLS as f32;
            let ch = (screen_height() - 30.0) / ROWS as f32;
            let col = (mx / cw) as usize;
            let row = (my / ch) as usize;
            if col < COLS && row < ROWS {
                let current = grid.get(col as i32, row as i32);
                grid.set(col, row, !current);
            }
        }

        // Automatický krok
        if !paused {
            accumulated += dt;
            if accumulated >= STEP_INTERVAL {
                accumulated -= STEP_INTERVAL;
                grid = grid.step();
                generation += 1;
            }
        }

        // Vykresľovanie buniek
        let cw = screen_width() / COLS as f32;
        let ch = (screen_height() - 30.0) / ROWS as f32;
        for row in 0..ROWS {
            for col in 0..COLS {
                if grid.get(col as i32, row as i32) {
                    draw_rectangle(
                        col as f32 * cw,
                        row as f32 * ch,
                        cw - 1.0,
                        ch - 1.0,
                        GREEN,
                    );
                }
            }
        }

        // Status bar
        let alive = grid.count_alive();
        let status = format!(
            "SPACE=pauza  N=krok  R=reset  LMB=kresli  [Generácia: {}]  [Živé: {}]{}",
            generation,
            alive,
            if paused { "  [PAUZOVANÉ]" } else { "" }
        );
        draw_rectangle(
            0.0,
            screen_height() - 30.0,
            screen_width(),
            30.0,
            DARKGRAY,
        );
        draw_text(&status, 6.0, screen_height() - 10.0, 18.0, WHITE);

        next_frame().await;
    }
}
