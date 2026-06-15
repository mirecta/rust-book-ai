# Kapitola 4 — Pattern Matching

Na predchádzajúcej kapitole sme si ukázali, že Rust enum nie je C enum — je to tagged union, ktorý môže niesť rôzne dáta pre rôzne varianty. Ale výber správneho variantu a extrakcia dát si vyžaduje mechanizmus. V Ruste je tým mechanizmom `match`.

`match` v Ruste je `switch` na steroidoch. Kombinuje destrukturovanie, guards, exhaustiveness checking a priradenie premenných — všetko naraz. Ale predtým, než sa ponoríme do syntaxe, stojí za to pochopiť, *prečo* taký mechanizmus vôbec potrebujeme.

V C, keď pracuješ s tagged union, si odsúdený na sériu `if (type == X) { ... } else if (type == Y) { ... }`. Ak zabudneš na jeden prípad, kompilátor mlčí. Ak pridáš nový variant, musíš ručne prehľadať celý kódovú základňu. Toto neškáluje. Pattern matching je riešenie, ktoré jazyk integruje priamo — nie ako knižnica, nie ako konvencia, ale ako jazyková konštrukcia s kompilátorovou podporou.

---

## Základné `match`

Začneme jednoduchým príkladom, ktorý ukazuje hlavné rozdiely od C `switch`:

```rust
fn http_status(code: u16) -> &'static str {
    match code {
        200 => "OK",
        301 | 302 => "Redirect",     // OR vzor
        400 => "Bad Request",
        404 => "Not Found",
        500..=599 => "Server Error", // range vzor
        _ => "Unknown",              // wildcard — povinný ak nie sú všetky prípady
    }
}

fn main() {
    println!("{}", http_status(200));  // OK
    println!("{}", http_status(503));  // Server Error
    println!("{}", http_status(999));  // Unknown
}
```

Tri veci tu sú okamžite iné než C switch. Po prvé, range vzor `500..=599` pokryje celý rozsah naraz — v C by si musel písať `case 500: case 501: ... case 599:` alebo použiť `if` mimo switchu. Po druhé, `|` kombinuje vzory do jednej vetvy. Po tretie — a toto je kľúčové — `_` wildcard je vyžadovaný ak nepokryješ všetky možné hodnoty. Kompilátor *overí*, či je match exhaustive.

Pozri, čo sa stane v C keď zabudneš prípad:

```c
// C — tiché zlyhanie
switch (code) {
    case 200: return "OK";
    case 404: return "Not Found";
    // 500? žiadny problém, padne do default... alebo aj nie
}
// Bez default: undefined behavior ak nenajde zhodu
// S default: zabudnuté prípady ticho ignorované
```

### Exhaustiveness checking

Toto je kľúčová vlastnosť. Ak pridáš nový variant do enumu, kompilátor ťa *núti* ošetriť ho všade kde robíš `match`:

```rust
enum State {
    Idle,
    Running,
    Error(String),
    // Paused,   // ← ak odkomentujeme, každý match sa nestane kompilátorným
}

fn describe(s: &State) -> &str {
    match s {
        State::Idle => "nečinný",
        State::Running => "beží",
        State::Error(_) => "chyba",
        // Zabudnúť na Paused? error[E0004]: non-exhaustive patterns
    }
}
```

V C `switch` zabudnutý `case` = tichý bug. V Ruste = compile error. Predstav si, že pracuješ na veľkom projekte s 50 miestami kde sa spracúva `State`. Pridáš `Paused` variant a Rust ti povie každé jedno miesto, kde treba aktualizovať kód. V C by si sa spoliehal na grepping a nádej.

### Pod kapotou: ako kompilátor spracúva match

Rust match kompilátor netransformuje naivne na sériu `if-else` inštrukcií. Pre jednoduché hodnoty (integers, enums bez dát) generuje jump table — rovnaký mechanizmus ako C switch s hustými hodnotami. Pre sparse ranges môže generovať binárne vyhľadávanie. Pre enums s dátami generuje kód, ktorý najprv skontroluje tag a potom pristúpi k správnym dátam.

Môžeš to overiť na [Compiler Explorer](https://godbolt.org/) — Rust match na jednoduchom enume generuje rovnako efektívny assembly ako C switch. Žiadny runtime overhead za bezpečnosť.

---

## Destrukturovanie

Toto je miesto, kde match začína byť skutočne výkonný nástroj. Destrukturovanie znamená, že v jednom match výraze môžeš súčasne rozpoznať vzor *a* extrahovať dáta z neho.

### Enum s dátami

```rust
#[derive(Debug)]
enum Packet {
    Ping { seq: u32 },
    Data { seq: u32, payload: Vec<u8> },
    Ack(u32),
    Reset,
}

fn handle(pkt: &Packet) {
    match pkt {
        Packet::Ping { seq } => {
            println!("PING seq={}", seq);
        }
        Packet::Data { seq, payload } => {
            println!("DATA seq={} len={}", seq, payload.len());
        }
        Packet::Ack(seq) => println!("ACK {}", seq),
        Packet::Reset => println!("RESET"),
    }
}
```

Všimni si, že v `Packet::Data { seq, payload }` sa automaticky extrahujú obe polia. Nie je potrebné `pkt->seq` alebo `pkt->payload` — v tele vetvy sú `seq` a `payload` priamo ako premenné. Ak potrebuješ len jedno pole, môžeš druhé ignorovať s `..`:

```rust
Packet::Data { seq, .. } => println!("DATA seq={}", seq),
```

Toto je veľmi odlišné od C prístupu, kde by si musel explicitne extrahovať každé pole manuálne po overení tagu.

### Struct destrukturovanie

```rust
struct Point { x: f64, y: f64 }

fn classify(p: &Point) {
    match p {
        Point { x: 0.0, y: 0.0 } => println!("origin"),
        Point { x, y: 0.0 } => println!("na osi X: {}", x),
        Point { x: 0.0, y } => println!("na osi Y: {}", y),
        Point { x, y } => println!("({}, {})", x, y),
    }
}
```

Vzory sa vyhodnocujú zhora nadol. Ak `p.x == 0.0` a `p.y == 0.0`, zodpovedá prvý vzor. Ak len `p.y == 0.0`, zodpovedá druhý. Match zaručí, že zodpovedá práve jeden vzor — presne ten prvý, ktorý pasuje.

Pozor: porovnávanie floatov v patternoch funguje, ale má rovnaké problémy ako porovnávanie floatov všade — floating-point aritmetika môže dávať `0.000000001` namiesto `0.0`. Pre reálne aplikácie zvyčajne použiješ guard s `(x).abs() < EPSILON`.

### Tuple destrukturovanie

```rust
fn tcp_state(state: (bool, bool)) -> &'static str {
    // (SYN, ACK)
    match state {
        (true, false) => "SYN",
        (false, true) => "ACK",
        (true, true)  => "SYN-ACK",
        (false, false) => "---",
    }
}
```

Tuple destrukturovanie je elegantný spôsob matchovania kombinácií booleanov bez vnorených `if-else` konštrukcií. V C by si toto napísal ako bitové masky alebo vnorené podmienky.

### Slice destrukturovanie

Toto je zvlášť mocné pri parsovaní binárnych protokolov:

```rust
fn parse_header(buf: &[u8]) -> Option<(u8, u8, u16)> {
    match buf {
        [ver, flags, len_hi, len_lo, ..] => {
            Some((*ver, *flags, u16::from_be_bytes([*len_hi, *len_lo])))
        }
        _ => None,
    }
}

fn main() {
    let buf = [0x01u8, 0x00, 0x00, 0x14, 0xDE, 0xAD];
    if let Some((ver, flags, len)) = parse_header(&buf) {
        println!("ver={} flags={:#04X} len={}", ver, flags, len);
    }
}
```

`[ver, flags, len_hi, len_lo, ..]` hovorí: "tento slice má aspoň 4 bajty, prvé štyri mi daj ako `ver`, `flags`, `len_hi`, `len_lo`, a zvyšok ignoruj". Je to čitateľnejšie ako `if (len >= 4) { ver = buf[0]; flags = buf[1]; ... }`.

Môžeš matchovať aj konkrétne hodnoty v slice:

```rust
fn identify_protocol(buf: &[u8]) -> &'static str {
    match buf {
        [0x47, 0x45, 0x54, ..] => "HTTP GET",    // "GET"
        [0x50, 0x4F, 0x53, 0x54, ..] => "HTTP POST", // "POST"
        [0xFF, 0xD8, 0xFF, ..] => "JPEG",
        [0x89, b'P', b'N', b'G', ..] => "PNG",
        _ => "unknown",
    }
}
```

V C by si toto robil s `memcmp()` a sériu `if` blokov.

---

## Guards

Podmienky v `match` vetvách — extra filter za vzorcom:

```rust
fn classify_port(port: u16) -> &'static str {
    match port {
        0 => "rezervovaný",
        p if p < 1024 => "privilegovaný (well-known)",
        p if p < 49152 => "registrovaný",
        _ => "dynamický/ephemeral",
    }
}

fn route_packet(src: u32, dst: u32, proto: u8) {
    match (src, dst, proto) {
        (_, _, 6) if dst == 80 || dst == 443 => println!("HTTP/S traffic"),
        (_, _, 17) => println!("UDP"),
        (s, d, p) if s == d => println!("loopback? src==dst proto={}", p),
        _ => println!("iné"),
    }
}
```

Guard (`if podmienka` za vzorcom) je vyhodnotený len ak vzor pasuje. Kombinácia vzoru a guardu je mocná — môžeš matchovať na štruktúru dát a zároveň na hodnoty polí. Toto je niečo, čo v C `switch` vôbec neexistuje.

Dôležitá poznámka: guard sa vzťahuje len na konkrétnu vetvu, nie na celý vzor. Ak guard nevyhodí `true`, match pokračuje na ďalšiu vetvu — nezastaví sa.

### Kombinovanie viacerých vzorov s guardmi

```rust
#[derive(Debug)]
enum Event {
    KeyPress(u8),
    MouseClick { x: u32, y: u32, button: u8 },
    Resize { width: u32, height: u32 },
}

fn handle_event(event: &Event) {
    match event {
        Event::KeyPress(k) if *k == b'q' || *k == 27 => {
            println!("ukončujem aplikáciu");
        }
        Event::KeyPress(k) if k.is_ascii_alphabetic() => {
            println!("písmeno: {}", *k as char);
        }
        Event::KeyPress(k) => {
            println!("iná klávesa: {:#04X}", k);
        }
        Event::MouseClick { x, y, button: 1 } => {
            println!("ľavý klik na ({}, {})", x, y);
        }
        Event::MouseClick { x, y, button } => {
            println!("klik {} na ({}, {})", button, x, y);
        }
        Event::Resize { width, height } if width * height > 4_000_000 => {
            println!("4K+ rozlíšenie: {}x{}", width, height);
        }
        Event::Resize { width, height } => {
            println!("rozlíšenie: {}x{}", width, height);
        }
    }
}
```

---

## `if let` a `while let`

`match` je mocný, ale niekedy je príliš obšírny keď ťa zaujíma len jeden variant. Na to slúži `if let`:

```rust
fn process(value: Option<u32>) {
    // Miesto match s jednou vetvou:
    if let Some(v) = value {
        println!("hodnota: {}", v);
    }

    // S else:
    if let Some(v) = value {
        println!("ok: {}", v);
    } else {
        println!("žiadna hodnota");
    }
}

// while let — spracovávaj kým má hodnotu
fn drain_queue(queue: &mut Vec<u32>) {
    while let Some(item) = queue.pop() {
        println!("spracovávam: {}", item);
    }
}
```

`if let` je syntaktický cukríček — v praxi je to match s jedným vzorcom a ignorovaním ostatných. Kompilátor to aj tak skompiluje rovnako. Výhoda je čitateľnosť.

`while let` je obzvlášť užitočné pri spracovaní dát z fronty alebo streamu — pokračuj pokiaľ dostávaš hodnoty, zastavuj keď dostaneš `None`.

### `let else` — guard pattern

```rust
fn process_packet(buf: &[u8]) -> Result<(), &'static str> {
    let [ver, flags, rest @ ..] = buf else {
        return Err("buffer príliš krátky");
    };
    // ver, flags, rest sú dostupné tu
    println!("ver={} flags={}", ver, flags);
    Ok(())
}
```

`let else` je novší pattern (stabilný od Rust 1.65). Hovorí: "prirad tieto premenné z tohto vzoru, alebo ak vzor nepasuje, spusti tento blok". Blok musí divergovať — `return`, `panic!`, `break`, alebo `continue`. Výsledok je, že premenné sú dostupné v pokračujúcom kóde bez zanorenia — tzv. "early return" pattern.

Bez `let else` by si musel písať:

```rust
fn process_packet_old(buf: &[u8]) -> Result<(), &'static str> {
    if buf.len() < 2 {
        return Err("buffer príliš krátky");
    }
    let ver = buf[0];
    let flags = buf[1];
    let rest = &buf[2..];
    // ďalší kód...
    Ok(())
}
```

`let else` verzia je kompaktnejšia a priamo dokumentuje "čo z buffra očakávam".

---

## Binding s `@`

Priradenie matchnutej hodnoty do premennej pri zachovaní testovania vzoru:

```rust
fn validate_port(port: u16) {
    match port {
        p @ 1..=1023 => println!("privilegovaný port {}", p),
        p @ 1024..=65535 => println!("neprivilegovaný port {}", p),
        0 => println!("port 0 — invalid"),
    }
}
```

`p @ 1..=1023` hovorí: "overí, či je hodnota v tomto rozsahu, a ak áno, prirad ju do premennej `p`". Bez `@` by si musel použiť guard:

```rust
match port {
    p if (1..=1023).contains(&p) => println!("privilegovaný port {}", p),
    // ...
}
```

`@` binding je úspornejší pre jednoduché rozsahy. Pre komplexnejšie podmienky sú guardy stále potrebné.

### Nested binding

```rust
#[derive(Debug)]
enum Message {
    Move { x: i32, y: i32 },
}

fn handle(msg: &Message) {
    match msg {
        // Prirad celú štruktúru do 'm' aj destrukturuj polia
        m @ Message::Move { x, y } if *x > 0 && *y > 0 => {
            println!("pohyb do prvého kvadrantu: {:?}", m);
        }
        Message::Move { x, y } => {
            println!("pohyb na ({}, {})", x, y);
        }
    }
}
```

---

## Praktický príklad: Stavový automat (FSM) pre TCP

Toto je miesto kde pattern matching naozaj svieti. Stavové automaty sú všadeprítomné v sieťovom kóde, protokoloch, parsersoch, hernej logike. Implementácia FSM s match je omnoho čistejšia než s `if-else if` reťazcom.

```rust
#[derive(Debug, PartialEq, Clone)]
enum TcpState {
    Closed,
    Listen,
    SynReceived,
    Established,
    FinWait1,
    TimeWait,
}

#[derive(Debug)]
enum TcpEvent {
    PassiveOpen,
    SynReceived,
    AckReceived,
    FinReceived,
    Timeout,
    Close,
}

fn tcp_transition(state: TcpState, event: TcpEvent) -> TcpState {
    match (&state, &event) {
        (TcpState::Closed, TcpEvent::PassiveOpen) => {
            println!("→ LISTEN");
            TcpState::Listen
        }
        (TcpState::Listen, TcpEvent::SynReceived) => {
            println!("→ SYN_RECEIVED (posielam SYN-ACK)");
            TcpState::SynReceived
        }
        (TcpState::SynReceived, TcpEvent::AckReceived) => {
            println!("→ ESTABLISHED");
            TcpState::Established
        }
        (TcpState::Established, TcpEvent::FinReceived) => {
            println!("→ FIN_WAIT_1 (posielam FIN-ACK)");
            TcpState::FinWait1
        }
        (TcpState::FinWait1, TcpEvent::AckReceived) => {
            println!("→ TIME_WAIT");
            TcpState::TimeWait
        }
        (TcpState::TimeWait, TcpEvent::Timeout) => {
            println!("→ CLOSED");
            TcpState::Closed
        }
        (s, e) => {
            println!("neplatný prechod: {:?} + {:?}", s, e);
            state.clone()
        }
    }
}

fn main() {
    let events = vec![
        TcpEvent::PassiveOpen,
        TcpEvent::SynReceived,
        TcpEvent::AckReceived,
        TcpEvent::FinReceived,
        TcpEvent::AckReceived,
        TcpEvent::Timeout,
    ];

    let mut state = TcpState::Closed;
    println!("štart: {:?}", state);

    for event in events {
        state = tcp_transition(state, event);
    }

    println!("koniec: {:?}", state);
}
```

Výstup:
```
štart: Closed
→ LISTEN
→ SYN_RECEIVED (posielam SYN-ACK)
→ ESTABLISHED
→ FIN_WAIT_1 (posielam FIN-ACK)
→ TIME_WAIT
→ CLOSED
koniec: Closed
```

FSM bez `if-else if` reťazca, bez `switch` s `default`, bez zabudnutých prechodov. Kompilátor zaručí exhaustiveness. A čo je dôležité — keď pridáš nový stav alebo event, kompilátorová chyba ti ukáže každé miesto, kde treba aktualizovať logiku prechodov.

### Rozšírený FSM s akciami

V reálnom kóde chceš, aby prechody nielen menili stav, ale aj vykonávali akcie. Tu môže byť pattern matching s tuple naozaj expresívny:

```rust
#[derive(Debug)]
enum Action {
    None,
    SendSynAck,
    SendAck,
    SendFinAck,
    NotifyApp,
    CloseSocket,
}

fn tcp_transition_with_action(
    state: &TcpState,
    event: &TcpEvent,
) -> (TcpState, Action) {
    match (state, event) {
        (TcpState::Closed, TcpEvent::PassiveOpen) =>
            (TcpState::Listen, Action::None),
        (TcpState::Listen, TcpEvent::SynReceived) =>
            (TcpState::SynReceived, Action::SendSynAck),
        (TcpState::SynReceived, TcpEvent::AckReceived) =>
            (TcpState::Established, Action::NotifyApp),
        (TcpState::Established, TcpEvent::FinReceived) =>
            (TcpState::FinWait1, Action::SendFinAck),
        (TcpState::FinWait1, TcpEvent::AckReceived) =>
            (TcpState::TimeWait, Action::None),
        (TcpState::TimeWait, TcpEvent::Timeout) =>
            (TcpState::Closed, Action::CloseSocket),
        (s, _) => (s.clone(), Action::None),
    }
}
```

Toto je de facto tabuľka prechodov FSM vyjadrená v kóde. Je ľahko čitateľná, ľahko testovateľná, a ľahko rozšíriteľná.

---

## Bežné chyby začiatočníkov

### Zabudnúť na exhaustiveness pri wildcarde

```rust
enum Color { Red, Green, Blue, Yellow }

fn describe(c: Color) -> &'static str {
    match c {
        Color::Red => "červená",
        Color::Green => "zelená",
        _ => "iná",  // pokryje Blue aj Yellow
    }
}

// Neskôr pridáš:
// enum Color { Red, Green, Blue, Yellow, Purple }
// Kompilátor NEUPOZORNÍ, pretože _ pokryje aj Purple
// Toto je legitímna use-case pre _ wildcard, ale si si vedomý kompromisu
```

Ak chceš, aby kompilátor upozornil pri novom variante, matchuj explicitne:

```rust
fn describe_explicit(c: Color) -> &'static str {
    match c {
        Color::Red => "červená",
        Color::Green => "zelená",
        Color::Blue => "modrá",
        Color::Yellow => "žltá",
        // Nový variant = compile error tu
    }
}
```

### Move vs referencia v match

```rust
fn process_option(opt: Option<String>) {
    match opt {
        Some(s) => println!("{}", s),  // s je owned String
        None => {}
    }
    // opt je tu consumed — nemôžeš ho použiť znova
}

fn process_option_ref(opt: &Option<String>) {
    match opt {
        Some(s) => println!("{}", s),  // s je &String
        None => {}
    }
    // opt je stále platný — matchoval si referenciu
}
```

Toto je zdroj zmätku. Keď matchuješ owned hodnotu, match ju consume-uje. Keď matchuješ referenciu, dostaneš referencie na polia. Rust 2021 edition zlepšil inferenziu tu, ale stale je dobré vedieť, čo sa deje.

### Shadowing v match

```rust
let x = 5u32;
match x {
    x => println!("x je {}", x),  // toto je NOVÉ x, nie pôvodné!
}
// Správnejšie:
match x {
    val => println!("x je {}", val),
}
// Alebo:
match x {
    _ => println!("x je {}", x),  // _ — ignoruj, použi vonkajšie x
}
```

Premenné v match vetve sú nové premenné, nie referencie na existujúce. Ak chceš otestovať, či hodnota *je rovna* existujúcej premennej, musíš použiť guard:

```rust
let expected = 42u32;
let value = 42u32;

match value {
    v if v == expected => println!("zhoda!"),
    _ => println!("nie je zhoda"),
}
```

---

## Zhrnutie

| C switch | Rust match |
|---|---|
| Len skalárne hodnoty | Akýkoľvek typ — enum, struct, tuple, slice |
| Fallthrough (goto) | Žiadny fallthrough, každá vetva izolovaná |
| Zabudnutý case = tichý bug | Nekompletný match = compile error |
| Žiadne premenné z vzoru | Destrukturovanie + binding |
| Žiadne podmienky | Guards (`if` v vetve) |

Pattern matching nie je len "lepší switch". Je to fundamentálne iný spôsob ako rozmýšľať o vetvení kódu — namiesto "aká je hodnota tejto premennej" sa pýtaš "akú štruktúru má tato hodnota a čo z nej potrebujem extrahovať". Keď ho skombinuješ s enumerami, dostaneš výrazový jazyk pre modelovanie doménových stavov, ktorý je súčasne bezpečný a expresívny.
