# Kapitola 0 — Prečo Rust?

Máš za sebou roky C, C++, možno trochu assembleru. Vieš čo je pointer, stack frame, cache line, interrupt handler. Nepotrebuješ vysvetľovať čo je heap. Pravdepodobne si už aj debuggoval race condition o tretej ráno, alebo strávil deň hľadajúc use-after-free bug ktorý sa prejavoval len na zákazníkových serveroch s konkrétnou verziou glibc. Vieš ako vyzerá core dump a vieš čo znamená `SIGSEGV`. Systémové programovanie ti nie je cudzie.

Tak prečo Rust?

Pretože väčšina bugov v systémovom kóde pochádza z tej istej trojice problémov — a Rust ich rieši na úrovni jazyka, nie toolov. Nie AddressSanitizerom (ktorý treba zapnúť). Nie Valgrindom (ktorý spomaľuje 10×). Nie code review (ktorý závisí od pozornosti ľudí). Priamo kompilátorom, pri každom buildu, zadarmo.

A tu je to čo ma osobne najviac prekvapilo keď som začínal: Rust nie je o tom že by vymyslel nejaké nové magické techniky. Pravidlá ktoré vynucuje borrow checker sú pravidlá ktoré dobrý C programátor *aj tak* musí dodržiavať — len ich musí udržiavať v hlave, v komentároch, v dokumentácii. Rust ich len formalizoval a preniesol do typového systému. Náhle ich kontroluje stroj, nie človek. A stroje sa nepomýlia pri štvrtom code review v týždi.

---

## Krátka história: odkiaľ Rust prišiel

Rok 2006. Graydon Hoare pracuje v Mozille a frustruje ho výťah v jeho bytovom dome, ktorý sa crashuje. Myslí si, že softvér pre tak jednoduchú vec by nemal mať takéto bugy. Začne sa hrať s jazykom pre osobné použitie — čistý hobby projekt. Jazyk bez garbage collectora, ale bez manuálnej správy pamäte. Bez null pointerov. So silným typovým systémom inšpirovaným ML rodinou jazykov (OCaml, Haskell).

Mozilla si všimne projekt v roku 2009 a začne ho sponzorovať. Dôvod je pragmatický: Firefox je napísaný v C++ a má neustále bezpečnostné problémy. Milióny riadkov C++ kódu, tisíce potenciálnych memory chýb. Ak existuje jazyk ktorý zabraňuje celej triede bugov na úrovni kompilátora, Mozilla to chce.

Rust 1.0 vyšiel v máji 2015. Odvtedy jazyk rastie raketovým tempom. V roku 2016 sa prvýkrát objavil v Stack Overflow Developer Survey ako "najobľúbenejší jazyk" — a ostáva tam každý rok až doteraz. Nie preto že je módny, ale preto že programátori ktorí ho vyskúšajú, nechcú späť.

V roku 2021 bol Rust prijatý do Linux kernelu — prvý nový jazyk v kerneli po viac ako 30 rokoch C. To nie je marketingový ťah. To je sedem rokov diskusií, technických argumentov a presvedčovania Linusa Torvaldsa. Ak kernel maintaineri považujú jazyk za dostatočne produkčný, je to silný signál.

---

## Tri problémy, ktoré Rust rieši za teba

### 1. Use-after-free / dangling pointer

Klasika v C. Každý kto písal C dlhší čas, narazil na toto. Kód vyzerá správne, testy prechádzajú, a potom sa v produkcii objaví záhadný crash alebo — oveľa horšie — tichá korupcia dát:

```c
char *buf = malloc(64);
process(buf);
free(buf);
// ... 300 riadkov neskôr ...
log_message(buf);  // UB. Možno segfault. Možno tichá korupcia.
```

Problém je že kompilátor vidí `buf` ako platnú premennú. Má adresu, má typ, všetko sedí. Fakt že pamäť na tej adrese bola uvoľnená — to kompilátor nevie sledovať. Nemá na to jazyk. Takže ťa nechá to urobiť a modlí sa za teba.

Čo sa skutočne stane závisí od alokátora, od toho či bola pamäť medzičasom prealokovaná iným `malloc` volaním, od threading modelu... Debuggovanie tohto je nočná mora, pretože symptóm (crash) je vzdialený od príčiny (free) o stovky riadkov kódu a možno o sekundy runtime.

V Ruste je toto compile error. Nie warning. Nie sanitizer upozornenie. Error — kód sa neskompiluje:

```rust
fn main() {
    let buf = String::from("hello");
    drop(buf);          // explicitný drop — vlastníctvo sa vzdáva
    println!("{}", buf); // error[E0382]: borrow of moved value: `buf`
}
```

Kompilátor *vie* kedy `buf` prestáva existovať. Nie runtime. Kompilátor. Je to súčasť analýzy ktorú robí pri každom buildu — sleduje tok vlastníctva cez celý program a ak nájde miesto kde by si použil hodnotu po jej dealokácii, odmietne kód skompilovať.

Toto nie je magia. Je to dôsledok ownership systému, o ktorom bude celá kapitola 2. Zatiaľ stačí vedieť: ak kód prešiel kompilátorom, use-after-free tam nie je. Garantovane.

### 2. Data races

V C/C++ stačí zabudnúť na mutex — a máš race condition, ktorý sa prejaví raz za mesiac v produkcii, keď je server pod záťažou a vlákna sa prekrývajú presne nešťastným spôsobom:

```c
// Thread A                Thread B
pthread_mutex_lock(&m);
counter++;
pthread_mutex_unlock(&m);
                           counter++;  // zabudnutý lock — UB
```

Toto je legálny C kód. Kompilátor ho skompilje bez slova. Code review možno zachytí, možno nie — `counter++` vyzerá nevinne. A race condition sa objaví len keď Thread A a Thread B bežia presne v správnom poradí, čo sa v testoch takmer nikdy nestane.

Rust to zachytí pri kompilácii. `Arc<Mutex<T>>` zaručuje, že k dátam môžeš pristúpiť len cez lock. Nie je to konvencia alebo dohoda v tíme — je to vynútené typovým systémom:

```rust
use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
    let counter = Arc::new(Mutex::new(0u32));
    let mut handles = vec![];

    for _ in 0..8 {
        let c = Arc::clone(&counter);
        handles.push(thread::spawn(move || {
            *c.lock().unwrap() += 1;
            // lock sa automaticky uvoľní — RAII, žiadny unlock()
        }));
    }

    for h in handles { h.join().unwrap(); }
    println!("výsledok: {}", *counter.lock().unwrap()); // vždy 8
}
```

Pokus preniesť `counter` do vlákna bez `Arc`? Compile error — `counter` by musel byť prístupný z viacerých vlákien, ale nevieš zaručiť jeho lifetime. Pokus pristúpiť bez `lock()`? Compile error — typ `Mutex<u32>` neimplementuje `Deref`, takže k vnútornej hodnote sa nedostaneš bez volania `lock()`. Typový systém doslova znemožňuje data race.

Dôležité: toto neznamená že Rust programy nemôžu mať deadlocky alebo logické chyby v concurrency. Môžu. Rust zaručuje *memory safety* a *race-free prístupy k dátam* — nie správnosť programu. Ale to je stále obrovský skok vpred.

### 3. NULL pointer dereference

Tony Hoare, ktorý vymyslel null pointer v ALGOLe 60, to neskôr nazval svojou "miliardodolárovou chybou". Null je vo všetkých C-like jazykoch všade, pretože je pohodlný. A je to zdroj nespočetných bugov, vrátane kritických bezpečnostných zraniteľností.

```c
struct Connection *conn = find_connection(id);
conn->send(data);  // čo ak find_connection vrátilo NULL?
```

`find_connection` môže vrátiť NULL. To je zdokumentované. Každý vývojár to vie. A každý vývojár to niekedy zabudne skontrolovať, pretože tých `if (ptr == NULL)` kontrol je v kóde stovky a po čase si na ne zvykneš a vynecháš jednu. Kompilátor ti nepomôže.

Rust nemá `null`. Nie že by ho zakázal — on ho jednoducho *nemá* v typovom systéme. Namiesto toho `Option<T>`:

```rust
fn find_connection(id: u32) -> Option<Connection> {
    // ak spojenie existuje, vrátime Some(conn)
    // ak nie, vrátime None
}

fn main() {
    match find_connection(42) {
        Some(conn) => conn.send(data),
        None => eprintln!("spojenie nenájdené"),
    }
    // Zabudnúť na None? Kompilátor nedovolí.
}
```

Exhaustiveness checking — ak neošetríš `None`, kód sa neskompiluje. Kompilátor ti nedovolí ignorovať možnosť "hodnota neexistuje". Musíš sa explicitne rozhodnúť čo urobíš. Môžeš volať `.unwrap()` a riskovať panic (ekvivalent assert), ale je to explicitné rozhodnutie, nie zabudnutie.

Toto zmení spôsob ako premýšľaš o nullable hodnotách. Po čase začneš vidieť `Optional` v Jave, `Maybe` v Haskelli, `?` v Kotline ako slabé napodobeniny niečoho čo Rust robí správne od základu.

---

## Kde sa Rust používa (2025)

Povedať "kde sa Rust používa" dnes je ako povedať "kde sa používa C" — je všade kde záleží na výkone a spoľahlivosti.

Linux kernel prijal Rust od verzie 6.1. To neznamená že kernel je prepisovaný v Ruste — to by trvalo dekády. Znamená to že nové ovládače a nové subsystémy môžu byť písané v Ruste a existujú reálne príklady: `Nova` GPU driver pre Nvidia hardware, rôzne filesystem utility, bezpečnostné moduly.

Android od verzie 13 písí nový kód v Bluetooth stacku, HAL vrstvách a niektorých systémových servisoch v Ruste. Google reportuje výrazný pokles pamäťových bugov v kóde písanom v Ruste oproti C++.

Microsoft integroval Rust do Windows kernela pre niektoré bezpečnostne kritické komponenty. Windows je historicky jeden z najväčších zdrojov memory safety bugov — CVE databáza je plná stack overflow a use-after-free chýb. Microsoft verejne hovorí o prechode na memory-safe jazyky ako dlhodobej stratégii.

AWS, Google a Microsoft všetci používajú Rust v cloudovej infraštruktúre. Firecracker — lightweight VM manager za Lambdou a Fargate — je napísaný v Ruste. AWS bol early adopter a aktívne prispievajú do Rust ekosystému.

Embedded svet: `no_std` Rust beží na ARM Cortex-M (STM32, Nordic nRF), ESP32, RISC-V mikrokontroléroch. Projekt `Embassy` je async embedded framework plne v Ruste. Pre embedded je Rust obzvlášť atraktívny — žiadny GC means deterministic timing, žiadne memory leaky, menšia binárka než C++.

WebAssembly: Rust kompiluje do WASM lepšie ako takmer čokoľvek iné. `wasm-pack` a `wasm-bindgen` urobia integráciu do JavaScriptu trivialnou. Cloudflare Workers, Figma, Fastly — všetci používajú Rust-to-WASM.

Nie je to experimentálny jazyk. Je to produkčná technológia s troma-štyrmi rokmi stabilizácie za sebou.

---

## Filozofia jazyka

### Zero-cost abstractions

Tento princíp pochádza priamo z C++ filozofie Bjarne Stroustrupa: "čo nepoužiješ, za to neplatíš; čo použiješ, nedokázal by si to napísať lepšie ručne." Rust to berie vážne — oveľa vážnejšie ako moderný C++.

Abstrakcie v Ruste nestoja výkon. To isté čo v C — len typovo bezpečné. Iterátory sú dokonalý príklad:

```rust
// Iterátor — vyzerá ako vyšší level
let sum: u64 = (0u64..1_000_000).filter(|x| x % 2 == 0).sum();

// Kompilátor vygeneruje prakticky rovnaký kód ako:
let mut sum: u64 = 0;
let mut i: u64 = 0;
while i < 1_000_000 {
    if i % 2 == 0 { sum += i; }
    i += 1;
}
```

Bench to. Výsledky sú identické. A nie, nie je to len optimistické tvrdenie — LLVM backend ktorý Rust používa je ten istý LLVM ktorý poháňa Clang. Rust generuje LLVM IR a LLVM to optimalizuje rovnako agresívne.

Ale zero-cost abstractions znamenajú viac než len rýchlosť. Znamenajú že môžeš písať kód na vyššej úrovni abstrakcie *bez obáv*. Nemôžeš sa rozhodnúť "toto napíšem low-level, lebo sa bojím že abstrakcia bude pomalá" — v Ruste takáto voľba väčšinou neexistuje, pretože abstrakcia je rovnako rýchla.

### Prečo to takto? — filozofia ownership

C a C++ dávajú programátorovi úplnú kontrolu a úplnú zodpovednosť. Garbage collected jazyky (Java, Python, Go) dávajú programátorovi bezpečnosť ale berú kontrolu. GC pauzuje program v nepredvídateľných momentoch, memory footprint je väčší, latency je horšia.

Rust hľadá tretiu cestu: *statická analýza vlastníctva*. Kompilátor sleduje kto vlastní akú pamäť a kedy ju uvoľniť — bez runtime overhead. Nie GC, nie manuálne `free()`, ale compile-time analýza.

Toto nie je nový nápad. Úzko súvisí s RAII (Resource Acquisition Is Initialization) z C++ — myšlienka že zdroj (pamäť, file handle, socket) je spravovaný životnosťou objektu. Rust to systematizuje a formalizuje do pravidiel ktoré kontroluje kompilátor.

Výsledok: pamäťová bezpečnosť *bez* runtime ceny. Je to to najlepšie z oboch svetov.

### Fearless concurrency

"Fearless" neznamená jednoduchý. Znamená: ak to skompiluje, neexistuje data race. Kompilátor ti zaručuje správnosť concurrency na úrovni typového systému — nie dokumentáciou, nie code review, nie sanitizermi.

Rust rozlišuje medzi "môže byť poslaný medzi vláknami" (`Send`) a "môže byť zdieľaný medzi vláknami" (`Sync`). Tieto tzv. marker traits sú automaticky odvodené kompilátorm a ich porušenie je compile error. Ak tvoj typ obsahuje `Rc<T>` (non-atomic reference count), automaticky nie je `Send` a nemôžeš ho poslať do iného vlákna. Ak obsahuje `*mut T`, nie je ani `Send` ani `Sync`. Kompilátor to vie a odmietne nesprávne použitie.

---

## "Toto by v C explodovalo"

Niekoľko konkrétnych scenárov kde Rust zachytí to čo C/C++ nechá prejsť. Každý z nich je typ bugu ktorý sa v reálnom kóde objavuje — nie akademické príklady.

### Iterator invalidation

Toto je jedna z najzákernejších chýb v C++. Modifikácia kontajnera počas iterácie invaliduje iterátor a výsledok je nedefinované správanie. Môže crashnúť, môže "fungovať", môže spôsobiť tichú korupciu dát:

```c
// C++ — UB, môže crashnúť alebo tichý heap corruption
std::vector<int> v = {1, 2, 3};
for (auto& x : v) {
    if (x == 2) v.push_back(4);  // invaliduje iterátor
}
```

`push_back` môže reallokovať interný buffer vektora, čím presunie dáta na iné miesto v pamäti. Iterátor teraz ukazuje na uvoľnenú pamäť. Čo sa stane ďalej závisí od alokátora a od šťastia.

```rust
// Rust — compile error
let mut v = vec![1, 2, 3];
for x in &v {
    if *x == 2 {
        v.push(4); // error: cannot borrow `v` as mutable because
                   // it is also borrowed as immutable
    }
}
```

`for x in &v` vytvorí immutable borrow na `v`. `v.push(4)` vyžaduje mutable borrow. Mať oboje naraz je zakázané — borrow checker to odmietne. Žiadna šanca na iterátor invalidation.

### Stack buffer overflow

Klasická bezpečnostná zraniteľnosť. `strcpy` bez kontroly dĺžky. `gets()`. Fixný buffer s premenlivým vstupom. Tieto veci sú v C kóde dodnes — v embedded systémoch, v sieťových protokoloch, v parsovaní:

```c
void copy_name(const char *src) {
    char buf[16];
    strcpy(buf, src);  // žiadna kontrola dĺžky — klasický stack smash
}
```

Ak `src` je dlhší ako 16 znakov, prepíše sa stack frame, return adresa, a útočník môže presmerovať tok programu na ľubovoľný kód. Toto je základ väčšiny exploitov posledných 30 rokov.

```rust
fn copy_name(src: &str) -> String {
    src.to_string() // alokuje presne toľko čo treba — žiadne pretečenie
}

// Alebo s pevnou veľkosťou:
fn copy_name_fixed(src: &str) -> [u8; 16] {
    let mut buf = [0u8; 16];
    let len = src.len().min(16);
    buf[..len].copy_from_slice(&src.as_bytes()[..len]);
    buf
}
```

`src.len().min(16)` — ručne robíme truncation, ale bez možnosti overflow. Slice `buf[..len]` má bounds checking (v debug mode panic, v release mode... stále kontrola, ale len pre indexy). Žiadna možnosť zapísať za koniec bufferu.

### Double free

```c
char *p = malloc(64);
free(p);
free(p);  // UB — heap corruption
```

Dvojité uvoľnenie tej istej pamäte je klasická heap corruption. Moderné alokátory to väčšinou detectujú a crashnú, ale starší alebo minimalistický alokátor (embedded) môže tichá korigovať pamäť útočníkovým smerom.

```rust
fn main() {
    let p = Box::new([0u8; 64]);
    drop(p);
    drop(p); // error[E0382]: use of moved value: `p`
}
```

`drop(p)` konzumuje `p` — presunie vlastníctvo do `drop` funkcie ktorá ho dealokuje. Po tom `p` neexistuje ako validná premenná. Druhý `drop(p)` je compile error. Nie runtime check — compile error.

---

## Pod kapotou: čo sa reálne deje v pamäti

Je dobré rozumieť čo Rust *skutočne* generuje, nie len čo sľubuje. Vezmime jednoduchý príklad:

```rust
fn main() {
    let s = String::from("hello");
    println!("{}", s);
}
```

Na stacku leží `String` struct — to je trojica `(ptr: *mut u8, len: usize, capacity: usize)`. Na 64-bit systéme to je 24 bajtov. `ptr` ukazuje na heap buffer kde leží "hello" (5 bajtov). `len` je 5, `capacity` je 5 (alebo viac, závisí od alokátora).

Keď sa `s` dostane na koniec scope (koniec `main`), kompilátor vloží volanie `drop_in_place::<String>(&s)` čo zavolá `dealloc` na heap buffer. Toto nie je runtime overhead — je to kód ktorý kompilátor vygeneruje na presnom mieste kde `s` prestáva byť validný.

Ak na to pozrieš cez `cargo rustc -- --emit=asm` alebo na [godbolt.org](https://godbolt.org), uvidíš explicitné volanie `free` (alebo `__rust_dealloc`) na konci funkcie. Žiadna GC paúza, žiadny finalizer, len deterministické volanie `free` na presnom mieste.

---

## Porovnanie: `malloc/free` vs ownership

| C | Rust |
|---|------|
| `malloc()` + `free()` manuálne | Drop trait — automaticky pri konci scope |
| Žiadna záruka kto vlastní pointer | Vždy jeden vlastník |
| Kopírovanie = kopírovanie adresy | Kopírovanie = move (alebo explicitný `.clone()`) |
| Reference counting manuálny | `Rc<T>` / `Arc<T>` |
| NULL pointer možný všade | `Option<T>` — null neexistuje |
| UB pri double free, use-after-free | Compile error |

```rust
{
    let data = Box::new(vec![1u32, 2, 3]); // alokácia na heape
    println!("{:?}", data);
} // <-- tu sa automaticky zavolá free() — RAII
```

Toto je RAII — Resource Acquisition Is Initialization. Koncept z C++ (Bjarne Stroustrup, okolo 1984), ale v C++ je voliteľný a závisí od disciplíny programátora. V Ruste je *povinný* — nemôžeš obísť Drop.

## Porovnanie: `pthread` vs `std::thread`

```c
// C — musíš si pamätať volať join, mutex lock/unlock
pthread_t t;
pthread_create(&t, NULL, worker_fn, &shared_data);
pthread_mutex_lock(&mutex);
shared_data.count++;
pthread_mutex_unlock(&mutex);
pthread_join(t, NULL);
```

Problém: `&shared_data` je void pointer. Kompilátor nevie čo je vnútri. Ak zabudneš lock alebo zabudneš join — nič sa nestane pri kompilácii. Za to zaplatíš v produkcii.

```rust
// Rust — kompilátor zaručuje správne použitie
let data = Arc::new(Mutex::new(0u32));
let d = Arc::clone(&data);
let handle = std::thread::spawn(move || {
    *d.lock().unwrap() += 1;
    // unlock je automatický
});
handle.join().unwrap(); // join je povinný — handle sa inak nedropuje čisto
```

`thread::spawn` vyžaduje `move` closure — zaručuje že všetky premenné použité vo vlákne sú buď `Send` (môžu byť prenesené medzi vláknami) alebo sú explicitne klonované. `Arc` je atomicky counted pointer — bezpečné pre sharing. `Mutex` zaručuje že k `u32` sa dostaneš len cez lock.

---

## Čo Rust nie je

Rust nie je GC jazyk. Žiadny garbage collector. Pamäť sa uvoľňuje deterministicky pri konci scope. To znamená predvídateľnú latenciu — žiadne GC pauzy, žiadne "prečo aplikácia zamrzla na 200ms" situácie. Pre embedded, real-time systémy a vysokovýkonný sieťový kód je toto zásadné.

Rust nie je pomalý. Porovnateľný s C/C++ — benchmark it yourself: Rust vs C na JSON parsing (serde_json vs rapidjson), na SIMD, na sieťových bufferi. Niekedy je Rust rýchlejší než C++ pretože borrow checker zaručuje no-aliasing čo umožňuje agresívnejšiu optimalizáciu než `__restrict__` v C.

Rust nie je len pre systémy. Web backendy (Axum, Actix-web dosahujú top miesta v TechEmpower benchmarkoch), CLI nástroje (ripgrep je rýchlejší než grep, bat nahradza cat, fd nahradza find), herný engine (Bevy), WASM aplikácie. Rust-based nástroje v tejto kategórii sú dnes štandardom v mnohých vývojárskych prostrediach.

Rust nie je jednoduchý. Krivka učenia je strmá, hlavne ownership a lifetimes. Prvé tri týždne budeš bojovať s borrow checkerom. Štvrtý týždeň začneš chápať čo ti hovorí. Po mesiaci budeš vidieť bugy v C kóde ktoré si predtým prehliadal. Je to investícia — ale vyplatí sa.

---

## Časté otázky od C/C++ programátorov

"Prečo nemôžem mať dva mutability pointery na to isté?" — Pretože aliasing a mutability spolu sú korene väčšiny pamäťových bugov. Ak máš dva `int*` ukazujúce na to isté miesto, kompilátor nemôže vedieť že sú aliased a optimalizácia môže sprobiť neočakávané veci. Rust to zakazuje a tým umožňuje agresívnejšiu optimalizáciu.

"Prečo musím vrátiť String namiesto &str keď funkcia vytvára string?" — Pretože `&str` je referencia na existujúce dáta. Ak funkcia vytvorí dáta interne, tieto dáta zanikajú keď funkcia skončí. Nemôžeš vrátiť referenciu na niečo čo zanikne — dangling pointer. Buď vrátíš vlastníka (`String`), alebo požičiavaš zo vstupu.

"Prečo je `.clone()` na každom kroku keď pracujem s Rustom?" — Pretože bojuješ s borrow checkerom namiesto toho aby si pochopil ownership. `clone` je zriedka správna odpoveď. Väčšinou chceš references alebo premyslieť ownership štruktúru. Ak vidíš `.clone()` všade, je to príznak že ownership model ešte nie je správne nastavený.

---

## Čo ťa čaká v tejto knihe

Každá kapitola má štruktúru: problém (ako by si to riešil v C/C++), Rust riešenie, "kompilátor hovorí nie" — čo sa nestane skompilovať a prečo — a spustiteľný príklad. Nie učebnicové snippety, ale veci ktoré sa naozaj spúšťajú a môžeš si ich zmeniť a vidieť čo sa stane.

Kapitola 1 je toolchain — `rustup`, `cargo`, a všetko čo potrebuješ vedieť predtým než napíšeš prvý riadok. Kapitola 2 je ownership — to najdôležitejšie. Ak pochopíš ownership, zvyšok Rustu plynie prirodzene.

Začíname.
