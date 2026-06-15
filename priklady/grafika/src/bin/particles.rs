// Particle physics — ukážka Vec<T>, structs, closures, update loop
// Ovládanie: LMB (držať) = pridaj particles, R = vyčisti všetky
use macroquad::prelude::*;

struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    color: Color,
    radius: f32,
}

impl Particle {
    fn new(x: f32, y: f32) -> Self {
        // pseudo-random rýchlosť bez external crate — seed z pozície
        let angle = (x * 7.3 + y * 3.7).sin() * 6.28;
        let speed = 50.0 + (x * 13.1).abs() % 150.0;
        Particle {
            x,
            y,
            vx: angle.cos() * speed,
            vy: angle.sin() * speed - 100.0, // trocha nahor
            color: Color::new(
                0.3 + (x * 0.01).sin().abs() * 0.7,
                0.5 + (y * 0.013).cos().abs() * 0.5,
                0.8,
                0.9,
            ),
            radius: 3.0 + (x * 0.1).abs() % 4.0,
        }
    }

    fn update(&mut self, dt: f32) {
        const GRAVITY: f32 = 300.0;
        self.vy += GRAVITY * dt;
        self.x += self.vx * dt;
        self.y += self.vy * dt;

        // odraz od stien
        let w = screen_width();
        let h = screen_height() - 30.0; // status bar
        if self.x < self.radius {
            self.x = self.radius;
            self.vx = self.vx.abs() * 0.8;
        }
        if self.x > w - self.radius {
            self.x = w - self.radius;
            self.vx = -self.vx.abs() * 0.8;
        }
        if self.y > h - self.radius {
            self.y = h - self.radius;
            self.vy = -self.vy.abs() * 0.8;
        }
        if self.y < self.radius {
            self.y = self.radius;
            self.vy = self.vy.abs();
        }
    }

    fn draw(&self) {
        draw_circle(self.x, self.y, self.radius, self.color);
    }
}

// Jednoduchý xorshift seed tracker pre offset pri spawne
fn pseudo_offset(seed: &mut u64) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    ((*seed & 0xFF) as f32 / 127.5) - 1.0 // rozsah [-1, 1]
}

#[macroquad::main("Particle Physics")]
async fn main() {
    let mut particles: Vec<Particle> = Vec::new();
    let mut seed: u64 = 12345;

    loop {
        let dt = get_frame_time();
        clear_background(Color::new(0.1, 0.1, 0.15, 1.0));

        // Vstup — hold LMB pridáva particles
        if is_mouse_button_down(MouseButton::Left) && particles.len() < 1000 {
            let (mx, my) = mouse_position();
            for _ in 0..3 {
                let ox = pseudo_offset(&mut seed) * 10.0;
                let oy = pseudo_offset(&mut seed) * 10.0;
                particles.push(Particle::new(mx + ox, my + oy));
                if particles.len() >= 1000 {
                    break;
                }
            }
        }

        // Reset
        if is_key_pressed(KeyCode::R) {
            particles.clear();
        }

        // Update
        particles.iter_mut().for_each(|p| p.update(dt));

        // Draw
        particles.iter().for_each(|p| p.draw());

        // Status bar
        let count = particles.len();
        let status = format!("LMB=pridaj  R=reset  [Particles: {}/1000]", count);
        draw_rectangle(
            0.0,
            screen_height() - 30.0,
            screen_width(),
            30.0,
            Color::new(0.05, 0.05, 0.1, 1.0),
        );
        draw_text(&status, 6.0, screen_height() - 10.0, 18.0, WHITE);

        next_frame().await;
    }
}
