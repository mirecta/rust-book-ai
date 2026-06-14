# Kapitola 13 — Bevy: Kolízie, Herná Logika, Stavy

Máme hráča ktorý sa hýbe. Teraz pridáme strely, nepriateľov, kolízie a herné stavy (menu, hra, game over). Ale predtým si povedzme prečo je správna architektúra hernej logiky tak dôležitá — a ako ECS robí veci, ktoré by v OOP boli komplikované, prirodzene jednoduché.

---

## Prečo Events namiesto priamych volaní

V OOP by si napísal niečo takéto:

```cpp
// Unity / C++ štýl
void PlayerController::Shoot() {
    Bullet* bullet = bulletPool.Get();
    bullet->SetPosition(this->position);
    bullet->Fire();
    audioManager->PlaySound("shoot");
    particleSystem->Emit(ShootEffect);
}
```

Problém: `PlayerController` priamo závisí na `BulletPool`, `AudioManager`, `ParticleSystem`. Keď zmeníš jedno, môžeš rozbúrať všetko. A ako testuješ Shoot() bez všetkých tých závislostí?

V Bevy používaš Events:

```rust
fn player_shoot(/* vstup */, mut ev: EventWriter<Fired>) {
    ev.send(Fired { pos: player_pos });
    // player_shoot NEVIE o guľkách, zvuku, ani efektoch
}

fn spawn_bullets(mut ev: EventReader<Fired>, mut commands: Commands) { /* ... */ }
fn play_shoot_sound(mut ev: EventReader<Fired>, /* audio resource */) { /* ... */ }
fn spawn_muzzle_flash(mut ev: EventReader<Fired>, mut commands: Commands) { /* ... */ }
```

Každý systém robí jednu vec a nevie o ostatných. Môžeš pridať alebo odstrániť zvuk bez toho aby si sa dotkol logiky strieľania. Toto je **decoupling** v čistej forme.

---

## Herné stavy — States

Každá hra má stavy — a správne riadenie stavov je rozdiel medzi "fungujúcim prototypom" a "hrou ktorá naozaj beží". Bez stavov skončíš s hromadou `if game_started && !game_over` podmienok všade.

```rust
use bevy::prelude::*;

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
enum GameState {
    #[default]
    Menu,
    Playing,
    GameOver,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_state::<GameState>()
        .add_systems(OnEnter(GameState::Menu), setup_menu)
        .add_systems(OnExit(GameState::Menu), cleanup_menu)
        .add_systems(OnEnter(GameState::Playing), setup_game)
        .add_systems(OnEnter(GameState::GameOver), show_game_over)
        .add_systems(Update, (
            menu_input.run_if(in_state(GameState::Menu)),
            (player_shoot, move_bullets, check_collisions)
                .run_if(in_state(GameState::Playing)),
        ))
        .run();
}
```

### Čo sa stane pri zmene stavu

Keď zavoláš `next_state.set(GameState::Playing)`, Bevy:
1. Dokončí aktuálny frame
2. Spustí `OnExit(GameState::Menu)` systémy — cleanup
3. Spustí `OnEnter(GameState::Playing)` systémy — setup
4. Od nasledujúceho framu beží len `Update` systémy s `run_if(in_state(Playing))`

Toto je elegantnejšie ako Unity's `SceneManager.LoadScene()` — nenahradzuješ celú scénu, len prepínaš aké systémy bežia.

### SubStates — vnorené stavy

Pre komplexnejšie hry môžeš mať substavy:

```rust
#[derive(SubStates, Debug, Clone, PartialEq, Eq, Hash, Default)]
#[source(GameState = GameState::Playing)]  // existuje len keď Playing
enum PlayingState {
    #[default]
    Normal,
    Paused,
    Cutscene,
}
```

---

## Events — komunikácia medzi systémami

```rust
// Jednoduché eventy — len to čo treba preniesť medzi systémami
#[derive(Event)]
struct Fired { pos: Vec2 }   // hráč vystrelil

#[derive(Event)]
struct Killed;               // nepriateľ bol zabitý (skóre++)

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_event::<Fired>()
        .add_event::<Killed>()
        .add_systems(Update, (
            player_shoot,    // posiela Fired
            spawn_bullets,   // číta Fired
            check_collisions, // posiela Killed
            tally_kills,     // číta Killed
        ))
        .run();
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
    if !timer.0.just_finished() { return; }   // rate limiting — 250ms medzi strelami
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
```

### Životný cyklus eventov

Eventy v Bevy žijú **dva framy**. Ak ich v tom čase nespracuješ, Bevy ich zahodí. To znamená:

- Systém ktorý číta `EventReader<Fired>` musí bežať každý frame
- Ak používaš `run_if(in_state(Playing))` — uisti sa, že reader aj writer majú rovnaké podmienky bežania

```rust
// CHYBA — writer beží len v Playing, ale reader je vždy aktívny
// -> reader dostane event z minulého stavu
.add_systems(Update, (
    player_shoot.run_if(in_state(GameState::Playing)),
    spawn_bullets,   // bude vidieť eventy aj po zmene stavu!
))

// SPRÁVNE — obaja majú rovnakú podmienku
.add_systems(Update, (
    player_shoot,
    spawn_bullets,
).run_if(in_state(GameState::Playing)))
```

---

## Kolízie — detekcia dotyku

Pre jednoduché objekty stačí vzdialenosť stredov (kruhová kolízia):

```rust
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
            // Vzdialenosť stredov < súčet polomerov = kolízia
            if (b_pos - e_tf.translation.truncate()).length() < 28.0 {
                commands.entity(b_ent).despawn();
                hp.0 -= 1.0;
                if hp.0 <= 0.0 {
                    commands.entity(e_ent).despawn();
                    killed.send(Killed);
                }
                break;  // strela zasiahla — ďalej nepokračuj
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
```

### Prečo Without<Bullet>, Without<Enemy> vo filtri?

Toto je jedno z miest kde Bevy začiatočníci dostanú záhadný panic. Keď máš dve mutable queries, Bevy musí vedieť, že sa neprekrývajú — inak by hrozil aliasing (dve mutable referencie na rovnakú entitu).

```rust
// Toto je problém: Enemy môže mať aj Bullet?
// Nie v tejto hre, ale Bevy to nevie — preto panics
mut enemies: Query<(Entity, &mut Health), With<Enemy>>,
mut player: Query<(&mut Health), With<Player>>,
// Ak by existovala entita s oboma Player + Enemy, obe queries by ju matchli!

// Riešenie: explicitné vylúčenie
mut enemies: Query<(Entity, &Transform, &mut Health), (With<Enemy>, Without<Player>, Without<Bullet>)>,
mut player: Query<(&Transform, &mut Health), (With<Player>, Without<Enemy>, Without<Bullet>)>,
// Teraz Bevy vie: tieto queries sa NIKDY neprekrývajú -> OK
```

### AABB kolízie (rectangles)

Pre obdĺžnikové objekty je presnejšia AABB (Axis-Aligned Bounding Box):

```rust
fn rect_collision(pos_a: Vec2, size_a: Vec2, pos_b: Vec2, size_b: Vec2) -> bool {
    let half_a = size_a / 2.0;
    let half_b = size_b / 2.0;

    (pos_a.x - half_a.x) < (pos_b.x + half_b.x) &&
    (pos_a.x + half_a.x) > (pos_b.x - half_b.x) &&
    (pos_a.y - half_a.y) < (pos_b.y + half_b.y) &&
    (pos_a.y + half_a.y) > (pos_b.y - half_b.y)
}

fn check_collisions_aabb(
    mut commands: Commands,
    bullets: Query<(Entity, &Transform, &Sprite), With<Bullet>>,
    enemies: Query<(Entity, &Transform, &Sprite), With<Enemy>>,
) {
    for (b_ent, b_tf, b_spr) in &bullets {
        let b_pos = b_tf.translation.truncate();
        let b_size = b_spr.custom_size.unwrap_or(Vec2::splat(8.0));

        for (e_ent, e_tf, e_spr) in &enemies {
            let e_pos = e_tf.translation.truncate();
            let e_size = e_spr.custom_size.unwrap_or(Vec2::splat(32.0));

            if rect_collision(b_pos, b_size, e_pos, e_size) {
                commands.entity(b_ent).despawn();
                commands.entity(e_ent).despawn();
                break;
            }
        }
    }
}
```

### Kedy použiť bevy_rapier2d

Manuálne kolízie sú OK pre jednoduché projekty (menej ako ~200 entít, jednoduché tvary). Pre:
- Fyziku (gravitácia, impulzy, trenie)
- Komplexné tvary (polygóny, capsule, heightmap)
- Kolízie medzi stovkami entít efektívne (broadphase)
- Trigger zóny (EventReader<CollisionEvent>)

...použi `bevy_rapier2d`:

```toml
[dependencies]
bevy_rapier2d = "0.27"
```

```rust
use bevy_rapier2d::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0))
        .add_plugins(RapierDebugRenderPlugin::default())  // vizualizácia colliderov
        .run();
}

// Entity s fyzikou
commands.spawn((
    RigidBody::Dynamic,
    Collider::ball(22.0),      // kruhový collider polomer 22
    Restitution::coefficient(0.7),  // "odskočivosť"
    CollisionGroups::new(Group::GROUP_1, Group::GROUP_2),
    Transform::from_xyz(0.0, 100.0, 0.0),
));
```

---

## Skóre a game over

```rust
#[derive(Resource, Default)]
struct Score(u32);

fn tally_kills(mut score: ResMut<Score>, mut ev: EventReader<Killed>) {
    for _ in ev.read() { score.0 += 100; }
}

// Game over trigguje check_collisions keď hráč príde o posledný život:
// p_hp.0 -= 1.0;
// if p_hp.0 <= 0.0 { next.set(GameState::GameOver); }
```

---

## Despawn a cleanup

```rust
// Zmaž entity čo opustili obrazovku
fn despawn_offscreen(
    mut commands: Commands,
    windows: Query<&Window>,
    query: Query<(Entity, &Transform), With<Bullet>>,
) {
    let Ok(window) = windows.get_single() else { return };
    let limit = window.height() / 2.0 + 50.0;

    for (entity, tf) in &query {
        if tf.translation.y > limit {
            commands.entity(entity).despawn();
        }
    }
}

// Cleanup celej hernej scény pri zmene stavu
fn cleanup_game(
    mut commands: Commands,
    query: Query<Entity, Or<(With<Player>, With<Enemy>, With<Bullet>)>>,
) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
}
```

`despawn()` vs `despawn_recursive()`: `despawn()` zmaže len entitu, `despawn_recursive()` zmaže entitu aj všetkých jej potomkov v hierarchii. Ak spawnaš entity s `with_children()`, použi `despawn_recursive()`.

### Memory leak cez zabudnutý cleanup

Toto je klasická chyba — entity ostanú v ECS world aj keď ich nevidíš:

```rust
// Pri prechode z Playing do Menu:
// Bez cleanup ostanú všetky Enemy, Bullet, Player entity v world-e!
// Keď sa vrátia do Playing, spawn_game vytvorí ďalšie -> duplicitné entity

fn cleanup_game(
    mut commands: Commands,
    // Or<(...)> matchne entity ktoré majú ASPOŇ jeden z týchto komponentov
    query: Query<Entity, Or<(With<Player>, With<Enemy>, With<Bullet>)>>,
) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

// Registruj na OnExit:
.add_systems(OnExit(GameState::Playing), cleanup_game)
```

---

## Timer — pravidelné udalosti

```rust
#[derive(Resource)]
struct SpawnTimer(Timer);

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(SpawnTimer(
            Timer::from_seconds(2.0, TimerMode::Repeating)
        ))
        .add_systems(Update, spawn_wave)
        .run();
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
```

### Timer modes

```rust
// Repeating — resetuje sa automaticky a beží znova
Timer::from_seconds(2.0, TimerMode::Repeating)

// Once — odpáli raz a ostane v "finished" stave
Timer::from_seconds(3.0, TimerMode::Once)

// Kontrola:
timer.0.just_finished()   // TRUE len v frame keď dobehol
timer.0.finished()        // TRUE každý frame po dobehnutí (Once)
timer.0.elapsed_secs()    // koľko sekúnd ubehlo
timer.0.fraction()        // 0.0..=1.0 — progres
```

`just_finished()` je kľúčové — ak by si použil `finished()` na Repeating timer, spawn by sa spustil každý frame po prvom dobehnutí (pretože timer ihneď resetuje a `finished()` by bol TRUE aj nasledujúci frame). `just_finished()` je TRUE len jeden frame.

---

## Komplexnejšia herná logika: systémové zoraďovanie

Keď máš viac systémov, poradie môže byť dôležité. Napríklad chceš:
1. Spracovať vstup
2. Pohnúť entity
3. Detekciu kolízií
4. Zničiť entity (despawn)

```rust
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .configure_sets(Update, (
            InputSet,
            MovementSet.after(InputSet),
            CollisionSet.after(MovementSet),
            CleanupSet.after(CollisionSet),
        ))
        .add_systems(Update, (
            player_input.in_set(InputSet),
            (move_player, move_enemies).in_set(MovementSet),
            check_collisions.in_set(CollisionSet),
            (despawn_offscreen, despawn_dead).in_set(CleanupSet),
        ))
        .run();
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
enum InputSet {}
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
enum MovementSet {}
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
enum CollisionSet {}
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
enum CleanupSet {}
```

`SystemSet` je mocný nástroj — zoskupíš systémy do sád a definuješ poradie sád. V rámci jednej sady môžu systémy bežať paralelne (ak nemajú konflikty). Medzi sadami je garantované poradie.

---

## Časté chyby v hernej logike

### 1. Kolízia sa spustí dvakrát

```rust
// Keď mučíš rovnaký pár entít v oboch smeroch:
for b in &bullets {
    for e in &enemies {
        // bullet B1 vs enemy E1 -> kolízia
        // despawn B1 a E1
    }
}
// Ak nebreak-uješ, B1 vs E2, E3... -> panic (B1 je už despawnovaný)
// Riešenie: break po prvej kolízii strele
for (b_ent, b_tf) in &bullets {
    'inner: for (e_ent, e_tf, mut hp) in &mut enemies {
        if /* kolízia */ {
            commands.entity(b_ent).despawn();
            /* ... */
            break 'inner;  // táto strela nič ďalej nezasahuje
        }
    }
}
```

### 2. Stav sa zmení ale systémy ešte bežia

```rust
// V jednom frame check_collisions zmení stav na GameOver
// ALE ostatné systémy (move_all, spawn_enemies) stále bežia tento frame!
// Riešenie: run_if podmienky, alebo akceptovať jeden frame "lag"

// Alebo použi systém oreder aby GameOver nastalo ako posledné:
.add_systems(Update, (
    move_all,
    spawn_enemies,
    check_collisions,    // zmení stav
).chain().run_if(in_state(GameState::Playing)))
// chain() garantuje, že zmena stavu nastane po všetkých pohyboch
```

### 3. EventReader bol vynechaný — eventy sa nahromadia

```rust
// Ak systém ktorý číta events nebeží (napr. lebo hra nie je v Playing stave),
// eventy sa nahromadia a ďalší frame dostaneš "staré" eventy
// Riešenie: uisti sa že EventReader beží vždy keď EventWriter môže pisať
// alebo použi EventReader::clear() na začiatku stavu
```

---

## Debuggovanie hernej logiky

### Logovanie stavu

```rust
fn debug_game_state(state: Res<State<GameState>>) {
    debug!("Aktuálny stav: {:?}", state.get());
}

fn debug_entity_count(
    enemies: Query<(), With<Enemy>>,
    bullets: Query<(), With<Bullet>>,
) {
    if enemies.iter().count() > 0 || bullets.iter().count() > 0 {
        debug!(
            "Nepriatelia: {}, Strely: {}",
            enemies.iter().count(),
            bullets.iter().count()
        );
    }
}
```

### Pausovanie hry pre debug

```rust
fn toggle_pause(
    keys: Res<ButtonInput<KeyCode>>,
    mut time: ResMut<Time<Virtual>>,
) {
    if keys.just_pressed(KeyCode::KeyP) {
        if time.is_paused() {
            time.unpause();
        } else {
            time.pause();
        }
    }
}

// Spomalenie pre debugovanie kolízií:
if keys.just_pressed(KeyCode::Minus) {
    time.set_relative_speed(0.1);  // 10% rýchlosti
}
```

`Time<Virtual>` je oddelený od reálneho času — môžeš ho pozastaviť a nemá to vplyv na timer systémy Bevy (audio, animácie). Systémy ktoré používajú `time.delta_secs()` automaticky dostanú 0 keď je virtuálny čas pozastavený.

---

## Zhrnutie

| Technika | Bevy API |
|---|---|
| Herné stavy | `States`, `OnEnter`, `run_if(in_state(...))` |
| Udalosti | `Event`, `EventWriter`, `EventReader` |
| Kolízie (jednoduché) | Vzdialenosť stredov alebo AABB check |
| Kolízie (komplexné) | `bevy_rapier2d` |
| Časovač | `Timer`, `time.delta()`, `just_finished()` |
| Mazanie entít | `commands.entity(e).despawn()` |
| Zmena stavu | `next_state.set(GameState::GameOver)` |
| Zoraďovanie systémov | `SystemSet`, `.after()`, `.before()`, `.chain()` |
| Debug | `tracing::debug!`, Gizmos, `Time<Virtual>` |

Ďalšia kapitola: Audio, UI a finálna hra — dáme to dokopy.
