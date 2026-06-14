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

// ---------------------------------------------------------------
fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Asteroid Shooter — Rust pre Systémových Programátorov".into(),
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
        // Menu
        .add_systems(OnEnter(GameState::Menu),   setup_menu)
        .add_systems(OnExit(GameState::Menu),    cleanup_ui)
        .add_systems(Update, menu_input.run_if(in_state(GameState::Menu)))
        // Playing
        .add_systems(OnEnter(GameState::Playing), (spawn_camera, setup_game, setup_hud).chain())
        .add_systems(OnExit(GameState::Playing),  cleanup_game)
        .add_systems(Update, (
            player_move,
            player_shoot,
            spawn_bullets,
            move_all,
            spawn_enemies,
            check_collisions,
            tally_kills,
            despawn_offscreen,
            update_hud,
        ).run_if(in_state(GameState::Playing)))
        // Game Over
        .add_systems(OnEnter(GameState::GameOver), show_gameover)
        .add_systems(Update, restart_input.run_if(in_state(GameState::GameOver)))
        .run();
}

// ---------------------------------------------------------------
// MENU
// ---------------------------------------------------------------
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

fn cleanup_ui(mut commands: Commands, q: Query<Entity, With<UiRoot>>) {
    for e in &q { commands.entity(e).despawn_recursive(); }
}

// ---------------------------------------------------------------
// SETUP
// ---------------------------------------------------------------
fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn setup_game(mut commands: Commands) {
    commands.spawn((
        Player,
        Speed(300.0),
        Health(3.0),
        Vel(Vec2::ZERO),
        Sprite {
            color: Color::srgb(0.2, 0.6, 1.0),
            custom_size: Some(Vec2::new(48.0, 48.0)),
            ..default()
        },
        Transform::from_xyz(0.0, -220.0, 1.0),
    ));
}

fn setup_hud(mut commands: Commands) {
    commands.spawn((
        UiRoot,
        Node {
            width: Val::Percent(100.0),
            padding: UiRect::all(Val::Px(14.0)),
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        },
    )).with_children(|p| {
        p.spawn((
            ScoreText,
            Text::new("Score: 0"),
            TextFont { font_size: 24.0, ..default() },
            TextColor(Color::WHITE),
        ));
        p.spawn((
            HealthText,
            Text::new("[*][*][*]"),
            TextFont { font_size: 24.0, ..default() },
            TextColor(Color::srgb(1.0, 0.3, 0.3)),
        ));
    });
}

fn cleanup_game(
    mut commands: Commands,
    q: Query<Entity, Or<(With<Player>, With<Enemy>, With<Bullet>)>>,
) {
    for e in &q { commands.entity(e).despawn(); }
}

// ---------------------------------------------------------------
// PLAYING SYSTEMS
// ---------------------------------------------------------------
fn player_move(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
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

    // Clamping
    if let Ok(win) = windows.get_single() {
        let (hw, hh) = (win.width() / 2.0 - 24.0, win.height() / 2.0 - 24.0);
        tf.translation.x = tf.translation.x.clamp(-hw, hw);
        tf.translation.y = tf.translation.y.clamp(-hh, hh);
    }
}

fn player_shoot(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut timer: ResMut<ShootTimer>,
    q: Query<&Transform, With<Player>>,
    mut ev: EventWriter<Fired>,
) {
    timer.0.tick(time.delta());
    if !keys.pressed(KeyCode::Space) { return; }
    if !timer.0.just_finished() { return; }
    let Ok(tf) = q.get_single() else { return };
    ev.send(Fired { pos: tf.translation.truncate() });
}

fn spawn_bullets(mut commands: Commands, mut ev: EventReader<Fired>) {
    for e in ev.read() {
        commands.spawn((
            Bullet,
            Vel(Vec2::new(0.0, 600.0)),
            Sprite {
                color: Color::srgb(1.0, 0.95, 0.2),
                custom_size: Some(Vec2::new(6.0, 18.0)),
                ..default()
            },
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

fn spawn_enemies(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: ResMut<SpawnTimer>,
) {
    timer.0.tick(time.delta());
    if !timer.0.just_finished() { return; }

    for i in 0..4i32 {
        let x = -225.0 + i as f32 * 150.0;
        commands.spawn((
            Enemy,
            Vel(Vec2::new(0.0, -110.0)),
            Health(2.0),
            Sprite {
                color: Color::srgb(1.0, 0.25, 0.2),
                custom_size: Some(Vec2::new(44.0, 44.0)),
                ..default()
            },
            Transform::from_xyz(x, 320.0, 1.0),
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
    // Strely vs nepriatelia
    for (b_ent, b_tf) in &bullets {
        let b_pos = b_tf.translation.truncate();
        for (e_ent, e_tf, mut hp) in &mut enemies {
            if (b_pos - e_tf.translation.truncate()).length() < 28.0 {
                commands.entity(b_ent).despawn();
                hp.0 -= 1.0;
                if hp.0 <= 0.0 {
                    commands.entity(e_ent).despawn();
                    killed.send(Killed);
                }
                break;
            }
        }
    }

    // Nepriatelia vs hráč
    let Ok((p_tf, mut p_hp)) = player.get_single_mut() else { return };
    for (e_ent, e_tf, _) in &enemies {
        if (p_tf.translation.truncate() - e_tf.translation.truncate()).length() < 44.0 {
            commands.entity(e_ent).despawn();
            p_hp.0 -= 1.0;
            if p_hp.0 <= 0.0 {
                next.set(GameState::GameOver);
            }
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
        if tf.translation.y > 360.0 || tf.translation.y < -360.0 {
            commands.entity(e).despawn();
        }
    }
}

fn update_hud(
    score: Res<Score>,
    player: Query<&Health, With<Player>>,
    mut s_txt: Query<&mut Text, (With<ScoreText>, Without<HealthText>)>,
    mut h_txt: Query<&mut Text, (With<HealthText>, Without<ScoreText>)>,
) {
    if let Ok(mut t) = s_txt.get_single_mut() {
        **t = format!("Score: {}", score.0);
    }
    if let (Ok(hp), Ok(mut t)) = (player.get_single(), h_txt.get_single_mut()) {
        **t = "[*]".repeat(hp.0.max(0.0) as usize);
    }
}

// ---------------------------------------------------------------
// GAME OVER
// ---------------------------------------------------------------
fn show_gameover(mut commands: Commands, score: Res<Score>) {
    commands.spawn((
        UiRoot,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: Val::Px(20.0),
            ..default()
        },
    )).with_children(|p| {
        p.spawn((
            Text::new("GAME OVER"),
            TextFont { font_size: 64.0, ..default() },
            TextColor(Color::srgb(1.0, 0.2, 0.2)),
        ));
        p.spawn((
            Text::new(format!("Score: {}", score.0)),
            TextFont { font_size: 36.0, ..default() },
            TextColor(Color::WHITE),
        ));
        p.spawn((
            Text::new("[ R ] — Reštart"),
            TextFont { font_size: 24.0, ..default() },
            TextColor(Color::srgb(0.7, 0.7, 0.7)),
        ));
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
    // Zmaž extra kamery (menu spawnula jednu, game spawnula druhú)
    for (i, e) in cameras.iter().enumerate() {
        if i > 0 { commands.entity(e).despawn(); }
    }
    next.set(GameState::Menu);
}
