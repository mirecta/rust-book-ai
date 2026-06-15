# Kapitola 6 — Lifetimes

Lifetimes sú pravdepodobne tá vec, pre ktorú najviac ľudí prvýkrát zatvára terminál s Rustom a vráti sa ku C++. "Prečo mi kompilátor dáva prednášky o životnosti referencií? Veď ja viem, čo robím." A práve to je problém — keď si myslíš, že vieš čo robíš, ale borrow checker vie, že nevieš. Nie preto, že si hlúpy. Preto, že dangling pointer je zo svojej podstaty nenápadný — program funguje tisíckrát za sebou a raz ho nechceš a segfaultne v produkácii o tri mesiace.

Lifetimes sú systém, ktorým Rust sleduje *ako dlho žije referencia*. Nie sú to runtime koncepty — sú to anotácie pre borrow checker, ktorý ich verifikuje v compile time. Inými slovami, platíš cenu za bezpečnosť v čase kompilácie, nie za behu programu. V C++ platíš za runtime — buď segfaultom, alebo tým, že napíšeš vlastný garbage collector v podobe shared_ptr-ov a dúfaš, že nikde nie je cyklická závislosť.

Väčšinou ich nemusíš písať — Rust má pravidlá "lifetime elision" ktoré lifetimes automaticky odvádzajú. Ale keď ich musíš napísať, musíš im rozumieť. A keď im rozumieš, zistíš, že to nie sú obmedzenia — sú to dokumentácia.

---

## Prečo lifetimes existujú — reálny problém v C

Predtým než skočíme do syntaxe, pozrime sa čo lifetimes zabraňujú. Toto je klasická C chyba, takzvaný "use after free" alebo "dangling pointer":

```c
// C: klasická chyba, kompilátor mlčí
const char* get_greeting(int formal) {
    char buffer[64];
    if (formal) {
        snprintf(buffer, sizeof(buffer), "Dobrý deň");
    } else {
        snprintf(buffer, sizeof(buffer), "Ahoj");
    }
    return buffer;  // ← vraciaš pointer na stack, ktorý zanikne!
}

int main() {
    const char* g = get_greeting(1);
    printf("%s\n", g);  // undefined behavior — buffer neexistuje
    return 0;
}
```

GCC s `-Wall` to síce upozorní, ale stovky reálnych bugov sú subtílnejšie — napríklad keď pointer do heap-allocated pamäti prežije objekt, ktorý ju vlastnil. V C++ s `std::string_view` alebo `std::span` je to ešte zákernejšie:

```cpp
// C++: string_view nedrží vlastníctvo — ľahko vznikne dangling reference
std::string_view get_name() {
    std::string temp = "Miroslav";
    return temp;  // temp sa zničí, string_view zostane visieť
}

// Alebo o niečo zákernejšie:
class Config {
    std::string raw_data;
public:
    std::string_view get_host() const {
        return raw_data.substr(0, raw_data.find(':'));  // OOPS — substr vracia string, nie view
    }
};
```

Rust tieto scenáre eliminuje nie v runtime kontrolami, ale tým, že takýto kód jednoducho nezkompiluje. Borrow checker sleduje, kedy každá referencia vznikla a kedy zanikla, a ak výsledok funkcie môže prežiť vstup — odmietne to.

```rust
// Rust: kompilátor odmietne vrátiť referenciu na lokálnu hodnotu
fn get_greeting(formal: bool) -> &str {  // error: missing lifetime specifier
    let s = if formal { "Dobrý deň" } else { "Ahoj" };
    s  // kde žije tento &str? Na staku? V binárke?
}
```

Rust sa pýta práve túto otázku — kde žije táto referencia a prežije volajúceho? String literály (`"Dobrý deň"`) sú v binárke a teda majú `'static` lifetime, takže v tomto konkrétnom prípade by to fungovalo. Ale kompilátor musí byť explicitný.

---

## Prečo lifetime anotácia nestačí bez kontextu

Najklasickejší prípad kde musíš napísať lifetime explicitne:

```rust
// Borrow checker nemôže zistiť čo vrátiš bez ďalšej informácie:
fn longer(a: &str, b: &str) -> &str {  // error: missing lifetime specifier
    if a.len() > b.len() { a } else { b }
}
```

Kompilátor sa pýta: vrátená referencia žije tak dlho ako `a`, alebo ako `b`? Sú to dve rôzne životnosti a funkcia za behu rozhoduje, ktorú z nich vráti — ale borrow checker pracuje staticky, pred spustením. Musíš mu to povedať:

```rust
fn longer<'a>(a: &'a str, b: &'a str) -> &'a str {
    if a.len() > b.len() { a } else { b }
}
```

`'a` hovorí: "vrátená referencia žije nanajvýš tak dlho ako kratší z argumentov". Borrow checker to overí na každom call site. Neznamená to, že životnosť oboch argumentov je rovnaká — znamená to, že výsledok nesmie prežiť ani jeden z nich.

---

## Čo lifetime anotácia neznamená

Toto je kľúčový bod, ktorý mate začiatočníkov: `'a` *nezmení* ako dlho hodnota žije. Len *pomenuje* existujúci vzťah životností. Je to kontrakt, nie príkaz.

```rust
fn main() {
    let result;
    {
        let s1 = String::from("dlhý reťazec");
        let s2 = String::from("X");
        result = longer(&s1, &s2);  // result má lifetime s1 ∩ s2 = s2 (kratší)
        println!("{}", result);     // ok — s2 ešte žije
    }
    // println!("{}", result);  // error — s2 tu už neexistuje
}
```

Rust tu nerobí žiadnu mágiu. Nekopíruje string, nepredlžuje život `s2`. Jednoducho zakazuje použitie `result` po tom, čo `s2` zaniklo. Celé overenie prebieha pri kompilácii — žiadny overhead za behu.

Porovnaj to s C++, kde `std::shared_ptr` robí presne toto, ale za cenu atomických inkrementov referencií za behu, a kde stále môžeš dostať dangling reference cez `std::weak_ptr` ak zabudneš na `.lock()`:

```cpp
// C++: zdanlivo bezpečné, ale weak_ptr môže byť expired
std::weak_ptr<std::string> dangerous;
{
    auto s = std::make_shared<std::string>("hello");
    dangerous = s;
}  // s sa zničí tu
if (auto ptr = dangerous.lock()) {
    // Toto sa nevykoná — expired — ale runtime check je nutný
    std::cout << *ptr << std::endl;
}
```

Rust by takéto použitie odmietol pri kompilácii, nie za behu.

---

## Pod kapotou — čo borrow checker skutočne robí

Borrow checker pracuje na úrovni MIR (Mid-level Intermediate Representation) — medzireprezentácii medzi zdrojovým kódom a strojovým kódom. Pre každú premennú sleduje tzv. "Non-Lexical Lifetimes" (NLL), čo bol významný upgrade v Rust 2018 edícii.

Pred NLL boli lifetimes striktne lexikálne — referencia žila celý blok `{}` v ktorom bola deklarovaná. To viedlo k frustrujúcim false positives. Po NLL borrow checker sleduje skutočný tok dát (control flow graph) a lifetime referencie trvá len dovtedy, kým sa naposledy použije.

```rust
// Pred NLL (Rust 2015) by toto nefungovalo — s by žil celý blok
// Po NLL (Rust 2018+) kompilátor vidí, že s sa nepoužíva po riadku 3
fn main() {
    let mut data = vec![1, 2, 3];
    let s = &data[0];      // immutable borrow začína tu
    println!("{}", s);     // ... a končí tu (posledné použitie)
    data.push(4);          // mutable borrow — ok, s už neexistuje
}
```

Borrow checker tiež robí tzv. "variance analysis" — sleduje, či je lifetime parameter kovariantný, kontravariantný, alebo invariantný. Pre bežné použitie to riešiš zriedka, ale je to dôvod prečo `&'a str` a `&'b str` nie sú vždy zameniteľné.

---

## Lifetime elision — kedy ich nemusíš písať

Rust má pravidlá ktoré odvádzajú lifetimes automaticky. Väčšina funkcií ich nepotrebuje explicitne písať, čo je výborné — inak by bol Rust úplne nepoužiteľný. Tieto pravidlá sa volajú "lifetime elision rules" a sú tri:

Prvé pravidlo: každý referenčný parameter dostane vlastný anonymný lifetime. Druhé pravidlo: ak je presne jeden vstupný lifetime parameter, výstupný lifetime je totožný. Tretie pravidlo: ak je jeden z parametrov `&self` alebo `&mut self`, výstupný lifetime je lifetime `self`-a.

```rust
// Tieto sú ekvivalentné — elision aplikuje pravidlo 2:
fn first_word(s: &str) -> &str { &s[..s.find(' ').unwrap_or(s.len())] }
fn first_word<'a>(s: &'a str) -> &'a str { &s[..s.find(' ').unwrap_or(s.len())] }

// Pravidlo 3 — metóda na struct:
impl Parser<'_> {
    fn current_slice(&self) -> &str {  // implicitne: -> &'self str
        &self.input[self.pos..]
    }
}
```

Kedy nestačia: ak máš viac vstupných referencií a výstup závisí od viacerých z nich, musíš byť explicitný. Kompilátor odmietne hádať.

```rust
struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Parser { input, pos: 0 }
    }

    // Elision pravidlo 3: vrátená &str má lifetime 'a (zdedí od &self, ale &self má 'a)
    // Presnejšie: zdedí lifetime 'a zo self.input, cez pravidlo 3
    fn current_slice(&self) -> &str {
        &self.input[self.pos..]
    }

    fn advance(&mut self, n: usize) {
        self.pos = (self.pos + n).min(self.input.len());
    }

    // Tu elision nestačí — výsledok môže byť z input alebo z externého zdroja
    fn slice_or_default<'b>(&self, default: &'b str) -> &'b str
    where
        'a: 'b,  // 'a outlives 'b — garantujeme, že input prežije default
    {
        if self.pos < self.input.len() {
            &self.input[self.pos..]
        } else {
            default
        }
    }
}
```

---

## Lifetimes v štruktúrach

Ak struct drží referenciu, musí deklarovať lifetime. Toto je jeden z najdôležitejších patternov v systémovom programovaní — zero-copy parsing. Namiesto kopírovania dát zo vstupného bufferu len ukladáme pointery (slices) do neho.

```rust
// Struct požičiava dáta — nevlastní ich
struct ZeroCopyFrame<'a> {
    header: &'a [u8],
    payload: &'a [u8],
}

impl<'a> ZeroCopyFrame<'a> {
    fn parse(buf: &'a [u8]) -> Option<Self> {
        if buf.len() < 4 { return None; }
        let hdr_len = buf[1] as usize;
        if buf.len() < 4 + hdr_len { return None; }
        Some(ZeroCopyFrame {
            header: &buf[..4 + hdr_len],
            payload: &buf[4 + hdr_len..],
        })
    }

    fn payload_len(&self) -> usize {
        self.payload.len()
    }
}

fn main() {
    let raw = [0x01u8, 0x02, 0xFF, 0xFF, 0xAA, 0xBB, 0x11, 0x22, 0x33];
    if let Some(frame) = ZeroCopyFrame::parse(&raw) {
        println!("header: {:02X?}", frame.header);
        println!("payload: {:02X?}", frame.payload);
    }
}
```

`ZeroCopyFrame` nerobí žiadnu kópiu — len drží slice pointery do pôvodného buffera. Lifetime `'a` zaručuje, že `raw` prežije `frame`. Keď sa `raw` zničí, `frame` nesmie existovať. Borrow checker to overí.

V C++ by si to riešil pomocou `std::span<uint8_t>` (C++20) alebo raw pointerov, ale bez lifetime garantie — musíš sám zabezpečiť, že buffer prežije span. V Ruste ti to garantuje kompilátor.

### Chyba začiatočníkov: lifetime v struct-e a výpočet

Bežná chyba je keď chceš do struct-u uložiť výsledok výpočtu nad zapožičanými dátami:

```rust
// NEFUNGUJE — Rust nevie vrátiť referenciu na dáta vytvorené vo funkcii
struct BadParser<'a> {
    tokens: Vec<&'a str>,  // ok, ak tokeny ukazujú na vstupný buffer
    current: &'a str,      // problém ak current je substring vytvorený parsovaním
}

impl<'a> BadParser<'a> {
    fn compute_token(&self) -> String {  // vracia owned String — žiadny problém
        self.tokens.join(", ")
    }

    // Toto nefunguje — lokálna String zanikne keď funkcia skončí
    // fn compute_token_ref(&self) -> &'a str {
    //     let s = self.tokens.join(", ");
    //     &s  // error: s zanikne na konci funkcie
    // }
}
```

Riešenie: ak potrebuješ vlastniť vypočítané dáta, drž `String`, nie `&str`. Zero-copy sa vzťahuje na vstup, nie na každý medzivýsledok.

---

## `'static` lifetime

`'static` znamená "žije celý program". Je to špeciálny lifetime, nie keyword, hoci vyzerá ako jeden. Existujú dva spôsoby ako ho získať:

Prvý: dáta sú uložené v binárke (string literály, statické premenné). Tieto existujú od štartu programu po jeho koniec a nikdy sa neuvoľnia.

Druhý: `Box::leak` — zámerný memory leak, kde odovzdáš ownership do heap-u a vzdáš sa možnosti uvoľniť pamäť. Rust to celý program bude považovať za živé.

```rust
// String literály sú 'static — uložené v binárke (.rodata sekcia)
let s: &'static str = "hello world";

// Statické premenné — žijú celý program
static CONFIG: &str = "production";
static MAX_CONNECTIONS: u32 = 1024;

// Box::leak — uvoľni Box, dostaneš 'static referenciu (zámerný memory leak)
// Typicky sa používa pre globálne inicializované konfigurácie
let leaked: &'static str = Box::leak(String::from("dynamic but static").into_boxed_str());

// Lazy statická inicializácia (cez once_cell alebo std::sync::OnceLock):
use std::sync::OnceLock;
static GLOBAL_CONFIG: OnceLock<String> = OnceLock::new();

fn get_config() -> &'static str {
    GLOBAL_CONFIG.get_or_init(|| {
        std::env::var("APP_CONFIG").unwrap_or_else(|_| "default".to_string())
    })
}
```

### `'static` a threads — prečo spawn vyžaduje `'static`

Toto je miesto kde začiatočníci narazí na `'static` ako requirement, nie len ako popis:

```rust
// std::thread::spawn má signatúru:
// fn spawn<F, T>(f: F) -> JoinHandle<T>
// where F: FnOnce() -> T + Send + 'static

// Prečo 'static? Lebo nový thread môže prežiť caller frame.
// Rust nemôže staticky vedieť kedy thread skončí — musí garantovať,
// že closure neobsahuje referencie na stack caller-a.

fn bad_example() {
    let data = vec![1, 2, 3];
    // std::thread::spawn(|| println!("{:?}", data));
    // error: `data` does not live long enough — data je na stack-u tejto funkcie
    // thread môže bežať po tom, čo bad_example() skončí
}

// Riešenie 1: move — prenesie ownership do closure
fn good_with_move() {
    let data = vec![1, 2, 3];
    std::thread::spawn(move || {
        println!("{:?}", data);  // data sa presunulo do closure
    }).join().unwrap();
    // data tu už neexistuje
}

// Riešenie 2: Arc — shared ownership pre thread-safe zdieľanie
use std::sync::Arc;

fn start_worker(data: Arc<Vec<u8>>) {
    let data_clone = Arc::clone(&data);
    std::thread::spawn(move || {
        println!("worker: {} bajtov", data_clone.len());
        // data_clone sa zničí keď thread skončí (refcount --)
    });
    // data stále funguje v caller-ovi (refcount je stále >= 1)
}
```

`Arc` je thread-safe referenčný počítač (Atomic Reference Count). Každý `clone` inkrementuje počítač atomicky, každý `drop` dekrementuje. Keď dosiahne nulu, pamäť sa uvoľní. Je to ako `std::shared_ptr` v C++, ale bez možnosti dangling referencie — a bez cyklických závislostí (na to existuje `Arc<Mutex<Weak<...>>>`).

---

## Lifetime bounds — `'a: 'b`

Niekedy potrebuješ povedať, že jeden lifetime musí prežiť druhý. Syntax je `'a: 'b` čo sa číta "lifetime 'a outlives lifetime 'b" alebo "`'a` je dlhší ako `'b`":

```rust
// Garantujeme, že referencia žije aspoň tak dlho ako 'b
fn longest_with_announcement<'a, 'b>(
    x: &'a str,
    y: &'a str,
    ann: &'b str,
) -> &'a str
where
    'a: 'b,  // x a y musia prežiť ann
{
    println!("Announcement: {}", ann);
    if x.len() > y.len() { x } else { y }
}
```

V praxi to narazíš hlavne keď pracuješ s viacúrovňovými štruktúrami alebo keď callback drží referenciu na vonkajší stav.

---

## Praktický príklad: zero-copy HTTP parser (hlavičky)

Toto je ukážka kde lifetimes skutočne žiaria — HTTP parser, ktorý nenaalokuje ani bajt navyše. Každý `&str` a `&[u8]` je len window (okno) do pôvodného vstupného buffera:

```rust
#[derive(Debug)]
struct HttpRequest<'a> {
    method: &'a str,
    path: &'a str,
    version: &'a str,
    headers: Vec<(&'a str, &'a str)>,
    body: &'a [u8],
}

fn parse_http<'a>(raw: &'a [u8]) -> Option<HttpRequest<'a>> {
    let text = std::str::from_utf8(raw).ok()?;
    let mut lines = text.split("\r\n");

    // Request line: "GET /path HTTP/1.1"
    let request_line = lines.next()?;
    let mut parts = request_line.split(' ');
    let method = parts.next()?;
    let path = parts.next()?;
    let version = parts.next().unwrap_or("HTTP/1.0");

    // Hlavičky kým nenarazíme na prázdny riadok
    let mut headers = Vec::new();
    let mut body_start = 0;
    let mut pos = request_line.len() + 2; // +2 pre \r\n

    for line in lines.by_ref() {
        if line.is_empty() {
            body_start = pos + 2;
            break;
        }
        if let Some(colon) = line.find(':') {
            let name = line[..colon].trim();
            let value = line[colon + 1..].trim();
            headers.push((name, value));
        }
        pos += line.len() + 2;
    }

    Some(HttpRequest {
        method,
        path,
        version,
        headers,
        body: if body_start < raw.len() { &raw[body_start..] } else { b"" },
    })
}

fn main() {
    let raw = b"GET /api/status HTTP/1.1\r\nHost: example.com\r\nContent-Length: 0\r\nUser-Agent: rust-client/1.0\r\n\r\n";

    if let Some(req) = parse_http(raw) {
        println!("Method:  {}", req.method);
        println!("Path:    {}", req.path);
        println!("Version: {}", req.version);
        println!("Headers:");
        for (name, value) in &req.headers {
            println!("  {}: {}", name, value);
        }
        println!("Body: {} bajtov", req.body.len());
    }
}
```

Žiadna alokácia v parseri — `Vec<(&str, &str)>` síce alokuje (pre vector samotný), ale všetky `&str` sú len pointery do `raw`. Lifetime `'a` zaručuje, že `HttpRequest` neprežije `raw`. Ak by si skúsil vrátiť `req` a zahodiť `raw`, kompilátor by odmietol skompilovať.

V produkčnom HTTP parseri (napr. `httparse`) je tento vzor štandardný. Výsledkom je parser, ktorý spracuje desaťtisíce requestov za sekundu bez alokácie.

---

## Časté chyby a ich riešenia

### Chyba 1: Vrátenie referencie na lokálnu hodnotu

```rust
// error: cannot return reference to local variable `result`
fn compute() -> &str {
    let result = String::from("hello");
    &result  // result zanikne keď funkcia skončí
}

// Riešenie A: vráť owned typ
fn compute_ok() -> String {
    String::from("hello")
}

// Riešenie B: static data
fn compute_static() -> &'static str {
    "hello"  // string literál žije celý program
}
```

### Chyba 2: Borrow trvá príliš dlho

```rust
// error: cannot borrow `map` as mutable because it is also borrowed as immutable
use std::collections::HashMap;

fn process(map: &mut HashMap<String, String>) {
    let key = "foo";
    let value = map.get(key);  // immutable borrow začína tu
    if value.is_none() {
        map.insert(key.to_string(), "bar".to_string());  // mutable borrow — conflict!
    }
}

// Riešenie: ukončiť borrow pred mutable operáciou
fn process_ok(map: &mut HashMap<String, String>) {
    let key = "foo";
    let exists = map.contains_key(key);  // borrow začne a hneď skončí
    if !exists {
        map.insert(key.to_string(), "bar".to_string());  // ok
    }

    // Alebo entry API:
    map.entry(key.to_string()).or_insert_with(|| "bar".to_string());
}
```

### Chyba 3: Lifetime v struct-e — zabudnutá anotácia

```rust
// error: missing lifetime specifier
struct Config {
    name: &str,  // Rust nevie odvodiť lifetime
}

// Riešenie A: owned String
struct ConfigOwned {
    name: String,  // vlastní dáta
}

// Riešenie B: explicit lifetime
struct ConfigBorrowed<'a> {
    name: &'a str,  // požičiava dáta
}

// Kedy použiť ktoré? Jednoduchá heuristika:
// - struct je dlhodobý (server config, global state) → owned
// - struct je krátkodobý view do buffera (parsovanie, serializácia) → borrowed
```

### Chyba 4: Zmätenie medzi 'static bound a 'static lifetime

```rust
// T: 'static neznamená že T musí byť 'static literal
// Znamená že T neobsahuje žiadne referencie kratšie ako 'static
fn store<T: 'static>(val: T) {
    // val môžeme bezpečne uložiť kamkoľvek — neobsahuje krátkodobé referencie
    Box::new(val);  // ok
}

store(String::from("hello"));  // String neobsahuje referencie → splňuje T: 'static
store(42u32);                   // u32 neobsahuje referencie → ok
// store(&local_var);           // &T obsahuje referenciu → nesplňuje T: 'static
```

---

## Reálne use-case zo systémového programovania

V systémovom programovaní narážaš na lifetimes v niekoľkých kľúčových scenároch. Prvý je zero-copy networking — keď čítaš dáta do bufferu a parsovanie prebieha bez kopírovania. Druhý je embedded systémy, kde heap allocácia môže byť zakázaná a všetko musí byť na stacku alebo v staticke. Tretí sú arena alokátory — všetky alokácie majú rovnaký lifetime (arenu) a uvoľňujú sa naraz.

```rust
// Arena pattern — všetky alokácie z jednej arény majú rovnaký lifetime
// Typické v kompilátoroch, parsovačoch, game engine-och

struct Arena {
    data: Vec<u8>,
    pos: usize,
}

impl Arena {
    fn new(capacity: usize) -> Self {
        Arena { data: vec![0u8; capacity], pos: 0 }
    }

    fn alloc<'a>(&'a mut self, size: usize) -> Option<&'a mut [u8]> {
        if self.pos + size > self.data.len() {
            return None;
        }
        let start = self.pos;
        self.pos += size;
        Some(&mut self.data[start..self.pos])
    }
}

fn main() {
    let mut arena = Arena::new(1024);

    let buf1 = arena.alloc(64).unwrap();
    buf1[0] = 42;

    let buf2 = arena.alloc(128).unwrap();
    buf2[0] = 99;

    // Obe alokácie žijú tak dlho ako arena
    // Keď arena zanikne, zanikne aj buf1 aj buf2 — žiadny individuálny free()
    println!("buf1[0]={}, buf2[0]={}", buf1[0], buf2[0]);
}
```

---

## Zhrnutie

```
'a          — pomenovaný lifetime parameter (môžeš ho nazvať čokoľvek: 'input, 'buf, 'req)
'static     — žije celú dobu behu programu (binárka alebo Box::leak)
elision     — Rust väčšinou odvodí lifetimes sám (3 pravidlá)
Štruktúry   — musia deklarovať lifetime ak drží referencie
'a: 'b      — 'a musí prežiť 'b (lifetime bound)
T: 'static  — T neobsahuje krátkodobé referencie (nie nutne literal)
```

Lifetimes sú komplikované len na prvý pohľad. Pravidlo: ak kompilátor žiada lifetime anotáciu, pýta sa ťa "čí je tento borrow?" alebo "ako dlho musí toto prežiť?". Odpovedz mu tým správnym menom. Keď si zvykneš, zistíš, že lifetime anotácie sú vlastne dokumentácia — hovoria budúcemu čitateľovi kódu presne ktoré dáta musia byť živé kedy.

A ak stále bojuješ s konkrétnym error-om — skús si ho prečítať pozorne. Rust compiler error správy pre lifetimes patria k najlepším v akomkoľvek jazyku. Zvyčajne ti priamo povie čo prežíva čo a kde konflikt nastáva.

Ďalšia kapitola: Error Handling — `Result`, `?`, `thiserror` a `anyhow`.

---

## Vizuálny príklad — Lifetime Scope Visualizer

    cargo run --bin k06_lifetimes

Lifetimes sú možno najťažší koncept v Ruste pre pochopenie z textu. Vizualizácia pomáha — uvidíš ich ako farebné horizontálne pruhy reprezentujúce *ako dlho žijú* rôzne referencie.

Demo má 4 scenáre (`SPACE` alebo klávesy `1`-`4`):

1. **Validný borrow** — tri pruhy (`'a`, `x`, `result`) v rovnakej oblasti; `result` je kratší ako `'a` → bezpečné
2. **Dangling reference** — `result` presahuje za koniec `x` → červená čiara, X symbol, borrow checker by to odmietol
3. **Struct s lifetime** — `struct Important<'a>` s šípkou ukazujúcou na dáta; struct nesmie prežiť referencovanú hodnotu
4. **'static** — lifetime ktorý trvá počas celého programu; string literals sú vždy `'static`

Trik na pochopenie: lifetime nie je *dĺžka trvania*, ale *región kódu* kde referencia musí byť validná. Vizuálne pruhy to ilustrujú lepšie než akákoľvek definícia.

Ovládanie: `SPACE`/`1-4` = scenár, `Q` = koniec.
