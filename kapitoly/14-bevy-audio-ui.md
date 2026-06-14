# Kapitola 14 — Bevy: UI a Finálna Hra

Posledná Bevy kapitola. Pridáme HUD so skóre, menu obrazovku a dáme všetko dokopy do funkčnej hry. Ale najprv sa porozprávajme o tom, čo je Bevy UI a prečo funguje inak ako si zvyknutý z Unity alebo webového vývoja.

> **Poznámka k audio:** Bevy má `AudioPlugin` (súčasť `DefaultPlugins`) a `AudioPlayer` komponent. Vyžaduje asset súbory (`.ogg`, `.wav`). Pre jednoduchosť tejto verzie hry audio vynecháme — hra funguje bez externých súborov.

---

## Prečo je Bevy UI iné (a prečo to dáva zmysel)

V Unity máš Canvas s GameObject hierarchiou. V HTML máš DOM. V Qt máš widgety. Každý z týchto systémov má UI ako špeciálny "podsystém" s vlastnými pravidlami.

Bevy ho nemá. UI v Bevy sú len entity s komponentmi — rovnaký ECS ako pre hráča a nepriateľov. `Node` komponent hovorí "toto je UI element", `Text` hovorí "toto je text". Renderer to nakreslí inak ako sprite, ale architektúra je rovnaká.

Prečo je to dobrá voľba? Pretože môžeš aplikovať úplne rovnaké patterny — query-e, eventy, stavy — na UI ako na herne entity. Chceš animovať HP bar? Systém ktorý mení `Node.width` na základe `Health`. Chceš UI ktoré reaguje na stav hry? `run_if(in_state(...))`. Žiadne špeciálne "UI scripting" API.

---

## UI — Bevy Node systém

Bevy UI je flex-based (podobné CSS Flexbox). Každý UI prvok je entita s `Node` komponentom. Ak poznáš Flexbox z webového vývoja, budeš sa cítiť ako doma. Ak nie — základný koncept je "kontajner s deťmi, ktorý rozkladá deti vodorovne alebo zvisle".

```rust
fn setup_hud(mut commands: Commands) {
    // Root node — pás naprieč celou obrazovkou
    commands.spawn((
        UiRoot,                         // marker pre cleanup
        Node {
            width: Val::Percent(100.0),
            padding: UiRect::all(Val::Px(14.0)),
            justify_content: JustifyContent::SpaceBetween,  // skóre vľavo, životy vpravo
            ..default()
        },
    )).with_children(|p| {
        p.spawn((
            ScoreText,
            Text::new("Skóre: 0"),
            TextFont { font_size: 24.0, ..default() },
            TextColor(Color::WHITE),
        ));
        p.spawn((
            HealthText,
            Text::new("♥ ♥ ♥"),
            TextFont { font_size: 24.0, ..default() },
            TextColor(Color::srgb(1.0, 0.3, 0.3)),
        ));
    });
}
```

### Aktualizácia textu

```rust
fn update_hud(
    score: Res<Score>,
    player: Query<&Health, With<Player>>,
    mut s_txt: Query<&mut Text, (With<ScoreText>, Without<HealthText>)>,
    mut h_txt: Query<&mut Text, (With<HealthText>, Without<ScoreText>)>,
) {
    if let Ok(mut t) = s_txt.get_single_mut() {
        **t = format!("Skóre: {}", score.0);  // ** = deref Text na String
    }
    if let (Ok(hp), Ok(mut t)) = (player.get_single(), h_txt.get_single_mut()) {
        **t = "♥ ".repeat(hp.0.max(0.0) as usize);
    }
}
```

`Without<HealthText>` v query — Bevy vyžaduje rozlíšenie keď queruješ rovnaký komponent (`Text`) cez viacero premenných. Je to mutable aliasing guard na úrovni ECS.

---

## Val — jednotky rozmerov

`Val` je enum ktorý reprezentuje CSS-like jednotky:

```rust
// Pevný počet pixelov
Val::Px(24.0)

// Percento rodiča
Val::Percent(100.0)

// Automatické — obsah určuje veľkosť
Val::Auto

// Viewport relatívne (nezávisle od rodiča)
Val::Vw(50.0)   // 50% šírky viewportu
Val::Vh(25.0)   // 25% výšky viewportu
```

Typické použitie:

```rust
Node {
    width: Val::Percent(100.0),     // celá šírka rodiča
    height: Val::Px(60.0),          // fixná výška HUD
    padding: UiRect::all(Val::Px(10.0)),
    margin: UiRect {
        top: Val::Px(5.0),
        bottom: Val::Auto,          // automatické centrování
        left: Val::Px(0.0),
        right: Val::Px(0.0),
    },
    ..default()
}
```

---

## Flexbox layout — ako rozmiestiť UI elementy

Flexbox má dva smery: hlavná os a krížová os. `FlexDirection` určuje hlavnú os:

```rust
// Deti vedľa seba (vodorovne) — default
flex_direction: FlexDirection::Row

// Deti pod sebou (zvisle)
flex_direction: FlexDirection::Column
```

`justify_content` riadi rozkladanie pozdĺž hlavnej osi:

```rust
justify_content: JustifyContent::Center          // v strede
justify_content: JustifyContent::SpaceBetween    // prvý vľavo, posledný vpravo, ostatné rovnomerne
justify_content: JustifyContent::FlexStart       // všetky vľavo (default)
justify_content: JustifyContent::FlexEnd         // všetky vpravo
```

`align_items` riadi zarovnanie pozdĺž krížovej osi:

```rust
align_items: AlignItems::Center      // centrovanie (vertikálne ak Row, horizontálne ak Column)
align_items: AlignItems::Stretch     // tiahnu sa na plnú šírku/výšku
align_items: AlignItems::FlexStart   // zarovnané na začiatok
```

Príklad — centrovanie menu:

```rust
Node {
    width: Val::Percent(100.0),
    height: Val::Percent(100.0),
    flex_direction: FlexDirection::Column,
    justify_content: JustifyContent::Center,  // vertikálne centrovanie
    align_items: AlignItems::Center,          // horizontálne centrovanie
    row_gap: Val::Px(24.0),                  // medzera medzi riadkami
    ..default()
}
```

---

## Menu obrazovka

```rust
fn setup_menu(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn((
        UiRoot,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: Val::Px(24.0),
            ..default()
        },
    )).with_children(|p| {
        p.spawn((
            Text::new("ASTEROID SHOOTER"),
            TextFont { font_size: 52.0, ..default() },
            TextColor(Color::WHITE),
        ));
        p.spawn((
            Text::new("[ ENTER ] — Štart"),
            TextFont { font_size: 26.0, ..default() },
            TextColor(Color::srgb(0.7, 0.7, 0.7)),
        ));
    });
}

fn menu_input(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<GameState>>) {
    if keys.just_pressed(KeyCode::Enter) {
        next.set(GameState::Playing);
    }
}
```

---

## Cleanup — UiRoot pattern

Všetky UI entity dostanú marker `UiRoot`. Pri zmene stavu stačí jeden query:

```rust
fn cleanup_ui(mut commands: Commands, q: Query<Entity, With<UiRoot>>) {
    for e in &q { commands.entity(e).despawn_recursive(); }
}
```

`despawn_recursive()` — zmaže entitu aj všetkých jej potomkov (children z `with_children`). Keby si použil len `despawn()`, rootová entita by sa zmazala, ale children by ostali "osirotení" v ECS world (bez parenta, ale stále existujúci). Vždy pre UI použi `despawn_recursive()`.

---

## Klikateľné tlačidlá

Bevy má `Button` komponent a `Interaction` komponent pre sledovanie hover/click stavu:

```rust
fn setup_button(mut commands: Commands) {
    commands.spawn((
        UiRoot,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
    )).with_children(|p| {
        p.spawn((
            Button,
            StartButton,                          // marker pre identifikáciu
            Node {
                width: Val::Px(200.0),
                height: Val::Px(60.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.2, 0.5, 0.8)),
        )).with_children(|btn| {
            btn.spawn((
                Text::new("ŠTART"),
                TextFont { font_size: 28.0, ..default() },
                TextColor(Color::WHITE),
            ));
        });
    });
}

#[derive(Component)]
struct StartButton;

fn button_interaction(
    mut interaction_q: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<StartButton>)
    >,
    mut next: ResMut<NextState<GameState>>,
) {
    for (interaction, mut bg) in &mut interaction_q {
        match interaction {
            Interaction::Pressed => {
                *bg = BackgroundColor(Color::srgb(0.1, 0.3, 0.6));  // tmavší
                next.set(GameState::Playing);
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(Color::srgb(0.3, 0.6, 1.0));  // svetlejší
            }
            Interaction::None => {
                *bg = BackgroundColor(Color::srgb(0.2, 0.5, 0.8));  // normálny
            }
        }
    }
}
```

`Changed<Interaction>` je efektívny filter — systém beží len keď sa interakcia zmenila, nie každý frame. Na tisíce UI elementov je to rozdiel.

---

## Progressbar — dynamický HP bar

```rust
#[derive(Component)]
struct HpBar;

fn setup_hp_bar(mut commands: Commands) {
    commands.spawn((
        UiRoot,
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(20.0),
            left: Val::Px(20.0),
            width: Val::Px(200.0),
            height: Val::Px(20.0),
            ..default()
        },
        BackgroundColor(Color::srgb(0.2, 0.2, 0.2)),
    )).with_children(|p| {
        p.spawn((
            HpBar,
            Node {
                width: Val::Percent(100.0),  // začíname na 100%
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.2, 0.8, 0.2)),
        ));
    });
}

fn update_hp_bar(
    player: Query<&Health, (With<Player>, Changed<Health>)>,  // len ak sa zmenilo HP
    mut hp_bar: Query<&mut Node, With<HpBar>>,
) {
    let Ok(hp) = player.get_single() else { return };
    let Ok(mut node) = hp_bar.get_single_mut() else { return };

    let percent = (hp.0 / 3.0 * 100.0).clamp(0.0, 100.0);
    node.width = Val::Percent(percent);
}
```

---

## Audio — zvukové efekty

Ak máš `.ogg` súbor v `assets/`:

```rust
fn play_shoot_sound(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut ev: EventReader<Fired>,
) {
    for _ in ev.read() {
        commands.spawn(AudioPlayer::new(
            asset_server.load("audio/shoot.ogg")
        ));
    }
}

// Hudba na pozadí (loopovaná)
fn start_music(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        AudioPlayer::new(asset_server.load("audio/music.ogg")),
        PlaybackSettings::LOOP,
    ));
}
```

Každý `AudioPlayer` spawn vytvorí nový zvukový kanál. Bevy automaticky mixuje viacero zvukov. Pre zastavenie hudby potrebuješ si zapamätať entity:

```rust
#[derive(Resource)]
struct MusicEntity(Entity);

fn start_music(mut commands: Commands, asset_server: Res<AssetServer>) {
    let entity = commands.spawn((
        AudioPlayer::new(asset_server.load("audio/music.ogg")),
        PlaybackSettings::LOOP,
    )).id();
    commands.insert_resource(MusicEntity(entity));
}

fn stop_music(mut commands: Commands, music: Res<MusicEntity>) {
    commands.entity(music.0).despawn();
}
```

---

## Finálna hra — kompletný zdrojový kód

Toto je skutočný fungujúci kód z `priklady/bevy-hra/src/main.rs`:

```rust
use bevy::prelude::*;
use std::time::Duration;

// --- Komponenty ---
#[derive(Component)] struct Player;
#[derive(Component)] struct Enemy;
#[derive(Component)] struct Bullet;
#[derive(Component)] struct Speed(f32);
#[derive(Component)] struct Vel(Vec2);
#[derive(Component)] struct Health(f32);
#[derive(Component)] struct ScoreText;
#[derive(Component)] struct HealthText;
#[derive(Component)] struct UiRoot;

// --- Resources ---
#[derive(Resource, Default)] struct Score(u32);
#[derive(Resource)] struct ShootTimer(Timer);
#[derive(Resource)] struct SpawnTimer(Timer);

// --- Events ---
#[derive(Event)] struct Fired { pos: Vec2 }
#[derive(Event)] struct Killed;

// --- Stavy ---
#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
enum GameState { #[default] Menu, Playing, GameOver }

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Asteroid Shooter".into(),
                resolution: (800.0, 600.0).into(),
                ..default()
            }),
            ..default()
        }))
        .init_state::<GameState>()
        .init_resource::<Score>()
        .insert_resource(ShootTimer(Timer::new(Duration::from_millis(250), TimerMode::Repeating)))
        .insert_resource(SpawnTimer(Timer::new(Duration::from_millis(1400), TimerMode::Repeating)))
        .add_event::<Fired>()
        .add_event::<Killed>()
        .add_systems(OnEnter(GameState::Menu),   setup_menu)
        .add_systems(OnExit(GameState::Menu),    cleanup_ui)
        .add_systems(Update, menu_input.run_if(in_state(GameState::Menu)))
        .add_systems(OnEnter(GameState::Playing), (spawn_camera, setup_game, setup_hud).chain())
        .add_systems(OnExit(GameState::Playing),  cleanup_game)
        .add_systems(Update, (
            player_move, player_shoot, spawn_bullets,
            move_all, spawn_enemies, check_collisions,
            tally_kills, despawn_offscreen, update_hud,
        ).run_if(in_state(GameState::Playing)))
        .add_systems(OnEnter(GameState::GameOver), show_gameover)
        .add_systems(Update, restart_input.run_if(in_state(GameState::GameOver)))
        .run();
}

fn spawn_camera(mut commands: Commands) { commands.spawn(Camera2d); }

fn setup_game(mut commands: Commands) {
    commands.spawn((
        Player, Speed(300.0), Health(3.0), Vel(Vec2::ZERO),
        Sprite { color: Color::srgb(0.2, 0.6, 1.0),
                 custom_size: Some(Vec2::new(48.0, 48.0)), ..default() },
        Transform::from_xyz(0.0, -220.0, 1.0),
    ));
}

fn player_move(
    keys: Res<ButtonInput<KeyCode>>, time: Res<Time>,
    windows: Query<&Window>,
    mut q: Query<(&mut Transform, &Speed), With<Player>>,
) {
    let Ok((mut tf, speed)) = q.get_single_mut() else { return };
    let mut dir = Vec2::ZERO;
    if keys.pressed(KeyCode::ArrowLeft)  || keys.pressed(KeyCode::KeyA) { dir.x -= 1.0; }
    if keys.pressed(KeyCode::ArrowRight) || keys.pressed(KeyCode::KeyD) { dir.x += 1.0; }
    if keys.pressed(KeyCode::ArrowUp)    || keys.pressed(KeyCode::KeyW) { dir.y += 1.0; }
    if keys.pressed(KeyCode::ArrowDown)  || keys.pressed(KeyCode::KeyS) { dir.y -= 1.0; }
    let v = dir.normalize_or_zero() * speed.0 * time.delta_secs();
    tf.translation.x += v.x;
    tf.translation.y += v.y;
    if let Ok(win) = windows.get_single() {
        let (hw, hh) = (win.width() / 2.0 - 24.0, win.height() / 2.0 - 24.0);
        tf.translation.x = tf.translation.x.clamp(-hw, hw);
        tf.translation.y = tf.translation.y.clamp(-hh, hh);
    }
}

fn player_shoot(
    keys: Res<ButtonInput<KeyCode>>, time: Res<Time>,
    mut timer: ResMut<ShootTimer>,
    q: Query<&Transform, With<Player>>,
    mut ev: EventWriter<Fired>,
) {
    timer.0.tick(time.delta());
    if !keys.pressed(KeyCode::Space) || !timer.0.just_finished() { return; }
    let Ok(tf) = q.get_single() else { return };
    ev.send(Fired { pos: tf.translation.truncate() });
}

fn spawn_bullets(mut commands: Commands, mut ev: EventReader<Fired>) {
    for e in ev.read() {
        commands.spawn((
            Bullet, Vel(Vec2::new(0.0, 600.0)),
            Sprite { color: Color::srgb(1.0, 0.95, 0.2),
                     custom_size: Some(Vec2::new(6.0, 18.0)), ..default() },
            Transform::from_translation(e.pos.extend(2.0)),
        ));
    }
}

fn move_all(time: Res<Time>, mut q: Query<(&Vel, &mut Transform), Without<Player>>) {
    for (vel, mut tf) in &mut q {
        tf.translation.x += vel.0.x * time.delta_secs();
        tf.translation.y += vel.0.y * time.delta_secs();
    }
}

fn spawn_enemies(mut commands: Commands, time: Res<Time>, mut timer: ResMut<SpawnTimer>) {
    timer.0.tick(time.delta());
    if !timer.0.just_finished() { return; }
    for i in 0..4i32 {
        commands.spawn((
            Enemy, Vel(Vec2::new(0.0, -110.0)), Health(2.0),
            Sprite { color: Color::srgb(1.0, 0.25, 0.2),
                     custom_size: Some(Vec2::new(44.0, 44.0)), ..default() },
            Transform::from_xyz(-225.0 + i as f32 * 150.0, 320.0, 1.0),
        ));
    }
}

fn check_collisions(
    mut commands: Commands,
    bullets: Query<(Entity, &Transform), With<Bullet>>,
    mut enemies: Query<(Entity, &Transform, &mut Health), (With<Enemy>, Without<Bullet>)>,
    mut player: Query<(&Transform, &mut Health), (With<Player>, Without<Enemy>, Without<Bullet>)>,
    mut next: ResMut<NextState<GameState>>,
    mut killed: EventWriter<Killed>,
) {
    for (b_ent, b_tf) in &bullets {
        let b_pos = b_tf.translation.truncate();
        for (e_ent, e_tf, mut hp) in &mut enemies {
            if (b_pos - e_tf.translation.truncate()).length() < 28.0 {
                commands.entity(b_ent).despawn();
                hp.0 -= 1.0;
                if hp.0 <= 0.0 { commands.entity(e_ent).despawn(); killed.send(Killed); }
                break;
            }
        }
    }
    let Ok((p_tf, mut p_hp)) = player.get_single_mut() else { return };
    for (e_ent, e_tf, _) in &enemies {
        if (p_tf.translation.truncate() - e_tf.translation.truncate()).length() < 44.0 {
            commands.entity(e_ent).despawn();
            p_hp.0 -= 1.0;
            if p_hp.0 <= 0.0 { next.set(GameState::GameOver); }
        }
    }
}

fn tally_kills(mut score: ResMut<Score>, mut ev: EventReader<Killed>) {
    for _ in ev.read() { score.0 += 100; }
}

fn despawn_offscreen(
    mut commands: Commands,
    q: Query<(Entity, &Transform), Or<(With<Bullet>, With<Enemy>)>>,
) {
    for (e, tf) in &q {
        if tf.translation.y.abs() > 360.0 { commands.entity(e).despawn(); }
    }
}

fn setup_hud(mut commands: Commands) {
    commands.spawn((UiRoot, Node {
        width: Val::Percent(100.0),
        padding: UiRect::all(Val::Px(14.0)),
        justify_content: JustifyContent::SpaceBetween,
        ..default()
    })).with_children(|p| {
        p.spawn((ScoreText, Text::new("Skóre: 0"),
            TextFont { font_size: 24.0, ..default() }, TextColor(Color::WHITE)));
        p.spawn((HealthText, Text::new("♥ ♥ ♥"),
            TextFont { font_size: 24.0, ..default() },
            TextColor(Color::srgb(1.0, 0.3, 0.3))));
    });
}

fn update_hud(
    score: Res<Score>, player: Query<&Health, With<Player>>,
    mut s_txt: Query<&mut Text, (With<ScoreText>, Without<HealthText>)>,
    mut h_txt: Query<&mut Text, (With<HealthText>, Without<ScoreText>)>,
) {
    if let Ok(mut t) = s_txt.get_single_mut() { **t = format!("Skóre: {}", score.0); }
    if let (Ok(hp), Ok(mut t)) = (player.get_single(), h_txt.get_single_mut()) {
        **t = "♥ ".repeat(hp.0.max(0.0) as usize);
    }
}

fn setup_menu(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn((UiRoot, Node {
        width: Val::Percent(100.0), height: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center, row_gap: Val::Px(24.0),
        ..default()
    })).with_children(|p| {
        p.spawn((Text::new("ASTEROID SHOOTER"),
            TextFont { font_size: 52.0, ..default() }, TextColor(Color::WHITE)));
        p.spawn((Text::new("[ ENTER ] — Štart"),
            TextFont { font_size: 26.0, ..default() },
            TextColor(Color::srgb(0.7, 0.7, 0.7))));
    });
}

fn menu_input(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<GameState>>) {
    if keys.just_pressed(KeyCode::Enter) { next.set(GameState::Playing); }
}

fn cleanup_ui(mut commands: Commands, q: Query<Entity, With<UiRoot>>) {
    for e in &q { commands.entity(e).despawn_recursive(); }
}

fn cleanup_game(
    mut commands: Commands,
    q: Query<Entity, Or<(With<Player>, With<Enemy>, With<Bullet>)>>,
) {
    for e in &q { commands.entity(e).despawn(); }
}

fn show_gameover(mut commands: Commands, score: Res<Score>) {
    commands.spawn((UiRoot, Node {
        width: Val::Percent(100.0), height: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center, row_gap: Val::Px(20.0),
        ..default()
    })).with_children(|p| {
        p.spawn((Text::new("GAME OVER"),
            TextFont { font_size: 64.0, ..default() },
            TextColor(Color::srgb(1.0, 0.2, 0.2))));
        p.spawn((Text::new(format!("Skóre: {}", score.0)),
            TextFont { font_size: 36.0, ..default() }, TextColor(Color::WHITE)));
        p.spawn((Text::new("[ R ] — Reštart"),
            TextFont { font_size: 24.0, ..default() },
            TextColor(Color::srgb(0.7, 0.7, 0.7))));
    });
}

fn restart_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut next: ResMut<NextState<GameState>>,
    mut score: ResMut<Score>,
    cameras: Query<Entity, With<Camera2d>>,
    mut commands: Commands,
) {
    if !keys.just_pressed(KeyCode::KeyR) { return; }
    score.0 = 0;
    for (i, e) in cameras.iter().enumerate() {
        if i > 0 { commands.entity(e).despawn(); }
    }
    next.set(GameState::Menu);
}
```

---

## Spustenie

```bash
cargo run -p bevy-hra --manifest-path priklady/Cargo.toml
```

Ovládanie:
- **ENTER** — štart
- **šípky / WASD** — pohyb
- **Medzera** (držať) — streľba
- **R** po Game Over — reštart

---

## Debuggovanie UI

### Viditeľnosť hraníc UI elementov

```toml
# Cargo.toml — debug feature
bevy = { version = "0.15", features = ["bevy_ui_debug"] }
```

Alebo cez kód — nakreslí ohraničenia všetkých UI nodov:

```rust
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // Zapni debug rendering pre UI:
        .insert_resource(UiDebugOptions {
            enabled: true,
            ..default()
        })
        .run();
}
```

### bevy-inspector-egui pre UI

```rust
use bevy_inspector_egui::quick::WorldInspectorPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(WorldInspectorPlugin::new())
        .run();
}
```

V inspector paneli vidíš celú hierarchiu UI entít, ich `Node` rozmery a `BackgroundColor` — môžeš editovať za behu. Je to ako Chrome DevTools pre Bevy UI.

### Logovaníe UI eventov

```rust
fn debug_interactions(
    q: Query<(Entity, &Interaction), Changed<Interaction>>,
) {
    for (entity, interaction) in &q {
        debug!("UI {:?} interaction: {:?}", entity, interaction);
    }
}
```

---

## Časté chyby s UI

### 1. Text sa nezobrazuje — zabudnutý root node

```rust
// CHYBA — Text entita bez Node rodiča
commands.spawn((
    Text::new("Hello"),
    TextFont { font_size: 24.0, ..default() },
));  // Môže fungovať ale bez layoutu bude na pozícii 0,0

// Lepší prístup — vždy v hierarchii s Node
commands.spawn(Node {
    width: Val::Percent(100.0),
    justify_content: JustifyContent::Center,
    ..default()
}).with_children(|p| {
    p.spawn((
        Text::new("Hello"),
        TextFont { font_size: 24.0, ..default() },
        TextColor(Color::WHITE),
    ));
});
```

### 2. UI sa zobrazuje pod hernou scénou

UI v Bevy má vlastnú kamerovú vrstvu — vždy je nad hernou scénou. Ak nie, skontroluj:

```rust
// Uisti sa že Camera2d je spawnovaná
commands.spawn(Camera2d);

// Bevy UI camera je automatická — nevytvára sa explicitne
// Ak si pridal vlastný RenderLayer, uisti sa že UI nie je vyradené
```

### 3. Klikateľné tlačidlo nereaguje

```rust
// Ak Button entita nereaguje na kliknutie:
// 1. Skontroluj že má Button komponent (nie len Node)
// 2. Skontroluj že Interaction query správne filtuje
// 3. Skontroluj že UI entita nemá ZIndex problém (iná entita nad ňou)

// Debug:
fn debug_buttons(q: Query<(Entity, &Interaction), With<Button>>) {
    for (e, i) in &q {
        debug!("Button {:?}: {:?}", e, i);
    }
}
```

---

## Čo robiť ďalej

- **`bevy_rapier2d`** — fyzika: gravitácia, kruhy, polygóny, impulzy
- **`bevy_asset_loader`** — štruktúrovaný loading obrázkov, zvukov, zabráni race condition pri štarte
- **WebAssembly**: `cargo build --target wasm32-unknown-unknown` + `wasm-bindgen` — hra v prehliadači
- **Animácie**: `TextureAtlas` + spritesheet + `AnimationPlayer` pre komplexnejšie animácie
- **Audio**: `commands.spawn(AudioPlayer::new(asset_server.load("shoot.ogg")))` — stačí pridať `.ogg` súbor do `assets/`
- **bevy-inspector-egui** — in-game editor pre debug
- **`bevy_ecs_tilemap`** — tile-based mapy (RPG, platformer)
- **Diel 2**: Embedded Rust — `no_std`, HAL, RTIC, ESP32, defmt, probe-rs

---

## Zhrnutie série Bevy (K11–K14)

| Kapitola | Téma |
|---|---|
| K11 | ECS koncepty, Archetypy, App, Plugin, Query, Resource, paralelizmus |
| K12 | Sprite, Transform, vstup, delta time, animácie, clamping |
| K13 | States, Events, kolízie, Timer, despawn, systémové zoraďovanie |
| K14 | UI, HUD, Flexbox, tlačidlá, audio, finálna hra |

ECS myslenie ostane — nie len v hrách. Data-oriented design, cache-friendly layout, oddelenie logiky od dát — to je jadro systémového programovania. Keď sa nabudúce pozrieš na Unity alebo Unreal, budeš vidieť kde skryte používajú podobné princípy — a kde naopak sa držia starých OOP vzorov na úkor výkonu.

Rust a Bevy ťa naučia jeden dôležitý zvyk: premýšľať o tom *kde sú dáta v pamäti* a *kto ich vlastní*. Tento zvyk sa ti zíde aj pri písaní bežného serverového kódu, pri návrhu databázových schém, alebo pri optimalizácii akéhokoľvek výkonnostne kritického kódu.
