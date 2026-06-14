# Kapitola 11 — Bevy: ECS a herná architektúra

Bevy je herný engine písaný čisto v Ruste. Nie je to len knižnica — je to iný spôsob myslenia o architektúre aplikácií. ECS (Entity Component System) je radikálne odlišný od OOP. Ale skôr než sa pozrieme na kód, porozprávajme sa o tom *prečo* — pretože bez toho je ECS len ďalší buzzword.

---

## Prečo vôbec ECS? Príbeh o bolesti

Predstav si, že robíš hru v Unity. Máš triedu `Player`, `Enemy`, `Boss`. Boss je nepriateľ, tak zdedíš z `Enemy`. Ale Boss má aj špeciálny pohyb ako hráč — skopíruješ kód? Alebo urobíš `IMovable` interface? A čo keď Boss môže byť zároveň NPC spojencom? Dedičnostný strom sa stáva nočnou morou.

```
// Unity / Unreal OOP prístup — "diamond problem"
class Entity { position, health }
class Enemy extends Entity { ai }
class Player extends Entity { input }
class Boss extends Enemy { ??? }    // chcem aj Player.input správanie...
```

Toto je tzv. **problém diamantu** — a riešia ho všetci herní programátori. C++ to rieši viacnásobnou dedičnosťou (a dostaneš nové problémy). Unity to rieši mixom komponentov (MonoBehaviour) + dedičnosti — ale kombinácia je ľubovoľne chaotická.

Druhý problém je **výkon**. V OOP máš pole objektov — každý objekt má v pamäti všetky svoje dáta vedľa seba. Keď chceš posunúť všetky entity, iteruješ cez pole `Enemy[]`, a pre každý objekt skáčeš na jeho `Position` — ktorá môže byť na úplne inom mieste v pamäti. Cache miss za cache missom. Na modernom CPU je cache miss 100-200x pomalší ako cache hit.

```
Pamäť s OOP objektmi:
[Enemy1: pos, vel, health, ai, sprite, ...][Enemy2: pos, vel, health, ai, sprite, ...]
                 ^                                   ^
                 |                                   |
         cache line #1                       cache line #47
         (načítaná)                          (nie je v cache — STALL!)
```

ECS rieši obe problémy naraz.

### Klasický OOP prístup (čo Bevy nie je)

```
Trieda Hráč:
  - health: int
  - position: Vec2
  - sprite: Sprite
  - fn update()
  - fn render()

Trieda Nepriateľ extends Objekt:
  - health: int
  - ai: BehaviorTree
  - fn update()
```

Problém: dedičnosť, tight coupling, cache unfriendly (objekty v pamäti nesúvisle).

### ECS prístup (Bevy)

```
Entity — len ID (u64)
Component — dáta bez logiky (struct)
System — logika bez dát (fn)
Resource — globálny stav (singleton)

Entity 1: [Position, Velocity, Health, PlayerTag]
Entity 2: [Position, Velocity, Health, EnemyAI]
Entity 3: [Position, Sprite, Background]

System "pohyb": for (pos, vel) in query<(&mut Position, &Velocity)> { ... }
System "AI":    for (pos, ai)  in query<(&Position, &EnemyAI)>  { ... }
```

Cache-friendly: všetky `Position` komponenty sú v jednom poli v pamäti — SIMD-priateľné.

---

## Ako Bevy organizuje pamäť pod kapotou

Toto je tá časť, ktorú väčšina tutoriálov preskočí, ale je kľúčová pre pochopenie prečo ECS funguje tak dobre.

Bevy používa koncept **Archetypy** (Archetypes). Archetype je skupina entít, ktoré majú *presne rovnakú sadu komponentov*. Každý archetype má pre každý komponent jedno contiguous pole (Vec) v pamäti.

```
Archetype A: Entity má [Position, Velocity, Health]
  Positions:  [pos1, pos2, pos3, pos4, ...]   <- jeden Vec<Position>
  Velocities: [vel1, vel2, vel3, vel4, ...]   <- jeden Vec<Velocity>
  Healths:    [hp1,  hp2,  hp3,  hp4,  ...]   <- jeden Vec<Health>

Archetype B: Entity má [Position, Sprite]
  Positions:  [pos5, pos6, ...]
  Sprites:    [spr1, spr2, ...]
```

Keď systém pýta `Query<(&mut Position, &Velocity)>`, Bevy prejde všetky archetypy, nájde tie čo obsahujú oba komponenty, a vráti zip iterátor cez ich polia. Všetko je v pamäti za sebou — CPU prefetcher je šťastný, SIMD inštrukcie fungujú naplno.

Keď pridáš entite nový komponent (`.insert(NewComp)`), Bevy **presunie entitu do iného archetypu**. Toto je prečo je `insert()` relatívne drahá operácia — treba preallokáciu a kopírovanie. Preto sa entitám raz nastavené komponenty nemenia príliš často.

```rust
// Lacné — pohyb v rámci existujúceho archetypu
tf.translation.x += 5.0;

// Drahšie — zmena archetypu (ale stále OK, len nie každý frame pre tisíce entít)
commands.entity(player_entity).insert(Stunned { duration: 2.0 });
```

Porovnaj s Unity: tam `GetComponent<T>()` je lookup do hashmapy — O(1) ale s konštantou cache missu. V Bevy je to index do poľa — O(1) s cache hitom.

---

## Prečo systémy môžu bežať paralelne

Toto je jedna z najpôsobivejších vlastností Bevy — automatický paralelizmus. Bevy analyzuje každý systém a zistí:

1. Ktoré komponenty číta (shared reference `&T`)
2. Ktoré komponenty zapisuje (mutable reference `&mut T`)
3. Ktoré resource používa

Ak dva systémy nemajú konflikty (žiadny z nich nezapisuje to, čo druhý číta), Bevy ich spustí **súčasne na rôznych vláknach** — automaticky, bez akéhokoľvek kódu od teba.

```rust
// Tieto dva systémy môžu bežať PARALELNE:
fn move_player(
    mut q: Query<&mut Transform, With<Player>>,  // píše Transform hráča
    // ...
) { /* ... */ }

fn enemy_ai(
    mut q: Query<&mut Transform, With<Enemy>>,   // píše Transform nepriateľov
    // ...
) { /* ... */ }
// -> iné entity, iné archetypy -> žiadny konflikt -> parallel!

// Tieto NIE MÔŽU bežať paralelne:
fn heal_player(
    mut q: Query<&mut Health, With<Player>>,     // píše Health
) { /* ... */ }

fn damage_player(
    mut q: Query<&mut Health, With<Player>>,     // TIEŽ píše Health!
) { /* ... */ }
// -> konflikt na mutable Health -> Bevy ich zoradí sekvenčne
```

V Unity/Unreal musíš paralelizmus riadiť ručne (Job System, TaskGraph). V Bevy to robí Bevy scheduler za teba — len sa musíš uistiť, že tvoje systémy naozaj nemajú logické konflikty.

Pokiaľ chceš vynútiť poradie, použi `.chain()`:

```rust
.add_systems(Update, (
    player_input,
    move_player,
    check_bounds,
).chain())   // garantovane v tomto poradí, ale sekvenčne
```

Alebo `after()` / `before()` pre čiastočné usporiadanie (ostatné môžu stále bežať paralelne):

```rust
.add_systems(Update, (
    move_player.before(check_bounds),
    animate_sprites,    // môže bežať paralelne s oboma
))
```

---

## Inštalácia

```toml
[dependencies]
bevy = "0.15"
```

Build môže trvať niekoľko minút prvýkrát. Pre rýchlejší vývoj:

```toml
# .cargo/config.toml
[profile.dev]
opt-level = 1

[profile.dev.package."*"]
opt-level = 3
```

Tieto dve nastavenia spôsobia, že tvoj kód sa builduje debug (rýchly kompilátor), ale všetky závislosti (vrátane Bevy) sa optimalizujú. Rozdiel je dramatický — debug build Bevy môže bežať 5-10x pomalšie ako optimalizovaný.

> **Tip:** Ak chceš ešte rýchlejší iteračný cyklus, pozri sa na `bevy_dylib` feature flag — umožňuje dynamické linkovanie Bevy a skráti rekompiláciu na sekundy namiesto minút.

---

## App — základ Bevy aplikácie

```rust
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)   // okno, renderer, vstup, zvuk...
        .add_systems(Startup, setup)   // zavolá sa raz na štarte
        .add_systems(Update, (         // zavolá sa každý frame
            player_input,
            move_player,
            check_bounds,
        ).chain())                     // .chain() = sekvenčné poradie
        .run();
}
```

`DefaultPlugins` je bundle ktorý obsahuje: okno (WindowPlugin), renderer (RenderPlugin), vstup (InputPlugin), asset loading (AssetPlugin), zvuk (AudioPlugin), a ďalšie. Môžeš ich pridávať aj jednotlivo ak chceš kontrolu nad tým, čo je zapnuté — napríklad pre headless server bez okna.

`Startup` je špeciálny **Schedule** — Bevy má viac schedulov:
- `Startup` — zavolá sa raz pred prvým Update
- `Update` — zavolá sa každý frame
- `FixedUpdate` — zavolá sa fixne (defaultne 64x za sekundu), ideálne pre fyziku
- `PostUpdate` — po Update, Bevy tu napríklad aktualizuje globálne transformácie

---

## Komponenty

```rust
// Akýkoľvek Rust struct/enum s Component derive
#[derive(Component)]
struct Vel(Vec2);          // rýchlosť (pixels/s)

#[derive(Component)]
struct Speed(f32);         // maximálna rýchlosť hráča

#[derive(Component)]
struct Health(f32);        // životy

// Marker komponenty — prázdne, len na označenie identity
#[derive(Component)] struct Player;
#[derive(Component)] struct Enemy;
#[derive(Component)] struct Bullet;
```

Komponent môže byť čokoľvek čo implementuje `Component` trait. Bevy to rieši cez `derive` makro. Dôležité: komponenty sú **len dáta** — žiadne metódy, žiadna logika. Ak pridáš metódy na komponent, nie je to koniec sveta, ale logiku radšej drž v systémoch — inak stratíš výhody ECS kompozície.

Marker komponenty sú elegantné — nulová veľkosť v pamäti, len tag. `With<Player>` filter v query má nulový runtime overhead.

---

## Spawning entít

```rust
fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    // Kamera
    commands.spawn(Camera2d);

    // Hráč — bundle komponentov
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

    // Nepriatelia
    for i in 0..4i32 {
        commands.spawn((
            Enemy,
            Vel(Vec2::new(0.0, -110.0)),
            Health(2.0),
            Sprite {
                color: Color::srgb(1.0, 0.25, 0.2),
                custom_size: Some(Vec2::new(44.0, 44.0)),
                ..default()
            },
            Transform::from_xyz(-225.0 + i as f32 * 150.0, 320.0, 1.0),
        ));
    }
}
```

`commands.spawn(...)` nevytvára entitu okamžite. `Commands` je **command buffer** — všetky príkazy sa zhromaždia počas systému a vykonajú sa až po jeho skončení (v špeciálnej "apply deferred" fáze). Preto nemôžeš okamžite čítať Entity ID entity ktorú si práve spawnol v tom istom systéme — musíš počkať na ďalší frame, alebo použiť `World` priamo (čo je pokročilejšia technika).

Tuple v `spawn((...))` — Bevy to volá "bundle". Každá n-tica komponentov je bundle. Môžeš si definovať vlastné bundly ako struct s `#[derive(Bundle)]` ak spawnaš rovnakú kombináciu na viacerých miestach.

---

## Query — databáza komponentov

Query je srdce ECS. Je to deklaratívny popis toho, s akými entitami chceš pracovať — a Bevy za teba najde všetky zodpovedajúce entity efektívne.

```rust
// Systém: pohyb všetkých entít s Vel a Transform (okrem hráča — ten si riadi sám)
fn move_all(time: Res<Time>, mut query: Query<(&Vel, &mut Transform), Without<Player>>) {
    for (vel, mut tf) in &mut query {
        tf.translation.x += vel.0.x * time.delta_secs();
        tf.translation.y += vel.0.y * time.delta_secs();
    }
}

// Filter — len hráčova entita
fn player_move(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut query: Query<(&mut Transform, &Speed), With<Player>>,
) {
    let Ok((mut tf, speed)) = query.get_single_mut() else { return };

    let mut dir = Vec2::ZERO;
    if keys.pressed(KeyCode::ArrowLeft)  { dir.x -= 1.0; }
    if keys.pressed(KeyCode::ArrowRight) { dir.x += 1.0; }
    if keys.pressed(KeyCode::ArrowUp)    { dir.y += 1.0; }
    if keys.pressed(KeyCode::ArrowDown)  { dir.y -= 1.0; }

    let v = dir.normalize_or_zero() * speed.0 * time.delta_secs();
    tf.translation.x += v.x;
    tf.translation.y += v.y;
}

// With<Player>  — query len entity s Player komponentom
// Without<Enemy> — vylúčiť entity s Enemy
// Changed<Health> — len entity kde sa Health zmenil tento frame
```

`Query<(&mut Transform, &Vel)>` — prvá časť tuple sú komponenty čo chceš, druhá (za čiarkou) sú filtre. Filtre ťa nič nestoja — sú to compile-time informácie pre scheduler.

Špeciálne filtre:
- `With<T>` — entita musí mať T, ale nechceš ho čítať
- `Without<T>` — entita nesmie mať T (dôležité pre rozlíšenie overlapping queries)
- `Changed<T>` — Bevy sleduje zmeny; efektívny spôsob "reagovať len keď sa niečo zmenilo"
- `Added<T>` — len entity kde bol T pridaný tento frame

`Changed<Health>` je obzvlášť elegantné — namiesto toho aby si každý frame kontroloval každého nepriateľa či má málo HP, môžeš mať systém ktorý beží len keď sa Health naozaj zmení. Na 10 000 entít je to dramatický rozdiel.

---

## Resources — globálny stav

```rust
#[derive(Resource)]
struct Score(u32);

#[derive(Resource)]
struct GameConfig {
    difficulty: f32,
    max_enemies: usize,
}

fn setup(mut commands: Commands) {
    commands.insert_resource(Score(0));
    commands.insert_resource(GameConfig {
        difficulty: 1.0,
        max_enemies: 20,
    });
}

fn tally_kills(mut score: ResMut<Score>, mut ev: EventReader<Killed>) {
    for _ in ev.read() { score.0 += 100; }
    // EventReader konzumuje udalosti — každý frame spracujeme len nové
}
```

Resource je ako singleton — jeden na celú aplikáciu. `Res<T>` je read-only prístup, `ResMut<T>` je mutable. Bevy garantuje, že nikdy nenastane data race — ak dvaja systémy chcú `ResMut<Score>`, automaticky sa zoradia sekvenčne.

Porovnaj s Unity: tam globálny stav znamená `static mut`, `GameManager.Instance`, alebo `PlayerPrefs` — každé s vlastnými problémami. V Bevy je resource typovaný, bezpečný, a trackovaný schedulerom.

---

## Plugin — modularizácia

Plugin je základný organizačný princíp Bevy. Každá feature hry by mala byť plugin — dá sa zapnúť/vypnúť, testovať samostatne, preniesť do iného projektu.

```rust
pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(Startup, spawn_player)
            .add_systems(Update, (
                player_input,
                player_shoot,
            ));
    }
}

pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(Startup, spawn_enemies)
            .add_systems(Update, (
                enemy_move,
                enemy_shoot,
            ));
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(PlayerPlugin)
        .add_plugins(EnemyPlugin)
        .run();
}
```

V Unity by toto zodpovedalo mať separátne scripty na Game Objectoch, ale s tým rozdielom, že v Bevy vidíš presne v `main()` čo je v hre zapnuté. Žiadne skryté závislosti na scene hierarchii.

Pluginy môžu mať konfiguráciju:

```rust
pub struct EnemyPlugin {
    pub initial_count: usize,
    pub spawn_interval_secs: f32,
}

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(EnemyConfig {
            initial_count: self.initial_count,
            spawn_interval_secs: self.spawn_interval_secs,
        })
        .add_systems(Startup, spawn_initial_enemies)
        .add_systems(Update, spawn_wave);
    }
}

// Použitie:
App::new()
    .add_plugins(EnemyPlugin {
        initial_count: 4,
        spawn_interval_secs: 2.0,
    })
```

---

## Časté chyby začiatočníkov

### 1. Mutovanie entity počas iterácie cez query

```rust
// CHYBA — compile error alebo panic
fn bad_system(mut commands: Commands, mut q: Query<(Entity, &mut Health)>) {
    for (entity, mut hp) in &mut q {
        if hp.0 <= 0.0 {
            // commands.entity(entity).despawn() -- toto JE OK (deferred)
            q.iter_mut(); // ale toto NIE -- nemôžeš borrowovať q dvakrát
        }
    }
}

// SPRÁVNE — despawn cez Commands je vždy deferred, teda bezpečné
fn good_system(mut commands: Commands, q: Query<(Entity, &Health)>) {
    for (entity, hp) in &q {
        if hp.0 <= 0.0 {
            commands.entity(entity).despawn();  // vykoná sa po skončení systému
        }
    }
}
```

### 2. Zabudnutý `time.delta_secs()` — pohyb závislý od FPS

```rust
// CHYBA — na 144Hz beží 2.4x rýchlejšie ako na 60Hz
fn bad_move(mut q: Query<&mut Transform, With<Player>>) {
    for mut tf in &mut q {
        tf.translation.x += 5.0;  // 5 pixelov za FRAME, nie za sekundu!
    }
}

// SPRÁVNE
fn good_move(time: Res<Time>, mut q: Query<&mut Transform, With<Player>>) {
    for mut tf in &mut q {
        tf.translation.x += 300.0 * time.delta_secs();  // 300 pixelov za sekundu
    }
}
```

### 3. `query.get_single()` bez ošetrenia chyby

```rust
// CHYBA — panic ak neexistuje entita (napr. pred spawn, alebo po despawn)
fn bad_system(q: Query<&Transform, With<Player>>) {
    let tf = q.single();  // .single() = panic! ak 0 alebo 2+ výsledkov
}

// SPRÁVNE
fn good_system(q: Query<&Transform, With<Player>>) {
    let Ok(tf) = q.get_single() else { return };
    // pokračuj len ak entita existuje
}
```

### 4. Overlapping mutable queries — Bevy panic

```rust
// CHYBA — Bevy nevie garantovať, že sa queries neprekrývajú
fn bad_system(
    mut q1: Query<&mut Transform>,           // všetky entity s Transform
    mut q2: Query<&mut Transform, With<Enemy>>, // subset — Bevy netuší v compile time!
) { /* panic v runtime */ }

// SPRÁVNE — explicitne rozlíš
fn good_system(
    mut q1: Query<&mut Transform, Without<Enemy>>,
    mut q2: Query<&mut Transform, With<Enemy>>,
) { /* OK — Bevy vie že sa neprekrývajú */ }
```

---

## Debuggovanie v Bevy

### bevy-inspector-egui

Toto je nepostrádateľný nástroj. Pridaj do `Cargo.toml`:

```toml
[dependencies]
bevy-inspector-egui = "0.27"
```

```rust
use bevy_inspector_egui::quick::WorldInspectorPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(WorldInspectorPlugin::new())  // pridá panel so všetkými entitami
        // ...
        .run();
}
```

Dostaneš okno kde vidíš každú entitu, jej komponenty a ich hodnoty — v reálnom čase, editovateľné. Je to ako Unreal's Details panel alebo Unity's Inspector, ale pre ECS.

### Bevy log systém

Bevy používa `tracing` crate. Môžeš nastaviť log level:

```rust
fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(bevy::log::LogPlugin {
            level: bevy::log::Level::DEBUG,
            filter: "wgpu=error,bevy_render=info,my_game=debug".into(),
            ..default()
        }))
        .run();
}

// V systémoch:
use bevy::prelude::*;

fn my_system(q: Query<(Entity, &Health)>) {
    for (entity, hp) in &q {
        if hp.0 < 1.0 {
            warn!("Entity {:?} má kriticky nízke HP: {}", entity, hp.0);
        }
        debug!("Entity {:?} HP: {}", entity, hp.0);
    }
}
```

Filter reťazec `"wgpu=error,bevy_render=info,my_game=debug"` hovorí: wgpu crate ukazuj len errory, bevy_render ukazuj info a vyššie, môj kód ukazuj debug a vyššie. Bez toho by ťa zaplavil log z rendereru.

### Vizualizácia colliderov a transformácií

```toml
[dependencies]
bevy = { version = "0.15", features = ["bevy_gizmos"] }
```

```rust
fn debug_draw(
    mut gizmos: Gizmos,
    q: Query<(&Transform, &Sprite)>,
) {
    for (tf, sprite) in &q {
        let size = sprite.custom_size.unwrap_or(Vec2::splat(32.0));
        // Nakresli červený obdĺžnik okolo každej entity
        gizmos.rect_2d(
            tf.translation.truncate(),
            size,
            Color::srgb(1.0, 0.0, 0.0),
        );
    }
}
```

Gizmos sú vykreslené len v debug mode — stačí ich systém odstrániť pre release build.

---

## Zhrnutie

| OOP | Bevy ECS |
|---|---|
| Objekt s dátami + metódami | Entity (ID) + Komponenty (dáta) + Systémy (logika) |
| Dedičnosť | Kompozícia komponentov |
| `obj.update()` | Systémy na všetkých entitách naraz |
| Globálny stav | Resource |
| Singleton | Resource s jednou inštanciou |
| Manuálny paralelizmus | Automatický (scheduler analyzuje závislosti) |
| Cache unfriendly (OOP heap) | Cache friendly (SoA layout v archetype) |

ECS nie je len pre hry — je to data-oriented design v čistej forme. Tisíce entít, cache-friendly, paralelizovateľné systémy. Bevy ti dá tieto výhody zadarmo, len musíš zmeniť spôsob myslenia: miesto toho aby si sa pýtal "čo robí tento objekt?", pýtaj sa "aké dáta má táto entita a aké systémy na ne reagujú?".

Ďalšia kapitola: Bevy grafika, vstup a pohyb — spravíme niečo čo sa hýbe na obrazovke.
