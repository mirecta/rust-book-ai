# Kapitola 8 — Closures & Iterators

Keď väčšina ľudí počuje "funkcionálne programovanie", pomyslia na Haskell, na pomalé abstrakcie, na akademické koncepty vzdialené od reálneho kódu. Rust dokazuje, že to tak nemusí byť. Closures a iterátory v Ruste nie sú len syntaktický cukor — kompilujú sa na presne rovnaký strojový kód ako ručne napísané for slučky. Máš výraznosť vysokoúrovňového jazyka a výkon C. Zero-cost abstractions nie je marketing, je to merateľný fakt.

Rust má funkcionálny toolkit prvej triedy. Closures, lazy iterators, pipeline operácie — a všetko sa kompiluje na rovnakú rýchlosť ako ručne napísané slučky.

---

## Closures — prečo existujú a čo sú

Closure je anonymná funkcia, ktorá môže "zavrieť" (capture) premenné z okolitého scope-u. Keby sme mali len regulárne funkcie, nemohli by sme jednoducho odovzdať "kus správania" inej funkcii spolu s dátami, s ktorými pracuje. Museli by sme buď použiť globálne premenné, alebo vymyslieť struct s metódou — čo je presne to, čo Rust robí pod kapotou pre closures.

```rust
fn main() {
    let threshold = 100u32;

    // Closure — capture threshold by reference
    let is_over = |x: u32| x > threshold;

    println!("{}", is_over(50));   // false
    println!("{}", is_over(200));  // true

    // Ako callback
    let values = vec![10u32, 200, 50, 150, 30];
    let over: Vec<u32> = values.iter()
        .filter(|&&x| x > threshold)
        .copied()
        .collect();
    println!("{:?}", over);  // [200, 150]
}
```

V C by si to riešil cez function pointer a void* pre kontext — klasický callback pattern. Je to funkčné, ale nepohodlné a type-unsafe:

```c
// C: callback s kontextom — boilerplate a void* madness
typedef int (*FilterFn)(uint32_t value, void* ctx);

int filter_values(uint32_t* arr, size_t len, uint32_t* out,
                  FilterFn fn, void* ctx) {
    int count = 0;
    for (size_t i = 0; i < len; i++) {
        if (fn(arr[i], ctx)) {
            out[count++] = arr[i];
        }
    }
    return count;
}

int is_over_threshold(uint32_t val, void* ctx) {
    uint32_t threshold = *(uint32_t*)ctx;  // void* cast — žiadna typová bezpečnosť
    return val > threshold;
}

// Volanie:
uint32_t threshold = 100;
filter_values(values, n, out, is_over_threshold, &threshold);
```

Rust closure robí presne toto, ale automaticky, type-safe a bez runtime overhead.

---

## Pod kapotou — čo je closure v pamäti

Každá closure v Ruste je implementovaná ako anonymný struct. Kompilátor vygeneruje unikátny typ pre každú closure, ktorý obsahuje všetky capture-ované premenné ako polia:

```rust
let threshold = 100u32;
let multiplier = 2u32;
let transform = |x: u32| x * multiplier + threshold;
```

Kompilátor to preloží zhruba na:

```rust
// Toto generuje kompilátor — ty to nevidíš, ale existuje
struct __Closure_transform {
    threshold: u32,   // capture by copy (Copy typ)
    multiplier: u32,  // capture by copy (Copy typ)
}

impl Fn(u32) -> u32 for __Closure_transform {
    fn call(&self, x: u32) -> u32 {
        x * self.multiplier + self.threshold
    }
}
```

Každá closure má teda **unikátny typ** — to je dôvod prečo nemôžeš uložiť dve rôzne closures do jedného `Vec` bez type erasure (`Box<dyn Fn(...)>`). Ale je to aj dôvod prečo monomorphization funguje — kompilátor vie presne aký kód generovať pre každú closure.

### Monomorphization — nulový overhead v praxi

Keď napíšeš generickú funkciu s `F: Fn(u32) -> u32`, kompilátor vygeneruje specializovanú verziu pre každú konkrétnu closure:

```rust
fn apply_twice<F: Fn(u32) -> u32>(f: F, x: u32) -> u32 {
    f(f(x))
}

apply_twice(|x| x * 2, 3);   // generuje: apply_twice__double(3)
apply_twice(|x| x + 10, 3);  // generuje: apply_twice__add10(3)
```

Žiadny virtual dispatch, žiadny runtime lookup. Každá verzia je plne inline-ovateľná. Toto sa volá monomorphization — jeden generický kód, viacero specializovaných implementácií.

Porovnaj s C++ templates kde je situácia identická, ale Rust má navyše jasné trait bounds (`Fn`, `FnMut`, `FnOnce`) ktoré presne hovoria aké vlastnosti closure musí mať.

---

## Fn, FnMut, FnOnce — tri traity a prečo ich treba

Prečo sú tri? Kvôli ownership a borrowing model. Capture-ovanie premenných ma rôzne formy — by reference, by mutable reference, alebo by value (move). Podľa toho aké capture-ovanie closure robí, implementuje iný trait:

```rust
// Fn — capture by reference, môže sa volať viackrát
// Kompilátor si drží &threshold
let x = 42;
let read = || println!("{}", x);  // &x — immutable borrow
read(); read();  // ok — x stále žije, borrow trvá

// FnMut — capture by &mut, môže sa volať viackrát
// Každé volanie zmení zachytený stav
let mut count = 0;
let mut increment = || { count += 1; count };  // &mut count
println!("{}", increment());  // 1
println!("{}", increment());  // 2
// println!("{}", count);  // error — count je stále mutably borrowed cez closure

// FnOnce — move capture, môže sa zavolať len raz
// name sa presunie do closure a closure ho vlastní
let name = String::from("hello");
let consume = move || {
    println!("{}", name);
    drop(name);  // name sa prenesie do closure — po drop() neexistuje
};
consume();
// consume();  // error — FnOnce closure môže byť zavolaná len raz
```

Dôležité: každý `Fn` je aj `FnMut`, každý `FnMut` je aj `FnOnce`. Je to hierarchia — `FnOnce` je najmenej obmedzený (vyžaduje len to, že closure sa dá zavolať aspoň raz), `Fn` je najobmedzujúcejší (vyžaduje, že closure sa dá zavolať opakovane bez zmeny stavu).

Keď píšeš funkciu, ber ten najmenej obmedzujúci trait, ktorý potrebuješ: ak stačí `Fn`, nepíš `FnMut`. Ak stačí `FnMut`, nepíš `FnOnce` (hoci `FnOnce` by fungovalo, bol by si menej flexibilný vo volaní).

---

## `move` closure — prenos ownership-u

`move` keyword pred closure vynúti capture by value pre všetky premenné, aj keď by closure inak zachytila by reference:

```rust
let data = vec![1u32, 2, 3];

let handle = std::thread::spawn(move || {
    // data sa presunulo do threadu — caller ho viac nemá
    println!("{:?}", data);
});
handle.join().unwrap();

// println!("{:?}", data);  // error — data bolo moved do threadu
```

Toto je nutné pre threading, lebo nový thread môže prežiť caller frame (viz kapitola o lifetime-och). `move` garantuje, že closure je `'static` — neobsahuje referencie na stack caller-a.

Dá sa to ale použiť aj mimo threadov, napríklad keď chceš closure ktorá vlastní dáta a môže existovať nezávisle:

```rust
fn make_adder(n: u32) -> impl Fn(u32) -> u32 {
    move |x| x + n  // n sa presunulo do closure — closure vlastní n
    // Bez move by sme vrátili referenciu na n, ktoré zanikne na konci funkcie
}

fn main() {
    let add5 = make_adder(5);
    let add10 = make_adder(10);
    println!("{}", add5(3));   // 8
    println!("{}", add10(3));  // 13
}
```

`make_adder` vráti closure ktorá vlastní kópiu `n`. `impl Fn(u32) -> u32` je "existuje nejaký konkrétny typ ktorý implementuje Fn(u32) -> u32" — kompilátor vie aký typ to je, ale caller nie.

---

## Closure ako parameter a návratový typ

```rust
// Generická funkcia — F je akákoľvek closure Fn(u32) -> u32
fn apply_twice<F: Fn(u32) -> u32>(f: F, x: u32) -> u32 {
    f(f(x))
}

// FnMut — closure môže meniť stav
fn apply_n<F: FnMut(u32) -> u32>(mut f: F, n: usize, x: u32) -> u32 {
    let mut val = x;
    for _ in 0..n {
        val = f(val);
    }
    val
}

fn main() {
    println!("{}", apply_twice(|x| x * 2, 3));   // 12
    println!("{}", apply_n(|x| x + 1, 5, 10));   // 15

    // Closure s mutable stavom
    let mut calls = 0;
    let counting_double = |x: u32| { calls += 1; x * 2 };
    println!("{}", apply_n(counting_double, 3, 5));  // 40
    // calls je teraz 3 — ale nemôžeme prečítať, lebo counting_double stále
    // drží &mut calls cez FnMut
}
```

### `dyn Fn` vs `impl Fn` — dynamic vs static dispatch

Keď typ closure neznáme v compile time (napr. keď ukladáš closures do Vec alebo ich vraciaš z funkcií podľa runtime podmienky), musíš použiť trait object:

```rust
// impl Fn — statický dispatch, monomorphization, nulový overhead
// Ale môže vrátiť len jeden konkrétny typ
fn make_doubler() -> impl Fn(u32) -> u32 {
    |x| x * 2
}

// Box<dyn Fn> — dynamický dispatch, vtable, heap allocation
// Môže vrátiť rôzne closure typy podľa runtime podmienky
fn make_transformer(double: bool) -> Box<dyn Fn(u32) -> u32> {
    if double {
        Box::new(|x| x * 2)
    } else {
        Box::new(|x| x + 10)
    }
}

// Vec s rôznymi closure typmi — nutne dyn
let operations: Vec<Box<dyn Fn(u32) -> u32>> = vec![
    Box::new(|x| x * 2),
    Box::new(|x| x + 1),
    Box::new(|x| x.pow(2)),
];

let result = operations.iter().fold(3u32, |acc, f| f(acc));
// 3 * 2 = 6, 6 + 1 = 7, 7^2 = 49
```

`dyn Fn` používa vtable (tabuľka virtuálnych funkcií) — rovnaký mechanizmus ako virtual funkcie v C++. Je to runtime lookup, malý overhead, ale umožňuje heterogénne kolekcie callbackov.

---

## Iterator — lazy evaluation a pipeline

Iterátory v Ruste sú lazy — nič sa nevykoná kým nepotrebuješ výsledok. Každý adaptér (`.filter()`, `.map()`, atď.) vracia nový typ iterátora, nie kolekciu. Celý chain je jeden veľký struct, ktorý sa vyhodnotí element po elemente keď zavoláš "konzumujúcu" operáciu (`.collect()`, `.sum()`, `.for_each()`, atď.).

```rust
fn main() {
    let v = vec![1u32, 2, 3, 4, 5, 6, 7, 8, 9, 10];

    // Toto nevykoná NIČ ešte — pipeline je len popis
    let pipeline = v.iter()
        .filter(|&&x| x % 2 == 0)   // lazy — vracia Filter<...>
        .map(|&x| x * x);            // lazy — vracia Map<Filter<...>, ...>

    // Až .collect() spustí celý pipeline:
    // Pre každý prvok: filter → ak prešiel, map → pridaj do Vec
    // Žiadne medzivýsledky, žiadne dočasné kolekcie
    let result: Vec<u32> = pipeline.collect();
    println!("{:?}", result);  // [4, 16, 36, 64, 100]
}
```

Vs C ekvivalent (kompiluje sa na to isté):

```c
uint32_t result[10]; int n = 0;
for (int i = 0; i < 10; i++) {
    if (v[i] % 2 == 0) result[n++] = v[i] * v[i];
}
```

Takýto kód Rust vygeneruje v podstate identický assembler. Žiadna abstrakcia bez nákladov — abstrakcia BEZ nákladov.

### Prečo je lazy evaluation dôležité

Lazy evaluation umožňuje pracovať s nekonečnými sekvensiami a zastavuje spracovanie hneď keď výsledok nie je potrebný:

```rust
// Nekonečná sekvencia — funguje lebo je lazy
let first_even_square_over_100: u32 = (1u32..)  // 1, 2, 3, 4, ...
    .map(|x| x * x)           // 1, 4, 9, 16, ...
    .filter(|&x| x % 2 == 0) // 4, 16, 36, ...
    .find(|&x| x > 100)       // zastaví pri prvom výsledku
    .unwrap();

println!("{}", first_even_square_over_100);  // 144 (12^2)
// Bez lazy by si alokoval nekonečný Vec — crash
```

`find()` zastaví pipeline hneď keď nájde prvý vyhovujúci prvok. `take(n)` obmedzí počet prvkov. `take_while(pred)` berie prvky kým predikát platí. Toto sú "short-circuit" operácie.

---

## Iterator trait — ako ho implementovať

Za každým iterátorom stojí `Iterator` trait s jednou povinnou metódou:

```rust
trait Iterator {
    type Item;
    fn next(&mut self) -> Option<Self::Item>;
    // všetky ostatné metódy sú default implementácie nad next()
}
```

To je celé. Stovky metód (`.map()`, `.filter()`, `.fold()`, ...) sú default implementácie nad `next()`. Ak implementuješ `next()`, dostaneš všetko ostatné zadarmo.

```rust
// Vlastný iterátor — fibonacci sekvencia
struct Fibonacci {
    a: u64,
    b: u64,
}

impl Fibonacci {
    fn new() -> Self { Fibonacci { a: 0, b: 1 } }
}

impl Iterator for Fibonacci {
    type Item = u64;

    fn next(&mut self) -> Option<u64> {
        let result = self.a;
        let new_b = self.a + self.b;
        self.a = self.b;
        self.b = new_b;
        Some(result)  // nekonečná sekvencia — nikdy None
    }
}

fn main() {
    // Prvých 10 Fibonacci čísel:
    let fibs: Vec<u64> = Fibonacci::new().take(10).collect();
    println!("{:?}", fibs);  // [0, 1, 1, 2, 3, 5, 8, 13, 21, 34]

    // Prvé Fibonacci číslo väčšie ako milión:
    let big = Fibonacci::new().find(|&x| x > 1_000_000).unwrap();
    println!("{}", big);  // 1346269
}
```

---

## Základné adaptéry — referenčný prehľad

```rust
let nums = vec![1i32, -2, 3, -4, 5, -6];

// map — transformácia každého prvku
let doubled: Vec<i32> = nums.iter().map(|&x| x * 2).collect();

// filter — filtrovanie predikátom
let positive: Vec<i32> = nums.iter().copied().filter(|&x| x > 0).collect();

// filter_map — filter + map naraz (None = preskočiť, Some(v) = použiť v)
let abs_positive: Vec<u32> = nums.iter()
    .filter_map(|&x| if x > 0 { Some(x as u32) } else { None })
    .collect();

// fold — akumulácia s počiatočnou hodnotou
let sum: i32 = nums.iter().copied().fold(0, |acc, x| acc + x);

// sum, product, min, max — špecializované
let total: i32 = nums.iter().sum();
let max = nums.iter().max().unwrap();  // None ak prázdny Vec

// any, all — short-circuit (zastaví pri prvom výsledku)
let has_negative = nums.iter().any(|&x| x < 0);   // true, zastaví pri -2
let all_positive = nums.iter().all(|&x| x > 0);   // false, zastaví pri -2

// zip — páruje prvky dvoch iterátorov
let a = [1u32, 2, 3];
let b = ["a", "b", "c"];
let pairs: Vec<_> = a.iter().zip(b.iter()).collect();
// [(1, "a"), (2, "b"), (3, "c")]
// Zastaví pri kratšom — ak má a 3 prvky a b 2, výsledok má 2 páry

// enumerate — pridá index
for (i, val) in nums.iter().enumerate() {
    println!("{}: {}", i, val);
}

// take / skip — obmedzenie počtu
let first_three: Vec<_> = nums.iter().take(3).collect();
let after_two: Vec<_> = nums.iter().skip(2).collect();

// take_while / skip_while — podmienené
let until_negative: Vec<_> = nums.iter()
    .take_while(|&&x| x > 0)
    .collect();  // [1] — zastaví pri -2

// chain — zreťazenie iterátorov
let combined: Vec<_> = [1, 2].iter().chain([3, 4].iter()).collect();

// flat_map — map + flatten (každý prvok sa rozbalí na sekvenciu)
let words = vec!["hello world", "foo bar"];
let tokens: Vec<&str> = words.iter()
    .flat_map(|s| s.split(' '))
    .collect();
// ["hello", "world", "foo", "bar"]

// flatten — rozbalí vnorené iterátory
let nested = vec![vec![1, 2], vec![3, 4], vec![5]];
let flat: Vec<i32> = nested.into_iter().flatten().collect();
// [1, 2, 3, 4, 5]

// peekable — umožní pozrieť na ďalší prvok bez konzumovania
let mut it = nums.iter().peekable();
while let Some(&&next) = it.peek() {
    if next < 0 { break; }
    println!("{}", it.next().unwrap());
}
```

---

## collect() do rôznych kolekcií

`collect()` je generický — vie vytvoriť akúkoľvek kolekciu ktorá implementuje `FromIterator`. Musíš povedať kompilátor aký typ chceš, buď anotáciou premennej alebo turbofish syntaxou:

```rust
use std::collections::{HashMap, HashSet, BTreeMap};

let pairs = vec![("a", 1u32), ("b", 2), ("a", 3)];

// Vec
let v: Vec<_> = pairs.iter().map(|&(k, v)| (k, v * 2)).collect();

// HashMap — pri duplicitách posledná hodnota vyhráva
let map: HashMap<&str, u32> = pairs.into_iter().collect();

// HashSet — automaticky deduplikuje
let nums = vec![1u32, 2, 2, 3, 3, 3];
let unique: HashSet<u32> = nums.into_iter().collect();

// BTreeMap — sortované kľúče (deterministic order)
let sorted: BTreeMap<&str, u32> = [("c", 3), ("a", 1), ("b", 2)]
    .iter().copied().collect();

// String zo znakov alebo &str
let chars = vec!['R', 'u', 's', 't'];
let s: String = chars.into_iter().collect();  // "Rust"

let parts = vec!["hello", " ", "world"];
let joined: String = parts.into_iter().collect();  // "hello world"

// Result z iterátora — ak akýkoľvek prvok je Err, výsledok je Err
let strings = vec!["1", "2", "abc", "4"];
let numbers: Result<Vec<u32>, _> = strings.iter()
    .map(|s| s.parse::<u32>())
    .collect();
// Err(ParseIntError) — zastaví pri "abc"

// Turbofish syntax — ak nechceš anotovať premennú
let numbers = strings.iter()
    .filter_map(|s| s.parse::<u32>().ok())
    .collect::<Vec<u32>>();
// [1, 2, 4] — preskočí chyby
```

---

## Ranges a windows — práca s dátovými tokmi

```rust
fn main() {
    // Range ako iterátor
    let sum: u64 = (1u64..=100).sum();  // Gauss: 5050

    // Step_by — každý n-tý prvok
    let evens: Vec<u64> = (0u64..20).step_by(2).collect();
    // [0, 2, 4, 6, 8, 10, 12, 14, 16, 18]

    // chunks — spracuj po N prvkoch (last chunk môže byť menší)
    let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
    for chunk in data.chunks(3) {
        println!("{:?}", chunk);
    }
    // [1, 2, 3]
    // [4, 5, 6]
    // [7, 8]

    // chunks_exact — len plné chunks (zvyšok je dostupný cez .remainder())
    let mut chunks = data.chunks_exact(3);
    for chunk in chunks.by_ref() {
        println!("plný chunk: {:?}", chunk);
    }
    println!("zvyšok: {:?}", chunks.remainder());

    // windows — sliding window (každé dve susedné hodnoty, atď.)
    for window in data.windows(3) {
        let avg: f32 = window.iter().map(|&x| x as f32).sum::<f32>() / 3.0;
        println!("{:?} avg={:.1}", window, avg);
    }
    // [1, 2, 3] avg=2.0
    // [2, 3, 4] avg=3.0
    // ...

    // split_at / split_first / split_last
    let (left, right) = data.split_at(4);
    println!("ľavá: {:?}, pravá: {:?}", left, right);
}
```

`windows` je obzvlášť užitočné v signálovom spracovaní, kĺzavých priemeroch, alebo keď potrebuješ pozerať na kontextové prvky. V C by si to riešil s pointerovou aritmetikou a môžeš ľahko ísť mimo hranice. `windows()` garantuje že každé okno má presne N prvkov.

---

## Praktický príklad: spracovanie sieťového logu

Nasledujúci príklad ukazuje ako kombinovať iterátory na spracovanie reálnych dát — log súbor sieťového prístupu. Je to typická dátová pipeline: parsovanie, filtrovanie, agregácia, triedenie, výstup.

```rust
use std::collections::HashMap;

#[derive(Debug)]
struct LogEntry {
    timestamp: u64,
    src_ip: String,
    dst_port: u16,
    bytes: usize,
    status: u16,
}

fn parse_line(line: &str) -> Option<LogEntry> {
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() != 5 { return None; }

    Some(LogEntry {
        timestamp: parts[0].trim().parse().ok()?,
        src_ip: parts[1].trim().to_string(),
        dst_port: parts[2].trim().parse().ok()?,
        bytes: parts[3].trim().parse().ok()?,
        status: parts[4].trim().parse().ok()?,
    })
}

fn analyze(log: &str) {
    // Parsovanie a filtrovanie v jednom pipeline
    let entries: Vec<LogEntry> = log.lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .filter_map(parse_line)
        .collect();

    println!("Celkom záznamov: {}", entries.len());

    // Celkový traffic — sum() je fold s 0 a +
    let total_bytes: usize = entries.iter().map(|e| e.bytes).sum();
    println!("Celkový traffic: {} bytes", total_bytes);

    // Errory (4xx, 5xx) — filter + count
    let error_count = entries.iter().filter(|e| e.status >= 400).count();
    println!("Chyby: {} ({:.1}%)", error_count,
             error_count as f64 / entries.len() as f64 * 100.0);

    // Top IP adresy podľa trafficu — fold do HashMap
    let ip_traffic: HashMap<&str, usize> = entries.iter()
        .fold(HashMap::new(), |mut acc, e| {
            *acc.entry(e.src_ip.as_str()).or_insert(0) += e.bytes;
            acc
        });

    // Sortovanie podľa trafficu
    let mut top_ips: Vec<(&str, usize)> = ip_traffic.into_iter().collect();
    top_ips.sort_by(|a, b| b.1.cmp(&a.1));  // descending

    println!("\nTop 3 IP adresy:");
    for (ip, bytes) in top_ips.iter().take(3) {
        println!("  {}: {} bytes", ip, bytes);
    }

    // Štatistika portov — fold do HashMap, potom BTreeMap pre sortovanie
    let port_dist: HashMap<u16, usize> = entries.iter()
        .fold(HashMap::new(), |mut acc, e| {
            *acc.entry(e.dst_port).or_insert(0) += 1;
            acc
        });

    println!("\nPorty:");
    let mut ports: Vec<_> = port_dist.iter().collect();
    ports.sort_by_key(|(port, _)| *port);
    for (port, count) in &ports {
        println!("  :{} — {} requestov", port, count);
    }

    // Percentilné štatistiky bytov — sort + index
    let mut byte_sizes: Vec<usize> = entries.iter().map(|e| e.bytes).collect();
    byte_sizes.sort_unstable();
    if !byte_sizes.is_empty() {
        let p50 = byte_sizes[byte_sizes.len() / 2];
        let p95 = byte_sizes[byte_sizes.len() * 95 / 100];
        println!("\nVeľkosť requestov: p50={} bytes, p95={} bytes", p50, p95);
    }
}

fn main() {
    let log = "\
# Network access log
1000,192.168.1.10,80,1024,200
1001,192.168.1.20,443,2048,200
1002,192.168.1.10,80,512,404
1003,10.0.0.5,22,256,200
1004,192.168.1.20,80,4096,200
1005,10.0.0.5,80,128,500
1006,192.168.1.10,443,8192,200
1007,10.0.0.5,80,64,403
";

    analyze(log);
}
```

Celá analýza je deklaratívna — hovoríš *čo* chceš, nie *ako* to spraviť. Žiadne manuálne indexy, žiadne off-by-one chyby, žiadne zabudnuté incrementy.

---

## Iterator vs for slučka — výkon a genrovaný kód

Toto je otázka ktorú každý položí: "nie je to pomalšie ako for loop?" Nie. V release build-e generuje kompilátor identický alebo lepší kód:

```rust
// Tieto tri sú prakticky identické v release build:

// 1. Iterátor
let sum1: u64 = (0u64..1_000_000).filter(|x| x % 2 == 0).sum();

// 2. for slučka
let mut sum2 = 0u64;
for i in 0u64..1_000_000 {
    if i % 2 == 0 { sum2 += i; }
}

// 3. while loop
let mut sum3 = 0u64;
let mut i = 0u64;
while i < 1_000_000 {
    if i % 2 == 0 { sum3 += i; }
    i += 1;
}
```

Generovaný assembler je rovnaký — a v mnohých prípadoch iterátor verzia je lepšia, lebo LLVM má viac informácií na optimalizáciu (bounds check elimination, auto-vectorization s SIMD).

Navyše, iterátor verzia nemôže mať off-by-one chybu. Nemôže zabudnúť incrementovať counter. Nemôže ísť mimo hranice. Tieto garancie sú zadarmo — nulový výkonový cost.

### SIMD a auto-vectorization

Keď Rust kompilátor (cez LLVM) vidí iterátor nad číslami, môže automaticky použiť SIMD inštrukcie:

```rust
// Toto sa môže skompilovať na SIMD instrukcie (SSE/AVX)
// keď kompilátorom povolíš target-feature=+avx2
let v: Vec<f32> = (0..1000).map(|x| x as f32).collect();
let sum: f32 = v.iter().sum();
// LLVM môže procesovať 8 f32 naraz pomocou AVX2
```

S `for` loop by si musel SIMD implementovať ručne v unsafe kóde. Iterátory dávajú kompilátorou čistejší pohľad na zámer.

---

## Pokročilé patterns — scan, unzip, partition

```rust
// scan — ako fold, ale vracia medzivýsledky
// Kumulatívný súčet:
let data = vec![1i32, 2, 3, 4, 5];
let cumulative: Vec<i32> = data.iter()
    .scan(0, |acc, &x| { *acc += x; Some(*acc) })
    .collect();
println!("{:?}", cumulative);  // [1, 3, 6, 10, 15]

// partition — rozdeľ na dve kolekcie podľa predikátu
let (positive, negative): (Vec<i32>, Vec<i32>) = data.iter()
    .map(|&x| x - 3)
    .partition(|&x| x >= 0);
// positive: [0, 1, 2], negative: [-2, -1]

// unzip — rozdeľ Vec párov na dve kolekcie
let pairs = vec![(1u32, "a"), (2, "b"), (3, "c")];
let (nums, letters): (Vec<u32>, Vec<&str>) = pairs.into_iter().unzip();
// nums: [1, 2, 3], letters: ["a", "b", "c"]

// inspect — debugging pipeline (nezníži výkon v release)
let result: Vec<u32> = (1u32..=5)
    .inspect(|x| eprintln!("pred filter: {}", x))
    .filter(|x| x % 2 == 0)
    .inspect(|x| eprintln!("po filter: {}", x))
    .collect();

// cycle — nekonečné opakovanie
let pattern: Vec<u8> = [0xAA, 0xBB, 0xCC]
    .iter()
    .cycle()
    .take(9)
    .copied()
    .collect();
// [0xAA, 0xBB, 0xCC, 0xAA, 0xBB, 0xCC, 0xAA, 0xBB, 0xCC]
```

---

## Praktický príklad zo systémového programovania: packet parser

Iterátory sa výborne hodia na spracovanie binárnych protokolov — parsuješ sekvenciu bajtov, extrakuješ polia, validuješ:

```rust
#[derive(Debug)]
struct TlvRecord {
    tag: u8,
    value: Vec<u8>,
}

// TLV = Type-Length-Value — bežný binárny formát (Bluetooth, SNMP, X.509, ...)
fn parse_tlv(data: &[u8]) -> impl Iterator<Item = TlvRecord> + '_ {
    let mut pos = 0;
    std::iter::from_fn(move || {
        if pos + 2 > data.len() { return None; }
        let tag = data[pos];
        let len = data[pos + 1] as usize;
        pos += 2;
        if pos + len > data.len() { return None; }
        let value = data[pos..pos + len].to_vec();
        pos += len;
        Some(TlvRecord { tag, value })
    })
}

fn main() {
    // TLV encoded data: tag=0x01, len=3, data=[1,2,3]; tag=0x02, len=2, data=[4,5]
    let packet = [0x01u8, 0x03, 0x01, 0x02, 0x03, 0x02, 0x02, 0x04, 0x05];

    let records: Vec<_> = parse_tlv(&packet).collect();
    for r in &records {
        println!("tag=0x{:02X} value={:?}", r.tag, r.value);
    }

    // Nájdi konkrétny tag
    let config_tag = parse_tlv(&packet)
        .find(|r| r.tag == 0x02)
        .map(|r| r.value);
    println!("Tag 0x02: {:?}", config_tag);
}
```

`std::iter::from_fn` je way ako vytvoriť iterátor z closure — closure vracia `Some(item)` kým má dáta, potom `None`. Elegantné a bez boilerplate.

---

## Chyby začiatočníkov s closures a iterátormi

### Chyba 1: Zabudnutie na `.copied()` alebo `.cloned()`

```rust
let v = vec![1u32, 2, 3];

// Problém: .iter() dáva &u32, nie u32
let doubled: Vec<u32> = v.iter().map(|x| x * 2).collect();  // ok — deref coercion
// Ale toto nefunguje:
let sum: u32 = v.iter().sum();  // error: implementácia Sum pre &u32 → &u32, nie u32

// Riešenie:
let sum: u32 = v.iter().copied().sum();   // .copied() konvertuje &u32 → u32
let sum: u32 = v.iter().cloned().sum();   // .cloned() — rovnaké pre Copy typy
let sum: u32 = v.iter().sum::<u32>();     // turbofish — niekedy funguje

// Alebo použij into_iter() — konzumuje Vec, dáva u32 priamo
let sum: u32 = v.into_iter().sum();
// v tu už neexistuje
```

### Chyba 2: Mutable closure v immutable kontexte

```rust
// Problém: filter() berie &Self closure — Fn, nie FnMut
let mut count = 0;
// let result: Vec<_> = v.iter().filter(|_| { count += 1; true }).collect();
// error: cannot borrow `count` as mutable in a closure in an immutable position

// Riešenie: použi for loop alebo inspect() pre side-effects
let result: Vec<_> = v.iter()
    .inspect(|_| { /* count += 1 */ })  // inspect dáva &Item, nie mut
    .collect();

// Alebo enumerate a count oddelene:
let count = v.iter().filter(|&&x| x > 1).count();
```

### Chyba 3: Collect bez špecifikácie typu

```rust
// error: type annotations needed
let result = v.iter().map(|&x| x * 2).collect();
// Rust nevie aký typ chceš — Vec? HashSet? String?

// Riešenie A: anotácia premennej
let result: Vec<u32> = v.iter().map(|&x| x * 2).collect();

// Riešenie B: turbofish
let result = v.iter().map(|&x| x * 2).collect::<Vec<u32>>();

// Riešenie C: typ z kontextu
fn process(data: Vec<u32>) { /* ... */ }
process(v.iter().map(|&x| x * 2).collect());  // Rust vie že process čaká Vec<u32>
```

### Chyba 4: Prekonzumovanie iterátora

```rust
let v = vec![1u32, 2, 3];
let it = v.into_iter();  // v je konzumovaný

// Teraz chceš použiť it dvakrát:
let sum: u32 = it.sum();  // it je konzumovaný
// let max = it.max();    // error — it bol presunutý do sum()

// Riešenie: použi .iter() namiesto .into_iter() ak nepotrebuješ ownership
let v = vec![1u32, 2, 3];
let sum: u32 = v.iter().copied().sum();   // v stále žije
let max = v.iter().copied().max();         // ok
```

---

## Zhrnutie

| Koncept | Rust |
|---|---|
| Closure bez capture | `\|x\| x * 2` — anonymný struct bez polí |
| Closure s &capture | `Fn` — closure drží imm. referencie |
| Closure s &mut capture | `FnMut` — closure drží mut. referencie |
| Closure s move capture | `FnOnce` / `move \|\| ...` — closure vlastní hodnoty |
| Statický dispatch | `impl Fn(...)` — monomorphization, nulový overhead |
| Dynamický dispatch | `dyn Fn(...)` — vtable, heap alloc, heterogénne kolekcie |
| Lazy pipeline | `.filter().map().collect()` — zero medzivýsledkov |
| Zero overhead | Identický kód s for loop, LLVM auto-vectorization |
| Vlastný iterátor | Implementuj `Iterator` trait s `fn next()` |

Closures a iterátory v Ruste nie sú len estetická voľba — sú to nástroje, ktoré ťa ochránia pred celou triedou chýb (off-by-one, out of bounds, zabudnutý increment) a zároveň ti dajú kód ktorý je tak rýchly ako C. To je kombinácia ktorú v žiadnom inom jazyku nenájdeš.

Ďalšia kapitola: Concurrency — threads, channels, Mutex a tokio async runtime.

---

## Projekt — Conway's Game of Life

Closures a iterátory sú najlepšie vidieť na reálnom probléme. Conway's Game of Life je simulácia s jednoduchými pravidlami, ale fascinujúcim správaním — a v Ruste sa dá elegantne implementovať cez iterátory.

```bash
cargo run --bin game_of_life
```

Každá bunka na 80×60 griede je buď živá alebo mŕtva. V každom kroku platia štyri pravidlá:

- Živá bunka s menej ako 2 susedmi **umrie** (izolácia)
- Živá bunka s 2–3 susedmi **prežije**
- Živá bunka s viac ako 3 susedmi **umrie** (preplnenie)
- Mŕtva bunka s presne 3 susedmi **ožije** (reprodukcia)

Kľúčová časť implementácie — výpočet novej generácie cez iterátory:

```rust
fn step(&self) -> Grid {
    let cells = (0..ROWS).flat_map(|row| {
        (0..COLS).map(move |col| {
            let neighbors = self.count_neighbors(col as i32, row as i32);
            let alive = self.get(col as i32, row as i32);
            matches!((alive, neighbors), (true, 2) | (true, 3) | (false, 3))
        })
    }).collect();
    Grid { cells }
}
```

`flat_map` + `collect` — žiadna mutácia, žiadny indexing, žiadna šanca na off-by-one. Celá logika nové generácie v jednom výraze. Porovnaj s ekvivalentným C kódom: dva vnorené for cykly, `new_grid[i][j] = ...`, manuálne hraničné podmienky.

Ovládanie: `SPACE` = pauza/spusti, `N` = jeden krok ručne, `R` = náhodný reset, `LMB` = kliknutím prepni bunku.

---

## Projekt — Particle Physics

Ďalší demo ukazuje ako sa `Vec<T>` a iterátory kombinujú s update looopom — základným vzorom pre akúkoľvek simuláciu alebo hru.

```bash
cargo run --bin particles
```

Program udržiava `Vec<Particle>` kde každá častica má pozíciu, rýchlosť, farbu a polomer. Každý snímok:

```rust
// update — iter_mut pre mutable prístup
particles.iter_mut().for_each(|p| p.update(dt));

// draw — iter pre immutable prístup
particles.iter().for_each(|p| p.draw());
```

Týchto dvoch riadkov je celý render loop. Rust vynucuje správne vlastníctvo — `iter_mut()` a `iter()` nemôžeš zameniť ak by to spôsobilo problém. Drž ľavé tlačidlo myši a pridávaj nové bodky, `R` vyčistí všetky.

Tento vzor — `Vec<T>` s `iter_mut().for_each(update)` — je základ každého systému entít, od fyzikálnych simulácií po herné enginy.
