# Kapitola 5 — Traits

V predchádzajúcej kapitole sme videli, ako pattern matching umožňuje pracovať s rôznymi variantami enumov. Ale čo ak chceš definovať *správanie*, ktoré je spoločné pre viacero úplne rôznych typov? V C++ by si siahol po virtuálnych funkciách alebo šablónach. V Jave po interfacoch. Rust má traits — mechanizmus, ktorý je silnejší než každý z týchto prístupov, a pritom nepotrebuje dedičnosť.

Prečo je dedičnosť problém? Dedičnosť v OOP je lákavá — zdieľaš kód medzi triedami, modeluješ "je-typ" vzťahy. Ale v praxi vedie k krehkým hierarchiám, kde zmena základnej triedy rozbije všetkých potomkov, kde je ťažké pochopiť, ktorá implementácia sa volá, a kde diamond problem spôsobuje hlavabolia. Rust sa rozhodol dedičnosť úplne vynechať a namiesto toho postavil trait systém, ktorý rieši rovnaké problémy bez týchto nástrah.

---

## Definícia a implementácia

Trait definuje množinu metód, ktorú môže ľubovolný typ implementovať. Je to v podstate kontrakt: "ak implementuješ tieto metódy, sľubujem, že ťa môžem použiť kdekoľvek kde sa očakáva tento trait".

```rust
trait Checksum {
    fn compute(&self, data: &[u8]) -> u32;
    fn verify(&self, data: &[u8], expected: u32) -> bool {
        self.compute(data) == expected  // default implementácia
    }
}

struct Crc32;
struct Adler32;

impl Checksum for Crc32 {
    fn compute(&self, data: &[u8]) -> u32 {
        // zjednodušená implementácia
        data.iter().fold(0xFFFF_FFFFu32, |crc, &b| {
            (crc >> 8) ^ (crc ^ b as u32).wrapping_mul(0x04C1_1DB7)
        }) ^ 0xFFFF_FFFF
    }
}

impl Checksum for Adler32 {
    fn compute(&self, data: &[u8]) -> u32 {
        let (mut a, mut b) = (1u32, 0u32);
        for &byte in data {
            a = (a + byte as u32) % 65521;
            b = (b + a) % 65521;
        }
        (b << 16) | a
    }
}

fn main() {
    let data = b"hello world";
    let crc = Crc32;
    let adler = Adler32;

    println!("CRC32:   0x{:08X}", crc.compute(data));
    println!("Adler32: 0x{:08X}", adler.compute(data));
    println!("CRC verify: {}", crc.verify(data, crc.compute(data)));
}
```

Všimni si niekoľko vecí. Po prvé, `Crc32` a `Adler32` sú prázdne štruktúry — `struct Crc32;`. V C by si toto riešil globálnymi funkciami `crc32_compute()` a `adler32_compute()`. V Ruste trait dáva týmto funkciám spoločné meno a signatúru.

Po druhé, `verify` má default implementáciu. Každý typ, ktorý implementuje `Checksum`, dostane `verify` zadarmo, pokiaľ si ho neprepíše. Toto je lepšie než C++ abstract class — môžeš mať čiastočné default implementácie bez vynútenia dedičnosti.

Po tretie, `Crc32` neprepísal `verify`. Automaticky zdedil default. Ak neskôr zistíš, že existuje efektívnejší algoritmus na overenie CRC32 (napr. robiť to v jednom prechode), môžeš pridať implementáciu `verify` bez zmeny existujúceho kódu.

### Traits ako pomenované kontrakty

Trait nie je len syntax — je to dokumentácia zámerov. Keď typ implementuje `Checksum`, hovorí: "viem vypočítať kontrolný súčet". Keď funkcia vyžaduje `T: Checksum`, hovorí: "daj mi niečo, čo vie vypočítať kontrolný súčet, je mi jedno čo presne".

Toto je *duck typing* zo statickými zárukami. Python tiež robí duck typing — ak má objekt metódu `compute`, môžeš ho použiť. Ale Python to zistí za runtime. Rust to vie za compile time — a keď `T` nemá `compute`, dostaneš chybovú hlášku pred spustením programu.

---

## Generics vs `dyn Trait`

Toto je jeden z najdôležitejších trade-offov v Ruste, a veľa ľudí ho spočiatku nechápe. Existujú dva spôsoby ako povedať "chcem parameter, ktorý implementuje tento trait" — a líšia sa fundamentálne tým, *kedy* sa rozhoduje, ktorá implementácia sa zavolá.

### Generics — monomorphization (compile time)

```rust
// Kompilátor vygeneruje dve verzie funkcie: jednu pre Crc32, jednu pre Adler32
fn validate<C: Checksum>(checker: &C, data: &[u8], checksum: u32) -> bool {
    checker.verify(data, checksum)
}

// Alternatívna syntax:
fn validate2(checker: &impl Checksum, data: &[u8], checksum: u32) -> bool {
    checker.verify(data, checksum)
}

// Použitie:
let ok = validate(&Crc32, b"test", 0xD87F7E0C);
```

Keď napíšeš `validate<C: Checksum>`, kompilátor vytvorí *šablónu* funkcie. Pre každý konkrétny typ, s ktorým túto funkciu zavoláš, vygeneruje osobitnú kópiu funkcie. Ak zavoláš `validate(&Crc32, ...)` aj `validate(&Adler32, ...)`, skompilujú sa dve úplne odlišné funkcie.

Toto je to, čo Rust nazýva *monomorphization* — to isté čo C++ templates. Výsledok je, že kompilátor môže inlinovat volania, optimalizovať pre konkrétny typ, a celkovo generovať rovnako rýchly kód ako keby si napísal špeciálnu funkciu pre každý typ ručne.

**Výsledok:** Rýchlosť C templates. Každý typ dostane svoju inlinovanú verziu funkcie. Binárka je väčšia, ale zero runtime overhead.

### Pod kapotou: monomorphization v assembly

Pozrime sa na konkrétny príklad. Funkcia:

```rust
fn sum_all<I: Iterator<Item = u32>>(iter: I) -> u32 {
    iter.sum()
}
```

Keď ju zavoláš s `sum_all(vec.iter().copied())` a `sum_all(0u32..100)`, kompilátor vygeneruje dve rôzne funkcie. Každá je optimalizovaná pre konkrétny iterator — jedna môže byť SIMD-vektorizovaná, druhá môže byť loop s pridávaním od 0 do 99. Kompilátor *vie* čo robí každý konkrétny iterator a môže optimalizovať podľa toho.

V C++ by si toto riešil templates. Výsledok je rovnaký — ale C++ templates majú notoricky zlé chybové hlášky. Rust generic chybové hlášky sú oveľa čitateľnejšie.

### `dyn Trait` — vtable (runtime)

```rust
// Dynamický dispatch — ako C++ virtuálne funkcie
fn validate_dynamic(checker: &dyn Checksum, data: &[u8], checksum: u32) -> bool {
    checker.verify(data, checksum)
}

// Homogénna kolekcia rôznych implementácií
fn run_all_checksums(checkers: &[Box<dyn Checksum>], data: &[u8]) {
    for checker in checkers {
        println!("0x{:08X}", checker.compute(data));
    }
}

fn main() {
    let checkers: Vec<Box<dyn Checksum>> = vec![
        Box::new(Crc32),
        Box::new(Adler32),
    ];
    run_all_checksums(&checkers, b"hello");
}
```

`dyn Checksum` je *trait object* — fat pointer pozostávajúci z dvoch ukazovateľov: jeden na dáta, druhý na *vtable* (tabuľku virtuálnych funkcií). Keď zavoláš `checker.compute(data)`, runtime pozrie do vtable, nájde adresu funkcie, a zavolá ju. Toto je presne to isté čo C++ virtuálne funkcie.

**Výsledok:** Flexibilita, menšia binárka, ~1ns overhead na volanie (pointer cez vtable). Použij keď potrebuješ heterogénne kolekcie alebo rozhodnutie za runtime.

### Pod kapotou: ako vyzerá vtable

```
Box<dyn Checksum> v pamäti:
┌─────────────────────────────────────┐
│ data pointer  → konkrétny Crc32 obj │
│ vtable pointer → vtable pre Crc32   │
└─────────────────────────────────────┘

vtable pre Crc32:
┌──────────────────────────────────────┐
│ drop glue (destruktor)               │
│ size                                 │
│ align                                │
│ ptr na Crc32::compute                │
│ ptr na default Checksum::verify      │
└──────────────────────────────────────┘
```

Každá implementácia `Checksum` má svoju vlastnú vtable. `dyn Checksum` pointer nesie vtable pointer, takže pri volaní metódy vie, ktorú konkrétnu implementáciu zavolať.

| | Generics `<T: Trait>` | `dyn Trait` |
|---|---|---|
| Rozhodovanie | Compile time | Runtime |
| Overhead | Nulový | vtable pointer |
| Binárka | Väčšia (duplikovaný kód) | Menšia |
| Heterogénna kolekcia | Nie | Áno |
| Inlining | Áno | Nie |

Pravidlo palca: ak vieš za compile time, ktoré typy budeš používať, použi generics. Ak potrebuješ zoznam rôznych typov, alebo ak typ závisí od runtime vstupu (napr. konfiguračný súbor hovorí, ktorý checksum algoritmus použiť), použi `dyn Trait`.

---

## Štandardné traits

### Display a Debug

`Display` a `Debug` sú najzákladnejšie formatting traits. `Debug` je pre vývojárov — má byť strojovo generovaný a obsahovať všetky detaily. `Display` je pre používateľov — má byť čitateľný.

```rust
use std::fmt;

struct MacAddr([u8; 6]);

impl fmt::Display for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.0[0], self.0[1], self.0[2],
            self.0[3], self.0[4], self.0[5])
    }
}

impl fmt::Debug for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MacAddr({})", self)
    }
}

fn main() {
    let mac = MacAddr([0x00, 0x1A, 0x2B, 0x3C, 0x4D, 0x5E]);
    println!("{}", mac);    // Display: 00:1A:2B:3C:4D:5E
    println!("{:?}", mac);  // Debug:   MacAddr(00:1A:2B:3C:4D:5E)
}
```

V C, ak chceš vypísať vlastnú štruktúru, musíš vždy volať dedikovanú funkciu `print_mac_addr(mac)`. V Ruste, keď implementuješ `Display`, automaticky funguje `println!("{}", mac)`, `format!("{}", mac)`, loggery, a akýkoľvek iný kód, ktorý pracuje s `Display`.

Väčšina typov v štandardnej knižnici implementuje `Debug` cez `#[derive(Debug)]` — automaticky generovanú implementáciu. `Display` sa zvyčajne implementuje ručne, pretože závisí od toho, ako chceš prezentovať dáta.

### From / Into — konverzie

V C konverzie medzi typmi sú buď implicitné (nebezpečné) alebo explicitné cast (bez typu bezpečnosti). Rust má `From` a `Into` — typovo bezpečné, explicitné konverzie, ktoré môžu zlyhať.

```rust
struct Port(u16);

impl From<u16> for Port {
    fn from(v: u16) -> Self {
        Port(v)
    }
}

// From<u16> automaticky implementuje Into<Port>
fn connect(port: impl Into<Port>) {
    let p: Port = port.into();
    println!("connecting to port {}", p.0);
}

fn main() {
    let p = Port::from(8080);
    let p2: Port = 443u16.into();
    connect(22u16);   // automatická konverzia cez Into
}
```

Keď implementuješ `From<A> for B`, Rust automaticky implementuje `Into<B> for A`. Takže ti stačí implementovať jednu stranu a druhá príde zadarmo — toto je príklad "blanket implementation" o ktorej budeme hovoriť neskôr.

Pre konverzie ktoré môžu zlyhať existuje `TryFrom` a `TryInto`:

```rust
use std::convert::TryFrom;

struct ValidPort(u16);

impl TryFrom<u32> for ValidPort {
    type Error = String;

    fn try_from(v: u32) -> Result<Self, Self::Error> {
        if v == 0 || v > 65535 {
            Err(format!("{} nie je platný port", v))
        } else {
            Ok(ValidPort(v as u16))
        }
    }
}

fn main() {
    let port = ValidPort::try_from(8080u32);   // Ok(ValidPort(8080))
    let bad  = ValidPort::try_from(70000u32);  // Err("70000 nie je platný port")
    let zero = ValidPort::try_from(0u32);      // Err("0 nie je platný port")

    println!("{:?}", port.is_ok());   // true
    println!("{:?}", bad.is_ok());    // false
}
```

### Iterator

`Iterator` trait je asi najpraktickejší štandardný trait. Keď implementuješ len jednu metódu `next()`, automaticky dostaneš stovky metód zadarmo — `map`, `filter`, `fold`, `sum`, `collect`, `enumerate`, `zip`, a ďalšie.

```rust
struct Counter {
    count: u32,
    max: u32,
}

impl Counter {
    fn new(max: u32) -> Self {
        Counter { count: 0, max }
    }
}

impl Iterator for Counter {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        if self.count < self.max {
            self.count += 1;
            Some(self.count)
        } else {
            None
        }
    }
}

fn main() {
    // Všetky iterator metódy zadarmo:
    let sum: u32 = Counter::new(10).sum();
    println!("suma: {}", sum);  // 55

    let evens: Vec<u32> = Counter::new(10)
        .filter(|x| x % 2 == 0)
        .map(|x| x * x)
        .collect();
    println!("{:?}", evens);  // [4, 16, 36, 64, 100]
}
```

Kľúčová vec tu je, že iterátorové adaptéry (`map`, `filter`) sú *lazy* — nevykonávajú sa pokiaľ ich nerealizuješ cez `collect()`, `sum()`, alebo iný "consuming" adaptér. Reťazenie `filter().map().collect()` nevytvára medzivektory. Je to ekvivalent C for-loopu, ale bez manuálneho manažmentu indexov.

### Pod kapotou: iterátory a nulový overhead

Pozri čo generuje Rust pre jednoduchý iterátor:

```rust
let sum: u32 = (0u32..100).sum();
```

Kompilátor to rozpozná ako súčet aritmetickej postupnosti a môže to vypočítať za compile time, alebo to vektorizovať pomocou SIMD inštrukcií. Žiadny overhead za abstrakciu — je to rovnako rýchle ako optimalizovaný C for-loop, ale oveľa čitateľnejšie.

---

## Praktický príklad: Iterator pre binárny protokol

Toto je príklad, kde traits naozaj ukazujú svoju silu. Implementujeme iterator nad binárnym TLV protokolom — rovnaká štruktúra ako v kapitole 3, ale tentoraz s iterátorovým rozhraním:

```rust
struct TlvIterator<'a> {
    buf: &'a [u8],
    pos: usize,
}

#[derive(Debug)]
struct TlvField<'a> {
    typ: u8,
    value: &'a [u8],
}

impl<'a> TlvIterator<'a> {
    fn new(buf: &'a [u8]) -> Self {
        TlvIterator { buf, pos: 0 }
    }
}

impl<'a> Iterator for TlvIterator<'a> {
    type Item = TlvField<'a>;

    fn next(&mut self) -> Option<TlvField<'a>> {
        if self.pos + 2 > self.buf.len() {
            return None;
        }
        let typ = self.buf[self.pos];
        let len = self.buf[self.pos + 1] as usize;
        self.pos += 2;

        if self.pos + len > self.buf.len() {
            return None;
        }
        let value = &self.buf[self.pos..self.pos + len];
        self.pos += len;

        Some(TlvField { typ, value })
    }
}

fn main() {
    let data = [
        0x01u8, 0x04, b'h', b'o', b's', b't',
        0x02, 0x04, 192, 168, 1, 1,
        0x03, 0x02, 0x00, 80,
    ];

    // Iterácia — lazy, bez alokácie
    for field in TlvIterator::new(&data) {
        println!("type={:#04X} value={:?}", field.typ, field.value);
    }

    // Filter konkrétneho typu
    let ports: Vec<u16> = TlvIterator::new(&data)
        .filter(|f| f.typ == 0x03)
        .filter_map(|f| f.value.try_into().ok())
        .map(u16::from_be_bytes)
        .collect();

    println!("porty: {:?}", ports);  // [80]
}
```

Tento iterator je zero-copy — `value: &'a [u8]` je priama referencia do pôvodného buffra. Implementácia `Iterator` nám dáva zadarmo všetky iterator metódy — `filter`, `filter_map`, `map`, `collect`. Môžeme teraz písať expresívny kód, ktorý parsuje protokol bez jedinej zbytočnej alokácie.

Porovnaj toto s C verziou, kde by si musel manuálne iterovať cez buffer, spravovať index, a každú filtračnú operáciu písať ako samostatný cyklus.

---

## Blanket implementations

Trait môžeš implementovať pre *všetky* typy spĺňajúce podmienku. Toto je veľmi mocná feature, ktorá nemá priamu analógiu v C++ ani v iných jazykoch:

```rust
trait PrettyPrint {
    fn pretty(&self) -> String;
}

// Implementácia pre všetky typy ktoré implementujú Display
impl<T: std::fmt::Display> PrettyPrint for T {
    fn pretty(&self) -> String {
        format!(">>> {} <<<", self)
    }
}

fn main() {
    println!("{}", 42u32.pretty());       // >>> 42 <<<
    println!("{}", "hello".pretty());     // >>> hello <<<
    println!("{}", 3.14f64.pretty());     // >>> 3.14 <<<
}
```

Stdlib to používa napríklad v `impl<T: Display> ToString for T` — každý typ s `Display` automaticky dostane `.to_string()`. Bez blanket implementations by to museli explicitne implementovať pre každý primitívny typ, každý custom typ, a každý typ v každej externej knižnici.

### Orphan rule — prečo nemôžeš implementovať čokoľvek kdekoľvek

Blanket implementations majú jedno dôležité obmedzenie — *orphan rule*. Nemôžeš implementovať cudzie trait na cudzom type:

```rust
// NEFUNGUJE: Display je z std, String je z std, obe sú "cudzie"
impl std::fmt::Display for String { ... }  // error[E0117]

// FUNGUJE: vlastný trait na cudzom type
impl PrettyPrint for String { ... }

// FUNGUJE: cudzie trait na vlastnom type
impl std::fmt::Display for MacAddr { ... }
```

Pravidlo je: aspoň jeden z (trait, typ) musí byť definovaný v tvojom crate. Toto zabraňuje konfliktom — ak by dve knižnice mohli implementovať rovnaký trait pre rovnaký typ, Rust by nevedel, ktorú implementáciu použiť.

---

## Trait bounds a where klauzuly

Keď píšeš generické funkcie, môžeš vyžadovať, aby generický typ implementoval určité traity:

```rust
use std::fmt::{Debug, Display};

// Inline bounds
fn log_value<T: Debug + Display>(value: &T) {
    println!("display: {}", value);
    println!("debug:   {:?}", value);
}

// where klauzula — pre dlhšie bounds
fn process<T, E>(result: Result<T, E>)
where
    T: Debug + Display,
    E: Debug + std::error::Error,
{
    match result {
        Ok(v) => println!("OK: {}", v),
        Err(e) => eprintln!("ERR: {}", e),
    }
}
```

`where` klauzula je len iná syntax pre rovnakú vec — použij ju keď máš veľa bounds a inline zápis by bol príliš dlhý. Obe sú sémanticky ekvivalentné.

### Trait bounds ako dokumentácia

Bounds nie sú len pre kompilátor — sú aj pre čitateľov kódu. Keď vidíš `fn send<T: Serialize + Send>(value: T)`, hneď vieš: táto funkcia potrebuje niečo, čo sa dá serializovať a bezpečne poslať medzi vláknami. To je oveľa expresívnejšie než C komentár `// value must be serializable`.

### Podmienené implementácie

Môžeš implementovať trait podmienene — len pre typy, ktoré spĺňajú ďalšie bounds:

```rust
use std::fmt::Display;

struct Wrapper<T>(T);

// Implementuj Display pre Wrapper<T> len ak T implementuje Display
impl<T: Display> Display for Wrapper<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Wrapper({})", self.0)
    }
}

fn main() {
    let w = Wrapper(42u32);
    println!("{}", w);  // ok — u32 implementuje Display

    // let w2 = Wrapper(vec![1u8]);
    // println!("{}", w2);  // error — Vec<u8> neimplementuje Display
}
```

Toto je niečo, čo v C++ template metaprogrammingom dosiahnuť môžeš, ale s omnoho komplikovanejšou syntaxou cez `std::enable_if` alebo `if constexpr`.

---

## Bežné chyby začiatočníkov

### Zabudnúť na `Sized` bound pri dyn Trait

```rust
// Toto nefunguje:
fn process(checker: dyn Checksum) { ... }  // error: dyn Checksum is not Sized

// Musíš použiť referenciu alebo Box:
fn process(checker: &dyn Checksum) { ... }  // ok
fn process(checker: Box<dyn Checksum>) { ... }  // ok
```

`dyn Trait` je *unsized type* — kompilátor nevie za compile time, aká veľká je hodnota, pretože to závisí od konkrétneho typu za runtime. Preto musíš vždy použiť pointer (`&`, `Box`, `Arc`, `Rc`) keď pracuješ s trait objects.

### Object safety

Nie každý trait môže byť použitý ako `dyn Trait`. Trait musí byť *object-safe*:

```rust
// NEFUNGUJE ako dyn Trait — má metódu s generickým parametrom
trait NotObjectSafe {
    fn clone_into<T>(&self) -> T;  // generická metóda → nie object-safe
}

// FUNGUJE ako dyn Trait
trait ObjectSafe {
    fn compute(&self) -> u32;  // konkrétna signatúra
    fn verify(&self, expected: u32) -> bool;
}

// Toto spôsobí chybu:
// let x: Box<dyn NotObjectSafe> = ...;  // error: the trait cannot be made into an object
```

Pravidlo je, že metódy musí byť možné volať cez fat pointer bez znalosti konkrétneho typu — teda žiadne generické metódy, žiadne metódy, ktoré vracajú `Self`.

### Implementácia trait pre externý typ cez newtype

Čo ak chceš implementovať `Display` pre `Vec<u8>`, ale orphan rule to nedovolí? Newtype pattern to rieši:

```rust
struct HexBytes(Vec<u8>);

impl std::fmt::Display for HexBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, byte) in self.0.iter().enumerate() {
            if i > 0 { write!(f, " ")?; }
            write!(f, "{:02X}", byte)?;
        }
        Ok(())
    }
}

fn main() {
    let data = HexBytes(vec![0xDE, 0xAD, 0xBE, 0xEF]);
    println!("{}", data);  // DE AD BE EF
}
```

Newtype je minimálny wrapper — `HexBytes(Vec<u8>)`. Je to vlastný typ, takže môžeš implementovať akýkoľvek trait pre neho. Pri potrebe pristúpiť k vnútorným dátam použiješ `.0`.

---

## Associated types — keď generické typy nie sú dosť

Niekedy chceš, aby trait sám určoval súvisiace typy. Napríklad `Iterator` má `Item` — typ hodnôt, ktoré produkuje:

```rust
trait Parser {
    type Output;    // associated type — každá implementácia definuje vlastný typ
    type Error;

    fn parse(&self, input: &str) -> Result<Self::Output, Self::Error>;
}

struct JsonParser;
struct CsvParser;

impl Parser for JsonParser {
    type Output = serde_json::Value;  // (ilustratívne)
    type Error = String;

    fn parse(&self, input: &str) -> Result<Self::Output, Self::Error> {
        todo!()
    }
}

impl Parser for CsvParser {
    type Output = Vec<Vec<String>>;
    type Error = String;

    fn parse(&self, input: &str) -> Result<Self::Output, Self::Error> {
        todo!()
    }
}
```

Associated types sú čistejšie než genericé parametre na traitoch keď chceš, aby každý typ mal *jednu* implementáciu traitu s konkrétnym output typom. Keby bol `Output` generický parameter na traitovi, mohol by `JsonParser` implementovať `Parser<String>` aj `Parser<Value>`, čo by bolo mätúce.

---

## Zhrnutie

| C++ | Rust |
|---|---|
| Virtual funkcie + vtable | `dyn Trait` |
| Templates | Generics `<T: Trait>` |
| Abstract class | Trait s default metódami |
| Operator overloading | `impl Add for T`, `impl Display for T` |
| SFINAE | Trait bounds |
| Multiple inheritance | Multiple trait bounds (`T: A + B + C`) |

Traits sú srdcom Rustovho type systemu. Všetko zaujímavé v štandardnej knižnici je postavené na traitoch — `Iterator`, `Display`, `From`, `Into`, `Error`, `Send`, `Sync`, `Clone`, `Copy`. Keď pochopíš, ako traity fungujú, začne ti dávať zmysel väčšina kódu v ekosystéme.

---

## Vizuálny príklad — Monomorphization vs Dynamic Dispatch

    cargo run --bin k05_traits

Demo ukazuje kľúčový rozdiel medzi `impl Trait` (generics) a `dyn Trait` v pamäti a pri kompilácii.

**Ľavá kolónka — Monomorphization**: keď stlačíš SPACE, vidíš ako kompilátor *generuje* samostatnú funkciu pre každý konkrétny typ — `print_area_Circle`, `print_area_Square`, `print_area_Triangle`. Nulový overhead za abstrakciu — volanie je priame.

**Pravá kolónka — vtable**: diagram ukazuje fat pointer (`data ptr` + `vtable ptr`) a samotnú vtable s ukazovateľmi na metódy. Pri runtime lookupe musíš skočiť cez vtable — jeden extra memory fetch.

`TAB` prepína focus medzi kolónkami. `SPACE` animuje aktuálnu kolónku krok po kroku.

Otázka na zamyslenie: kedy je vtable *lepšia* voľba? (Tip: `Vec<Box<dyn Shape>>` — nemôžeš mať heterogénnu kolekciu s monomorphizáciou.)

Ovládanie: `TAB` = prepnúť kolónku, `SPACE` = ďalší krok animácie, `Q` = koniec.

Ďalšia kapitola: Lifetimes — formálny jazyk pre "ako dlho žije referencia". Videli sme ich náznak v TLV iterátore s `'a` anotáciou. Teraz si vysvetlíme, čo to presne znamená, prečo to kompilátor potrebuje vedieť, a ako myslieť o životnosti referencií bez toho, aby si sa zbláznil.
