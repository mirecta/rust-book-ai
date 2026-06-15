# Kapitola 2 — Ownership & Borrowing ⚠️ THE THING

Toto je kapitola ktorá rozhoduje o všetkom. Ak pochopíš ownership, zvyšok Rustu plynie prirodzene. Ak nie — bojuješ s kompilátorom každý deň a pýtaš sa prečo si to vôbec začal.

Dobrá správa: ak vieš ako funguje stack a heap v C, máš základ. Rust len formalizuje pravidlá ktoré si doteraz musel dodržiavať hlavou. Tá "bolesť" s borrow checkerom v prvých týždňoch je presne tá bolesť keď si si v C uvedomil "aha, toto by bol dangling pointer" — len že Rust to povie pred spustením, nie po crashu.

Chcem tiež povedať narovinu: toto je najťažšia kapitola tejto knihy. Nie preto že by ownership bol komplikovaný matematicky — je to relatívne malá sada pravidiel. Je ťažká preto že si zvyknutý na iný mentálny model. V C myslíš na pointery a adresy. V Ruste musíš myslieť na *vlastníctvo* a *životnosť*. Je to rovnaká pamäť, rovnaký stack, rovnaký heap — len iná perspektíva. Po tom čo sa to klikne, zistíš že Rust vlastne opisuje to čo si aj v C mal robiť správne — len to teraz kontroluje stroj.

---

## Stack vs Heap v Ruste

Rovnaké ako v C — s jedným rozdielom: Rust *vie* kde čo leží. Nie za runtime, ale pri kompilácii. A táto znalosť je základ pre celý ownership systém.

```rust
fn main() {
    let x: u32 = 42;          // stack — Copy typ, 4 bajty
    let arr = [0u8; 1024];    // stack — 1KB, fixná veľkosť v compile time
    let v = vec![1u32, 2, 3]; // heap — Vec<u32>, pointer+len+capacity na stacku
    let s = String::from("hello"); // heap — String, rovnaká štruktúra ako Vec
}
// tu sa všetko automaticky uvoľní — RAII
```

Pozrime sa na `Vec<u32>` bližšie, pretože toto je kľúčové pre pochopenie ownership. Na stacku leží trojica — `(ptr: *mut u32, len: usize, cap: usize)`. Na 64-bit systéme sú to tri 8-bajtové hodnoty, čo je 24 bajtov na stacku. `ptr` ukazuje na heap buffer kde sú uložené samotné `u32` hodnoty. `len` hovorí koľko prvkov je inicializovaných, `cap` hovorí koľko miesta je alokované.

To je presne `struct { uint32_t *data; size_t len; size_t cap; }` v C. Žiadna mágia. Rovnaké rozloženie v pamäti.

`String` je identické — tiež trojica `(ptr, len, capacity)`, len `ptr` ukazuje na UTF-8 bytes namiesto `u32` hodnôt.

Čo Rust pridáva oproti C: kompilátor *sleduje* kde žijú tieto hodnoty a kto ich vlastní. Toto sledovanie prebieha pri compile time, má nulový runtime overhead, a je základ pre všetko čo nasleduje.

### Pod kapotou: čo sa stane keď funkcia skončí

```rust
fn example() {
    let s = String::from("hello");
    println!("{}", s);
} // <-- čo sa tu stane?
```

Keď kompilátor generuje kód pre koniec scope `}`, vie že `s` (tá trojica na stacku) vlastní heap buffer. Vloží volanie `drop_in_place::<String>` čo zavolá `dealloc` na heap buffer, a potom stackový frame sa uvoľní normálne (stack pointer sa len posunie). Toto volanie `dealloc` je deterministické — stane sa *presne* tu, nie neskôr, nie v náhodnom čase ako pri GC.

Ak si to pozrieš cez `cargo rustc -- --emit=asm | grep -A5 "example"`, uvidíš priame volanie `free` alebo `__rust_dealloc`. Nie GC paúza. Nie finalizer. Priame `free` na presnom mieste.

---

## Ownership — tri pravidlá

Celý ownership systém stojí na troch pravidlách. Sú jednoduché. Ich dôsledky sú hlboké.

1. Každá hodnota má presne jedného vlastníka (`owner`)
2. V danom čase môže existovať len jeden vlastník
3. Keď vlastník opustí scope, hodnota sa uvoľní (`drop`)

Zvyčajne keď ľudia prvýkrát čítajú tieto pravidlá, kývajú hlavou a myslia si "to je jednoduché". Potom narazí na prvý compile error a myslia si "čo to tu do pekla robí". Problém nie je v pravidlách — problém je v tom že v C si tieto pravidlá *porušoval* celý čas a nič sa nedialo (alebo sa dialo, ale ťažko sa to debuggovalo). Rust len najednou začne reálne vynucovať pravidlá ktoré si predtým len musel mať v hlave.

```rust
fn main() {
    let s1 = String::from("hello"); // s1 vlastní "hello" na heape
    let s2 = s1;                    // MOVE — s1 prestáva existovať
    // println!("{}", s1);          // error[E0382]: use of moved value
    println!("{}", s2);             // ok
} // s2 opúšťa scope → free()
```

Čo sa reálne stalo: `s1` bola trojica `(ptr=0x1234, len=5, cap=5)` na stacku. `let s2 = s1` skopíruje túto trojicu — teraz `s2` má `(ptr=0x1234, len=5, cap=5)`. Ale `ptr` ukazuje na ten *istý* heap buffer. Kto teraz vlastní ten buffer?

V C by mali oba pointery na to isté — a pri double free by sa heap pokoril. Rust rieši toto elegantne: `s1` sa *invaliduje*. Kompilátor si zaznamená že `s1` bola "moved" a akýkoľvek pokus o použitie `s1` po tomto bode je compile error. `s2` je teraz jediný vlastník buffer a keď opustí scope, buffer sa uvoľní presne raz.

V C by si to napísal takto — a bol by si v problémoch:

```c
char *s1 = strdup("hello");
char *s2 = s1;   // kópia pointera — teraz máš dva pointery na rovnaké miesto
free(s1);
printf("%s\n", s2);  // UB — dangling pointer — s2 ukazuje na uvoľnenú pamäť
```

Rust nedovolí dvom premenným *vlastniť* rovnaké dáta. Buď vlastní `s1`, alebo `s2`. Nie obaja.

---

## Move semantics

Move v Ruste nie je `memcpy` celých dát. Je to *transfer vlastníctva* — hodnota na stacku (tá trojica pointer+len+cap) sa skopíruje, ale pôvodná premenná sa invaliduje a heap buffer sa nepresúva:

```rust
let v1 = vec![1, 2, 3];     // v1 vlastní heap buffer na adrese 0x1234
let v2 = v1;                 // pointer na buffer sa skopíroval — v1 invalidovaný
// v1 je teraz "moved" — kompilátor ti nedovolí ho použiť
// buffer na 0x1234 sa NEPRESUNUL — stále je na tom istom mieste
// len vlastník sa zmenil z v1 na v2
```

Toto je konceptuálne ekvivalent:

```c
// C — manuálna "move semantics":
uint32_t *v1 = malloc_and_fill();
uint32_t *v2 = v1;
v1 = NULL;  // "invalidácia" — ale vynútiť toto musíš sám
// ak omylom použiješ v1, kompilátor neprotestuje
```

V Ruste je toto vynútené. Nie konvencia, nie komentár `// DO NOT USE v1 AFTER THIS`. Ak to skúsiš, dostaneš:

```
error[E0382]: use of moved value: `v1`
  --> src/main.rs:4:20
   |
2  |     let v1 = vec![1, 2, 3];
   |         -- move occurs because `v1` has type `Vec<i32>`, which does not implement the `Copy` trait
3  |     let v2 = v1;
   |              -- value moved here
4  |     println!("{:?}", v1);
   |                      ^^ value used here after move
```

Všimni si kvalitu chybovej hlášky. Rust ti *presne* povie: "toto je kde sme mali value", "toto je kde sme ju moved", "a toto je kde sa ju pokúšaš použiť po move". Toto nie je náhodné — Rust kompilátor investoval enormné množstvo práce do kvalitných error správ. Sú navrhnuté tak aby ťa naučili, nie len povedali "niečo je zle".

### Copy typy — výnimka z move

Primitívne typy implementujú trait `Copy` — kopírujú sa automaticky bez invalidácie pôvodnej premennej:

```rust
let x: u32 = 42;
let y = x;         // COPY — nie move
println!("{}", x); // ok — x je stále validný
println!("{}", y); // ok — y je tiež validný

// Copy typy: u8/u16/u32/u64/u128/usize, i8..i128/isize, f32/f64, bool, char
// A tuple/array z Copy typov: (u32, bool), [u8; 4]

// Nie Copy: String, Vec<T>, Box<T>, čokoľvek čo vlastní heap pamäť
```

Dôvod je priamy: `Copy` typy majú fixnú veľkosť a "kopírovanie" je triviálne — len skopíruj niekoľko bajtov na stacku, žiaden heap nie je zapojený. `u32 = 42` je 4 bajty na stacku. `let y = x` skopíruje tých 4 bajtov. Hotovo. Žiadna otázka vlastníctva, žiadna alokácia.

`String` *nemôže* byť `Copy`. Keby bola, `let y = x` by skopíroval pointer — a mali by sme dvoch vlastníkov rovnakého heap bufferu. Double free pri konci scope. Presne to čo sme chceli zabrániť.

Vlastné typy môžeš označiť ako `Copy` ak obsahujú len `Copy` typy:

```rust
#[derive(Clone, Copy, Debug)]
struct Point {
    x: f64,
    y: f64,
}

let p1 = Point { x: 1.0, y: 2.0 };
let p2 = p1;  // Copy — p1 je stále validné
println!("{:?}", p1); // ok
```

`Point` neobsahuje žiadne heap alokácie — sú to len dve `f64` čísla na stacku. Kopírovanie je triviálne a bezpečné.

---

## Clone — explicitná kópia

Keď naozaj chceš dve nezávislé kópie heap-alokovanej hodnoty, použiješ `.clone()`. Toto je *explicitné* — programátor musí vedome povedať "toto chcem skopirovať vrátane heap dát":

```rust
let s1 = String::from("hello");
let s2 = s1.clone(); // alokuje NOVÝ heap buffer, kopíruje bytes "hello" do neho

println!("{}", s1); // ok — s1 stále vlastní pôvodný buffer na 0x1234
println!("{}", s2); // ok — s2 vlastní nový buffer na 0x5678
```

`.clone()` je v podstate `strdup()` z C — alokácia nového bufferu a `memcpy`. To má reálnu cenu: alokácia, kopírovanie dát, prípadné dealokácie. Pre `String` s "hello" je to triviálne. Pre `Vec<Vec<String>>` s tisícom prvkov to môže byť drahé.

Prvá vec na ktorú si všimneš keď sa učíš Rust: všade vidíš `.clone()`. Toto je bežná začiatočnícka chyba — namiesto toho aby si pochopil čo chceš urobiť s hodnotou, hodíš `.clone()` a kompilátor prestane protestovať. Funguje to, ale je to zvyčajne zbytočné a pomalé. Zvyčajne chceš borrow. Porovnaj:

```rust
// Zbytočný clone — začiatočnícky vzor
fn print_len(s: String) {        // funkcia preberie vlastníctvo
    println!("{}", s.len());
}
fn main() {
    let s = String::from("hello");
    print_len(s.clone());        // .clone() len preto aby sme mohli použiť s znovu
    println!("{}", s);           // bez clone by toto zlyhalo
}

// Správne — použij borrow
fn print_len(s: &String) {       // borrow — nevzíma vlastníctvo
    println!("{}", s.len());
}
fn main() {
    let s = String::from("hello");
    print_len(&s);               // požičiavame — s je stále náš
    println!("{}", s);           // ok
}
```

Pravidlo palca: `.clone()` v hot path je code smell. Ak ho vidíš, zamysli sa či nechceš referenci namiesto toho.

---

## Borrowing — požičiavanie

Namiesto move môžeš *požičať* hodnotu — dočasný prístup bez prevzatia vlastníctva. Toto je kľúčový mechanizmus, bez ktorého by každá funkcia musela dostávať vlastníctvo a vracať ho späť.

```rust
fn print_len(s: &String) {      // & = shared reference — "požičaj si"
    println!("dĺžka: {}", s.len());
}

fn main() {
    let s = String::from("hello");
    print_len(&s);   // požičiame s, nevzali ho
    print_len(&s);   // môžeme požičať viackrát
    println!("{}", s); // s je stále validný — stále ho vlastníme
}
```

`&s` vytvorí referenciu — pointer na `s` ale bez prevzatia vlastníctva. `print_len` dostane tento pointer, môže čítať hodnotu, a keď funkcia skončí, referencia zanikne. `s` ostane validné u pôvodného vlastníka.

Ekvivalent v C: `void print_len(const char *s)` — pointer bez zodpovednosti za `free`. Rust pridáva jednu vec: garantuje že pointer je vždy validný počas celej doby jeho existencie. V C môžeš mať `const char *s` ukazujúci na uvoľnenú pamäť. V Ruste je to nemožné — borrow checker to overí pri kompilácii.

### Čo sa reálne deje v pamäti pri borrowing

Keď napíšeš `&s`, kompilátor vytvorí pointer na `s`. Na 64-bit systéme je to 8-bajtová hodnota na stacku obsahujúca adresu. Vo funkcii `print_len(s: &String)` je parameter `s` tento pointer. `s.len()` dereferencuje pointer a prečíta `len` pole z `String` struct. Žiadna mágia, žiadny overhead oproti `const char*` v C.

Rozdiel je v *garantiách*. Kompilátor sleduje lifetime každej referencie a zaručuje že referencia nikdy nevyžije svoju hodnotu. Toto sledovanie má nulový runtime overhead — je to len kompilačná analýza.

### Shared vs Exclusive reference

| | C | Rust |
|---|---|---|
| Read-only | `const T*` | `&T` |
| Read-write | `T*` | `&mut T` |

Ale Rust pridáva kritické pravidlo ktoré v C neexistuje: môžeš mať buď ľubovoľný počet `&T` súčasne, alebo presne jeden `&mut T` bez akýchkoľvek iných referencií. Nie oboje. Nikdy.

Toto je **mutex v compile time**. V C môžeš mať `const int *a` a `int *b` ukazujúce na to isté — a to je zdroj aliasing bugov, race conditions (v multi-thread kóde), a kompilačných pesimizácií (kompilátor nemôže predpokladať no-aliasing). Rust zakazuje aliasing medzi mutable a immutable referenciami systémovo.

```rust
fn main() {
    let mut v = vec![1, 2, 3];

    let r1 = &v;        // shared borrow — pointer na v
    let r2 = &v;        // ok — ďalší shared borrow, stále len čítame
    println!("{:?} {:?}", r1, r2);
    // r1, r2 sú tu naposledy použité — ich lifetime končí

    let r3 = &mut v;    // exclusive borrow — ok, r1/r2 už nie sú aktívne
    r3.push(4);
    println!("{:?}", r3);
}
```

Všimni si kde `r1` a `r2` naposledy použiješ — borrow checker (v modernom Ruste s "Non-Lexical Lifetimes") vidí že ich lifetime končí pri `println!`, nie pri konci bloku `}`. Takže `let r3 = &mut v` je OK — v tom momente už neexistujú aktívne shared borrows.

### "Kompilátor hovorí nie" — aliasing

```rust
fn main() {
    let mut v = vec![1, 2, 3];
    let r1 = &v;
    let r2 = &mut v;    // error[E0502]: cannot borrow `v` as mutable
                        // because it is also borrowed as immutable
    println!("{:?}", r1);
}
```

Prečo je toto chyba? Predstav si čo by sa mohlo stať: `r2.push(4)` môže prealokovať interný buffer `v` — ak kapacita nestačí, `Vec` alokuje nový väčší buffer, skopíruje dáta, a uvoľní starý. `r1` teraz ukazuje na uvoľnený starý buffer. Použitie `r1` po `push` je dangling pointer. V C toto je klasická iterator invalidation. V Ruste je to compile error.

V C by kompilátor nemohol zoptimalizovať `r1` a `r2` agresívne pretože mohol by byť aliasing. Rust to zakazuje, čím umožňuje optimalizácie podobné `__restrict__` — ale vynútené, nie voliteľné. To je jeden z dôvodov prečo Rust kód môže byť rýchlejší než ekvivalentný C kód bez explicitného `restrict`.

---

## Dangling pointers — prečo to nejde

```rust
fn get_ref() -> &String {   // error: missing lifetime specifier
    let s = String::from("temporary");
    &s  // s sa dropuje na konci funkcie — dangling pointer
}
```

Toto je Rust verzia klasického C bugu:

```c
// C — UB:
const char* get_ref() {
    char buf[64] = "temporary";  // na stacku
    return buf;  // vrátenie adresy lokálnej premennej — dangling po návrate
}
```

V C kompilátor niekedy dá warning, niekedy nie. Program "funguje" kým stack frame nie je prepísaný. V Ruste je toto tvrdý compile error — kompilátor vie že `s` zanikne pri konci funkcie a referencia na ňu by bola dangling pointer.

Toto je miesto kde sa prvýkrát stretneš s *lifetimes*. Rust vidí každú referenciu ako "žije od X do Y". Keď sa vracia referencia z funkcie, musí mať lifetime ktorý prežije funkciu. `&s` kde `s` je lokálna premenná toto nesplňuje.

Správne riešenie: vráť vlastníka, nie referenciu:

```rust
fn get_string() -> String {   // vrátime vlastníctvo — String sa presunie von z funkcie
    String::from("hello")
}
```

Alebo ak vraciaš referenciu na niečo čo prežije funkciu:

```rust
fn first_element(v: &Vec<u32>) -> Option<&u32> {
    v.first()  // referencia na prvok Vec — žije kým Vec žije
}
```

Tu je `&u32` referencia ktorej lifetime je viazaný na vstupný `&Vec<u32>`. Kým volajúci drží `Vec`, referencia je validná. Borrow checker to overí.

---

## Non-Lexical Lifetimes (NLL) — prečo je moderný borrow checker lepší

Do Rust 2018 edition bol borrow checker "lexical" — borrow trval do konca lexikálneho bloku `{}`, nie do posledného použitia. Toto spôsobovalo falošné pozitívy kde správny kód neprešiel kompilovaním. Rust 2018 a 2021 edition majú NLL — borrow trvá len do posledného použitia:

```rust
// Pred NLL — zlyhalo by s "cannot borrow `map` as mutable"
// Po NLL — funguje
let mut map = HashMap::new();
map.insert("a", 1);

let val = map.get("a");   // immutable borrow
if let Some(v) = val {
    println!("{}", v);    // posledné použitie val — borrow končí tu
}

map.insert("b", 2);       // mutable borrow — ok, immutable borrow skončil vyššie
```

Ak ešte narážaš na "ugh borrow checker" problémy s jednoduchým kódom, je dobré si overiť že máš `edition = "2021"` v `Cargo.toml`.

---

## Praktický príklad: zásobník bez unsafe

Pozrime sa ako ownership pravidlá vedú k čistej API. Zásobník (Stack) je klasický príklad:

```rust
pub struct Stack<T> {
    data: Vec<T>,
}

impl<T> Stack<T> {
    pub fn new() -> Self {
        Stack { data: Vec::new() }
    }

    pub fn push(&mut self, item: T) {
        // &mut self — potrebujeme modifikovať Stack (exclusive borrow)
        // item: T — Stack prevezme vlastníctvo item
        self.data.push(item);
    }

    pub fn pop(&mut self) -> Option<T> {
        // &mut self — modifikujeme Stack
        // vracia Option<T> — ak prázdny, None; inak Some(T) a presúvame vlastníctvo VON
        self.data.pop()
    }

    pub fn peek(&self) -> Option<&T> {
        // &self — len čítame, nepotrebujeme mutable borrow
        // vracia Option<&T> — referencia na prvok, Stack ostáva vlastník
        self.data.last()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }
}

fn main() {
    let mut stack: Stack<u32> = Stack::new();
    stack.push(1);
    stack.push(2);
    stack.push(3);

    // peek požičia — stack stále vlastní hodnotu, len nám dovolí nahliadnuť
    if let Some(top) = stack.peek() {
        println!("na vrchu: {}", top);
        // tu životnosť `top` referencie skončí
    }
    // teraz môžeme stack mutovať

    // pop presunie vlastníctvo von — hodnota "opustí" Stack
    while let Some(val) = stack.pop() {
        println!("pop: {}", val);
    }
    // všetky hodnoty sú preč, Stack je prázdny
}
```

Všimni si rozdiel medzi `peek` a `pop`. `peek` vracia `Option<&T>` — *referenciu* na prvok vo Stacku. Stack ostáva vlastníkom. Volajúci môže hodnotu čítať ale nie vlastniť. `pop` vracia `Option<T>` — samotnú *hodnotu* presunutú zo Stacku von. Stack ju stratil. Toto je *presne* ako by si to navrhol v C, len tu to kompilátor vynúti.

Žiadny `malloc`, `free`, žiadne null pointery. Stack sa uvoľní automaticky keď opustí scope, vrátane všetkých prvkov ktoré ešte obsahuje. `Vec<T>` volá `drop` na každom prvku (ak implementuje `Drop`) a potom uvoľní heap buffer.

---

## "Toto by v C explodovalo"

### Príklad 1: use-after-free

```rust
let v = vec![1u32, 2, 3];
let ptr = v.as_ptr();   // raw pointer na heap buffer — *const u32

drop(v);                // uvoľníme buffer — ptr je teraz dangling

// V safe Rust toto nie je možné urobiť nevedome:
// Použitie ptr by vyžadovalo unsafe blok:
// unsafe { println!("{}", *ptr); }  // UB — rovnaké ako v C
```

`v.as_ptr()` vráti raw pointer `*const u32`. Raw pointery existujú v Ruste — potrebuješ ich pre FFI a pre low-level dátové štruktúry. Ale ich *použitie* (dereferencovanie) vyžaduje `unsafe` blok. Toto je explicitné opt-in do "tu viem čo robím a preberám zodpovednosť". V bezpečnom Ruste — čo je 99% bežného kódu — raw pointery nemôžeš derefencovať bez `unsafe`. Borrow checker sleduje lifetime raw pointerov a ak môže dokázať že to je nebezpečné, odmietne to.

### Príklad 2: iterator invalidation

```rust
let mut v = vec![1, 2, 3, 4, 5];

// Toto NEfunguje v Ruste:
for x in &v {
    if *x == 3 {
        v.push(99); // error: cannot borrow `v` as mutable
                    // because it is also borrowed as immutable
    }
}

// Správny vzor: collect do nového vektora
let new_v: Vec<u32> = v.iter()
    .flat_map(|&x| if x == 3 { vec![x, 99] } else { vec![x] })
    .collect();
println!("{:?}", new_v); // [1, 2, 3, 99, 4, 5]
```

V C++ toto je jeden z najčastejších bugov. `std::vector` môže reallokovať pri `push_back`, čím invaliduje všetky iterátory a pointery na prvky. Výsledok je UB — môže crashnúť alebo tichý heap corruption. Rust to zachytí: `for x in &v` vytvorí immutable borrow, `v.push(99)` vyžaduje mutable borrow — nemôžu existovať súčasne.

### Príklad 3: return of local address

```c
// C — klasický bug, warningu sa dočkáš možno, možno nie:
int* get_local() {
    int x = 42;
    return &x;  // UB — x zanikne, pointer visí
}
// Volajúci má pointer na stack frame ktorý sa medzičasom prepísal
```

```rust
fn get_local() -> &u32 {  // error: missing lifetime specifier
    let x = 42u32;
    &x  // kompilátor nedovolí — x zanikne keď funkcia skončí
}

// Musíš vrátiť hodnotu:
fn get_value() -> u32 {
    42u32  // Copy typ — kópia sa vráti cez return value (register alebo stack)
}
```

Keď Rust hovorí "missing lifetime specifier", hovorí ti: "vrátenie referencie z funkcie je OK len ak mi povieš ako dlho tá referencia žije". Keď referencia závisí od lokálnej premennej, to nie je možné povedať — lokálna premenná zanikne, referencia by bola dangling. Kompilátor to odmietne.

---

## Lifetimes — rýchly pohľad (viac v kapitole 5)

Lifetimes sú súčasť Rustu ktorá vyzerá desivo keď ju prvýkrát uvidíš:

```rust
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() { x } else { y }
}
```

Ten `'a` je lifetime parameter. Hovorí: "vstupné referencie a výstupná referencia všetky žijú aspoň tak dlho ako `'a`". Kompilátor to použije na overenie že výstupná referencia je vždy validná.

V praxi toto musíš písať ručne len keď kompiler nemôže odvodiť lifetime sám (lifetime elision). Pre jednoduché funkcie (jeden vstupný borrow → výstupný borrow) kompilátor to odvodí automaticky. Explicitné lifetimes sú len pre zložitejšie prípady — keď funkcia má viac vstupných referencií a kompilátor nevie sám určiť od ktorej závisí výstup.

Viac o lifetimes bude v kapitole 5. Zatiaľ stačí vedieť že existujú a že sú spôsob ako kompilátor sleduje "kto žije dlhšie".

---

## Mentálny model pre borrow checker

Predstav si každú hodnotu ako fyzický objekt s visačkou s menom vlastníka. Len jedna osoba môže vlastniť objekt v jednom čase. Ktokoľvek si môže pozrieť objekt (shared borrow), ale keď niekto pracuje s objektom (mutable borrow), ostatní musia počkať vonku.

`let s = String::from("hello")` — objekt s visačkou `s`.
`let t = s` — visačka sa presunula na `t`, `s` je prázdna ruka.
`fn foo(s: &String)` — predmet si *požičiaš* na pohľad, visačka ostáva pôvodnému vlastníkovi.
`fn foo(s: &mut String)` — požičanie s právom modifikovať (kľúče od auta, nie len pohľad).
Koniec scope — predmet zmizne, visačka s ním.

Borrow checker len overuje že pravidlá sú konzistentné. To čo on zachytí, by v C skôr či neskôr explodovalo v produkcii — možno o hodinu, možno o rok.

### Časté chyby začiatočníkov a ako ich čítať

**"use of moved value"** — pokúšaš sa použiť hodnotu po tom čo bola presunutá (move). Riešenie: buď ju klonuj pred move, alebo zmeň API aby bral borrow namiesto ownership.

**"cannot borrow as mutable because it is also borrowed as immutable"** — máš aktívnu immutable referenciu a pokúšaš sa muteovať. Riešenie: ukonči immutable borrow pred mutáciou (neukladaj referenciu do premennej ak ju nepotrebuješ ďalej), alebo preštruktúruj kód.

**"returns a reference to data owned by the current function"** — funkcia sa pokúša vrátiť referenciu na lokálnu premennú. Riešenie: vráť hodnotu (owned), nie referenciu. Alebo ak potrebuješ vrátiť referenciu, musí ukazovať na niečo čo prežije funkciu (vstupný parameter, static data).

**"does not live long enough"** — referencia sa pokúša prežiť hodnotu na ktorú ukazuje. Riešenie: predlženie life valueadresy (move ju von z bloku), alebo skrátenie lifetimeadu referencie.

---

## Zhrnutie pravidiel

```
Každá hodnota:  jeden vlastník v danom čase
Move:           transfer vlastníctva (pôvodná premenná neplatí)
Copy:           primitívne typy — kópia bez move (len stack, žiadny heap)
Clone:          explicitná hĺbková kópia (nová heap alokácia + memcpy)
&T:             shared borrow — čítanie, viacero naraz, nemôže koexistovať s &mut T
&mut T:         exclusive borrow — zápis, presne jeden naraz
Drop:           automatické uvoľnenie pri konci scope (RAII, deterministické)
Lifetime:       ako dlho je referencia garantovane validná (compile time analýza)
```

Toto nie je len teória. Každý z týchto pravidiel má priamy odraz v reálnych bugoch ktoré Rust zabraňuje — use-after-free, double-free, iterator invalidation, dangling pointers, data races. Nie preto že programátori v Ruste sú múdrejší. Preto že tieto pravidlá kontroluje stroj.
