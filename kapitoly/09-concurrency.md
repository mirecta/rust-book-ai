# Kapitola 9 — Concurrency

Paralelizmus je jednou z tých tém, kde sa C a C++ programátori tradične správajú ako vojaci v minenom poli — každý krok je opatrný, každý mutex je s hrôzou skontrolovaný trikrát a aj tak sa raz za čas niečo pokazí. Data race v C++ je undefined behavior. To znamená, že kompilátor môže v prítomnosti data race urobiť čokoľvek — a robí to, pretože optimalizátor predpokladá, že UB nenastane. Výsledkom sú bugy, ktoré sa objavujú len v produkčnom builde, len v utorok o 3:17 ráno, a len na konkrétnom serveri.

Rust prináša iný prístup: "fearless concurrency". Nie dokumentáciou, nie code review, nie sanitizermi — typovým systémom. Ak kód skompiluje, nie sú data races. Táto garancia nie je marketingová — je matematicky dokázateľná z pravidiel ownership a typov `Send`/`Sync`. Kompromis je, že sa naučíš niečo nové. Ale je to rovnaké nové, čo ťa aj tak donúti naučiť sa každý seriózny crash v produkčnom C++ kóde, akurát teraz to príde skôr a s lepšou chybovou hláškou.

V tejto kapitole prejdeme celé spektrum — od OS threadov cez message passing až po plnohodnotný async runtime. Pri každom kroku ukazujem, čo by to znamenalo v C/C++ a kde bežne padajú začiatočníci.

---

## std::thread — ako pthread, ale s ownership

V C píšeš `pthread_create`, odovzdáš `void*` argument a dúfaš, že lifetime toho, čo pointer ukazuje, prežije thread. Rust to rieši inak: closure, ktorú posielaš do threadu, musí implementovať trait `Send`, a ak chytáš premenné z okolitého scope, musíš ich explicitne preniesť pomocou `move`. Kompilátor ti nedovolí náhodne zdieľať referenciu na stack premennú, pretože stackový rámec volajúceho môže byť dávno gone, kým thread beží.

```rust
use std::thread;
use std::time::Duration;

fn main() {
    let handle = thread::spawn(|| {
        for i in 0..5 {
            println!("thread: {}", i);
            thread::sleep(Duration::from_millis(50));
        }
    });

    for i in 0..3 {
        println!("main: {}", i);
        thread::sleep(Duration::from_millis(80));
    }

    handle.join().unwrap();  // čakaj na dokončenie
}
```

`thread::spawn` vracia `JoinHandle<T>` kde `T` je návratová hodnota closure. Keď zavoláš `handle.join()`, dostaneš `Result<T, Box<dyn Any + Send>>` — thread mohol spanikovať a v tom prípade dostaneš `Err`. V C by si zavolal `pthread_join` a nevedel by si nič o tom, či thread skončil panikom alebo normálne.

Čo sa stane, ak `join` nezavoláš? `JoinHandle` sa dropne a thread beží ako detached — Rust tu nerobí nic špeciálne odlišne od pthreads. Program môže skončiť skôr ako thread. V dlhobežiacich aplikáciách to zvyčajne nie je čo chceš, preto odporúčam `join` vždy explicitne volať alebo uchovávať `JoinHandle` v nejakej kolekcii a joinovať pri shutdowne.

### Move closure pre threading

Toto je klasická chyba začiatočníka:

```rust
let data = vec![1u32, 2, 3, 4, 5];

// Toto NESKOMPILUJE — data môže žiť kratšie ako thread
// let handle = thread::spawn(|| {
//     println!("{:?}", data);  // chyba: data môže prežiť stack frame
// });

// Správne — data sa prenesie do threadu (move sémantika)
let handle = thread::spawn(move || {
    let sum: u32 = data.iter().sum();
    println!("suma: {}", sum);
    sum  // thread môže vrátiť hodnotu
});

let result = handle.join().unwrap();
println!("výsledok: {}", result);
```

Kľúčové slovo `move` pred closure spôsobí, že všetky zachytené premenné sa presunú (nie skopírujú) do closure. Po tomto riadku `data` v pôvodnom scope neexistuje — vlastníctvo prešlo do threadu. Ak chceš pristupovať k dátam z oboch strán, musíš použiť `Arc` (pozri nižšie).

V C ekvivalen by si alokoval štruktúru na heape, naplnil ju dátami, odovzdal `void*` do `pthread_create` a v threade castol naspäť. Rust to robí automaticky a bez `void*`.

### Paralelné spracovanie — fan-out/fan-in pattern

Klasický vzor pre CPU-bound prácu: rozdeliť na kusy, každý kus spracovať v separátnom threade, výsledky zbierať:

```rust
fn parallel_sum(chunks: Vec<Vec<u32>>) -> u32 {
    let handles: Vec<_> = chunks.into_iter()
        .map(|chunk| {
            thread::spawn(move || chunk.iter().sum::<u32>())
        })
        .collect();

    handles.into_iter()
        .map(|h| h.join().unwrap())
        .sum()
}
```

Všimni si, že `.collect()` je nutné medzi `map` (spawn) a druhým `map` (join). Keby si to napísal ako jeden reťazec iterátora, thready by sa spawnovali a okamžite joinovali jeden po druhom — stratil by si paralelizmus. Toto je jeden z tých jemných gotchov, na ktorý naraziš prvýkrát.

### Threadpool vs. spawn-per-request

Spawnovanie OS threadu nie je zadarmo — alokácia stacku (typicky 8 MB na Linuxe), syscall, kernel scheduling overhead. Pre tisíce requestov za sekundu nechceš spawnovať thread na každý request. V produkčnom kóde sa buď používa `rayon` (pozri nižšie) alebo vlastný threadpool. Tu je jednoduchý príklad:

```rust
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Sender};

type Job = Box<dyn FnOnce() + Send + 'static>;

struct ThreadPool {
    workers: Vec<thread::JoinHandle<()>>,
    sender: Sender<Job>,
}

impl ThreadPool {
    fn new(size: usize) -> Self {
        let (tx, rx) = mpsc::channel::<Job>();
        let rx = Arc::new(Mutex::new(rx));

        let workers = (0..size)
            .map(|id| {
                let rx = Arc::clone(&rx);
                thread::spawn(move || loop {
                    let job = rx.lock().unwrap().recv();
                    match job {
                        Ok(f) => {
                            println!("worker {id} beží job");
                            f();
                        }
                        Err(_) => {
                            println!("worker {id} končí");
                            break;
                        }
                    }
                })
            })
            .collect();

        ThreadPool { workers, sender: tx }
    }

    fn execute<F: FnOnce() + Send + 'static>(&self, f: F) {
        self.sender.send(Box::new(f)).unwrap();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        // sender sa dropne, channel sa uzavrie, workeri skončia
        // workers sa joinujú automaticky cez drain
        for w in self.workers.drain(..) {
            w.join().unwrap();
        }
    }
}

fn main() {
    let pool = ThreadPool::new(4);

    for i in 0..8 {
        pool.execute(move || {
            println!("job {i} na threade {:?}", thread::current().id());
        });
    }
    // drop(pool) počká na všetkých workerov
}
```

Toto je minimalistický threadpool, ale demonštruje kombinovanie `mpsc`, `Arc<Mutex<_>>` a RAII cleanup cez `Drop`. Produkčné projekty zvyčajne používajú `rayon` alebo `crossbeam`.

---

## Channels — message passing

Go popularizoval motto "share memory by communicating, don't communicate by sharing memory". Rust má `mpsc` (multiple producer, single consumer) kanály ako súčasť štandardnej knižnice. Fungujú ako goroutine channels, ale bez goroutines — pošleš správu a ownership správy sa prenesie na druhú stranu. Žiadne kopírovanie, žiadne zdieľanie, žiadna synchronizácia potrebná.

`mpsc` = multiple producer, single consumer (ako Go channels ale bez goroutines):

```rust
use std::sync::mpsc;

fn main() {
    let (tx, rx) = mpsc::channel::<String>();

    // Viac producentov
    let tx2 = tx.clone();
    let t1 = thread::spawn(move || {
        tx.send("od t1: hello".to_string()).unwrap();
        tx.send("od t1: world".to_string()).unwrap();
    });

    let t2 = thread::spawn(move || {
        tx2.send("od t2: ping".to_string()).unwrap();
    });

    t1.join().unwrap();
    t2.join().unwrap();

    // Receiver — iteruje kým channel nie je uzavretý
    // Channel sa uzavrie keď padnú všetci senders (tx aj všetky klony)
    for msg in rx {
        println!("{}", msg);
    }
}
```

Čo sa tu deje s ownership: `tx.send(msg)` presunie `msg` do kanála. Keď `rx` prijme správu, dostane ju s plným vlastníctvom. Ak by si chcel správu poslať a zároveň si ju ponechať, musel by si ju naklonovať alebo použiť `Arc`. Toto je zámerné — channel je typicky najjednoduchší spôsob ako sa vyhnúť zdieľanému stavu.

Dôležitý detail: `for msg in rx` blokuje volajúci thread a iteruje kým nie sú všetci senders dropnutí. V príklade vyššie je to správne — po `join()` sú oba thready mŕtve, teda oba `tx` sú dropnuté, channel je uzavretý a iterátor skončí. Ak zabudneš dropnúť sender, `for msg in rx` bude čakať večne.

### Bounded channel — backpressure

```rust
// Unbounded channel — send nikdy neblokuje (môže narásť do OOM)
let (tx_unbounded, rx_unbounded) = mpsc::channel::<Vec<u8>>();

// Bounded channel — buffer max 10 správ
// send() blokuje keď je buffer plný — prirodzený backpressure
let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(10);
```

Backpressure je dôležitý koncept systémového programovania. Ak producent generuje dáta rýchlejšie ako ich konzument spracúva a nemáš backpressure, skončíš s neobmedzeným rastom pamäte. `sync_channel` rieši toto elegantne — ak je buffer plný, `send` sa zablokuje a tým spomalí producenta. V C by si musel implementovať semafór, podmienkovú premennú a buffer manuálne.

### Pipeline pattern — reťazenie channelov

Reálny príklad: sieťový proxy, ktorý číta raw byty, dekóduje správy, filtruje a zapisuje:

```rust
fn pipeline_example() {
    // Stage 1: raw bytes reader
    let (raw_tx, raw_rx) = mpsc::sync_channel::<Vec<u8>>(32);
    // Stage 2: decoder
    let (decoded_tx, decoded_rx) = mpsc::sync_channel::<String>(32);

    // Producent: generuje raw byty
    let producer = thread::spawn(move || {
        for i in 0..10u32 {
            let msg = format!("message {i}");
            raw_tx.send(msg.into_bytes()).unwrap();
        }
    });

    // Stage 1->2: dekódovanie bajtov na String
    let decoder = thread::spawn(move || {
        for bytes in raw_rx {
            if let Ok(s) = String::from_utf8(bytes) {
                decoded_tx.send(s).unwrap();
            }
        }
    });

    // Konzument: vypíše správy
    let consumer = thread::spawn(move || {
        for msg in decoded_rx {
            println!("prijatá: {msg}");
        }
    });

    producer.join().unwrap();
    decoder.join().unwrap();
    consumer.join().unwrap();
}
```

Toto je klasický Unix pipe model preniesený do threaded kódu. Každá stage beží nezávisle, backpressure sa šíri automaticky cez bounded channels. V Go by si použil goroutines s channels — v Ruste si musíš spawnovať OS thready alebo prejsť na tokio (pozri nižšie).

---

## Arc\<T\> + Mutex\<T\> — zdieľaný stav

Niekedy message passing nestačí. Ak máš veľkú dátovú štruktúru (napr. cache, in-memory databázu), nechceš kopírovať celú pri každej operácii. Tu prichádza klasická kombinovanie `Arc<Mutex<T>>`.

`Arc` = Atomic Reference Counting — thread-safe verzia `Rc`. Každý klon `Arc` zvýši atómový counter a každý drop ho zníži. Keď counter padne na nulu, pamäť sa uvoľní. `Mutex<T>` obaľuje `T` a garantuje exkluzívny prístup cez RAII guard.

```rust
use std::sync::{Arc, Mutex};

fn main() {
    // Arc = Atomic Reference Counting — thread-safe Rc
    // Mutex<T> = Guard objekt ktorý dáva exkluzívny prístup k T
    let counter = Arc::new(Mutex::new(0u64));

    let handles: Vec<_> = (0..8).map(|_| {
        let c = Arc::clone(&counter);
        thread::spawn(move || {
            for _ in 0..1000 {
                *c.lock().unwrap() += 1;
                // MutexGuard sa automaticky uvoľní pri drop (RAII)
                // V C: pthread_mutex_lock / pthread_mutex_unlock
            }
        })
    }).collect();

    for h in handles { h.join().unwrap(); }
    println!("výsledok: {}", *counter.lock().unwrap()); // vždy 8000
}
```

Porovnaj s C: `pthread_mutex_t` musíš inicializovať, lockovať, odomknúť v každom `return` a `goto` path, a zničiť. Rust to robí cez `Drop` — keď `MutexGuard` vypadne zo scope, mutex sa automaticky odomkne. Nie je možné zabudnúť odomknúť.

### Mutex Poisoning — čo je `unwrap()` na `lock()`

Zaujímavý detail: `mutex.lock()` vracia `Result`, nie priamo guard. Prečo? Ak thread spanikovával kým držal zámok, Rust mutex sa "otrávi" (poisoned). Každý ďalší `lock()` vráti `Err(PoisonError)`. Toto ti dá vedieť, že chránené dáta mohli byť v nekonzistentnom stave. V produkčnom kóde:

```rust
let data = match shared.lock() {
    Ok(guard) => guard,
    Err(poisoned) => {
        // Môžeme sa rozhodnúť pokračovať s dátami aj tak
        eprintln!("Mutex bol otrávený, pokračujeme...");
        poisoned.into_inner()
    }
};
```

### Deadlock — klasická chyba

Deadlock nastane keď dva thready čakajú na mutex, ktorý drží ten druhý. Rust ti nepomáha detekovať deadlocky staticky (je to NP-ťažký problém v generálnom prípade). Ale dodržiavanie konzistentného poradia lockovania pomáha:

```rust
// NEBEZPEČNÉ — môže deadlocknúť
fn transfer_bad(
    account_a: &Mutex<u64>,
    account_b: &Mutex<u64>,
    amount: u64,
) {
    let mut a = account_a.lock().unwrap();
    let mut b = account_b.lock().unwrap();  // thread 2 môže mať b a čakať na a
    *a -= amount;
    *b += amount;
}

// BEZPEČNÉ — vždy lockuj v konzistentnom poradí (napr. podľa adresy)
fn transfer_safe(
    account_a: &Mutex<u64>,
    account_b: &Mutex<u64>,
    amount: u64,
) {
    // Lockuj v poradí podľa raw adresy — vždy deterministické
    let a_ptr = account_a as *const _ as usize;
    let b_ptr = account_b as *const _ as usize;

    if a_ptr < b_ptr {
        let mut a = account_a.lock().unwrap();
        let mut b = account_b.lock().unwrap();
        *a -= amount;
        *b += amount;
    } else {
        let mut b = account_b.lock().unwrap();
        let mut a = account_a.lock().unwrap();
        *a -= amount;
        *b += amount;
    }
}
```

### RwLock — viacero čitateľov

Pre read-heavy workloady (napr. konfigurácia, cache) je `Mutex` zbytočne agresívny. `RwLock` umožňuje buď exkluzívny write zámok alebo ľubovoľný počet paralelných read zámkov:

```rust
use std::sync::RwLock;
use std::collections::HashMap;

let config = Arc::new(RwLock::new(HashMap::<String, String>::new()));

// Writer — exkluzívny, blokuje všetkých readerov
{
    let mut cfg = config.write().unwrap();
    cfg.insert("host".into(), "localhost".into());
    cfg.insert("port".into(), "8080".into());
}  // write zámok sa uvoľní tu

// Viacero readerov súčasne — nebloking navzájom
let readers: Vec<_> = (0..4).map(|_| {
    let cfg = Arc::clone(&config);
    thread::spawn(move || {
        let cfg = cfg.read().unwrap();
        println!("host: {:?}", cfg.get("host"));
    })
}).collect();

for r in readers { r.join().unwrap(); }
```

Pozor: na niektorých platformách môžu write-heavy workloady s `RwLock` byť pomalšie ako s `Mutex`, pretože `RwLock` má vyšší overhead. Meranie je nutné.

### Send a Sync traity — základ bezpečnosti

Celý Rust concurrency systém stojí na dvoch markerových traitoch:

- `Send`: typ môže byť prenesený (move) do iného threadu
- `Sync`: referencia na typ môže byť zdieľaná medzi threadmi (`&T: Send`)

```rust
// Kompilátor overuje automaticky:
fn requires_send<T: Send>(val: T) {}
fn requires_sync<T: Sync>(val: &T) {}

// Rc<T> nie je Send — počítač referencií nie je atómový
// → použiť Arc<T>
//
// Cell<T>, RefCell<T> nie sú Sync — interior mutability bez synchronizácie
// → použiť Mutex<T> alebo AtomicT
//
// Mutex<T> je Send + Sync — správna ochrana
//
// *mut T, *const T nie sú Send/Sync — raw pointer, unsafe teritorium
// → musíš implementovať Send/Sync manuálne s unsafe impl
```

Ak sa pokúsiš poslať `Rc<T>` do threadu, kompilátor ti vyhodí chybu: `` `Rc<Vec<i32>>` cannot be sent between threads safely ``. V C by si to urobil bez problémov a dostal data race v runtime. V Ruste to vidíš v kompile.

Vlastné typy sú automaticky `Send + Sync` ak všetky ich polia sú `Send + Sync`. Ak máš typ, ktorý nie je ani jedno (napr. obsahuje `*mut T`), musíš implementovať `Send`/`Sync` s `unsafe impl` a manuálne garantovať bezpečnosť.

---

## Tokio — async runtime

Teraz sa dostávame k najväčšej téme tejto kapitoly. Async Rust je mocný, ale má reputáciu byť ťažký. Poďme si to rozpitvať od základov.

### Prečo async vôbec existuje?

Predstav si webový server, ktorý obsluhuje 10 000 súčasných HTTP requestov. S thread-per-request modelom potrebuješ 10 000 OS threadov. Každý thread má stack (~8 MB default na Linuxe) = 80 GB RAM len na stacky. Navyše context switching medzi tisíckami threadov je drahý.

Riešenie: jeden (alebo niekoľko) OS threadov, ktoré obsluhujú tisíce "virtuálnych" taskáv pomocou cooperative scheduling a event loop. Keď task čaká na I/O (socket read, disk write, timer), vzdá sa CPU a nechá executor spustiť iný task. Toto je model, na ktorom beží Node.js, nginx, Redis, a teraz aj Rust s Tokio.

Tokio je najrozšírenejší async runtime v Ruste. Pod kapotou používa epoll na Linuxe (kqueue na macOS, IOCP na Windows), executor ktorý plánuje futures na threadoch, a reaktor ktorý prekladá I/O udalosti na wakeup calls.

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
```

### Čo je async/await

```rust
use std::time::Duration;

// Synchronná funkcia — BLOKUJE thread počas celého spánku
fn sync_delay() {
    std::thread::sleep(Duration::from_millis(100));
    // thread nemôže robiť nič iné tých 100ms
}

// Asynchrónna funkcia — UVOĽNÍ thread počas čakania
async fn async_delay() {
    tokio::time::sleep(Duration::from_millis(100)).await;
    // thread môže obsluhovať iné tasky počas tých 100ms
}

// async fn vracia Future<Output = ()> — je LAZY
// Nespustí sa kým sa neawaituje alebo nedá do spawn/block_on
```

`async fn` je syntaktický cukor pre funkciu, ktorá vracia `impl Future<Output = T>`. `Future` je trait s jednou metódou `poll(cx: &mut Context) -> Poll<Output>`. Executor opätovne volá `poll` na každom future kým nevrátí `Poll::Ready(value)`. Keď future vrátí `Poll::Pending`, executor vie, že má čakať na wakeup (ktorý príde cez `cx.waker()`).

### Pod kapotou — ako Tokio funguje

Poďme sa pozrieť, čo sa naozaj deje, keď píšeš `tokio::time::sleep(d).await`:

1. `sleep(d)` vytvorí `Sleep` future a zaregistruje timer v **reaktore** (globálna inštancia, ktorá drží epoll/kqueue file descriptor)
2. `.await` rozbalí future a zavolá `poll(cx)` po prvý raz
3. `Sleep::poll` zistí, že timer ešte nebehí → vráti `Poll::Pending`
4. **Executor** (thread pool pracovníkov) presunie tento task na "čakaciu" frontu
5. Po uplynutí timeoutu reaktor dostane notifikáciu (pomocou `timerfd` na Linuxe alebo `DISPATCH_SOURCE_TYPE_TIMER` na macOS)
6. Reaktor zavolá `waker.wake()` — tým sa task vráti do "pripravených" fronty
7. Executor znova zavolá `poll(cx)` na danom future
8. `Sleep::poll` vráti `Poll::Ready(())` — `.await` pokračuje

```
                    ┌─────────────────────────────────┐
                    │           Tokio Runtime          │
                    │                                  │
  .await  ────────► │  Executor (thread pool)          │
                    │   - work-stealing queues         │
                    │   - poll() loop                  │
                    │         │                        │
                    │         ▼                        │
                    │  Reactor (event loop)            │
                    │   - epoll/kqueue/IOCP            │
                    │   - socket readiness             │
                    │   - timer expiration             │
                    │         │                        │
                    │         ▼                        │
                    │  Waker → wake() → re-schedule   │
                    └─────────────────────────────────┘
```

Tokio multi-thread runtime (`#[tokio::main]` default) používa work-stealing scheduler — každý worker thread má vlastnú deque frontu taskov. Keď je worker idle, kradne tasky od iných workerov. Toto minimalizuje blokovanie.

### #[tokio::main]

```rust
#[tokio::main]
async fn main() {
    println!("štart");
    tokio::time::sleep(Duration::from_millis(100)).await;
    println!("po 100ms");
}

// Makro expanduje na:
fn main() {
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async {
            println!("štart");
            tokio::time::sleep(Duration::from_millis(100)).await;
            println!("po 100ms");
        });
}

// Pre testy a single-thread runtime:
#[tokio::main(flavor = "current_thread")]
async fn main() {
    // Jeden thread, cooperative scheduling — ideálne pre testy
}
```

### Častá chyba: blokovanie async runtimeu

Toto je najčastejší performance bug v async Ruste. Nikdy nevolaj blokujúce operácie v async kontexte:

```rust
// ZLÉ — blokuje celý worker thread, ostatné tasky nemôžu bežať
#[tokio::main]
async fn main() {
    // std::thread::sleep blokuje OS thread!
    std::thread::sleep(Duration::from_secs(5));  // ← NIKDY TOTO
    println!("po 5 sekundách, ale ostatné tasky boli zablokované");
}

// ZLÉ — rovnaký problém so súborovými operáciami
async fn read_file_bad(path: &str) -> String {
    std::fs::read_to_string(path).unwrap()  // ← blokuje thread!
}

// SPRÁVNE — tokio::time::sleep uvoľní thread
async fn delay_good() {
    tokio::time::sleep(Duration::from_secs(5)).await;
}

// SPRÁVNE — tokio::fs pre async file I/O
async fn read_file_good(path: &str) -> String {
    tokio::fs::read_to_string(path).await.unwrap()
}

// Ak MUSÍŠ volať blokujúci kód (napr. CPU-heavy operácia alebo blokujúce API),
// použi spawn_blocking — spustí na separátnom blocking thread pool:
async fn heavy_computation() -> u64 {
    tokio::task::spawn_blocking(|| {
        // Tu môžeš blokuvať bez problémov
        (0..10_000_000u64).sum()
    })
    .await
    .unwrap()
}
```

Pravidlo palca: ak funkcia nemá `.await`, buď je rýchla CPU operácia (μs), alebo patrí do `spawn_blocking`. Nikdy `std::thread::sleep`, nikdy blokujúce file/network I/O priamo v async funkciách.

### tokio::spawn — asynchrónne tasky

```rust
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    let t1 = tokio::spawn(async {
        sleep(Duration::from_millis(200)).await;
        "A hotovo"
    });

    let t2 = tokio::spawn(async {
        sleep(Duration::from_millis(100)).await;
        "B hotovo"
    });

    // Oba bežia súčasne — cooperative scheduling na rovnakých threadoch
    // join! čaká na OBA, select! by čakal na PRVÉHO
    let (r1, r2) = tokio::join!(t1, t2);
    println!("{}", r1.unwrap());
    println!("{}", r2.unwrap());
    // Celkový čas: ~200ms, nie 300ms
}
```

`tokio::spawn` je ekvivalent `thread::spawn` pre async svet — vytvorí nový task na executore. Vracia `JoinHandle<T>`. Rozdiel: OS thready sú drahé (MBs stack), tokio tasky sú lacné (niekoľko kilobytov).

Pozor na lifetime: podobne ako `thread::spawn`, `tokio::spawn` vyžaduje `'static` bounds. Nemôžeš spawnovať task, ktorý drží referenciu na lokálnu premennú. Ak chceš zdieľať dáta, použi `Arc`.

### tokio::select! — čakaj na prvého

Async ekvivalent `epoll`/`select()` — čakaj na ktorúkoľvek udalosť:

```rust
use tokio::time::{sleep, Duration, timeout};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    let (tx, mut rx) = mpsc::channel::<String>(16);

    tokio::spawn(async move {
        sleep(Duration::from_millis(500)).await;
        tx.send("správa od spawnovaného tasku".to_string()).await.unwrap();
    });

    let fast = sleep(Duration::from_millis(100));
    let slow = sleep(Duration::from_millis(1000));

    // select! čaká na prvú hotovú vetvu, ostatné zruší (cancel)
    tokio::select! {
        _ = fast => println!("fast vyhral"),
        _ = slow => println!("slow vyhral"),
        msg = rx.recv() => println!("správa: {:?}", msg),
    }

    // Timeout wrapper — elegantnejší ako select! pre jednoduchý timeout
    match timeout(Duration::from_millis(500), slow_operation()).await {
        Ok(result) => println!("výsledok: {:?}", result),
        Err(_) => println!("timeout!"),
    }
}

async fn slow_operation() -> u32 {
    sleep(Duration::from_millis(300)).await;
    42
}
```

`select!` je mocný nástroj, ale má subtilitu: keď vyhrá jedna vetva, ostatné futures sa **zrušia** (dropped). To môže byť problém ak mal iný branch napríklad otvorený databázový transakciu. Pre cancellation-safe kód treba používať `tokio::select!` opatrne alebo kombinovať s `CancellationToken`.

### Async TCP server — reálny príklad

Toto je ten moment, kedy async Rust svieti. Tisíce súčasných klientov bez tisícok threadov:

```rust
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn handle_client(mut stream: TcpStream) {
    let peer = stream.peer_addr().unwrap();
    println!("klient pripojen: {peer}");

    let mut buf = [0u8; 1024];
    loop {
        match stream.read(&mut buf).await {
            Ok(0) => {
                println!("klient odpojený: {peer}");
                break;  // klient zatvoril spojenie (EOF)
            }
            Ok(n) => {
                // Echo server — pošli naspäť rovnaké bajty
                if stream.write_all(&buf[..n]).await.is_err() {
                    break;
                }
            }
            Err(e) => {
                eprintln!("chyba pri čítaní od {peer}: {e}");
                break;
            }
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Počúvam na :8080");

    loop {
        let (stream, addr) = listener.accept().await?;
        println!("nový klient: {}", addr);

        // Každý klient dostane vlastný task — nie thread
        // task je lacný (~KB), thread je drahý (~MB)
        tokio::spawn(async move {
            handle_client(stream).await;
        });
        // Keď task skončí, stream sa automaticky zatvorí (Drop)
    }
}
```

V C by toto bol ~300 riadkov s epoll, non-blocking sockets, event loop a manuálnym state machine pre každé spojenie. V Ruste s Tokio je to 30 riadkov a async/await sa preloží na strojovo efektívný state machine za teba.

### Graceful shutdown — reálny pattern

Produkčný server potrebuje vedieť zastaviť sa čisto — dokončiť bežiace requesty, zatvoriť spojenia:

```rust
use tokio::signal;
use tokio::sync::broadcast;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn handle_client(mut stream: TcpStream, mut shutdown: broadcast::Receiver<()>) {
    let mut buf = [0u8; 1024];
    loop {
        tokio::select! {
            result = stream.read(&mut buf) => {
                match result {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if stream.write_all(&buf[..n]).await.is_err() {
                            break;
                        }
                    }
                }
            }
            _ = shutdown.recv() => {
                println!("shutdown signál — zatvárám spojenie");
                break;
            }
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    loop {
        tokio::select! {
            Ok((stream, _)) = listener.accept() => {
                let shutdown_rx = shutdown_tx.subscribe();
                tokio::spawn(handle_client(stream, shutdown_rx));
            }
            _ = signal::ctrl_c() => {
                println!("Ctrl+C — zastavujem server");
                let _ = shutdown_tx.send(());
                break;
            }
        }
    }

    Ok(())
}
```

### tokio::sync — async synchronizačné primitívy

Pozor: `std::sync::Mutex` môžeš použiť v async kóde, ale len ak ho nikdy nedržíš cez `.await`. Ak potrebuješ držať mutex cez await point, použi `tokio::sync::Mutex`:

```rust
use tokio::sync::{Mutex, RwLock, Semaphore};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    // tokio::sync::Mutex — async-aware, bezpečný cez .await
    let shared = Arc::new(Mutex::new(vec![1u32, 2, 3]));

    let s = Arc::clone(&shared);
    tokio::spawn(async move {
        let mut data = s.lock().await;  // .await, nie .unwrap()
        data.push(42);
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        // Tu môžeme držať zámok cez await — tokio::sync::Mutex to podporuje
    });

    // Semaphore — limit počtu súčasných operácií (napr. DB connections)
    let semaphore = Arc::new(Semaphore::new(10));  // max 10 súčasných

    let sem = Arc::clone(&semaphore);
    tokio::spawn(async move {
        let _permit = sem.acquire().await.unwrap();
        // Maximálne 10 taskov tu môže byť naraz
        // permit sa dropne na konci scope
    });
}
```

### tokio::time — async časovače

```rust
use tokio::time::{sleep, interval, Duration, Instant};

#[tokio::main]
async fn main() {
    // Jednorázový delay
    sleep(Duration::from_secs(1)).await;

    // Periodický interval (tick-based) — ideálne pre heartbeaty, polling
    let mut ticker = interval(Duration::from_millis(100));
    for _ in 0..5 {
        ticker.tick().await;  // prvý tick je okamžitý
        println!("tick o {:?}", Instant::now());
    }

    // Interval s MissedTickBehavior — čo ak tick zaostane?
    use tokio::time::MissedTickBehavior;
    let mut precise_ticker = interval(Duration::from_millis(100));
    precise_ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    // Skip — preskočí vynechané ticky (vhodné pre rate limiting)
    // Burst — doženie zmeškaný čas (vhodné pre metriky)
    // Delay — posunie interval (vhodné pre pravidelné úlohy)
}
```

### tokio::fs a tokio::net — async I/O

```rust
use tokio::fs;
use tokio::net::UdpSocket;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Async file I/O
    let content = fs::read_to_string("/etc/hostname").await?;
    println!("hostname: {}", content.trim());

    fs::write("/tmp/rust-test.txt", b"hello async").await?;

    // Async UDP socket
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.connect("8.8.8.8:53").await?;

    // DNS query (simplified)
    let query = [0u8; 12]; // real DNS packet would go here
    socket.send(&query).await?;

    let mut buf = [0u8; 512];
    tokio::select! {
        Ok(n) = socket.recv(&mut buf) => {
            println!("prijatých {} bajtov DNS odpovede", n);
        }
        _ = tokio::time::sleep(Duration::from_secs(2)) => {
            println!("DNS timeout");
        }
    }

    Ok(())
}
```

---

## Rayon — data parallelism

Pre CPU-bound workloady (nie I/O), kde chceš využiť všetky jadrá bez ručného spawnovania threadov, `rayon` je dokonalé riešenie. Stačí zmeniť `.iter()` na `.par_iter()` a Rayon automaticky rozdelí prácu na threadpool.

```toml
[dependencies]
rayon = "1"
```

```rust
use rayon::prelude::*;

fn main() {
    let data: Vec<u64> = (0..10_000_000).collect();

    // Sekvenčne — jeden thread
    let sum_seq: u64 = data.iter().sum();

    // Paralelne — automatické rozdelenie na N threadov (N = počet jadier)
    let sum_par: u64 = data.par_iter().sum();

    assert_eq!(sum_seq, sum_par);

    // Paralelný map+filter+collect
    let processed: Vec<u64> = data.par_iter()
        .filter(|&&x| x % 2 == 0)
        .map(|&x| x * x % 1_000_007)
        .collect();

    println!("spracovaných {} položiek", processed.len());

    // Paralelný sort — Rayon má vlastný parallel merge sort
    let mut v: Vec<i64> = (0..100_000i64).rev().collect();
    v.par_sort();
    assert!(v.windows(2).all(|w| w[0] <= w[1]));
}
```

Rayon pod kapotou tiež používa work-stealing scheduler podobne ako Tokio. Ale je synchronný — každý `par_iter()` call blokuje aktuálny thread kým nie je hotový.

### Rayon s vlastnými typmi

```rust
use rayon::prelude::*;

#[derive(Debug)]
struct Packet {
    id: u32,
    payload: Vec<u8>,
}

fn process_packet(p: &Packet) -> u32 {
    // Simulácia CPU-heavy spracovania (napr. checksum)
    p.payload.iter().fold(0u32, |acc, &b| acc.wrapping_add(b as u32))
}

fn main() {
    let packets: Vec<Packet> = (0..1000)
        .map(|i| Packet {
            id: i,
            payload: vec![i as u8; 1024],
        })
        .collect();

    // Paralelné spracovanie paketov
    let checksums: Vec<(u32, u32)> = packets
        .par_iter()
        .map(|p| (p.id, process_packet(p)))
        .collect();

    println!("spracovaných {} paketov", checksums.len());
}
```

### Rayon vs Tokio — kedy čo

| | Rayon | Tokio |
|---|---|---|
| Use case | CPU-bound (výpočty, kompresia, kryptografia) | I/O-bound (siete, disky, databázy) |
| Model | Synchronný, blokujúci | Asynchronný, non-blokujúci |
| Overhead | Threadpool switch | Task poll + wake cycle |
| Kedy použiť | Data parallelism, batch processing | Concurrent connections, microservices |
| Kombinácia | `spawn_blocking` v Tokio volá Rayon | Rayon môže volať Tokio v `block_on` |

V reálnom sieťovom serveri je typická kombinácia: Tokio pre sieťové I/O, a `spawn_blocking` alebo Rayon pre CPU-heavy spracovanie requestov (napr. JSON parsing, kompresia).

---

## Zhrnutie — concurrency toolbox

| Situácia | Rust nástroj | C/C++ ekvivalent |
|---|---|---|
| OS thread | `thread::spawn` | `pthread_create` / `std::thread` |
| Čakanie na thread | `handle.join()` | `pthread_join` |
| Mutex (thread) | `Mutex<T>` (RAII) | `pthread_mutex_lock/unlock` (manuálne) |
| Read-many write-once | `RwLock<T>` | `pthread_rwlock_t` |
| Zdieľané vlastníctvo | `Arc<T>` | `shared_ptr` (ale nie thread-safe bez mutex) |
| Message passing | `mpsc::channel` | Semafór + ring buffer / Go channels |
| Backpressure | `mpsc::sync_channel(N)` | Bounded POSIX queue |
| Concurrent connections | `tokio::spawn` | epoll + callback soup |
| Event multiplexing | `tokio::select!` | `epoll_wait` / `select()` |
| Data parallelism | `rayon::par_iter()` | OpenMP / TBB |
| CPU-heavy v async | `spawn_blocking` | Thread na strane |
| Data race | Compile error | Undefined behavior |

Rust výhody oproti C/C++ nie sú len bezpečnosť — sú to aj ergonómia (RAII všade, žiadne ručné unlock), expressivita (type system kóduje kontrakty), a výkon (zero-cost abstrakcie). `Arc<Mutex<T>>` nie je pomalší ako `pthread_mutex_t` + ručné riadenie lifetimov — kompilátor generuje rovnaký strojový kód.

Ďalšia kapitola: Unsafe Rust — keď potrebuješ vystúpiť zo zóny bezpečnosti.
