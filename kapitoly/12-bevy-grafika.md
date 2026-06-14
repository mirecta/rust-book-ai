# Kapitola 12 — Bevy: Grafika, Vstup, Pohyb

Teraz dostaneme niečo na obrazovku. Sprite, pohyb hráča, ohraničenia okna. Ale predtým sa porozprávajme o tom, ako Bevy premýšľa o grafike — pretože je to iné ako Unity alebo Unreal, a tá odlišnosť má dopad na to, ako píšeš kód.

---

## Ako Bevy renderuje — bez "render loop" kódu

V Unity píšeš `OnRenderObject()` alebo `Graphics.DrawMesh()`. V Unreal override-uješ `Draw()`. Ty ručne hovoríš rendereru čo nakresliť.

Bevy to robí inak: **renderer je systém ktorý reaguje na komponenty**. Ak entita má `Sprite` a `Transform`, Bevy renderer to automaticky nakreslí. Nevieš kedy presne — len vieš, že sa to stane. Toto je čistý ECS prístup: dáta (komponenty) diktujú správanie (renderer).

```
Unity:           Bevy:
OnRenderObject() <-- ty   Sprite + Transform --> Bevy renderer system
Graphics.Draw()  <-- ty   [nič nepíšeš]      --> automaticky
```

Výhoda: renderer môže robiť frustum culling, batching a sorting automaticky bez tvojho zásahu. Nevýhoda: menej priamej kontroly (ale Bevy má `RenderLayers` a ďalšie mechanizmy ak to potrebuješ).

---

## Súradnicový systém

Bevy používa pravotočivý súradnicový systém:
- X: vpravo +
- Y: hore +
- Z: bližšie ku kamere + (vrstvy/layers)

Stred obrazovky je `(0, 0)`. Okno 800×600 → viditeľná oblasť `[-400, 400]` × `[-300, 300]`.

Pozor: Unity 2D používa rovnaký systém, ale Unreal 2D používa Y dole. Ak prídeš z Unrealu, Y os je otočená.

```rust
// Transformácia entít
Transform::from_xyz(x, y, z)
Transform::from_translation(Vec3::new(x, y, z))

// Rotácia (radiány)
Transform::from_rotation(Quat::from_rotation_z(std::f32::consts::PI / 4.0))

// Zložená transformácia
commands.spawn((
    Transform {
        translation: Vec3::new(100.0, -50.0, 1.0),
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    },
    // ...
));
```

### Z-os ako vrstvy (layers)

V 2D hrách je Z os kritická — určuje poradie kreslenia. Vyššie Z = bližšie ku kamere = nakreslené navrchu.

```rust
// Pozadie (nakreslí sa ako prvé, teda pod všetkým)
Transform::from_xyz(0.0, 0.0, 0.0)

// Herné objekty — vrstva 1
Transform::from_xyz(100.0, 50.0, 1.0)

// HUD elementy — vždy navrchu herného sveta
Transform::from_xyz(0.0, 0.0, 10.0)
```

Bevy automaticky sortuje sprite-y podľa Z — nemusíš ručne riadiť poradie vykresľovania.

---

## Sprite — obrázok alebo farebný obdĺžnik

```rust
// Farebný obdĺžnik (bez textúry)
commands.spawn((
    Sprite {
        color: Color::srgb(0.2, 0.7, 1.0),
        custom_size: Some(Vec2::new(64.0, 64.0)),
        ..default()
    },
    Transform::from_xyz(0.0, 0.0, 0.0),
));

// S textúrou
commands.spawn((
    Sprite {
        image: asset_server.load("sprites/player.png"),
        ..default()
    },
    Transform::from_xyz(0.0, 0.0, 1.0),
));

// Spritesheet (animácia)
commands.spawn((
    Sprite {
        image: asset_server.load("sprites/sheet.png"),
        texture_atlas: Some(TextureAtlas {
            layout: texture_atlas_layouts.add(
                TextureAtlasLayout::from_grid(UVec2::new(32, 32), 4, 1, None, None)
            ),
            index: 0,
        }),
        ..default()
    },
    Transform::default(),
));
```

### Ako funguje asset loading

`asset_server.load("sprites/player.png")` **neblokuje**. Vracia `Handle<Image>` okamžite — handle je len ID, obrázok sa načíta na pozadí. Kým nie je načítaný, Bevy nekreslí sprite (alebo kreslí placeholder). Toto je neblokujúci async loading — podobne ako Unity's `Resources.LoadAsync()`.

Preto ak chceš čakať kým sa assety načítajú (napr. loading screen), potrebuješ `AssetServer::is_loaded_with_dependencies()` alebo `bevy_asset_loader` crate (odporúčané pre väčšie projekty).

### Color sRGB vs lineárne

```rust
// sRGB — "vizuálne" farby (čo vidíš v Photoshope)
Color::srgb(1.0, 0.5, 0.0)   // oranžová

// Lineárne RGB — fyzikálne korektné (pre shading výpočty)
Color::linear_rgb(1.0, 0.5, 0.0)

// Hex kód (sRGB)
Color::srgb_u8(255, 128, 0)
```

Pre UI a sprite-y použi sRGB — to zodpovedá tomu čo vidíš v grafickom editore. Pre svetlá a shading použi lineárne (alebo nechaj Bevy konvertovať automaticky).

---

## Vstup — klávesnica a myš

```rust
use bevy::input::ButtonInput;

fn handle_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut Transform, With<Player>>,
) {
    let Ok(mut transform) = query.get_single_mut() else { return };

    // pressed() — drží sa kláves (kontinuálny pohyb)
    // just_pressed() — len v momente stlačenia (toggle)
    // just_released() — len v momente uvoľnenia

    if keys.pressed(KeyCode::ArrowLeft) || keys.pressed(KeyCode::KeyA) {
        transform.translation.x -= 5.0;
    }
    if keys.pressed(KeyCode::ArrowRight) || keys.pressed(KeyCode::KeyD) {
        transform.translation.x += 5.0;
    }
    if keys.just_pressed(KeyCode::Space) {
        println!("Medzera stlačená raz");
    }
    if keys.just_pressed(KeyCode::Escape) {
        std::process::exit(0);
    }
}

fn handle_mouse(
    mouse: Res<ButtonInput<MouseButton>>,
    mut motion: EventReader<bevy::input::mouse::MouseMotion>,
) {
    if mouse.just_pressed(MouseButton::Left) {
        println!("ľavé tlačidlo");
    }

    for event in motion.read() {
        println!("pohyb myši: {:?}", event.delta);
    }
}
```

### Rozdiely medzi pressed, just_pressed, just_released

Toto je jedno z miest kde začiatočníci robia chyby:

```rust
// pressed() — TRUE každý frame kým je kláves dole
// Použitie: kontinuálny pohyb, držanie tlačidla
if keys.pressed(KeyCode::ArrowLeft) {
    tf.translation.x -= speed * dt;  // pohyb každý frame
}

// just_pressed() — TRUE len JEDEN frame (frame kedy kláves bol stlačený)
// Použitie: skok, strela, toggle
if keys.just_pressed(KeyCode::Space) {
    ev.send(JumpEvent);   // odošle sa raz, nie každý frame
}

// just_released() — TRUE len JEDEN frame (frame kedy bol uvoľnený)
// Použitie: "charge and release" mechanic, nabíjanie
if keys.just_released(KeyCode::Space) {
    let charge = timer.elapsed_secs();
    ev.send(ChargedShotEvent { power: charge });
}
```

### Pozícia myši v hernom svete

```rust
fn mouse_world_position(
    windows: Query<&Window>,
    camera: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
) {
    let Ok(window) = windows.get_single() else { return };
    let Ok((camera, camera_tf)) = camera.get_single() else { return };

    if let Some(cursor_pos) = window.cursor_position() {
        // Konverzia z window koordinát do world koordinát
        if let Ok(world_pos) = camera.viewport_to_world_2d(camera_tf, cursor_pos) {
            info!("Myš je na pozícii: {:?}", world_pos);
        }
    }
}
```

Toto je dôležité — `window.cursor_position()` vracia pixely obrazovky (0,0 vľavo hore). Na konverziu do herného sveta (kde 0,0 je stred) potrebuješ kameru.

---

## Delta time — frame-rate independent pohyb

```rust
// NIKDY nepohybuj o fixnú hodnotu za frame — fps závisí od hardvéru
// VŽDY násob delta time-om

fn move_player(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&mut Transform, &Speed), With<Player>>,
) {
    let Ok((mut transform, speed)) = query.get_single_mut() else { return };

    let dt = time.delta_secs();  // sekundy od posledného frame-u (~0.016 pri 60fps)

    let mut velocity = Vec2::ZERO;
    if keys.pressed(KeyCode::ArrowLeft)  { velocity.x -= 1.0; }
    if keys.pressed(KeyCode::ArrowRight) { velocity.x += 1.0; }
    if keys.pressed(KeyCode::ArrowUp)    { velocity.y += 1.0; }
    if keys.pressed(KeyCode::ArrowDown)  { velocity.y -= 1.0; }

    let velocity = velocity.normalize_or_zero() * speed.0;
    transform.translation.x += velocity.x * dt;
    transform.translation.y += velocity.y * dt;
}

#[derive(Component)]
struct Speed(f32);  // pixels per second
```

### Prečo normalize_or_zero() a nie len normalize()

Keď stlačíš ArrowLeft aj ArrowUp zároveň, smer je `Vec2(-1, 1)`. Jeho dĺžka je `√2 ≈ 1.41`. Bez normalizácie by hráč šiel diagonálne 41% rýchlejšie. `normalize()` panics ak je vektor nulový — `normalize_or_zero()` vráti `Vec2::ZERO` ak si nestlačil nič. Vždy použi `normalize_or_zero()` pre smer pohybu.

### FixedUpdate pre fyziku

Pre fyzikálne simulácie je `Update` problematický — ak fps klesne, `dt` sa zvýši a fyzika môže byť nestabilná (predmety prechádzajú stenami). Riešenie:

```rust
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(FixedUpdate, physics_step)  // beží fixne 64x/s
        .add_systems(Update, render_interpolation)  // interpolácia pre plynulosť
        .run();
}
```

`FixedUpdate` beží s fixným timestepom — ak je hra pomalá, beží viac kráv za frame; ak je rýchla, preskočí niektoré. Fyzika je vždy deterministická.

---

## Ohraničenie pohybu (clamping)

```rust
fn clamp_to_window(
    windows: Query<&Window>,
    mut query: Query<(&mut Transform, &Sprite)>,
) {
    let Ok(window) = windows.get_single() else { return };
    let half_w = window.width() / 2.0;
    let half_h = window.height() / 2.0;

    for (mut transform, sprite) in &mut query {
        let size = sprite.custom_size.unwrap_or(Vec2::new(32.0, 32.0));
        let half_size = size / 2.0;

        transform.translation.x = transform.translation.x
            .clamp(-half_w + half_size.x, half_w - half_size.x);
        transform.translation.y = transform.translation.y
            .clamp(-half_h + half_size.y, half_h - half_size.y);
    }
}
```

Clamping odpočítava `half_size` od okraja — tým zaistíme, že sprite zostáva celý vo viditeľnej oblasti, nie len jeho stred.

---

## Animácie so SpriteSheety

Spritesheet je jeden obrázok s viacerými frameami animácie vedľa seba. Je to starý ale efektívny trik — jeden draw call namiesto viacerých.

```rust
#[derive(Component)]
struct AnimationTimer(Timer);

#[derive(Component)]
struct AnimationConfig {
    first_frame: usize,
    last_frame: usize,
}

fn setup_animated_sprite(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let layout = layouts.add(
        TextureAtlasLayout::from_grid(
            UVec2::new(32, 32),  // veľkosť jedného frame-u
            8,                    // stĺpce
            1,                    // riadky
            None, None,
        )
    );

    commands.spawn((
        Sprite {
            image: asset_server.load("sprites/player_walk.png"),
            texture_atlas: Some(TextureAtlas { layout, index: 0 }),
            ..default()
        },
        Transform::default(),
        AnimationTimer(Timer::from_seconds(0.1, TimerMode::Repeating)),
        AnimationConfig { first_frame: 0, last_frame: 7 },
    ));
}

fn animate_sprites(
    time: Res<Time>,
    mut q: Query<(&mut AnimationTimer, &mut Sprite, &AnimationConfig)>,
) {
    for (mut timer, mut sprite, config) in &mut q {
        timer.0.tick(time.delta());
        if timer.0.just_finished() {
            if let Some(atlas) = &mut sprite.texture_atlas {
                atlas.index = if atlas.index >= config.last_frame {
                    config.first_frame
                } else {
                    atlas.index + 1
                };
            }
        }
    }
}
```

Toto je čistý ECS prístup k animáciám — žiadny `Animator` objekt, len komponenty a systém. Môžeš ľahko pridať rôzne konfigurácie animácie pre rôzne stavy (beh, útok, idle) zmenou `AnimationConfig`.

---

## Transform hierarchia — rodič-potomok

V hrách potrebuješ aby sa zbraň pohybovala spolu s hráčom, alebo aby kamera nasledovala cieľ. Bevy rieši toto cez parent-child hierarchiu.

```rust
fn spawn_player_with_weapon(mut commands: Commands) {
    // Spawn hráča a zároveň jeho "detí"
    commands.spawn((
        Player,
        Transform::from_xyz(0.0, 0.0, 1.0),
        Sprite {
            color: Color::srgb(0.2, 0.6, 1.0),
            custom_size: Some(Vec2::new(48.0, 48.0)),
            ..default()
        },
    )).with_children(|parent| {
        // Zbraň — súradnice sú RELATÍVNE k hráčovi
        parent.spawn((
            Weapon,
            Transform::from_xyz(20.0, 0.0, 0.1),  // 20px vpravo od hráča
            Sprite {
                color: Color::srgb(0.8, 0.8, 0.1),
                custom_size: Some(Vec2::new(10.0, 30.0)),
                ..default()
            },
        ));
    });
}
```

Keď hráč sa pohne, zbraň sa pohne automaticky — lebo jej `Transform` je relatívny k parentovi. `GlobalTransform` komponent (automaticky pridaný Bevy) vždy obsahuje absolútnu pozíciu vo svete.

---

## Kompletný príklad: pohyblivý hráč

```rust
use bevy::prelude::*;

#[derive(Component)]
struct Player;

#[derive(Component)]
struct Speed(f32);

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Pohyb hráča".into(),
                resolution: (800.0, 600.0).into(),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(Update, (move_player, clamp_to_window).chain())
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    commands.spawn((
        Player,
        Speed(300.0),
        Sprite {
            color: Color::srgb(0.2, 0.6, 1.0),
            custom_size: Some(Vec2::new(48.0, 48.0)),
            ..default()
        },
        Transform::from_xyz(0.0, -200.0, 1.0),
    ));
}

fn move_player(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&mut Transform, &Speed), With<Player>>,
) {
    let Ok((mut tf, speed)) = query.get_single_mut() else { return };
    let mut dir = Vec2::ZERO;
    if keys.pressed(KeyCode::ArrowLeft)  { dir.x -= 1.0; }
    if keys.pressed(KeyCode::ArrowRight) { dir.x += 1.0; }
    if keys.pressed(KeyCode::ArrowUp)    { dir.y += 1.0; }
    if keys.pressed(KeyCode::ArrowDown)  { dir.y -= 1.0; }
    let v = dir.normalize_or_zero() * speed.0;
    tf.translation.x += v.x * time.delta_secs();
    tf.translation.y += v.y * time.delta_secs();
}

fn clamp_to_window(
    windows: Query<&Window>,
    mut query: Query<&mut Transform, With<Player>>,
) {
    let Ok(window) = windows.get_single() else { return };
    let (hw, hh) = (window.width() / 2.0 - 24.0, window.height() / 2.0 - 24.0);
    let Ok(mut tf) = query.get_single_mut() else { return };
    tf.translation.x = tf.translation.x.clamp(-hw, hw);
    tf.translation.y = tf.translation.y.clamp(-hh, hh);
}
```

Spusti: `cargo run -p bevy-hra` — modrý štvorec ovládaný šípkami.

---

## Časté chyby pri grafike

### 1. Zabudnutá kamera

```rust
// Bez kamera entita — obrazovka ostane čierna
fn setup(mut commands: Commands) {
    // commands.spawn(Camera2d);  <-- ZABUDNUTÉ!
    commands.spawn(Sprite { /* ... */ });  // nikdy sa nezobrazí
}
```

### 2. Sprite sa zobrazuje ale nie je vidieť — Z conflict

```rust
// Hráč a pozadie na rovnakom Z — nedefinované poradie
commands.spawn(( /* background */ Transform::from_xyz(0.0, 0.0, 0.0) ));
commands.spawn(( /* player */     Transform::from_xyz(0.0, 0.0, 0.0) ));  // môže byť pod BG!

// Správne — explicitné Z vrstvy
commands.spawn(( /* background */ Transform::from_xyz(0.0, 0.0, 0.0) ));
commands.spawn(( /* player */     Transform::from_xyz(0.0, 0.0, 1.0) ));
```

### 3. Asset path — relatívna cesta od assets/

```rust
// Bevy hľadá súbory v priečinku "assets/" relatívne k executable
// Správna štruktúra:
// my_game/
//   assets/
//     sprites/
//       player.png
//   src/
//     main.rs

asset_server.load("sprites/player.png")  // NIE "assets/sprites/player.png"
```

---

## Debuggovanie grafiky

### Zobraziť pozície entít

```rust
// Pridaj do Update systémov len pre debug:
fn debug_positions(q: Query<(Entity, &Transform, Option<&Player>)>) {
    for (entity, tf, player) in &q {
        if player.is_some() {
            debug!("Player pos: {:?}", tf.translation);
        }
    }
}
```

### Gizmos — vizuálne debugovanie

```rust
fn draw_collider_gizmos(
    mut gizmos: Gizmos,
    q: Query<&Transform, With<Enemy>>,
) {
    for tf in &q {
        // Nakresli kruh s polomerom kolízie
        gizmos.circle_2d(
            tf.translation.truncate(),
            22.0,  // polomer
            Color::srgb(1.0, 0.0, 0.0),
        );
    }
}
```

Aktivuj v systémoch len počas vývoja — jednoducho odober z `.add_systems(Update, ...)` pre release.

---

Ďalšia kapitola: kolízie, strely a herná logika.
