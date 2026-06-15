# Kapitola 3 — Typy, Štruktúry, Enums

Keď som prvýkrát prešiel z C do Rustu, jedna vec ma udivila viac ako čokoľvek iné — nie borrow checker, nie lifetimes, ale typový systém. V C si si zvyknutý na to, že `int` môže byť 16 bitov na embedded systéme a 32 bitov na desktopu, že implicitná konverzia z `double` na `int` potichu zahodí desatinnú časť, a že `NULL` je len iný spôsob ako napísať `0`. Rust toto celé zahodil a postavil niečo konzistentnejšie.

Prečo na tom záleží? Pretože väčšina bezpečnostných zraniteľností a zákerných bugov v C/C++ kóde pochádza z nepresností v typovom systéme — pretečení, zámen znamienka, prístupu k null pointeru, zámeny jednotiek. Typový systém Rustu nie je len syntaktický cukríček; je to prvá línia obrany pred triedou chýb, ktoré stáli IT priemysel miliardy dolárov.

---

## Primitívne typy

### Prečo explicitná šírka?

V C, keď napíšeš `int`, dostaneš číslo. Akej veľkosti? Závisí to od platformy, kompilátora a dokonca od nastavení kompilácie. Na väčšine moderných 64-bitových systémov je `int` 32-bitový, ale na niektorých embedded platformách môže byť 16-bitový. `long` je na Windowse 32-bitový aj na 64-bitovom systéme, ale na Linuxe je 64-bitový. Toto je zdroj subtílnych portability bugov, ktoré sa objavujú až keď kód nasadíš na inú platformu.

Rust to rieši jednoducho: každý numerický typ má explicitnú šírku v bite, zabudovanú priamo do názvu. Žiadne dohady, žiadne platformové závislosti.

```rust
// Celé čísla — explicitná šírka, žiadne "int závisí od platformy"
let a: u8  = 255;          // unsigned 8-bit
let b: i32 = -1_000_000;   // signed 32-bit (podčiarknik = oddeľovač)
let c: u64 = 0xDEAD_BEEF;  // hex
let d: usize = 42;         // pointer-sized (ako size_t v C)

// Žiadne implicitné konverzie — musíš byť explicitný
let x: u32 = 100;
let y: u64 = x as u64;     // explicitný cast — ako (uint64_t)x v C
// let z: u64 = x;         // error — typy sa nezhodujú

// Pretečenie
let max = u8::MAX;         // 255
// let over = max + 1;     // panic v debug, wraparound v release
let safe = max.saturating_add(1);  // 255 — nenastal overflow
let wrap = max.wrapping_add(1);    // 0   — explicitný wraparound
let chk  = max.checked_add(1);    // None — overflow detekcia
```

Všimni si ten riadok s `saturating_add`, `wrapping_add` a `checked_add`. V C máš v podstate len jednu možnosť — unsigned wraparound je undefined behavior pre signed typy, a pre unsigned typy je definovaný ako modulo, ale nič ti nehovorí, či si to chcel. V Ruste explicitne vyjadruješ zámer: "chcem, aby sa to nasýtilo", alebo "chcem wraparound", alebo "chcem vedieť, či nastalo pretečenie".

### Pod kapotou: ako Rust kontroluje pretečenie

Vo verzii debug (cargo build bez --release) Rust vkladá do assembly kódu extra inštrukcie na kontrolu pretečenia. Napríklad pre sčítanie dvoch `u8` hodnôt:

```
; debug build — checked add
add al, bl
jo overflow_handler   ; jump if overflow flag set
```

V release build (cargo build --release) sa tieto kontroly pre bežné aritmetické operácie odstránia, pretože by spomalili kód. Toto je deliberate trade-off: počas vývoja dostaneš panic na prvom pretečení, v produkcii si musíš explicitne zvoliť správanie pomocou `saturating_add` alebo `wrapping_add`.

Ak chceš pretečenie kontrolovať aj v release builde, existuje [overflow-checks = true] v Cargo.toml. Mnohé security-sensitive projekty to zapínajú.

### Floating point

```rust
let f: f64 = 3.14159;
let g: f32 = 1.0f32;

// IEEE 754 — rovnaké ako C double/float
println!("{:.2}", f);   // "3.14"
println!("{:e}", f);    // "3.14159e0"

// NaN, infinity
let inf = f64::INFINITY;
let nan = f64::NAN;
println!("{}", nan == nan);  // false — ako v C
```

Floating point v Ruste je IEEE 754, rovnaký štandard ako v C. Žiadne prekvapenia — ak si zvyknutý na `nan != nan` v C, bude to fungovať rovnako. Rust ale pridáva metódy ako `f64::is_nan()`, `f64::is_infinite()` a `f64::is_finite()`, čo je oveľa čitateľnejšie než `isnan()` z C's `math.h`.

Jedna vec, kde sa Rust líši: v Ruste nemôžeš priamo porovnávať `f32` a `f64`. Musíš explicitne konvertovať. Znie to otravne, ale zachráni ťa to pred subtílnymi bugmi kde sa pýtaš "prečo je výsledok porovnania nesprávny" a hodiny neskôr zistíš, že si porovnával float s double a stratil si presnosť.

### Bežná chyba začiatočníkov: `as` cast verzus bezpečná konverzia

Veľa ľudí, ktorí prídu z C, začne používať `as` na všetko. To je chyba. `as` v Ruste je truncating cast — presne ako `(uint8_t)some_u32` v C:

```rust
let big: u32 = 1000;
let small: u8 = big as u8;  // 232 — ticho oreže!

// Správny spôsob — explicitná kontrola:
let small: u8 = big.try_into().expect("hodnota sa nezmestí do u8");
// alebo:
let small: Result<u8, _> = big.try_into();
match small {
    Ok(v) => println!("ok: {}", v),
    Err(_) => println!("hodnota je príliš veľká"),
}
```

`as` je vhodné keď si *istý* že konverzia je bezpečná — napríklad pretváraš `usize` index na `u32` a vieš, že hodnota nikdy nepresiahne 32-bitový rozsah. V ostatných prípadoch preferuj `try_into()`.

---

## struct

### Prečo struct s metódami?

V C je `struct` len dátový kontajner. Operácie na dátach žijú oddelene — v free funkciách, s konvenciou prefixu (`eth_frame_init()`, `eth_frame_set_payload()`). Toto funguje, ale má to problémy: nie je jasné, ktoré funkcie patria ku ktorej štruktúre, chýba zapuzdrenie, a každý si vymyslí vlastnú konvenciu.

Rust kombinuje dáta a operácie do jednej konštrukcie cez `impl` bloky. Je to viac OOP ako C, ale menej ako C++ — žiadna dedičnosť, žiadne virtuálne funkcie štandardne.

```rust
#[derive(Debug, Clone)]              // automaticky generované traity
struct EthernetFrame {
    dst_mac: [u8; 6],
    src_mac: [u8; 6],
    ether_type: u16,
    payload: Vec<u8>,
}

impl EthernetFrame {
    // Asociovaná funkcia (nie metóda) — ako statická factory
    fn new(dst: [u8; 6], src: [u8; 6], etype: u16) -> Self {
        EthernetFrame {
            dst_mac: dst,
            src_mac: src,
            ether_type: etype,
            payload: Vec::new(),
        }
    }

    // Metóda — &self = const this*, &mut self = this*
    fn set_payload(&mut self, data: Vec<u8>) {
        self.payload = data;
    }

    fn total_len(&self) -> usize {
        14 + self.payload.len()  // 6+6+2 header
    }

    fn dst_mac_str(&self) -> String {
        self.dst_mac.iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(":")
    }
}

fn main() {
    let broadcast = [0xFFu8; 6];
    let my_mac = [0x00, 0x1A, 0x2B, 0x3C, 0x4D, 0x5E];

    let mut frame = EthernetFrame::new(broadcast, my_mac, 0x0800);
    frame.set_payload(vec![0x45, 0x00, 0x00, 0x28]);

    println!("dst: {}", frame.dst_mac_str());
    println!("celková dĺžka: {} bajtov", frame.total_len());
    println!("{:?}", frame);  // Debug derive
}
```

### Pod kapotou: ako vyzerá struct v pamäti

Rust struct je v pamäti rozložený podobne ako C struct — polia sú za sebou, s paddingom pre alignment. Rust ale môže reusporadúvať polia pre optimálne zarovnanie, čo C nerobí bez `__attribute__((packed))`.

```rust
struct Zarovnaný {
    a: u8,    // 1 bajt
    b: u32,   // 4 bajty — musí byť na adrese deleniteľnej 4
    c: u8,    // 1 bajt
}
// V C: sizeof = 12 (padding za 'a' a za 'c')
// V Ruste: kompilátor môže preusporiadať na b, a, c → sizeof = 8
```

Ak potrebuješ C-kompatibilné rozloženie (napr. pre FFI alebo sieťové protokoly), použiješ `#[repr(C)]`:

```rust
#[repr(C)]
struct CCompatible {
    a: u8,    // garantovane v C poradí
    b: u32,
    c: u8,
}
```

Pre sieťové protokoly to je bežná potreba — chceš, aby sa `struct` dalo priamo castovať na bajtový buffer.

### Tuple structs a newtype pattern

```rust
// Tuple struct — pomenovaná n-tica
struct Point(f64, f64);
struct Color(u8, u8, u8);

let p = Point(1.0, 2.0);
println!("{} {}", p.0, p.1);

// Newtype — zabalenie primitívu pre typovú bezpečnosť
struct Milliseconds(u64);
struct Bytes(usize);

fn set_timeout(ms: Milliseconds) { /* ... */ }
// set_timeout(1000);    // error — u64 nie je Milliseconds
set_timeout(Milliseconds(1000)); // ok
```

Newtype pattern zabraňuje pomýleniu jednotiek — klasický zdroj bugov (Mars Climate Orbiter).

Táto havária v roku 1999 stála 327 miliónov dolárov, pretože jeden tím počítal impulzy v pound-force sekundách a druhý v newton sekundách. Typ `f64` mal rovnakú reprezentáciu pre obe jednotky. S newtype by kompilátor odmietol skompilovať kód, kde by si pomiešal tieto dve hodnoty.

Newtype má ešte jednu výhodu: je to zárodok abstrakcie. Neskôr môžeš pridať metódy, implementovať traity, a meniť internú reprezentáciu bez toho, aby si zmenil verejné API.

### Bežná chyba začiatočníkov: zabudnúť na `mut`

```rust
struct Counter {
    value: u32,
}

impl Counter {
    fn increment(&mut self) {  // potrebuje &mut self
        self.value += 1;
    }
}

fn main() {
    let c = Counter { value: 0 };
    c.increment();  // ERROR: cannot borrow `c` as mutable, as it is not declared as mutable
    // Správne:
    let mut c = Counter { value: 0 };
    c.increment();  // ok
}
```

Toto je frekventovaná chyba — zabudneš `mut` na `let` a kompilátor ti povie presne kde je problém a ako ho opraviť. V C++ by si mohol omylom volať non-const metódu na const objekte a dostať oveľa menej jasné chybové hlásenie.

---

## enum — algebraické dátové typy

### Prečo enum s dátami?

Toto je jeden z najväčších rozdielov medzi Rustom a C/C++. V C, `enum` je len pomenovaná sada číselných konštánt — je to syntaktický cukríček nad `int`. Nemôže obsahovať dáta. Keď potrebuješ "buď toto alebo tamto, s rôznymi dátami", musíš si sám postaviť tagged union.

Rust enum je to, čo akademici nazývajú *algebraický dátový typ* alebo *sum type*. Každá varianta môže mať iné dáta, iný počet polí, iné typy. A čo je kľúčové — nemôžeš pristupovať k dátam bez toho, aby si explicitne overil, ktorá varianta to je.

```rust
#[derive(Debug)]
enum IpAddr {
    V4(u8, u8, u8, u8),         // variant s tuple dátami
    V6(String),                  // variant s hodnotou
    Unspecified,                 // variant bez dát (ako C enum)
}

fn format_ip(addr: &IpAddr) -> String {
    match addr {
        IpAddr::V4(a, b, c, d) => format!("{}.{}.{}.{}", a, b, c, d),
        IpAddr::V6(s) => s.clone(),
        IpAddr::Unspecified => "0.0.0.0".to_string(),
    }
}

fn main() {
    let addr = IpAddr::V4(192, 168, 1, 1);
    println!("{}", format_ip(&addr));

    let v6 = IpAddr::V6("::1".to_string());
    println!("{:?}", v6);
}
```

V C by si to musel robiť s `union` + `enum` tag + manuálna disciplína:

```c
enum ip_type { IPV4, IPV6, UNSPECIFIED };
struct ip_addr {
    enum ip_type type;
    union {
        uint8_t v4[4];
        char v6[40];
    };
};
// A dúfať, že vždy skontroluješ type pred prístupom

// Toto C pokojne skompiluje, aj keď je to undefined behavior:
struct ip_addr addr;
addr.type = IPV4;
memcpy(addr.v4, "\xC0\xA8\x01\x01", 4);
printf("%s\n", addr.v6);  // prečíta pamäť ako string — UB
```

Rust to robí automaticky a bezpečne. Nie je fyzicky možné pristúpiť k `IpAddr::V4` dátam cez `V6` cestu, pretože match musí byť exhaustive a každá vetva extractuje správne dáta.

### Pod kapotou: ako enum vyzerá v pamäti

Rust enum je v podstate tagged union — presne to, čo by si chcel mať v C, ale automaticky. Veľkosť enum je veľkosť najväčšieho variantu plus tag.

```rust
// Tento enum
enum Msg {
    Quit,                    // žiadne dáta — 0 bajtov
    Move { x: i32, y: i32 }, // 8 bajtov
    Write(String),           // 24 bajtov (String = ptr + len + cap)
    Color(u8, u8, u8),       // 3 bajty
}
// sizeof(Msg) = 24 + niekoľko bajtov pre tag
```

Rust má ale dôležitú optimalizáciu — *null pointer optimization*. Keď máš `Option<Box<T>>` alebo `Option<&T>`, Rust vie, že `Box` a referencie nikdy nemôžu byť null. Takže môže použiť null hodnotu ako `None` tag, a `sizeof(Option<Box<T>>)` je rovnaká ako `sizeof(Box<T>)` — žiaden extra overhead.

```rust
use std::mem::size_of;

fn main() {
    println!("{}", size_of::<Option<Box<u32>>>());  // 8 (len pointer)
    println!("{}", size_of::<Option<u32>>());       // 8 (potrebuje tag)
}
```

---

## Rozloženie dát v pamäti

Toto je kapitola pre C/ASM programátorov — väčšina z vás to intuitívne chápe, ale Rust má niekoľko vlastností ktoré sa líšia od C, a niektoré optimalizácie ktoré C nemá. Poďme si to prejsť systematicky.

### Veľkosti primitívnych typov

Na rozdiel od C (`int` = závisí od platformy), Rust má pevné veľkosti pre všetky základné typy:

| Typ | Veľkosť | Zarovnanie | Poznámka |
|-----|---------|-----------|---------|
| `bool` | 1 B | 1 B | `true` = 1, `false` = 0 |
| `u8` / `i8` | 1 B | 1 B | |
| `u16` / `i16` | 2 B | 2 B | |
| `u32` / `i32` | 4 B | 4 B | |
| `u64` / `i64` | 8 B | 8 B | |
| `u128` / `i128` | 16 B | 16 B | |
| `f32` | 4 B | 4 B | IEEE 754 single |
| `f64` | 8 B | 8 B | IEEE 754 double |
| `char` | 4 B | 4 B | Unicode scalar (nie u8!) |
| `usize` / `isize` | 8 B (64-bit) | 8 B | veľkosť pointera |
| `&T` / `*const T` | 8 B | 8 B | thin pointer |
| `&[T]` / `&str` | 16 B | 8 B | fat pointer: ptr + len |
| `&dyn Trait` | 16 B | 8 B | fat pointer: ptr + vtable |
| `Box<T>` | 8 B | 8 B | ako pointer |
| `String` | 24 B | 8 B | ptr + len + cap |
| `Vec<T>` | 24 B | 8 B | ptr + len + cap |
| `Option<&T>` | 8 B | 8 B | null ptr optimization |

```rust
use std::mem::{size_of, align_of};

fn main() {
    println!("bool:   {} B, align {}", size_of::<bool>(),   align_of::<bool>());
    println!("char:   {} B, align {}", size_of::<char>(),   align_of::<char>());
    println!("usize:  {} B, align {}", size_of::<usize>(),  align_of::<usize>());
    println!("String: {} B, align {}", size_of::<String>(), align_of::<String>());
    println!("&str:   {} B, align {}", size_of::<&str>(),   align_of::<&str>());
}
// Na 64-bit systéme:
// bool:   1 B, align 1
// char:   4 B, align 4
// usize:  8 B, align 8
// String: 24 B, align 8
// &str:   16 B, align 8
```

Poznámka ku `char` — v Ruste `char` je vždy 4 bajty (Unicode scalar value, U+0000 až U+10FFFF). Na rozdiel od C kde `char` je 1 bajt (ASCII alebo implementation-defined). `&str` a `String` uchovávajú text v UTF-8 (1–4 bajty na znak), `char` sa používa len keď explicitne pracuješ s jedným Unicode znakom.

### Struct — padding a zarovnanie

Každý typ má zarovnanie (alignment) — adresa v pamäti musí byť násobkom tohto čísla. CPU požaduje `u32` na adrese deleniteľnej 4, `u64` na adrese deleniteľnej 8. Ak polia nesedí priamo za sebou, kompilátor pridá **padding** (prázdne bajty).

Rust preusporiadava polia štruktúry pre minimálny padding, čo C bez `__attribute__((packed))` nerobí:

```
// C-poradie (zlé): a, b, c
//
// Offset  Obsah
// 0       [a u8 ]
// 1       [pad  ] ← 3 bajty padding (b musí byť na offset % 4 == 0)
// 2       [pad  ]
// 3       [pad  ]
// 4       [b u32] [b u32] [b u32] [b u32]
// 8       [c u8 ]
// 9       [pad  ] ← 3 bajty padding (struct musí mať veľkosť % 4 == 0)
// 10      [pad  ]
// 11      [pad  ]
// sizeof = 12

// Rust preusporiadá na: b, a, c
//
// Offset  Obsah
// 0       [b u32] [b u32] [b u32] [b u32]
// 4       [a u8 ]
// 5       [c u8 ]
// 6       [pad  ] ← 2 bajty padding (struct musí mať veľkosť % 4 == 0)
// 7       [pad  ]
// sizeof = 8
```

```rust
use std::mem::size_of;

struct ZlePoradie {
    a: u8,
    b: u32,
    c: u8,
}

struct Dobre {
    b: u32,
    a: u8,
    c: u8,
}

fn main() {
    println!("ZlePoradie: {} B", size_of::<ZlePoradie>());  // 12
    println!("Dobre:      {} B", size_of::<Dobre>());       // 8
}
```

Rust to automaticky preusporiadava — takže v praxi ti na poradí polí nezáleží pre výkon. **Ale**: Rust neurčuje presné poradie polí v pamäti (bez `#[repr(C)]`). Nespoliehaj sa naň pri FFI alebo priamom castovaní na bajty.

### `#[repr(...)]` — kontrola rozloženia

Rust má viacero `repr` atribútov na explicitné riadenie pamäťového rozloženia:

```rust
// Predvolené — Rust môže preusporiadať polia
struct Default { a: u8, b: u32, c: u8 }

// C-kompatibilné — polia v definovanom poradí, C padding pravidlá
#[repr(C)]
struct CLayout { a: u8, b: u32, c: u8 }  // sizeof = 12

// Packed — žiadny padding, polia tesne za sebou
// POZOR: môže spôsobiť unaligned prístupy = pomalé alebo crash na niektorých CPU
#[repr(packed)]
struct Packed { a: u8, b: u32, c: u8 }   // sizeof = 6

// Transparentné — newtype wrapper s rovnakým rozložením ako vnútorný typ
#[repr(transparent)]
struct Wrapper(u32);  // sizeof = 4, zarovnanie = 4
```

`#[repr(C)]` je nevyhnutné pri:
- FFI (volanie C funkcií, ktoré berú ukazovatele na struct)
- `mmap` súborov s fixným binárnym formátom
- Priamom castovaní na `&[u8]` (network packets, protokoly)

```rust
#[repr(C)]
struct EthernetFrame {
    dst_mac:  [u8; 6],
    src_mac:  [u8; 6],
    ethertype: u16,
    // payload nasleduje
}

// Môžeš castovať sieťový buffer priamo:
let frame = unsafe {
    &*(raw_bytes.as_ptr() as *const EthernetFrame)
};
```

### Enum — veľkosť a ako ju zmenšiť

Enum je tagged union. Veľkosť = veľkosť najväčšieho variantu + tag (zaoblené na zarovnanie):

```rust
use std::mem::size_of;

enum Maly {
    A(u8),    // 1 B
    B(u16),   // 2 B
}
// tag: 1 B, dáta: 2 B, padding: 1 B → sizeof = 4

enum Velky {
    Quit,
    Move { x: i32, y: i32 },  // 8 B
    Write(String),              // 24 B ← najväčší
    Color(u8, u8, u8),         // 3 B
}
// sizeof(Velky) ≈ 32 B (24 dáta + tag + padding)

fn main() {
    println!("Maly:  {} B", size_of::<Maly>());   // 4
    println!("Velky: {} B", size_of::<Velky>());  // 32
}
```

Problém nastáva keď jeden variant je oveľa väčší ako ostatné — každá inštancia enum zaberá toľko pamäte ako ten najväčší variant, aj keď je aktívny malý variant.

**Riešenie: `Box<T>` pre veľké varianty**

`Box<T>` presunie veľké dáta na heap — v enum zostane len pointer (8 bajtov):

```rust
// Pred: každý Expression zaberá toľko ako najväčší variant
enum ExpressionBig {
    Number(f64),                           // 8 B
    Variable(String),                      // 24 B
    BinaryOp {                             // 8 + 2 rekurzívne = ∞ bez Box
        op: char,
        left:  Box<ExpressionBig>,         // 8 B (pointer na heap)
        right: Box<ExpressionBig>,         // 8 B
    },
    FunctionCall {
        name: String,                      // 24 B
        args: Vec<ExpressionBig>,          // 24 B
    },
}

// Po optimalizácii — veľké varianty sa baxujú:
enum Expression {
    Number(f64),                           // 8 B
    Variable(String),                      // 24 B
    BinaryOp(Box<BinaryOp>),               // 8 B (pointer)
    FunctionCall(Box<FunctionCall>),       // 8 B (pointer)
}

struct BinaryOp {
    op: char,
    left: Expression,
    right: Expression,
}
```

Kompilátor ťa na to upozorní: `clippy` má lint `large_enum_variant` ktorý varuje keď jeden variant je oveľa väčší ako ostatné.

**`#[repr(u8)]` — zmenšenie tagu**

Predvolene Rust môže použiť `i32` pre tag enumu. Pre malé enumy bez dát (alebo s malými dátami) môžeš tag zmenšiť:

```rust
#[repr(u8)]
enum Status {
    Idle    = 0,
    Running = 1,
    Stopped = 2,
    Error   = 255,
}
// sizeof = 1 B (bez extra paddingu)

#[repr(u8)]
enum Compact {
    A,       // 0
    B(u8),   // 1, dáta: u8
    C(u16),  // 2, dáta: u16
}
// sizeof = 4 B (tag 1B + max dáta 2B + padding 1B)
```

### Null pointer optimization (NPO)

Rust robí špeciálnu optimalizáciu pre `Option<T>` kde `T` nemôže byť null — pointer typy, referencie, `NonNull<T>`, `Box<T>`:

```rust
use std::mem::size_of;

fn main() {
    // Referencie a Box nemôžu byť null — None reprezentuje null
    println!("{}", size_of::<Option<&u32>>());       // 8 (nie 16!)
    println!("{}", size_of::<Option<Box<u32>>>());   // 8 (nie 16!)
    println!("{}", size_of::<Option<fn()>>());       // 8

    // Pre u32 Rust nemá "reserved" hodnotu — musí pridať tag
    println!("{}", size_of::<Option<u32>>());        // 8 (4 dáta + 4 tag+padding)
    println!("{}", size_of::<Option<bool>>());       // 1 (bool má rezervovanú hodnotu 2)
}
```

`Option<bool>` je tiež optimalizovaný — `bool` má platné hodnoty 0 a 1, takže Rust použije hodnotu 2 ako `None`. Výsledok: `size_of::<Option<bool>>() == 1`.

### Zero-sized types (ZST)

Niektoré typy majú veľkosť 0 bajtov — existujú len pre typovú bezpečnosť, nemajú runtime overhead:

```rust
struct Unit;           // ZST — 0 bajtov
struct Marker<T>(std::marker::PhantomData<T>);  // ZST

// Vec<ZST> je špeciálny — alokuje 0 bajtov ale počíta len
let v: Vec<Unit> = (0..1000).map(|_| Unit).collect();
println!("{}", std::mem::size_of_val(&v));  // 24 (Vec metadata)
println!("{}", v.len());                    // 1000

// Marker komponenty v Bevy sú ZST:
#[derive(Component)]
struct Player;  // 0 bajtov na entity — len tag v ECS
```

### Praktický postup: meranie a optimalizácia

```rust
use std::mem::{size_of, size_of_val, align_of};

#[derive(Debug)]
enum AstNode {
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<AstNode>),
    Map(std::collections::HashMap<String, AstNode>),
}

fn main() {
    // Zisti veľkosť pred optimalizáciou
    println!("AstNode: {} B", size_of::<AstNode>());

    // Zisti veľkosť konkrétnej hodnoty
    let node = AstNode::Int(42);
    println!("AstNode::Int instance: {} B", size_of_val(&node));

    // Zarovnanie
    println!("align: {} B", align_of::<AstNode>());
}
```

Ak `AstNode` je príliš veľký (napr. 56+ B), vybožuj veľké varianty:

```rust
enum AstNodeOpt {
    Int(i64),
    Float(f64),
    Str(Box<str>),                              // Box<str> = 16 B namiesto String 24 B
    List(Box<Vec<AstNodeOpt>>),                 // 8 B namiesto Vec 24 B
    Map(Box<std::collections::HashMap<String, AstNodeOpt>>), // 8 B
}
// sizeof ≈ 16 B (max: Str = pointer 8 + len 8)
```

**`Box<str>` vs `String`**: `String` je 24 B (ptr + len + capacity). `Box<str>` je 16 B (ptr + len) — bez capacity, pretože box nie je mutovateľný buffer. Pre read-only stringy v dátových štruktúrach je `Box<str>` menší.

### Zhrnutie pamäťového rozloženia

| Typ | Stack veľkosť | Heap |
|-----|--------------|------|
| `u8`..`u64`, `f32`/`f64` | 1–8 B | nie |
| `&T` (thin ref) | 8 B | nie |
| `&[T]`, `&str` (fat ref) | 16 B | nie |
| `Box<T>` | 8 B | sizeof(T) |
| `String` | 24 B | dĺžka textu |
| `Vec<T>` | 24 B | len * sizeof(T) |
| `Option<&T>` | 8 B (NPO) | nie |
| `Option<u32>` | 8 B | nie |
| Enum | max(varianty) + tag | závisí od obsahu |
| ZST | 0 B | nie |

---

### Enum v praxi: chybové stavy

Enum je ideálny na modelovanie chybových stavov, kde rôzne chyby majú rôzne kontextové informácie:

```rust
#[derive(Debug)]
enum NetworkError {
    ConnectionRefused { host: String, port: u16 },
    Timeout { elapsed_ms: u64 },
    InvalidResponse { status_code: u16, body: String },
    DnsResolutionFailed(String),
    Disconnected,
}

fn connect(host: &str, port: u16) -> Result<(), NetworkError> {
    if port == 0 {
        return Err(NetworkError::ConnectionRefused {
            host: host.to_string(),
            port,
        });
    }
    // ... ďalšia logika
    Ok(())
}

fn handle_error(err: NetworkError) {
    match err {
        NetworkError::ConnectionRefused { host, port } => {
            eprintln!("Odmietnuté spojenie na {}:{}", host, port);
        }
        NetworkError::Timeout { elapsed_ms } => {
            eprintln!("Timeout po {} ms", elapsed_ms);
        }
        NetworkError::InvalidResponse { status_code, body } => {
            eprintln!("HTTP {}: {}", status_code, body);
        }
        NetworkError::DnsResolutionFailed(name) => {
            eprintln!("DNS zlyhal pre: {}", name);
        }
        NetworkError::Disconnected => {
            eprintln!("Spojenie prerušené");
        }
    }
}
```

Porovnaj to s C prístupom kde máš `errno` a jeden int — nedostaneš ani hostname, ani port, ani status code priamo v chybovej hodnote. Musíš si ich uchovávať externe alebo výsledok zahodiť.

---

## Option\<T\> — koniec NULL

### Prečo je NULL nebezpečný?

Tony Hoare, vynálezca null referencie, ju nazval svojou "miliardodolárovou chybou". Null existuje v C, C++, Jave, C#, Pythone — takmer všade. A všade spôsobuje rovnakú triedu chýb: NullPointerException, segfault, UB z dereferencovania null pointera.

Problém nie je v null samotnom — problém je, že v týchto jazykoch *každý* pointer alebo referencia môže byť null, a typový systém ti nepomôže skontrolovať to. Musíš si sám pamätať, ktoré hodnoty môžu byť null a kontrolovať ich.

Rust úplne eliminoval null. Namiesto neho máš `Option<T>` — typ, ktorý explicitne hovorí "tato hodnota môže existovať alebo nemusí". A kompilátor ťa núti ošetriť oba prípady.

```rust
fn find_port(name: &str) -> Option<u16> {
    match name {
        "http"  => Some(80),
        "https" => Some(443),
        "ssh"   => Some(22),
        _       => None,
    }
}

fn main() {
    // Musíš ošetriť None — kompilátor nedovolí ignorovať
    match find_port("http") {
        Some(port) => println!("port: {}", port),
        None       => println!("neznáma služba"),
    }

    // Skrátená syntax
    let port = find_port("ssh").unwrap_or(0);        // default ak None
    let port = find_port("ftp").unwrap_or_else(|| {  // lazy default
        eprintln!("FTP nie je podporovaný");
        21
    });

    // Reťazenie — ? v Option kontexte
    let doubled = find_port("https").map(|p| p * 2); // Some(886)
    let _: Option<String> = find_port("http")
        .filter(|&p| p < 1024)
        .map(|p| format!("privilegovaný port: {}", p));

    // Len ak vieš že value tam je (panic ak nie):
    let p = find_port("http").unwrap();  // ok tu, ale radšej nepoužívaj v produkcii
    let p = find_port("http").expect("http musí mať port"); // lepší panic message
}
```

### Option je len enum

```rust
// Stdlib definícia:
enum Option<T> {
    Some(T),
    None,
}
```

Žiadna špeciálna syntax. Žiadny null pointer. Ak funkcia môže zlyhať pri hľadaní, vrátis `Option`. Ak volajúci zabudne ošetriť — compile error. Jednoduchšie ani byť nemôže.

### Metódy na Option — funkcionálny štýl

Option má bohatú sadu metód, ktorá ti umožňuje reťaziť transformácie bez explicitného matchovania:

```rust
fn get_config_port(config: Option<&str>) -> u16 {
    config
        .filter(|s| !s.is_empty())           // None ak prázdny string
        .and_then(|s| s.parse::<u16>().ok()) // None ak parse zlyhal
        .filter(|&p| p > 0 && p < 65536)    // None ak mimo rozsahu
        .unwrap_or(8080)                     // default hodnota
}

fn main() {
    println!("{}", get_config_port(Some("443")));   // 443
    println!("{}", get_config_port(Some("abc")));   // 8080 (parse failed)
    println!("{}", get_config_port(Some("")));       // 8080 (empty)
    println!("{}", get_config_port(None));           // 8080
}
```

Toto je elegantnejšie než séria `if (val != NULL)` kontrol, a navyše je čitateľnejšie — vidíš presne aký je tok dát.

### Bežná chyba začiatočníkov: `unwrap()` všade

```rust
// Zlý kód — panics ak port nie je nájdený
let port = find_port("ftp").unwrap();  // runtime panic!

// Lepšie:
let port = find_port("ftp").unwrap_or(21);

// Alebo propaguj chybu:
fn setup() -> Option<u16> {
    let port = find_port("ftp")?;  // ? vráti None z funkcie ak je None
    Some(port + 1)
}
```

`unwrap()` je legitímny nástroj v testoch a príkladoch, kde vieš, že hodnota tam bude. V produkčnom kóde je to červená vlajka — každý `unwrap()` je potenciálny runtime panic.

---

## Result\<T, E\> — koniec ignorovaných chýb

### Prečo je errno nedostatočný?

C má niekoľko konvencií pre chybové stavy, a žiadna z nich nie je dobrá. Funkcia môže vrátiť záporné číslo, nulu, NULL pointer, alebo nastaviť globálnu premennú `errno`. Volajúci môže jednoducho ignorovať návratovú hodnotu — C to dovolí bez varovania. Štúdie ukázali, že programátori ignorujú chybové kódy v 50-90% prípadov.

Rust má `Result<T, E>` — buď `Ok(hodnota)` alebo `Err(chyba)`. Kompilátor vygeneruje varovanie ak zabudneš použiť `Result` (je označený ako `#[must_use]`). Nemôžeš ho jednoducho ignorovať.

```rust
use std::num::ParseIntError;

fn parse_port(s: &str) -> Result<u16, ParseIntError> {
    s.trim().parse::<u16>()  // Result<u16, ParseIntError>
}

fn main() {
    match parse_port("8080") {
        Ok(port)  => println!("port: {}", port),
        Err(e)    => eprintln!("chyba: {}", e),
    }

    // Chybový? Prepropaguj vyššie s ?
    // (len ak má caller Result návratový typ)
    // let port = parse_port("abc")?;

    // Transformácie
    let port: Result<u16, _> = parse_port("invalid");
    let port = port.unwrap_or(80);  // default

    let port = parse_port("443")
        .map(|p| p + 1)             // 444
        .map_err(|e| format!("neplatný port: {}", e));
}
```

V C by si mal:

```c
int parse_port(const char *s, uint16_t *out) {
    // errno, *out, return code — všetky tri konvencie v praxi
    char *end;
    long val = strtol(s, &end, 10);
    if (*end != '\0') return -1;
    if (val < 0 || val > 65535) return -1;
    *out = (uint16_t)val;
    return 0;
}
// A väčšina volajúcich ignoruje return code
```

### Operátor `?` — propagácia chýb

Toto je jeden z najkrajších prvkov Rustu. Operátor `?` vezme `Result` a ak je to `Err`, okamžite vráti tú chybu z aktuálnej funkcie. Ak je to `Ok`, extractuje hodnotu:

```rust
use std::io;
use std::fs;
use std::num::ParseIntError;

#[derive(Debug)]
enum ConfigError {
    Io(io::Error),
    Parse(ParseIntError),
}

impl From<io::Error> for ConfigError {
    fn from(e: io::Error) -> Self { ConfigError::Io(e) }
}

impl From<ParseIntError> for ConfigError {
    fn from(e: ParseIntError) -> Self { ConfigError::Parse(e) }
}

fn read_port_from_file(path: &str) -> Result<u16, ConfigError> {
    let content = fs::read_to_string(path)?;  // io::Error → ConfigError::Io
    let port = content.trim().parse::<u16>()?; // ParseIntError → ConfigError::Parse
    Ok(port)
}
```

Bez `?` by si musel písať:

```rust
fn read_port_from_file_verbose(path: &str) -> Result<u16, ConfigError> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => return Err(ConfigError::Io(e)),
    };
    let port = match content.trim().parse::<u16>() {
        Ok(p) => p,
        Err(e) => return Err(ConfigError::Parse(e)),
    };
    Ok(port)
}
```

`?` je len syntaktický cukríček, ale robí kód omnoho čitateľnejším — vidíš logiku, nie správu chýb.

---

## Praktický príklad: parser sieťového protokolu

Jednoduchý binárny protokol — TLV (Type-Length-Value). Toto je reálny vzor, ktorý uvidíš v SNMP, LDAP, TLS a mnohých iných protokoloch:

```rust
#[derive(Debug, PartialEq)]
enum TlvType {
    HostName = 1,
    IpAddress = 2,
    Port = 3,
    Unknown(u8),
}

impl From<u8> for TlvType {
    fn from(v: u8) -> Self {
        match v {
            1 => TlvType::HostName,
            2 => TlvType::IpAddress,
            3 => TlvType::Port,
            n => TlvType::Unknown(n),
        }
    }
}

#[derive(Debug)]
struct TlvField<'a> {
    typ: TlvType,
    value: &'a [u8],
}

#[derive(Debug)]
enum ParseError {
    TooShort,
    LengthOverflow { declared: usize, available: usize },
}

fn parse_tlv(buf: &[u8]) -> Result<Vec<TlvField<'_>>, ParseError> {
    let mut fields = Vec::new();
    let mut pos = 0;

    while pos < buf.len() {
        if buf.len() - pos < 2 {
            return Err(ParseError::TooShort);
        }
        let typ = TlvType::from(buf[pos]);
        let len = buf[pos + 1] as usize;
        pos += 2;

        if pos + len > buf.len() {
            return Err(ParseError::LengthOverflow {
                declared: len,
                available: buf.len() - pos,
            });
        }

        fields.push(TlvField { typ, value: &buf[pos..pos + len] });
        pos += len;
    }

    Ok(fields)
}

fn main() {
    let packet = [
        0x01, 0x05, b'h', b'e', b'l', b'l', b'o',  // HostName: "hello"
        0x03, 0x02, 0x1F, 0x90,                      // Port: 8080 (0x1F90)
    ];

    match parse_tlv(&packet) {
        Ok(fields) => {
            for f in &fields {
                match &f.typ {
                    TlvType::HostName => {
                        println!("hostname: {}", std::str::from_utf8(f.value).unwrap_or("?"));
                    }
                    TlvType::Port => {
                        let port = u16::from_be_bytes(f.value.try_into().unwrap());
                        println!("port: {}", port);
                    }
                    other => println!("{:?}: {:02X?}", other, f.value),
                }
            }
        }
        Err(e) => eprintln!("parse error: {:?}", e),
    }
}
```

Výstup:
```
hostname: hello
port: 8080
```

Všimni si, čo sa deje v `TlvField` — `value: &'a [u8]` je referencia priamo do pôvodného buffra. Bez kópie, bez alokácie. Parser je zero-copy, čo je kľúčové pre výkon v sieťovom kóde. Životnosť `'a` zaručuje, že referencia je platná pokiaľ existuje pôvodný buffer — žiadny dangling pointer možný.

---

## Zhrnutie

| C | Rust |
|---|------|
| `int`, `long` (platform-dependent) | `i32`, `i64` (explicitná šírka) |
| Implicitné konverzie | Explicitné `as` cast |
| `struct` bez metód | `struct` + `impl` blok |
| `union` + manuálny tag | `enum` s dátami (tagged union) |
| `NULL` pointer | `Option<T>` |
| `errno` / return code | `Result<T, E>` |

Typový systém Rustu nie je len iná syntax — je to iný spôsob myslenia o dátach. Namiesto "každá hodnota môže byť čokoľvek, kontroluj za runtime" dostaneš "typy popisujú presne čo hodnota môže byť, a kompilátor to overí za compile time". Keď si na to zvykneš, bude ti C pôsobiť ako chôdza so zaviazanými očami.
