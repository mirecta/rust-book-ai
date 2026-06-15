// k06_lifetimes.rs — Vizualizácia Rust lifetimes
// Pre C/C++ programátorov: lifetime = doba platnosti referencie
// SPACE = ďalší scenár | Q = koniec

use macroquad::prelude::*;

const PADDING: f32 = 60.0;
const BAR_HEIGHT: f32 = 36.0;
const BAR_GAP: f32 = 18.0;
const LABEL_W: f32 = 160.0;
const INFO_FONT: f32 = 18.0;
const TITLE_FONT: f32 = 22.0;
const CODE_FONT: f32 = 17.0;

// Vykreslí jeden horizontálny pruh (lifetime scope)
fn draw_bar(label: &str, x_start: f32, x_end: f32, y: f32, color: Color, alpha: f32) {
    let c = Color { r: color.r, g: color.g, b: color.b, a: alpha };
    draw_rectangle(x_start, y, x_end - x_start, BAR_HEIGHT, c);
    draw_rectangle_lines(x_start, y, x_end - x_start, BAR_HEIGHT, 2.0, WHITE);
    // label naľavo od pruhu
    draw_text(label, x_start - LABEL_W, y + BAR_HEIGHT * 0.65, INFO_FONT, WHITE);
}

// Scenár 1: validný borrow — fn longest<'a>(x, y) -> &'a str
fn scene_valid_borrow(t: f32) {
    let sw = screen_width();
    let sh = screen_height();

    let bar_x0 = PADDING + LABEL_W;
    let bar_x1 = sw - PADDING;

    draw_text(
        "Scenár 1: Validný borrow — fn longest<'a>(x: &'a str, y: &'a str) -> &'a str",
        PADDING, 40.0, TITLE_FONT, GREEN,
    );
    draw_text(
        "Lifetime 'a zaručuje, že result nemôže prežiť vstupy.",
        PADDING, 68.0, CODE_FONT, GRAY,
    );

    // animate reveal
    let reveal = ((t * 0.5).min(1.0)) as f32;

    let y0 = 110.0;
    // 'a — modrý, celá šírka
    let a_x1 = bar_x0 + (bar_x1 - bar_x0) * reveal;
    draw_bar("'a (scope)", bar_x0, a_x1, y0, BLUE, 0.8);

    // x: &'a str — zelený, rovnaká dĺžka
    draw_bar("x: &'a str", bar_x0, a_x1, y0 + BAR_HEIGHT + BAR_GAP, GREEN, 0.85);

    // y: &'a str — zelený, rovnaká dĺžka
    draw_bar("y: &'a str", bar_x0, a_x1, y0 + (BAR_HEIGHT + BAR_GAP) * 2.0, GREEN, 0.85);

    // result: &'a str — žltý, kratší (75 % 'a, ale vnútri)
    let result_end = bar_x0 + (a_x1 - bar_x0) * 0.75;
    draw_bar("result: &'a str", bar_x0, result_end, y0 + (BAR_HEIGHT + BAR_GAP) * 3.0, YELLOW, 0.9);

    // šipka / legenda
    let legend_y = y0 + (BAR_HEIGHT + BAR_GAP) * 3.0 + BAR_HEIGHT + 20.0;
    draw_line(result_end, legend_y - 4.0, result_end, y0 + (BAR_HEIGHT + BAR_GAP) * 3.0, 2.0, YELLOW);
    draw_text("result končí skôr ako 'a", result_end + 8.0, legend_y, INFO_FONT, YELLOW);

    // výsledok
    let ok_y = sh - 90.0;
    draw_rectangle(PADDING - 10.0, ok_y - 28.0, sw - PADDING * 2.0 + 20.0, 44.0,
        Color { r: 0.0, g: 0.3, b: 0.0, a: 0.9 });
    draw_text(
        "✓  OK — result zije kratsie ako 'a, borrow je vzdy validny",
        PADDING, ok_y, INFO_FONT, GREEN,
    );
}

// Scenár 2: dangling reference — chyba
fn scene_dangling(t: f32) {
    let sw = screen_width();
    let sh = screen_height();

    let bar_x0 = PADDING + LABEL_W;
    let bar_x1 = sw - PADDING;
    let reveal = ((t * 0.5).min(1.0)) as f32;

    draw_text(
        "Scenár 2: Dangling reference — CHYBA kompilácie",
        PADDING, 40.0, TITLE_FONT, RED,
    );
    draw_text(
        "result by prezil 'x' — Rust to zachyti pri kompilácii, nie za behu!",
        PADDING, 68.0, CODE_FONT, GRAY,
    );

    let y0 = 110.0;
    let x_short = bar_x0 + (bar_x1 - bar_x0) * 0.45 * reveal;  // x je kratké
    let result_end = bar_x0 + (bar_x1 - bar_x0) * 0.85 * reveal; // result dlhší

    // x — zelený, krátky
    draw_bar("x: &str", bar_x0, x_short, y0, GREEN, 0.85);

    // result — červený, dlhší ako x
    draw_bar("result: &str", bar_x0, result_end, y0 + BAR_HEIGHT + BAR_GAP, RED, 0.85);

    // zvislá čiara kde x končí
    if reveal > 0.5 {
        let line_x = x_short;
        let line_top = y0 - 10.0;
        let line_bot = y0 + (BAR_HEIGHT + BAR_GAP) * 2.0;
        draw_line(line_x, line_top, line_x, line_bot, 3.0, RED);

        // X symbol
        let cx = line_x;
        let cy = y0 + BAR_HEIGHT + BAR_GAP * 0.5;
        draw_line(cx - 12.0, cy - 12.0, cx + 12.0, cy + 12.0, 4.0, RED);
        draw_line(cx + 12.0, cy - 12.0, cx - 12.0, cy + 12.0, 4.0, RED);

        draw_text("'x' zaniká tu!", line_x + 8.0, y0 - 12.0, INFO_FONT, RED);
        draw_text("result stale zije →", line_x + 8.0,
            y0 + BAR_HEIGHT + BAR_GAP + BAR_HEIGHT * 0.65, INFO_FONT, RED);
    }

    // C++ komentár
    let note_y = y0 + (BAR_HEIGHT + BAR_GAP) * 2.0 + 20.0;
    draw_text(
        "// V C++ by toto bol undefined behavior (visíci pointer)",
        PADDING, note_y, CODE_FONT,
        Color { r: 1.0, g: 0.6, b: 0.0, a: 1.0 },
    );
    draw_text(
        "// Rust odmietne skompilovať — ziadny runtime crash!",
        PADDING, note_y + 24.0, CODE_FONT,
        Color { r: 1.0, g: 0.6, b: 0.0, a: 1.0 },
    );

    let ok_y = sh - 90.0;
    draw_rectangle(PADDING - 10.0, ok_y - 28.0, sw - PADDING * 2.0 + 20.0, 44.0,
        Color { r: 0.3, g: 0.0, b: 0.0, a: 0.9 });
    draw_text(
        "✗  CHYBA — result by prezil 'x' | error[E0106]: missing lifetime specifier",
        PADDING, ok_y, INFO_FONT, RED,
    );
}

// Scenár 3: struct s lifetime
fn scene_struct_lifetime(t: f32) {
    let sw = screen_width();
    let sh = screen_height();
    let _ = t;

    draw_text(
        "Scenár 3: Struct s lifetime — struct Important<'a> { part: &'a str }",
        PADDING, 40.0, TITLE_FONT, YELLOW,
    );
    draw_text(
        "Struct nesmie prezit data na ktore ukazuje.",
        PADDING, 68.0, CODE_FONT, GRAY,
    );

    // String data box (vpravo hore)
    let data_x = sw - PADDING - 220.0;
    let data_y = 110.0;
    let data_w = 200.0;
    let data_h = 60.0;
    draw_rectangle(data_x, data_y, data_w, data_h, Color { r: 0.2, g: 0.4, b: 0.2, a: 1.0 });
    draw_rectangle_lines(data_x, data_y, data_w, data_h, 2.0, GREEN);
    draw_text("String data", data_x + 10.0, data_y + 22.0, INFO_FONT, GREEN);
    draw_text("\"hello world\"", data_x + 10.0, data_y + 44.0, CODE_FONT, WHITE);

    // lifetime 'a badge
    draw_text("lifetime 'a", data_x, data_y - 22.0, INFO_FONT, YELLOW);

    // Struct box (vľavo)
    let struct_x = PADDING + LABEL_W * 0.5;
    let struct_y = 100.0;
    let struct_w = 200.0;
    let struct_h = 120.0;
    draw_rectangle(struct_x, struct_y, struct_w, struct_h,
        Color { r: 0.2, g: 0.2, b: 0.4, a: 1.0 });
    draw_rectangle_lines(struct_x, struct_y, struct_w, struct_h, 2.0, BLUE);
    draw_text("Important<'a>", struct_x + 8.0, struct_y + 24.0, INFO_FONT, BLUE);
    // vnútorné pole
    let field_x = struct_x + 12.0;
    let field_y = struct_y + 52.0;
    draw_rectangle(field_x, field_y, struct_w - 24.0, 36.0,
        Color { r: 0.1, g: 0.1, b: 0.3, a: 1.0 });
    draw_rectangle_lines(field_x, field_y, struct_w - 24.0, 36.0, 1.5, YELLOW);
    draw_text("part: &'a str", field_x + 6.0, field_y + 22.0, CODE_FONT, YELLOW);

    // šípka z part na data
    let arrow_x0 = field_x + struct_w - 24.0;
    let arrow_y0 = field_y + 18.0;
    let arrow_x1 = data_x;
    let arrow_y1 = data_y + data_h * 0.5;
    draw_line(arrow_x0, arrow_y0, arrow_x1, arrow_y1, 2.5, YELLOW);
    // hrot šípky
    draw_line(arrow_x1 - 14.0, arrow_y1 - 8.0, arrow_x1, arrow_y1, 2.5, YELLOW);
    draw_line(arrow_x1 - 14.0, arrow_y1 + 8.0, arrow_x1, arrow_y1, 2.5, YELLOW);
    draw_text("ukazuje na →", (arrow_x0 + arrow_x1) * 0.5 - 50.0,
        (arrow_y0 + arrow_y1) * 0.5 - 12.0, INFO_FONT, YELLOW);

    // timeline pruhy
    let tl_y = struct_y + struct_h + 50.0;
    let tl_x0 = PADDING + LABEL_W;
    let tl_x1 = sw - PADDING;

    // 'a lifetime data
    draw_bar("'a (String data)", tl_x0, tl_x1, tl_y, GREEN, 0.7);
    // struct kratší
    draw_bar("Important<'a>", tl_x0, tl_x0 + (tl_x1 - tl_x0) * 0.7, tl_y + BAR_HEIGHT + BAR_GAP, BLUE, 0.8);

    draw_text(
        "Important zanikne skor — OK! Keby trvala dlhsie, Rust by odmietol.",
        PADDING, tl_y + (BAR_HEIGHT + BAR_GAP) * 2.0 + 10.0, INFO_FONT, WHITE,
    );

    // kód
    let code_y = sh - 150.0;
    draw_text("struct Important<'a> {", PADDING, code_y, CODE_FONT,
        Color { r: 0.7, g: 0.9, b: 1.0, a: 1.0 });
    draw_text("    part: &'a str,   // 'a = doba zivota dat", PADDING, code_y + 24.0, CODE_FONT,
        Color { r: 0.7, g: 0.9, b: 1.0, a: 1.0 });
    draw_text("}", PADDING, code_y + 48.0, CODE_FONT,
        Color { r: 0.7, g: 0.9, b: 1.0, a: 1.0 });

    let ok_y = sh - 70.0;
    draw_rectangle(PADDING - 10.0, ok_y - 28.0, sw - PADDING * 2.0 + 20.0, 44.0,
        Color { r: 0.3, g: 0.3, b: 0.0, a: 0.9 });
    draw_text(
        "Struct nesmie prezit data na ktore ukazuje — kompilator to hlida!",
        PADDING, ok_y, INFO_FONT, YELLOW,
    );
}

// Scenár 4: 'static lifetime
fn scene_static_lifetime(_t: f32) {
    let sw = screen_width();
    let sh = screen_height();

    draw_text(
        "Scenár 4: 'static lifetime — zije pocas celeho programu",
        PADDING, 40.0, TITLE_FONT, WHITE,
    );
    draw_text(
        "String literaly su vlozene priamo do binarky — nikdy nezaniknú.",
        PADDING, 68.0, CODE_FONT, GRAY,
    );

    let bar_x0 = PADDING + LABEL_W;
    let bar_x1 = sw - PADDING;
    let y0 = 120.0;

    // 'static — dlhý modrý pruh
    draw_bar("'static", bar_x0, bar_x1, y0, BLUE, 0.85);

    // label s šípkami na oboch koncoch
    draw_line(bar_x0, y0 + BAR_HEIGHT + 10.0, bar_x0, y0 + BAR_HEIGHT + 30.0, 2.0, WHITE);
    draw_line(bar_x1, y0 + BAR_HEIGHT + 10.0, bar_x1, y0 + BAR_HEIGHT + 30.0, 2.0, WHITE);
    draw_line(bar_x0, y0 + BAR_HEIGHT + 20.0, bar_x1, y0 + BAR_HEIGHT + 20.0, 2.0, WHITE);
    let mid = (bar_x0 + bar_x1) * 0.5;
    draw_text("od spustenia po ukoncenie programu",
        mid - 170.0, y0 + BAR_HEIGHT + 46.0, INFO_FONT, WHITE);

    // príklady
    let ex_y = y0 + BAR_HEIGHT + 80.0;
    let examples: &[(&str, &str, Color)] = &[
        ("let s: &'static str = \"hello\";",
         "// vlozene v .rodata sekcii binarky", GREEN),
        ("static GREETING: &str = \"Ahoj!\";",
         "// staticka premenna — 'static implicitne", YELLOW),
        ("const VERSION: &str = \"1.0.0\";",
         "// konstanty su tiez 'static", YELLOW),
    ];
    for (i, (code, comment, col)) in examples.iter().enumerate() {
        let ey = ex_y + i as f32 * 52.0;
        draw_rectangle(PADDING - 4.0, ey - 22.0, sw - PADDING * 2.0 + 8.0, 46.0,
            Color { r: 0.05, g: 0.05, b: 0.15, a: 1.0 });
        draw_text(code, PADDING, ey, CODE_FONT, *col);
        draw_text(comment, PADDING + 4.0, ey + 22.0, 15.0, GRAY);
    }

    // C++ porovnanie
    let cmp_y = ex_y + 3.0 * 52.0 + 20.0;
    draw_text("// C++ ekvivalent:", PADDING, cmp_y, CODE_FONT,
        Color { r: 1.0, g: 0.6, b: 0.0, a: 1.0 });
    draw_text("const char* s = \"hello\";   // string literal ma static storage duration",
        PADDING, cmp_y + 24.0, CODE_FONT,
        Color { r: 1.0, g: 0.6, b: 0.0, a: 1.0 });
    draw_text("// Rust 'static je explicitny — ziadne surprisy s dobou zivota.",
        PADDING, cmp_y + 48.0, CODE_FONT,
        Color { r: 1.0, g: 0.6, b: 0.0, a: 1.0 });

    let ok_y = sh - 70.0;
    draw_rectangle(PADDING - 10.0, ok_y - 28.0, sw - PADDING * 2.0 + 20.0, 44.0,
        Color { r: 0.0, g: 0.0, b: 0.25, a: 0.9 });
    draw_text(
        "String literals su 'static — ziju pocas celeho programu, vlozene v binarke",
        PADDING, ok_y, INFO_FONT, WHITE,
    );
}

#[macroquad::main("Lifetimes — Rust pre C/C++ programátorov")]
async fn main() {
    let mut scene: usize = 0;
    let num_scenes = 4;
    let mut scene_start = get_time();

    // ikony pre scenáre
    let scene_colors = [GREEN, RED, YELLOW, WHITE];
    let scene_icons = ["✓", "✗", "△", "★"];

    loop {
        let now = get_time();
        let t = (now - scene_start) as f32;

        if is_key_pressed(KeyCode::Space) {
            scene = (scene + 1) % num_scenes;
            scene_start = get_time();
        }
        if is_key_pressed(KeyCode::Q) {
            break;
        }
        // priame prepínanie číslicami 1-4
        for (i, key) in [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3, KeyCode::Key4]
            .iter()
            .enumerate()
        {
            if is_key_pressed(*key) {
                scene = i;
                scene_start = get_time();
            }
        }

        clear_background(Color { r: 0.05, g: 0.05, b: 0.1, a: 1.0 });

        match scene {
            0 => scene_valid_borrow(t),
            1 => scene_dangling(t),
            2 => scene_struct_lifetime(t),
            3 => scene_static_lifetime(t),
            _ => {}
        }

        // Dolný stavový panel
        let sw = screen_width();
        let sh = screen_height();
        draw_rectangle(0.0, sh - 34.0, sw, 34.0,
            Color { r: 0.1, g: 0.1, b: 0.1, a: 0.95 });

        // ikony scenárov
        let total_icons_w = num_scenes as f32 * 36.0;
        let icons_x = sw * 0.5 - total_icons_w * 0.5 - 80.0;
        for i in 0..num_scenes {
            let col = if i == scene { scene_colors[i] }
                      else { Color { r: 0.4, g: 0.4, b: 0.4, a: 1.0 } };
            draw_text(scene_icons[i], icons_x + i as f32 * 36.0, sh - 10.0, 20.0, col);
        }

        let status = format!(
            "SPACE = dalsi | 1-4 = scenár | Q = quit  [{}/{}]",
            scene + 1, num_scenes
        );
        draw_text(&status, icons_x + total_icons_w + 16.0, sh - 10.0, INFO_FONT, DARKGRAY);

        next_frame().await;
    }
}
