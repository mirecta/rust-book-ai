/// Mandelbrot fraktál renderer — ukážka concurrency v Ruste
///
/// Kapitola 9: Concurrency
/// Porovnáva single-thread a multi-thread rendering toho istého obrazu.
///
/// Spustenie:
///   cargo run --bin mandelbrot
///
/// Výstup:
///   mandelbrot_single.png  — renderované jedným vláknom
///   mandelbrot_multi.png   — renderované N vláknami (podľa CPU)

use std::thread;

// ---------------------------------------------------------------------------
// Konštanty
// ---------------------------------------------------------------------------

/// Šírka výsledného obrázka v pixeloch
const WIDTH: u32 = 800;

/// Výška výsledného obrázka v pixeloch
const HEIGHT: u32 = 600;

/// Maximálny počet iterácií — čím vyšší, tým detailnejší, ale pomalší render
const MAX_ITER: u32 = 256;

// ---------------------------------------------------------------------------
// Matematika Mandelbrotovej množiny
// ---------------------------------------------------------------------------

/// Vypočíta, koľko iterácií potrebuje bod (cx, cy) kým "unikne" z kruhu |z| > 2.
///
/// Mandelbrotova množina: iterujeme z_{n+1} = z_n² + c, kde c = cx + i·cy.
/// Bod patrí do množiny, ak |z| zostane ohraničené → vracia MAX_ITER.
/// Inak vráti číslo iterácie, pri ktorej bod "utiekol" — to určí farbu.
fn mandelbrot(cx: f64, cy: f64) -> u32 {
    let (mut x, mut y) = (0.0_f64, 0.0_f64);

    for i in 0..MAX_ITER {
        // Kontrola úniku: |z|² > 4  ⟺  |z| > 2
        if x * x + y * y > 4.0 {
            return i;
        }
        // z = z² + c:  (x + iy)² + (cx + icy)
        //   Re: x² - y² + cx
        //   Im: 2xy   + cy
        (x, y) = (x * x - y * y + cx, 2.0 * x * y + cy);
    }

    MAX_ITER // bod je (pravdepodobne) v množine
}

// ---------------------------------------------------------------------------
// Farebná schéma
// ---------------------------------------------------------------------------

/// Mapuje počet iterácií na RGB farbu pomocou smooth "orbit trap" gradientu.
///
/// - Vnútro množiny (MAX_ITER) → čierna
/// - Okolie → plynulý prechod modrá→cyan→biela na základe t = iter/MAX_ITER
fn iter_to_color(iter: u32) -> [u8; 3] {
    if iter == MAX_ITER {
        return [0, 0, 0]; // vnútro množiny je čierne
    }

    // Normalizovaný parameter t ∈ [0, 1)
    let t = iter as f64 / MAX_ITER as f64;

    // Bernsteinove polynómy — klasická "escape time" farebná schéma
    let r = (9.0 * (1.0 - t) * t * t * t * 255.0) as u8;
    let g = (15.0 * (1.0 - t) * (1.0 - t) * t * t * 255.0) as u8;
    let b = (8.5 * (1.0 - t) * (1.0 - t) * (1.0 - t) * t * 255.0) as u8;

    [r, g, b]
}

// ---------------------------------------------------------------------------
// Single-thread rendering
// ---------------------------------------------------------------------------

/// Renderuje celý obrázok v jedinom vlákne.
///
/// Vracia surový pixel buffer: `[R, G, B, R, G, B, ...]`, riadok za riadkom,
/// celkovo `WIDTH * HEIGHT * 3` bajtov.
fn render_single() -> Vec<u8> {
    let mut pixels = vec![0u8; (WIDTH * HEIGHT * 3) as usize];

    for py in 0..HEIGHT {
        for px in 0..WIDTH {
            // Mapovanie pixela na komplexnú rovinu:
            //   x-os: [-2.5, 1.0]  (šírka 3.5)
            //   y-os: [-1.0, 1.0]  (výška 2.0)
            let cx = px as f64 / WIDTH as f64 * 3.5 - 2.5;
            let cy = py as f64 / HEIGHT as f64 * 2.0 - 1.0;

            let iter = mandelbrot(cx, cy);
            let [r, g, b] = iter_to_color(iter);

            // Uloženie RGB trojice na správnu pozíciu v bufferi
            let offset = ((py * WIDTH + px) * 3) as usize;
            pixels[offset] = r;
            pixels[offset + 1] = g;
            pixels[offset + 2] = b;
        }
    }

    pixels
}

// ---------------------------------------------------------------------------
// Multi-thread rendering
// ---------------------------------------------------------------------------

/// Renderuje obrázok rozdelením riadkov medzi `num_threads` vlákien.
///
/// Každé vlákno dostane svoju skupinu riadkov (chunk), spočíta ich
/// nezávisle do lokálneho Vec<u8> a vráti ho. Hlavné vlákno poskladá
/// výsledky do finálneho buffera.
///
/// Prečo tento prístup (lokálny Vec namiesto Arc<Mutex<Vec>>):
///  - Žiadna synchronizácia počas výpočtu → žiadne čakanie
///  - Vlákna sú úplne nezávislé → maximálne využitie CPU
///  - Poskladanie na konci je lacná O(n) operácia
fn render_multi(num_threads: usize) -> Vec<u8> {
    // Počet riadkov na jedno vlákno
    let chunk_size = HEIGHT / num_threads as u32;

    // Vytvoríme rozsahy riadkov pre každé vlákno.
    // Posledný chunk dostane aj zvyšné riadky (HEIGHT nemusí byť deliteľné).
    let chunks: Vec<(u32, u32)> = (0..num_threads as u32)
        .map(|i| {
            let start = i * chunk_size;
            let end = if i == num_threads as u32 - 1 {
                HEIGHT // posledný chunk zahŕňa všetky zostatok riadkov
            } else {
                start + chunk_size
            };
            (start, end)
        })
        .collect();

    // Spustíme vlákno pre každý chunk.
    // `move` prenesie vlastníctvo (start_row, end_row) do vlákna.
    let handles: Vec<_> = chunks
        .into_iter()
        .map(|(start_row, end_row)| {
            thread::spawn(move || {
                // Lokálny buffer len pre riadky tohto vlákna
                let rows = (end_row - start_row) as usize;
                let mut local = vec![0u8; rows * WIDTH as usize * 3];

                for py in start_row..end_row {
                    for px in 0..WIDTH {
                        let cx = px as f64 / WIDTH as f64 * 3.5 - 2.5;
                        let cy = py as f64 / HEIGHT as f64 * 2.0 - 1.0;

                        let iter = mandelbrot(cx, cy);
                        let [r, g, b] = iter_to_color(iter);

                        // Offset v lokálnom bufferi (riadky počítame od 0)
                        let local_row = (py - start_row) as usize;
                        let offset = (local_row * WIDTH as usize + px as usize) * 3;
                        local[offset] = r;
                        local[offset + 1] = g;
                        local[offset + 2] = b;
                    }
                }

                local // vlákno vracia svoj chunk
            })
        })
        .collect();

    // Poskladáme výsledky — každý handle.join() blokuje, kým vlákno neskončí
    let mut pixels = vec![0u8; (WIDTH * HEIGHT * 3) as usize];

    for (i, handle) in handles.into_iter().enumerate() {
        let local = handle.join().expect("Vlákno zlyhalo");

        // Pozícia chunku vo finálnom bufferi
        let offset = i * chunk_size as usize * WIDTH as usize * 3;
        pixels[offset..offset + local.len()].copy_from_slice(&local);
    }

    pixels
}

// ---------------------------------------------------------------------------
// Pomocná funkcia — uloženie do PNG
// ---------------------------------------------------------------------------

/// Uloží pixel buffer (WIDTH × HEIGHT × 3 bajtov RGB) ako PNG súbor.
fn save_png(pixels: &[u8], filename: &str) {
    use image::{ImageBuffer, Rgb};

    // ImageBuffer::from_raw overí rozmery a zabalí naše dáta
    let img = ImageBuffer::<Rgb<u8>, _>::from_raw(WIDTH, HEIGHT, pixels.to_vec())
        .expect("Nesprávne rozmery pixelového buffera");

    img.save(filename).expect("Nepodarilo sa uložiť PNG súbor");
    println!("  -> Uložené: {filename}");
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() {
    // Zistime počet dostupných logických CPU jadier
    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    println!("Mandelbrot fraktál — {}×{}, max {} iterácií", WIDTH, HEIGHT, MAX_ITER);
    println!("Použité vlákna pre multi-thread render: {num_threads}");
    println!();

    // --- Single-thread ---
    let t1 = std::time::Instant::now();
    let pixels_single = render_single();
    let elapsed_single = t1.elapsed().as_secs_f64();
    println!("Single-thread: {elapsed_single:.3}s");
    save_png(&pixels_single, "mandelbrot_single.png");

    println!();

    // --- Multi-thread ---
    let t2 = std::time::Instant::now();
    let pixels_multi = render_multi(num_threads);
    let elapsed_multi = t2.elapsed().as_secs_f64();
    println!("Multi-thread ({num_threads} vlákien): {elapsed_multi:.3}s");
    save_png(&pixels_multi, "mandelbrot_multi.png");

    println!();
    println!(
        "Zrýchlenie: {:.1}×",
        elapsed_single / elapsed_multi.max(0.001)
    );
}
