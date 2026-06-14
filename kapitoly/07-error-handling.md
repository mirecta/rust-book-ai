# Kapitola 7 — Error Handling

Chybové spracovanie je jeden z tých tém, kde sa jazyky naozaj odlišujú — nie v tom čo môžu robiť, ale v tom čo ťa nútia robiť. C ťa nič nenúti — vrátiš `-1`, zabudneš skontrolovať, segfault o týždeň. C++ má exceptions, ale volanie ktoré hádže výnimku vyzerá rovnako ako volanie ktoré nehádže. Java má checked exceptions ale každý ich wrapuje do `RuntimeException` lebo sú otravné. Go má `if err != nil` všade, čo je aspoň explicitné, ale ľahko ho preskoč.

Rust má dve úrovne chybového spracovania: `panic!` pre nerecoverable chyby a `Result<T, E>` pre recoverable. Žiadne exceptions, žiadne `setjmp/longjmp`. A čo je kľúčové — ak funkcia vráti `Result`, nemôžeš ho ignorovať bez explicit `let _ =`. Kompilátor ti to povie. Toto je nie náhoda, je to dizajn.

---

## Prečo nie exceptions

Exceptions majú jeden fundamentálny problém: porušujú princíp "čo vidíš, to dostaneš". Keď voláš funkciu v C++ alebo Jave, nemôžeš z jej signatúry zistiť, či môže hodiť výnimku a akú. V C++ môže každá funkcia hodiť čokoľvek ak nie je označená `noexcept`. To vedie k dvom extrémom: buď ignoruješ exceptions úplne (a máš memory leaky pri unwindingu), alebo wrappuješ každé volanie do try-catch (a máš kód horší než Rust's `Result`).

Druhý problém je výkon. Exception handling v C++ používa tabuľky pre stack unwinding (zero-cost v happy path, ale binárka je väčšia a unwind je pomalý). V embedded systémoch sú exceptions often zakázané úplne (`-fno-exceptions`), čo znamená, že celá knižnica musí mať alternatívne API.

`Result<T, E>` je enum — buď `Ok(T)` alebo `Err(E)`. Je to len typ, žiadna špeciálna jazyková feature. Môžeš ho uložiť do premennej, preniesť cez channel, vrátiť z closure. A čo je dôležité — v happy path neexistuje žiadny overhead. `Result` je stack-allocated, väčší o veľkosť `E`. Žiadne tabuľky, žiadny unwinding.

---

## panic! vs Result

```rust
// panic! — program sa ukončí (alebo unwind)
// Použiť keď: invariant je porušený, pokračovanie by bolo nebezpečné
fn get_element(v: &[u32], idx: usize) -> u32 {
    if idx >= v.len() {
        panic!("index {} mimo hraníc (len={})", idx, v.len());
    }
    v[idx]  // alebo: v[idx] — automatický bounds check, panic ak out of bounds
}

// Result — caller môže chybu ošetriť
fn read_config(path: &str) -> Result<String, std::io::Error> {
    std::fs::read_to_string(path)
}
```

Pravidlo je jednoduché: `panic!` = "bug v programe — niekto zavolal funkciu so zlými argumentami alebo je systém v nevalidnom stave". `Result` = "očakávaná chybová situácia — súbor neexistuje, sieť nereaguje, vstup má zlý formát".

V praxi: `panic!` v knižničnom kóde je vždy podozrivý. Knižnica nikdy nevie, v akom kontexte beží — možno je to server, ktorý musí ostať hore. Vlastné knižnice by mali vždy vrátiť `Result` a nechať aplikačný kód rozhodnúť, či je chyba fatálna. Výnimkou sú invarianty — ak tvoja funkcia má pre-condition (napr. vstupný slice musí mať aspoň 4 bajty), môžeš panick-ovať na porušenie. Ale dokumentuj to.

### unwrap() a expect() — kedy je to OK

```rust
// unwrap() v testoch — ok, test fail-ne zreteľne
#[test]
fn test_parsing() {
    let result = parse_port("8080").unwrap();
    assert_eq!(result, 8080);
}

// expect() s popisom — lepší ako unwrap() v produkčnom kóde
let config = std::fs::read_to_string("app.toml")
    .expect("app.toml musí existovať vedľa binárky");

// unwrap() pri inicializácii kde panic = config error
// (ak regex je zle, je to bug, nie runtime error)
use regex::Regex;
static EMAIL_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"[^@]+@[^@]+\.[^@]+").unwrap());
```

`expect()` je lepší než `unwrap()` lebo pri panicu ukáže tvoju správu. Debugovanie `thread 'main' panicked at 'called Result::unwrap() on Err: Os { code: 2, message: "No such file or directory" }, src/main.rs:47` je oveľa lepšie než len `called Result::unwrap() on an Err value`.

---

## `?` operátor — propagácia chýb

`?` za `Result` hodnotou je syntaktický cukor ktorý robí tri veci: ak je hodnota `Ok(v)`, extrahuje `v` a pokračuje. Ak je hodnota `Err(e)`, volá `From::from(e)` pre konverziu error typu a okamžite vráti `Err`. Je to ako checked exception, ale explicitné a nulovo-overhead.

```rust
use std::io;
use std::fs;

fn load_and_parse(path: &str) -> Result<u32, io::Error> {
    let content = fs::read_to_string(path)?;  // Err → return okamžite
    let trimmed = content.trim();
    let value: u32 = trimmed.parse().map_err(|e| {
        io::Error::new(io::ErrorKind::InvalidData, e)
    })?;
    Ok(value)
}
```

Bez `?` by to bolo explicitné a oveľa dlhšie:

```rust
fn load_and_parse_verbose(path: &str) -> Result<u32, io::Error> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => return Err(e),
    };
    let trimmed = content.trim();
    let value: u32 = match trimmed.parse() {
        Ok(v) => v,
        Err(e) => return Err(io::Error::new(io::ErrorKind::InvalidData, e)),
    };
    Ok(value)
}
```

Obe sú funkčne ekvivalentné. `?` verzia je čitateľnejšia a presne odráža "happy path" — čítame súbor, parsujeme číslo, vraciame ho. Chyby sú ošetrené implicitne ale nie skryté — každý `?` je viditeľný signál, že tu môže nastať chyba.

### `?` v `main()` — od Rust 2018

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let val = load_and_parse("config.txt")?;
    println!("hodnota: {}", val);
    Ok(())
}
```

`Box<dyn std::error::Error>` je dynamický typ pre akýkoľvek error. Výhodný pre `main()` a jednoduchý prototyping, ale v produkčnom kóde chceš konkrétny typ (alebo `anyhow::Error`).

### Pod kapotou — čo `?` generuje

`?` je desugared na zhruba toto:

```rust
// Toto:
let x = some_result?;

// Sa rozbalí na:
let x = match some_result {
    Ok(val) => val,
    Err(e) => return Err(From::from(e)),
};
```

`From::from(e)` je kľúčové — umožňuje automatickú konverziu error typov. Ak tvoj error typ implementuje `From<io::Error>`, môžeš použiť `?` na `io::Error` aj keď tvoja funkcia vracia iný error typ. Toto je základ pre ergonomické error handling.

---

## Vlastné error typy — správny spôsob pre knižnice

Pre knižnice (library crates) je best practice definovať vlastný error typ. Dôvod: caller musí vedieť čo môže ísť zle, a string chybovej správy na to nestačí — potrebuje vedieť, či chyba je recoverable, či je to I/O chyba alebo chyba vstupu, a či môže retry.

```rust
use std::fmt;
use std::num::ParseIntError;

#[derive(Debug)]
enum ConfigError {
    Io(std::io::Error),
    Parse(ParseIntError),
    MissingKey(String),
    InvalidValue { key: String, value: String },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "I/O chyba: {}", e),
            ConfigError::Parse(e) => write!(f, "chyba parsovania: {}", e),
            ConfigError::MissingKey(k) => write!(f, "chýbajúci kľúč: {}", k),
            ConfigError::InvalidValue { key, value } => {
                write!(f, "neplatná hodnota pre '{}': '{}'", key, value)
            }
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::Io(e) => Some(e),
            ConfigError::Parse(e) => Some(e),
            _ => None,
        }
    }
}

// From konverzie — umožnia ? operátor
impl From<std::io::Error> for ConfigError {
    fn from(e: std::io::Error) -> Self {
        ConfigError::Io(e)
    }
}

impl From<ParseIntError> for ConfigError {
    fn from(e: ParseIntError) -> Self {
        ConfigError::Parse(e)
    }
}

fn load_port(path: &str) -> Result<u16, ConfigError> {
    let content = std::fs::read_to_string(path)?;  // io::Error → ConfigError::Io
    let port: u16 = content.trim().parse()?;        // ParseIntError → ConfigError::Parse
    if port == 0 {
        return Err(ConfigError::InvalidValue {
            key: "port".to_string(),
            value: "0".to_string(),
        });
    }
    Ok(port)
}
```

Toto je verbose — musíš implementovať `Display`, `Error`, a každý `From`. Pre knižnicu s desiatkami error variantov je to desiatky riadkov boilerplate. Preto existuje `thiserror`.

---

## thiserror — derive macro pre error typy

`thiserror` generuje presne ten boilerplate čo by si napísal ručne — `Display` a `From` implementácie. Žiadna runtime závislosť, žiadny overhead. Je to clean zero-cost abstraction.

```toml
[dependencies]
thiserror = "2"
```

```rust
use thiserror::Error;

#[derive(Debug, Error)]
enum ConfigError {
    #[error("I/O chyba: {0}")]
    Io(#[from] std::io::Error),

    #[error("chyba parsovania: {0}")]
    Parse(#[from] std::num::ParseIntError),

    #[error("chýbajúci kľúč: {0}")]
    MissingKey(String),

    #[error("neplatná hodnota pre '{key}': '{value}'")]
    InvalidValue { key: String, value: String },
}

// #[from] automaticky generuje From<io::Error> for ConfigError
// #[error("...")] generuje Display s formátovaním
// Hotovo — rovnaká funkcionalita, 1/3 kódu
```

Formátovací string v `#[error("...")]` podporuje `{0}` pre tuple varianty a `{field_name}` pre struct varianty. Môžeš kombinovať `{source}` pre implicitné source chaining.

Dôležitá vec: `#[from]` na poli automaticky generuje `From<FieldType> for ThisError`. Ale môžeš mať najviac jeden `#[from]` per variant — ak chceš dva rôzne I/O errory, musíš ich odlíšiť typom alebo wrappovať:

```rust
#[derive(Debug, Error)]
enum AppError {
    #[error("chyba čítania konfigurácie: {0}")]
    ConfigRead(#[from] std::io::Error),

    // NEFUNGUJE — druhý From<io::Error> by bol konflikt
    // #[error("chyba zápisu výstupu: {0}")]
    // OutputWrite(#[from] std::io::Error),

    // Riešenie: wrapper typ
    #[error("chyba zápisu výstupu: {0}")]
    OutputWrite(std::io::Error),
}

impl AppError {
    fn output_write(e: std::io::Error) -> Self {
        AppError::OutputWrite(e)
    }
}
```

---

## anyhow — rýchly error handling v aplikáciách

Pre knižnice: vlastné error typy (`thiserror`). Pre aplikácie / binárky: `anyhow`. Rozdiel je filozofický. Knižnica musí dať callerovi štruktúrovaný error ktorý môže programaticky spracovať. Aplikácia (binárka) typicky chybu buď zobrazí používateľovi alebo zaloguje a skončí — nepotrebuje rozlíšiť `ConfigError::Io` od `NetworkError::Timeout`, potrebuje dobrú chybovú správu s kontextom.

`anyhow::Error` je type-erased kontajner ktorý akceptuje akýkoľvek typ implementujúci `std::error::Error + Send + Sync + 'static`. Vnútorne drží box s dynamickým dispatchom a reťazec kontextových správ.

```toml
[dependencies]
anyhow = "1"
```

### Základné použitie

```rust
use anyhow::{Context, Result, bail, ensure};

fn read_config(path: &str) -> Result<Config> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("neviem prečítať konfig: {}", path))?;

    let config: Config = toml::from_str(&content)
        .context("konfig nie je validný TOML")?;

    Ok(config)
}
```

`anyhow::Result<T>` = `Result<T, anyhow::Error>`. `.context("popis")` pridá vrstvu kontextu k chybe — ak chyba prebublá nahor, každý `.context()` sa pridá do reťazca. `.with_context(|| ...)` používa closure pre lazy formátovanie (nevyhodnotí string ak chyba nenastala).

### bail! a ensure!

`bail!` a `ensure!` sú helper makrá ktoré šetria písanie:

```rust
fn connect(host: &str, port: u16) -> Result<()> {
    ensure!(port > 0, "port nesmie byť 0");
    ensure!(port < 65535, "port {} je mimo rozsahu", port);

    if host.is_empty() {
        bail!("host nesmie byť prázdny");
    }

    // ... skutočné pripojenie
    Ok(())
}
```

`bail!(msg)` je skratka pre `return Err(anyhow::anyhow!(msg))`. `ensure!(cond, msg)` je `if !cond { bail!(msg) }`. V praxi ich používaš pre rýchle validácie vstupov na začiatku funkcie.

### Context reťazenie — debugovanie chýb

Toto je jedna z najväčších výhod `anyhow` oproti holému `Box<dyn Error>`. Každý `.context()` pridá vrstvu, a keď error vypíšeš cez `{:#}` (alternate format), uvidíš celý reťazec:

```rust
fn full_pipeline(config_path: &str, output_path: &str) -> Result<()> {
    let config = read_config(config_path)
        .context("inicializácia zlyhala")?;

    let data = fetch_data(&config.url)
        .with_context(|| format!("sťahovanie z {} zlyhalo", config.url))?;

    write_output(output_path, &data)
        .with_context(|| format!("zápis do {} zlyhal", output_path))?;

    Ok(())
}
```

Keď chyba prebubláva nahor, každý `.context()` pridá vrstvu:

```
Error: inicializácia zlyhala

Caused by:
    0: neviem prečítať konfig: /etc/app.toml
    1: No such file or directory (os error 2)
```

V porovnaní s C kde dostaneš len `errno = 2` a musíš hádať kde nastalo — toto je luxus. Každý level zásobníka popísal čo robil keď nastala chyba.

### anyhow a downcast — keď potrebuješ pôvodný typ

`anyhow::Error` ti dovolí downcast-ovať naspäť na pôvodný typ ak potrebuješ programaticky reagovať na konkrétny druh chyby:

```rust
fn handle_error(err: anyhow::Error) {
    if let Some(io_err) = err.downcast_ref::<std::io::Error>() {
        match io_err.kind() {
            std::io::ErrorKind::NotFound => eprintln!("Súbor nenájdený, použijem default"),
            std::io::ErrorKind::PermissionDenied => eprintln!("Nedostatočné práva"),
            _ => eprintln!("I/O chyba: {}", io_err),
        }
    } else {
        eprintln!("Neočakávaná chyba: {:#}", err);
    }
}
```

Toto je kompromis — stratíš statický typ, ale môžeš ho získať späť za cenu runtime check-u. Pre aplikačný kód je to zvyčajne akceptovateľné.

---

## Vlastné error typy — pokročilé vzory

### Error s kontextom

Niekedy chceš vlastný error aj v aplikácii, napríklad ak chceš rozlíšiť rôzne druhy zlyhaní pre exit kód:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
enum AppError {
    #[error("konfigurácia: {0}")]
    Config(#[from] ConfigError),

    #[error("sieť: {0}")]
    Network(#[from] NetworkError),

    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
}

impl AppError {
    fn exit_code(&self) -> i32 {
        match self {
            AppError::Config(_) => 78,   // EX_CONFIG
            AppError::Network(_) => 69,  // EX_UNAVAILABLE
            AppError::Io(_) => 74,       // EX_IOERR
        }
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Chyba: {:#}", e);
        std::process::exit(e.exit_code());
    }
}
```

### Result ako návratový typ v trait-och

Keď definuješ trait, môžeš použiť associated type pre error:

```rust
trait DataSource {
    type Error: std::error::Error + Send + Sync + 'static;

    fn fetch(&self, key: &str) -> Result<Vec<u8>, Self::Error>;
}

struct FileSource { base_path: std::path::PathBuf }

#[derive(Debug, thiserror::Error)]
#[error("file source error: {0}")]
struct FileSourceError(#[from] std::io::Error);

impl DataSource for FileSource {
    type Error = FileSourceError;

    fn fetch(&self, key: &str) -> Result<Vec<u8>, Self::Error> {
        let path = self.base_path.join(key);
        Ok(std::fs::read(&path)?)
    }
}
```

---

## Kompletný príklad: CLI nástroj s poriadnym error handlingom

Čítanie konfiguračného súboru, HTTP request, zápis výsledku — s plným error handlingom, kontextom a štruktúrovanými errormi:

```toml
[dependencies]
anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

```rust
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Deserialize, Debug)]
struct Config {
    host: String,
    port: u16,
    timeout_secs: u64,
}

#[derive(Serialize, Debug)]
struct Output {
    status: String,
    bytes_received: usize,
}

fn load_config(path: &Path) -> Result<Config> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("konfig súbor '{}'", path.display()))?;

    serde_json::from_str(&content)
        .with_context(|| format!("neplatný JSON v '{}'", path.display()))
}

fn run(config: &Config) -> Result<Output> {
    if config.port == 0 {
        bail!("port nesmie byť 0");
    }

    // Simulácia sieťového volania
    let addr = format!("{}:{}", config.host, config.port);
    println!("Pripájam sa na {} (timeout: {}s)...", addr, config.timeout_secs);

    // Reálny príklad by použil TcpStream alebo reqwest
    Ok(Output {
        status: "OK".to_string(),
        bytes_received: 1024,
    })
}

fn write_output(path: &Path, output: &Output) -> Result<()> {
    let json = serde_json::to_string_pretty(output)
        .context("serializácia výstupu zlyhala")?;

    std::fs::write(path, json)
        .with_context(|| format!("zápis do '{}'", path.display()))
}

fn main() -> Result<()> {
    let config = load_config(Path::new("config.json"))
        .context("načítanie konfigurácie")?;

    let output = run(&config)
        .context("spustenie")?;

    write_output(Path::new("output.json"), &output)
        .context("zápis výsledku")?;

    println!("Hotovo: {:?}", output);
    Ok(())
}
```

Spustenie s chybou keď `config.json` neexistuje:
```
Error: načítanie konfigurácie

Caused by:
    0: konfig súbor 'config.json'
    1: No such file or directory (os error 2)
```

Spustenie s neplatným JSON-om:
```
Error: načítanie konfigurácie

Caused by:
    0: neplatný JSON v 'config.json'
    1: expected ident at line 1 column 2
```

Každý error jasne hovorí kde nastal problém, čo sa pokúšalo a aká bola pôvodná systémová chyba. Toto je štandard v produkčných nástrojoch.

---

## Časté chyby začiatočníkov

### Chyba 1: Používanie `unwrap()` všade "zatiaľ"

```rust
// Toto sa dostane do produkcie
let port: u16 = config.get("port").unwrap().parse().unwrap();
```

`unwrap()` v produkčnom kóde je technický dlh. "Zatiaľ" sa stane "navždy". Pravidlo: ak je to v `main.rs` alebo v `bin/`, použi `?` s `anyhow`. Ak je to v `lib.rs`, vráť `Result` s vlastným error typom. `unwrap()` iba v testoch a pri provably correct hodnotiach (regex kompilovaný zo string literálu, napríklad).

### Chyba 2: Ignorovanie Result

```rust
// Rust ti dá warning, ale nie error
std::fs::write("output.txt", data);  // warning: unused Result

// Explicitné ignorovanie ak naozaj nevadí
let _ = std::fs::write("/tmp/debug.txt", data);  // ok, zámerné
```

Rust dáva `#[must_use]` warning na `Result` — je to warning, nie error, ale moderné codebase s `#![deny(warnings)]` to zachytí.

### Chyba 3: Príliš granulárne error typy

```rust
// Zbytočne komplexné
#[derive(Debug, Error)]
enum DatabaseError {
    ConnectionFailed,
    QueryFailed,
    ParseFailed,
    SerializationFailed,
    DeserializationFailed,
    // ... 20 ďalších variantov
}

// Jednoduchšie a rovnako použiteľné
#[derive(Debug, Error)]
enum DatabaseError {
    #[error("pripojenie k DB zlyhalo: {0}")]
    Connection(#[source] std::io::Error),

    #[error("databázová operácia zlyhala: {message}")]
    Operation { message: String, #[source] source: Option<Box<dyn std::error::Error + Send + Sync>> },
}
```

Príliš granulárne error typy vedú k tomu, že caller musí ošetriť 20 vetiev kde mu stačí vedieť "DB je nedostupná" alebo "vstupné dáta sú zlé".

### Chyba 4: Strata kontextu pri konverzii

```rust
// Stratíme informáciu o tom AKÝ súbor nefungoval
fn process_files(paths: &[&str]) -> Result<Vec<String>, std::io::Error> {
    paths.iter()
        .map(|path| std::fs::read_to_string(path))  // ?
        .collect()
}

// Lepšie — zachováme kontext
fn process_files_ctx(paths: &[&str]) -> Result<Vec<String>> {
    paths.iter()
        .map(|path| {
            std::fs::read_to_string(path)
                .with_context(|| format!("čítanie súboru '{}'", path))
        })
        .collect()
}
```

Kontext je extrémne cenný pri debugging-u. "No such file or directory" bez názvu súboru je bezcenný. S názvom súboru je to okamžite actionable.

---

## Error handling v async kóde

V async kontexte funguje `?` rovnako — `async fn` môže vrátiť `Result` a `?` funguje vnútri:

```rust
use anyhow::Result;

async fn fetch_and_process(url: &str) -> Result<String> {
    let response = reqwest::get(url)
        .await
        .with_context(|| format!("GET {} zlyhalo", url))?;

    let status = response.status();
    if !status.is_success() {
        anyhow::bail!("server vrátil {}", status);
    }

    let text = response.text()
        .await
        .context("čítanie response body zlyhalo")?;

    Ok(text)
}
```

Async nezavádza žiadnu komplikáciu pre error handling — `?` funguje rovnako, `anyhow` funguje rovnako. Jediný rozdiel je `.await` pred `?`.

---

## Kedy čo použiť

| Situácia | Riešenie |
|---|---|
| Bug, nevalidný stav (nikdy by sa to nemalo stať) | `panic!` / `unreachable!()` |
| Invariant porušený na vstupe funkcie | `panic!` s popisom alebo `assert!` |
| Knižnica, vlastné chyby | `thiserror` s derive macro |
| Aplikácia / bin crate | `anyhow::Result` |
| Propagácia chyby | `?` operátor |
| Konverzia error typov automatická | `From` trait / `#[from]` |
| Kontext k chybe | `.context()` / `.with_context()` |
| Podmienená chyba | `ensure!` (anyhow) |
| Okamžitý return s chybou | `bail!` (anyhow) |
| Test kde chyba = test failure | `.unwrap()` / `.expect()` |
| Zámerné ignorovanie Result | `let _ = ...` |

Pár slov k filozofii: Rust ťa núti myslieť o chybách pri písaní kódu, nie pri debugovaní produkcie. Áno, je to viac práce vopred. Ale jeden raz keď zachytíš potenciálny výpadok v compile time namiesto toho aby si sa budil o polnoci kvôli segfaultu — to stojí za to.

Ďalšia kapitola: Closures & Iterators — funkcionálny štýl v systémovom jazyku.
