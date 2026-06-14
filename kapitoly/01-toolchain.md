# Kapitola 1 — Toolchain & Cargo

Predtým než napíšeš prvý riadok Rustu, potrebuješ vedieť ako funguje ekosystém nástrojov. Je to iné ako C — žiadny `gcc` volaný priamo, žiadne ručne písané Makefile pre deps. A je to iné ako Python alebo Node — žiadne globálne `pip install` kde nevieš čo máš nainstalované a prečo. Rust toolchain je jeden z najlepšie premyslených ekosystémov čo som videl — a to hovorím ako niekto kto prežil éru `autoconf`, `cmake` 2.x a npm dependency hell.

Celá inštalácia je jeden príkaz. Správa verzií je zabudovaná. Závislosti sú deterministické. Build systém, test runner, dokumentácia, linter, formátovač — všetko je súčasť jedného nástroja. Porovnaj to s C projektom kde potrebuješ cmake, conan alebo vcpkg, clang-tidy, clang-format, doxygen, a všetko musíš nastaviť a integrovať manuálne.

---

## Inštalácia: rustup

`rustup` je správca verzií Rustu — kombinuje `pyenv`, `nvm` a `apt install` do jedného nástroja. Nie je to len "nainštaluj Rust" — je to správca toolchainov, správca cieľových platforiem, správca komponentov. Keď sa objaví nová stable verzia, jeden príkaz ťa aktualizuje.

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Tento curl-to-shell prístup niektorých desí z bezpečnostných dôvodov. Ak ti to vadí, môžeš stiahnuť inštalátor ručne z rustup.rs alebo použiť package manager distribúcie (ale potom dostaneš staršiu verziu). Skript inštaluje `rustup` do `~/.cargo/bin/` a pridá to do PATH.

Po inštalácii:

```bash
rustup show          # aktívna toolchain a ciele (targets)
rustup update        # aktualizácia na najnovší stable
rustup toolchain list
# stable-x86_64-unknown-linux-gnu (default)
# nightly-x86_64-unknown-linux-gnu
```

`rustup show` ti ukáže kompletný stav — akú toolchain máš aktívnu, aké targets sú nainštalované, kde sú súbory na disku. Je to oveľa transparentnejšie než väčšina package managerov.

### Kanály

Rust má tri vydávacie kanály a každý existuje z konkrétneho dôvodu.

| Kanál | Popis | Kedy použiť |
|-------|-------|-------------|
| `stable` | Vydania každých 6 týždňov | Produkcia, vždy |
| `beta` | RC pred stable | Testovanie kompatibility |
| `nightly` | Denné buildy | Experimentálne featury, `#![feature(...)]` |

`stable` je to čo budeš používať 95% času. Rust má 6-týždňový release cyklus — relatívne rýchly, ale každý stable release prešiel testami a beta periódou. Backward compatibility je brané vážne: kód ktorý sa kompiloval na Rust 1.0 sa väčšinou kompiluje aj na 1.87.

`nightly` potrebuješ pre embedded (`no_std` s niektorými funkciami), pre proc-macro development, pre `async_fn_in_trait` keď to ešte nebolo stabilizované (teraz je). Keď pracuješ s nightly, daj do koreňa projektu súbor `rust-toolchain.toml`:

```toml
[toolchain]
channel = "nightly-2025-03-15"  # fixovaná verzia pre reprodukovateľnosť
```

Pre embedded (`no_std`) a niektoré proc-macro craty budeš potrebovať `nightly`:

```bash
rustup toolchain install nightly
rustup override set nightly  # len pre aktuálny adresár
```

`rustup override set` nastaví toolchain len pre aktuálny adresár — ostatné projekty ostanú na stable. Toto je elegantné riešenie problému "tento projekt potrebuje inú verziu kompilatora". V C svete by si musel meniť PATH alebo používať Docker.

---

## rustc — kompilátor

Priamo `rustc` voláš zriedka — podobne ako `cc` keď máš `cmake`. Cargo to robí za teba. Ale je dobré vedieť čo `rustc` robí, hlavne keď debuguješ build problémy alebo skúmaš čo kompilátor vygeneroval.

```bash
rustc --version
# rustc 1.87.0 (17067e9ac 2025-05-09)

rustc hello.rs -o hello
rustc hello.rs --edition 2021 -O  # optimizovaný build
```

`rustc` interně generuje LLVM IR a odovzdáva ho LLVM backendu. To je dôvod prečo Rust dosahuje porovnateľný výkon s Clangom — používa ten istý optimizačný backend. Môžeš si to aj pozrieť:

```bash
rustc --emit=llvm-ir hello.rs   # vypíše LLVM IR do hello.ll
rustc --emit=asm hello.rs       # vypíše assembly do hello.s
rustc --emit=mir hello.rs       # Mid-level IR (Rust-specific)
```

MIR (Mid-level Intermediate Representation) je zaujímavý — je to reprezentácia kódu *po* borrow checkingu, pred LLVM. Ak chceš pochopiť čo borrow checker robí, MIR je miesto kde to vidieť. Borrow checker operuje nad MIR, nie nad pôvodným kódom.

### Cross-compilation

Rust má vynikajúcu podporu cross-compilácie — oveľa lepšiu ako typické C toolchains kde potrebuješ správnu verziu `arm-linux-gnueabihf-gcc`, správne sysroot, správne hlavičkové súbory...

```bash
rustup target add thumbv7m-none-eabi    # ARM Cortex-M3 (no_std)
rustup target add wasm32-unknown-unknown # WebAssembly
rustup target add aarch64-unknown-linux-gnu

cargo build --target thumbv7m-none-eabi
```

`rustup target add` stiahne prekompilovanie štandardnej knižnice pre daný target. Pre `no_std` targety (embedded) toto stačí — `rustc` vie generovať kód pre ARM bez externého GCC. Pre Linux targety stále potrebuješ linker — `aarch64-linux-gnu-gcc` — ale kompilácia samotného Rust kódu prebehne bez problémov.

Target triple ako `thumbv7m-none-eabi` ti hovorí: ARM Thumb2 instruction set, Cortex-M3 variant, žiaden OS (`none`), Embedded ABI. Rust pozná stovky targetov a pre každý vie skompilovať `core` knižnicu.

---

## Cargo — build systém a správca závislostí

Cargo je všetko v jednom: `make` + `cmake` + `apt` + `pip` + `npm`. To znie ako overengineering, ale v praxi je to úžasne pohodlné. Jeden nástroj, jeden príkazový riadok, konzistentné správanie naprieč projektmi. Žiadne "ako skompilovať tento projekt?" — `cargo build` funguje všade.

Cargo je zodpovedný za niekoľko vecí naraz: resolvuje závislosti (z crates.io alebo git repozitárov), sťahuje a kompiluje ich, spravuje features, spúšťa testy a benchmarky, generuje dokumentáciu. Rozumie workspacom (viacero crates v jednom repozitári). A robí to správne — Cargo.lock zaručuje deterministické buildy.

### Základné príkazy

```bash
cargo new my_project          # nový binárny projekt (src/main.rs)
cargo new my_lib --lib        # knižnica (src/lib.rs)
cargo init                    # inicializuj Cargo projekt v existujúcom adresári

cargo build                   # debug build → target/debug/my_project
cargo build --release         # release build → target/release/my_project
cargo run                     # build + spusti
cargo run -- --port 8080      # argumenty pre program (za --)

cargo test                    # spusti všetky testy
cargo test integration        # testy obsahujúce "integration" v názve
cargo test -- --nocapture     # zobraz stdout aj pri úspechu

cargo check                   # kontrola typov BEZ linkovania — rýchlejšie ako build
cargo clippy                  # linter — zachytí bugy ktoré kompilátor prepustí
cargo fmt                     # formátovanie kódu (rustfmt)
cargo doc --open              # vygeneruj a otvor dokumentáciu
cargo clean                   # vymaž target/ adresár
```

`cargo check` si zaslúži osobitné miesto. Robí plnú typovú kontrolu vrátane borrow checkera, ale negeneruje binárku ani nerobí linking. Je 3–5× rýchlejší ako `cargo build`. Počas vývoja, keď iteruješ rýchlo, `cargo check` je tvoj priateľ. Daj si ho na klávesovú skratku v editore — väčšina LSP integrácií (rust-analyzer) to robí automaticky pri ukladaní.

`cargo test` je zabudovaný test runner. Testy sa píšu priamo v kóde so `#[test]` atribútom — žiadne externé frameworky potrebné pre základné unit testy. Integračné testy idú do `tests/` adresára.

### Debug vs Release build

Toto je dôležité pochopiť. Debug build (`cargo build`) kompiluje rýchlo ale výsledok je pomalý — bez optimalizácii, s plnými debug symbolmi, s bounds checking panics. Release build (`cargo build --release`) kompiluje dlho ale výsledok je rýchly — `-O3`, LTO, strip.

Rozdiely vo výkone môžu byť dramatické — 10× až 100× pre compute-heavy kód. Vždy benchmarkuj release build. Debug build je len pre vývoj a debugovanie.

---

## Cargo.toml — konfigurácia projektu

`Cargo.toml` je srdce projektu. Je to oveľa čitateľnejšie ako `CMakeLists.txt` a oveľa menej krehké ako `package.json`. TOML (Tom's Obvious, Minimal Language) je jednoduchý config formát — nie YAML (kde záleží na odsadení) nie JSON (žiadne komentáre).

```toml
[package]
name = "my_tool"
version = "0.1.0"
edition = "2021"        # Rust edition — vždy 2021 pre nové projekty
authors = ["Miro <miro@example.com>"]

[dependencies]
tokio = { version = "1", features = ["full"] }   # async runtime
serde = { version = "1", features = ["derive"] } # serializácia
anyhow = "1"                                      # error handling
clap = { version = "4", features = ["derive"] }  # CLI argumenty

[dev-dependencies]
# závislosti len pre testy
tempfile = "3"

[build-dependencies]
# závislosti pre build.rs script
cc = "1"

[profile.release]
opt-level = 3
lto = true          # link-time optimization
strip = true        # odstráň debug symboly z binárky
codegen-units = 1   # pomalší build, rýchlejší výsledok

[profile.dev]
opt-level = 0
debug = true
```

Sekcia `[dependencies]` je kde definuješ čo projekt potrebuje. Verzie sú SemVer — `"1"` znamená "akákoľvek 1.x verzia kompatibilná s 1.0.0". Cargo resolvuje najvyššiu kompatibilnú verziu. `Cargo.lock` zaznamenáva presné verzie pre reprodukovateľné buildy.

`[dev-dependencies]` sú závislosti dostupné len počas `cargo test` a `cargo bench` — nie v produkčnej binárke. Vhodné pre mock knižnice, test utilities, property-based testing frameworky.

`[profile.release]` s `codegen-units = 1` a `lto = true` výrazne zlepšuje výkon finálnej binárky na úkor doby kompilácie. Pre produkčné buildy v CI je toto štandardné nastavenie. `strip = true` odstráni debug symboly a zmenší binárku — dôležité pre embedded a WASM.

### Editions

`edition = "2021"` je dôležité. Rust editions sú mechanizmus ako jazyk môže zmeniť syntax a sémantiku bez porušenia spätnej kompatibility. Každý crate deklaruje svoju edition a kompilátor ju rešpektuje. Crates s rôznymi editions môžu existovať v tom istom projekte.

Editions sú 2015, 2018, 2021, a čoskoro 2024. Pre každý nový projekt vždy používaj najnovšiu edition — dostaneš najlepšie ergonomics a najmenej "gotchas". Migrovanie starého kódu na novú edition zvládne `cargo fix --edition`.

### Features (podmienená kompilácia)

Ekvivalent `#ifdef` v C, ale na úrovni Cargo a oveľa čistejší. Features ti umožňujú mať optional functionality, optional dependencies, a rôzne konfigurácie bez duplikácie kódu.

```toml
[features]
default = ["std"]
std = []
async = ["dep:tokio"]
tls = ["dep:rustls"]
```

```rust
#[cfg(feature = "async")]
pub async fn connect() { /* ... */ }

#[cfg(not(feature = "std"))]
use core::fmt;  // no_std: core namiesto std
```

```bash
cargo build --features "async,tls"
cargo build --no-default-features --features "async"
```

Pre `no_std` vývoj (embedded) je bežné mať `default = ["std"]` a pri cross-compile použiť `--no-default-features`. Knižnica potom môže byť použiteľná na desktopu aj na mikrokontroléri bez duplikácie kódu.

Rozdiel oproti C `#ifdef`: Cargo features sú *additive* — nedajú sa použiť na podmienené vypnutie funkčnosti. To je zámerné — zabraňuje situácii kde crate funguje s jednou kombináciou features ale nie s inou. Ak potrebuješ skutočne mutual exclusive features, musíš to riešiť inak.

---

## Cargo workspace

Pre väčšie projekty s viacerými crates — presne ako tento projekt, alebo ako produkčný Rust monorepo:

```toml
# priklady/Cargo.toml
[workspace]
members = [
    "ownership",
    "types",
    "concurrency",
]
resolver = "2"   # dôležité pri features s async crates
```

```bash
cargo build --workspace        # všetky members
cargo test -p ownership        # len jeden crate z workspace
cargo run -p concurrency       # spusti konkrétny crate
```

Výhoda: zdieľaná `target/` — každý crate sa kompiluje len raz aj keď je závislým viacerých iných crates vo workspace. Bez workspace by každý projekt mal vlastný `target/` a dependencies by sa kompilovalo n-krát. S workspace kompiluješ jednorazovo.

`resolver = "2"` je moderný feature resolver ktorý lepšie zvláda situácie kde rovnaký crate je stiahnutý dvakrát s rôznymi features. Pre projekty s async dependencies (tokio, async-std) je toto prakticky povinné.

---

## Clippy — linter

Clippy je linter pre Rust — ale nie bežný linter. C `lint` alebo moderný clang-tidy zachytávajú syntaktické problémy a niektoré jednoduché logické chyby. Clippy zachytáva idiomatické problémy, výkonnostné antipatterns a logické bugy ktoré kompilátor z princípu nedostane (pretože sú validný Rust).

```bash
cargo clippy
cargo clippy -- -D warnings   # treatuj warningy ako errory (vhodné v CI)
```

Príklady čo clippy zachytí — a prečo na tom záleží:

```rust
// clippy::needless_range_loop
for i in 0..v.len() {          // ← clippy: použij `for x in &v`
    println!("{}", v[i]);
}
```

Toto nie je len estetika. Indexovanie `v[i]` robí bounds check pri každom prístupe. Iterátor cez `&v` kompilátor môže optimalizovať — vie že iteration je bezpečná a nemusí bounds checkovať každý prvok zvlášť. Clippy tu ukazuje nielen idiomatickejší kód, ale aj potenciálne rýchlejší.

```rust
// clippy::redundant_clone
let s = String::from("hello");
let _x = s.clone();            // ← clippy: clone nie je potrebný (move by stačil)
drop(s);

// clippy::comparison_to_empty
if v.len() == 0 {              // ← clippy: použij `v.is_empty()`
    println!("prázdne");
}
```

`v.is_empty()` nie je len peknejšie — pre niektoré dátové štruktúry (linked list) je `len()` O(n) a `is_empty()` je O(1). Clippy ťa núti robiť to správne.

Clippy má stovky lintov roztriedených podľa závažnosti — od "style" po "correctness". Môžeš ich konfigurovať v `Cargo.toml` alebo lokálne s `#[allow(clippy::...)]`. V CI odporúčam `-D warnings` — zabrán to tomu aby sa warningy nazbierali a zabudli.

---

## rustfmt — formátovanie

Jeden z najcennejších nástrojov nie pre to čo robí technicky, ale čo eliminuje. V každom C/C++ tíme existuje nekonečná diskusia o zátvorkách: Na rovnakom riadku alebo na novom? Medzera pred `{` alebo nie? Tabulátory alebo medzery? Koľko? Tieto diskusie sú stratou času a energie.

`rustfmt` tieto diskusie eliminiuje. Jeden štýl, jeden nástroj, žiadne voľby. Konfigurácia v `rustfmt.toml` existuje ale väčšina projektov ju nepoužíva — defaulty sú dobré:

```toml
edition = "2021"
max_width = 100
tab_spaces = 4
```

```bash
cargo fmt          # formátuj
cargo fmt --check  # len skontroluj (exit 1 ak treba zmeny) — pre CI
```

`cargo fmt --check` v CI zabraňuje commitovaniu neformátovaného kódu. Nie je to voliteľné odporúčanie — je to gating condition. Ak kód nie je formátovaný, pipeline zlyhá.

Dôležité pochopiť: `rustfmt` zmení *layout* kódu, nie jeho *sémantiku*. Môže preformátovať dlhé výrazy na viac riadkov, zmeniť odsadenie, upraviť medzery. Výsledok je vždy ekvivalentný pôvodnému kódu. Commituj výsledok `cargo fmt` ako súčasť PR — revieweri uvidia logické zmeny, nie formátovacie shluky.

---

## Príklad: CLI nástroj s argumentmi

Celý spustiteľný príklad — parsovanie argumentov cez `clap`. Toto ilustruje ako rýchlo sa v Ruste dostaneš k funkčnému CLI nástroju, vrátane `--help` generovania, validácie typov a default hodnôt:

```toml
# Cargo.toml
[dependencies]
clap = { version = "4", features = ["derive"] }
```

```rust
use clap::Parser;

/// Jednoduchý sieťový scanner — demo pre Kapitolu 1
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Cieľová adresa
    #[arg(short, long, default_value = "127.0.0.1")]
    host: String,

    /// Port na skenovanie
    #[arg(short, long, default_value_t = 80)]
    port: u16,

    /// Timeout v sekundách
    #[arg(short, long, default_value_t = 5)]
    timeout: u64,

    /// Verbose výstup
    #[arg(short, long)]
    verbose: bool,
}

fn main() {
    let args = Args::parse();

    if args.verbose {
        println!("Skenujem {}:{} (timeout: {}s)", args.host, args.port, args.timeout);
    }

    // Simulácia skenovania
    println!("{}:{} — open", args.host, args.port);
}
```

Čo sa tu deje za kulisami je zaujímavé. `#[derive(Parser)]` je proc-macro — kód ktorý beží počas kompilácie a generuje implementáciu parsera zo struct definície. Docstringy (`///`) sa stávajú nápoveďou v `--help`. Typy polí (`u16`, `u64`, `bool`) určujú ako sa argumenty parsujú a validujú — `--port abc` dá chybu pri parsovaní, nie runtime panic.

```bash
cargo run -- --host 192.168.1.1 --port 22 --verbose
# Skenujem 192.168.1.1:22 (timeout: 5s)
# 192.168.1.1:22 — open

cargo run -- --help
# Jednoduchý sieťový scanner — demo pre Kapitolu 1
# Usage: scanner [OPTIONS]
# Options:
#   -h, --host <HOST>        [default: 127.0.0.1]
#   -p, --port <PORT>        [default: 80]
#   ...
```

`clap` derive API generuje celý parser zo struct anotácií — nič netreba ručne písať. Pre porovnanie, ekvivalent v C by vyžadoval `getopt_long`, manuálne definovanie `option` struct, switch/case na každý argument, manuálne default hodnoty, ručne písaný `--help` text. Asi 80 riadkov kódu čo je tu 25.

---

## Štruktúra projektu v praxi

Cargo konvencie sú pevné a dobre premyslené. Keď otvoríš cudzí Rust projekt, vieš presne kde čo nájsť:

```
my_project/
├── Cargo.toml
├── Cargo.lock           ← verzované do gitu (binárky), nie knižnice
├── src/
│   ├── main.rs          ← vstupný bod (alebo lib.rs pre knižnice)
│   ├── lib.rs           ← ak chceš aj bin aj lib v jednom crate
│   └── bin/
│       └── helper.rs    ← extra binárky: cargo run --bin helper
├── tests/
│   └── integration.rs   ← integračné testy
├── benches/
│   └── perf.rs          ← benchmarky (criterion)
├── examples/
│   └── basic.rs         ← cargo run --example basic
└── build.rs             ← build script (generovanie kódu, FFI bindgen)
```

`tests/` adresár je špeciálny — každý súbor je samostatný binary crate ktorý linkuje proti `src/lib.rs`. Toto je ideálne pre integračné testy ktoré testujú verejné API. Unit testy (s `#[test]`) idú priamo do modulov v `src/` — konvencia je mať `#[cfg(test)] mod tests { ... }` na konci každého súboru.

`examples/` adresár je podceňovaný. Príklady sú spustiteľné (`cargo run --example basic`), kompilujú sa pri `cargo test`, a slúžia ako žijúca dokumentácia. Ak príklad nefunguje, `cargo test` to zachytí.

`build.rs` je build script — Rust súbor ktorý sa spustí pred kompiláciou. Použiteľné pre generovanie kódu z protobuf/flatbuffers, pre FFI bindings cez `bindgen`, pre kompiláciu C kódu cez `cc` crate. Je to oveľa čistejšie než cmake custom commands.

### Cargo.lock a reprodukovateľné buildy

```
# Binárky (aplikácie): Cargo.lock verzuj do gitu — reprodukovateľné buildy
# Knižnice (crates.io): Cargo.lock do .gitignore — verzie určuje konzument
```

Toto je dôležitá konvencia. Ak píšeš aplikáciu (binárku), `Cargo.lock` do gitu zaručí že všetci v tíme a CI systém budú kompilovať presne rovnaké verzie dependencies. Žiadne "u mňa to funguje" situácie spôsobené rôznymi verziami transitive dependencies.

Ak píšeš knižnicu pre crates.io, `Cargo.lock` do `.gitignore`. Používatelia tvojej knižnice majú vlastný `Cargo.lock` a chcú mať kontrolu nad verziami. Tvoj lock by im len prekážal.

---

## Rýchlostný tip: sccache

Kompilácia Rustu môže byť pomalá — hlavne pre veľké projekty alebo projekty s mnohými dependencies. `sccache` (shared compilation cache) pomáha keď pracuješ na viacerých projektoch alebo keď builduje CI:

```bash
cargo install sccache
export RUSTC_WRAPPER=sccache
cargo build  # prvý build normálne, ďalší rýchlejšie
```

`sccache` kešuje kompilačné výsledky na disku (alebo v S3 pre CI zdieľanie). Ak kompiluješ rovnakú verziu rovnakého cratu viackrát (napríklad `tokio` v piatich rôznych projektoch), druhý a ďalšie buildy použijú kešovaný výsledok. Na veľkých CI farmách toto môže ušetriť hodiny build time denne.

Ďalší tip: `cargo build` s `--timings` flagom (`cargo build --timings=html`) vygeneruje HTML report o tom čo trvalo najdlhšie. Môžeš vidieť ktoré crates sú bottleneck a optimalizovať features alebo dependency tree.

---

## Typické chyby začiatočníkov s toolchainom

Prvá chyba ktorú robí každý: `cargo build` v debug mode, benchmarknutie výsledku, záver "Rust je pomalý". Debug build má vypnuté optimalizácie a zapnuté extra kontroly. Vždy benchmarkuj `cargo build --release`.

Druhá chyba: `cargo clean` ako riešenie na každý problém. `cargo clean` vymaže celý `target/` adresár a nucuje rekompilovať všetko od nuly. Toto trvá minúty. Potrebuješ to len zriedka — zvyčajne keď meníš `build.rs` alebo keď Cargo cache je naozaj poškodená. Väčšinou stačí `cargo build` a Cargo si vysporiada čo treba prekompilovať.

Tretia chyba: ignorovanie `cargo clippy` warningov. Clippy nie je pedantný teacher — jeho warnings sú zvyčajne o reálnych problémoch alebo suboptimalnom kóde. Venuj im pozornosť, hlavne na začiatku keď sa učíš idiomatický Rust.

---

V ďalšej kapitole ideme na to podstatné — Ownership & Borrowing. Všetko čo sme teraz nastavili, budeme používať na spúšťanie a testovanie príkladov. Toolchain je nastavený, Cargo funguje, máš linter aj formátovač. Teraz sa môžeme sústrediť na to čo robí Rust skutočne unikátnym.
