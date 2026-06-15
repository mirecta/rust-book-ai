# Kapitola 10 — Unsafe Rust

Safe Rust garantuje memory safety. Unsafe Rust ti dôveruje, že vieš čo robíš — a vypne niektoré záruky kompilátora. Je to presne ako `__attribute__((noinline))` alebo `#pragma optimize` v C — exit hatch keď bežné nástroje nestačia.

Ale poďme byť úprimní: väčšina Rust programátorov nikdy nepotrebuje napísať `unsafe` blok vo vlastnom kóde. Štandardná knižnica, Tokio, Serde, a väčšina ekosystému to rieši za teba. Napriek tomu je dôležité vedieť, čo `unsafe` dovoľuje a čo neznamená — pretože keď čítaš cudzí kód alebo píšeš systémovú knižnicu, stretnúť sa s ním nevyhneš.

Analogia z C++: `reinterpret_cast`, `volatile`, `__builtin_unreachable()`, a inline assembler sú tiež escape hatche z typového systému. Ale v C++ sú tieto úniky všade, implicitné, a kompilátor ti nepomôže. V Ruste je `unsafe` explicitné, ohraničené, a auditovateľné. Keď robíš security review Rust kódu, grepdneš `unsafe {` a vieš presne kde hľadať.

---

## Čo unsafe dovolí

Len päť vecí (nie viac):

1. Dereferencovanie raw pointera (`*const T`, `*mut T`)
2. Volanie `unsafe` funkcie alebo metódy
3. Prístup k mutable global state (`static mut`)
4. Implementácia `unsafe` traitu
5. Prístup k poliam `union`

**Všetko ostatné — ownership, borrow checker, typy, generics, traits — platí aj v `unsafe` bloku.** Toto je kľúčová vec, ktorú začiatočníci nepochopia. `unsafe` nie je "vypni Rust". Je to "povolujem päť špeciálnych operácií naviac". Borrow checker stále beží. Typy stále musia sediať. Lifetimes stále platia. Jediný rozdiel je, že kompilátor ti verí pri týchto piatich operáciách.

### Čo unsafe nerieši — bežné mýty

```rust
// MÝTUS 1: unsafe vypne borrow checker
fn myth_1() {
    let x = 5;
    let r1 = &x;
    unsafe {
        // Stále nemôžeš mať mutable a immutable ref naraz:
        // let r2 = &mut x;  // ← compile error aj v unsafe bloku
    }
    println!("{r1}");
}

// MÝTUS 2: unsafe dovolí čítať neinicializovanú pamäť bez raw pointera
fn myth_2() {
    unsafe {
        // let x: u32;
        // println!("{x}");  // ← stále compile error
        // Ale môžeš použiť MaybeUninit:
        let mut x = std::mem::MaybeUninit::<u32>::uninit();
        x.write(42);
        let val = x.assume_init();  // unsafe — musíš garantovať init
        println!("{val}");
    }
}
```

---

## Raw pointery

V C je každý pointer raw. V Ruste existujú dva druhy pointerov: referencie (`&T`, `&mut T`) s plnými zárukami lifetimov a borrow checkera, a raw pointery (`*const T`, `*mut T`) bez akýchkoľvek záruk.

Raw pointer je ekvivalent C pointera — ukazuje na adresu v pamäti, nehovorí ti nič o tom, či je tá adresa validná, či objekt ešte existuje, či sú tam správne dáta. Vytvoriť raw pointer je bezpečné (len vytvoríš číslo reprezentujúce adresu). Dereferencovať ho (čítať/písať cez neho) je unsafe.

```rust
fn main() {
    let mut x = 42u32;

    // Vytvorenie raw pointera — bezpečné, žiadny unsafe blok
    let ptr: *const u32 = &x;
    let mut_ptr: *mut u32 = &mut x;

    // Dereferencovanie — unsafe, kompilátor ti verí
    unsafe {
        println!("cez *const: {}", *ptr);
        *mut_ptr += 1;
        println!("po zápise: {}", *mut_ptr);
    }
    println!("x = {}", x);  // 43

    // Null raw pointer — cast z 0 na pointer
    let null_ptr: *const u32 = std::ptr::null();
    unsafe {
        // Vždy over null pred dereferencovaním — inak segfault
        if !null_ptr.is_null() {
            println!("{}", *null_ptr);
        }
    }

    // Dangling pointer — klasický bug v C
    let dangling: *const u32;
    {
        let temp = 99u32;
        dangling = &temp as *const u32;
        // temp bude dropnuté na konci tohto bloku
    }
    // dangling teraz ukazuje na uvoľnenú pamäť
    // V C by si toto prečítal bez varovania → UB
    // V Ruste je to síce unsafe, ale aspoň je to EXPLICITNÉ:
    unsafe {
        // println!("{}", *dangling);  // UB — nerob to
    }
    // Rust ti nedovolí vytvoriť visiacu REFERENCIU, len dangling raw pointer
}
```

### Pointer aritmetika — ako v C, ale bezpečnejšie pomenovaná

```rust
fn main() {
    let arr = [10u32, 20, 30, 40, 50];
    let ptr = arr.as_ptr();  // *const u32, ukazuje na arr[0]

    unsafe {
        // ptr.add(n) — ekvivalent ptr + n v C, ale typovo bezpečný posun
        // (posúva o n * sizeof(T) bajtov, nie o n bajtov)
        let val = *ptr.add(2);
        println!("arr[2] = {}", val);  // 30

        // ptr.offset(n) — môže byť záporný
        let last = ptr.add(4);
        let third = last.offset(-2);
        println!("arr[2] cez offset: {}", *third);  // 30

        // Kopírovanie pamäte — ekvivalent memcpy
        let mut dst = [0u32; 5];
        std::ptr::copy_nonoverlapping(ptr, dst.as_mut_ptr(), 5);
        println!("{:?}", dst);  // [10, 20, 30, 40, 50]

        // copy (overlapping) — ekvivalent memmove
        std::ptr::copy(ptr, dst.as_mut_ptr().add(1), 3);
    }
}
```

Dôležitý rozdiel od C: `ptr.add(n)` je definovaný len pre pointery v rámci toho istého alokovaného objektu (plus jeden bajt za koniec). Ak pôjdeš mimo, je to UB aj v Ruste. Miri (Rust interpret) ti toto odhalí v testoch.

### Vlastná implementácia Vec — príklad kde raw pointery dávajú zmysel

Toto je ilustrácia toho, prečo `std::vec::Vec` interne používa raw pointery. Nejde to bez nich:

```rust
use std::alloc::{alloc, dealloc, realloc, Layout};
use std::ptr;

pub struct MyVec<T> {
    ptr: *mut T,
    len: usize,
    cap: usize,
}

impl<T> MyVec<T> {
    pub fn new() -> Self {
        MyVec {
            ptr: ptr::NonNull::dangling().as_ptr(),
            len: 0,
            cap: 0,
        }
    }

    pub fn push(&mut self, val: T) {
        if self.len == self.cap {
            self.grow();
        }
        unsafe {
            // Zapíš hodnotu na koniec, preveď vlastníctvo do alokovaného miesta
            ptr::write(self.ptr.add(self.len), val);
        }
        self.len += 1;
    }

    fn grow(&mut self) {
        let new_cap = if self.cap == 0 { 4 } else { self.cap * 2 };
        let new_layout = Layout::array::<T>(new_cap).unwrap();

        let new_ptr = if self.cap == 0 {
            unsafe { alloc(new_layout) }
        } else {
            let old_layout = Layout::array::<T>(self.cap).unwrap();
            unsafe { realloc(self.ptr as *mut u8, old_layout, new_layout.size()) }
        };

        self.ptr = new_ptr as *mut T;
        self.cap = new_cap;
    }

    pub fn get(&self, idx: usize) -> Option<&T> {
        if idx < self.len {
            unsafe { Some(&*self.ptr.add(idx)) }
        } else {
            None
        }
    }

    pub fn len(&self) -> usize { self.len }
}

impl<T> Drop for MyVec<T> {
    fn drop(&mut self) {
        // Dropni každý element
        unsafe {
            for i in 0..self.len {
                ptr::drop_in_place(self.ptr.add(i));
            }
            if self.cap > 0 {
                let layout = Layout::array::<T>(self.cap).unwrap();
                dealloc(self.ptr as *mut u8, layout);
            }
        }
    }
}

fn main() {
    let mut v = MyVec::new();
    v.push(10u32);
    v.push(20);
    v.push(30);
    println!("len={}, val[1]={:?}", v.len(), v.get(1));
}
```

Toto je zjednodušená verzia toho, čo robí `std::vec::Vec`. V reálnej implementácii je navyše `NonNull`, `PhantomData`, a ošetrenie ZST (zero-sized types). Pointa: unsafe je tu nutné, pretože alokátor pracuje s `*mut u8` a my potrebujeme typové pointery.

---

## unsafe funkcia a blok

```rust
// Funkcia označená unsafe — caller musí garantovať invarianty
// Invarianty MUSIA byť zdokumentované v /// komentároch
/// # Safety
/// `bytes` musí mať dĺžku rovnú `std::mem::size_of::<T>()`.
/// Bajty musia reprezentovať validnú hodnotu typu T.
/// Zarovnanie bytes.as_ptr() musí zodpovedať `align_of::<T>()`.
unsafe fn transmute_bytes<T: Copy>(bytes: &[u8]) -> T {
    assert_eq!(bytes.len(), std::mem::size_of::<T>());
    std::ptr::read_unaligned(bytes.as_ptr() as *const T)
}

fn main() {
    let bytes = [0x01u8, 0x00, 0x00, 0x00]; // little-endian 1

    // Caller musí splniť safety komentár vyššie
    let val: u32 = unsafe { transmute_bytes(&bytes) };
    println!("hodnota: {}", val);  // 1

    // Bezpečná alternatíva (preferuj toto keď existuje):
    let val2 = u32::from_le_bytes(bytes);
    println!("bezpečne: {}", val2);  // 1
}
```

Konvencia dokumentovania `unsafe` funkcií: sekcia `/// # Safety` popisuje čo musí volajúci garantovať. Toto je dôležité nielen pre ľudí, ale aj pre nástroje ako `cargo clippy` a `cargo deny`.

### `transmute` — najnebezpečnejšia funkcia v štandardnej knižnici

```rust
fn main() {
    // transmute<T, U>(v: T) -> U — reinterpretácia bitov
    // Ekvivalent type punning cez union v C, alebo *(U*)&v
    // Kompilátor odmietne ak sizeof(T) != sizeof(U)

    let f: f32 = 1.0;
    let bits: u32 = unsafe { std::mem::transmute(f) };
    println!("f32 1.0 = 0x{:08X}", bits);  // 0x3F800000 (IEEE 754)

    // Bezpečnejšia alternatíva od Rust 1.20:
    let bits2 = f.to_bits();
    let f2 = f32::from_bits(bits2);
    assert_eq!(f, f2);

    // Transmute na fn pointer — legitímny use case pre JIT/plugin systémy
    let code: Vec<u8> = vec![0x48, 0xC7, 0xC0, 0x2A, 0x00, 0x00, 0x00, 0xC3];
    // mov rax, 42; ret  — x86_64
    // V reálnom JIT by si mmap s PROT_EXEC a potom transmutoval na fn ptr

    // NIKDY toto:
    // let r: &u32 = unsafe { std::mem::transmute(0usize as *const u32) };
    // — null reference je okamžité UB

    // NIKDY transmute referenciu na pointer iného lifetime:
    // let s = String::from("hello");
    // let evil: &'static str = unsafe { std::mem::transmute(s.as_str()) };
    // — po drop(s) je evil dangling reference
}
```

V C je `*(float*)&int_val` bežný pattern pre bit manipulation (napr. quake fast inverse sqrt). V Ruste použi `f32::to_bits()` / `f32::from_bits()` — je to safe a rovnako rýchle. `transmute` si nechaj pre prípady, kde naozaj nič iné nestačí.

---

## Pod kapotou — vtable a Trait Objects

Keď píšeš `Box<dyn Trait>` alebo `&dyn Trait`, Rust pod kapotou vytvára **fat pointer** — dvojicu (data pointer, vtable pointer). Vtable je štruktúra s function pointermi pre každú metódu traitu. Toto je priamym ekvivalentom C++ virtual dispatch cez `vptr`.

```rust
trait Animal {
    fn sound(&self) -> &str;
    fn legs(&self) -> u32;
}

struct Dog;
struct Cat;

impl Animal for Dog {
    fn sound(&self) -> &str { "haf" }
    fn legs(&self) -> u32 { 4 }
}

impl Animal for Cat {
    fn sound(&self) -> &str { "mňau" }
    fn legs(&self) -> u32 { 4 }
}

fn main() {
    // Statická dispatch — monomorphization, nulový overhead
    fn static_dispatch<A: Animal>(a: &A) {
        println!("{}", a.sound());
    }

    // Dynamická dispatch — fat pointer + vtable lookup
    fn dynamic_dispatch(a: &dyn Animal) {
        println!("{}", a.sound());
    }

    let dog = Dog;
    let cat = Cat;

    static_dispatch(&dog);   // generuje dog_sound() priamo
    dynamic_dispatch(&cat);  // pointer -> vtable -> fn ptr -> call

    // Pozrieme sa na fat pointer v raw forme
    let animal: &dyn Animal = &dog;
    // Fat pointer je dvojica: (data ptr, vtable ptr)
    // sizeof(&dyn Animal) == 2 * sizeof(usize) == 16 bajtov na 64-bit
    println!("veľkosť fat pointera: {} bajtov", std::mem::size_of_val(&animal));

    // Vtable v pamäti (schematicky):
    // struct DogVtable {
    //     drop_fn: fn(*mut Dog),      // destruktor
    //     size: usize,                // sizeof(Dog)
    //     align: usize,               // alignof(Dog)
    //     sound: fn(*const Dog) -> &str,
    //     legs: fn(*const Dog) -> u32,
    // }
}
```

### Manuálna vtable — keď potrebuješ C-kompatibilné plugin systémy

Toto je vzor, ktorý sa používa pri budovaní plugin systémov, kde plugin je shared library (.so / .dll) a musíš komunikovať cez C ABI:

```rust
// Plugin interface — C-kompatibilné vtable
#[repr(C)]
pub struct PluginVtable {
    pub version: u32,
    pub init: unsafe extern "C" fn() -> i32,
    pub process: unsafe extern "C" fn(data: *const u8, len: usize) -> i32,
    pub destroy: unsafe extern "C" fn(),
}

// Plugin ktorý implementuje toto vtable
mod my_plugin {
    use super::PluginVtable;

    static mut INITIALIZED: bool = false;

    pub unsafe extern "C" fn init() -> i32 {
        INITIALIZED = true;
        println!("plugin inicializovaný");
        0
    }

    pub unsafe extern "C" fn process(data: *const u8, len: usize) -> i32 {
        if data.is_null() { return -1; }
        let slice = std::slice::from_raw_parts(data, len);
        println!("plugin spracoval {} bajtov", slice.len());
        0
    }

    pub unsafe extern "C" fn destroy() {
        println!("plugin zničený");
    }

    pub static VTABLE: PluginVtable = PluginVtable {
        version: 1,
        init,
        process,
        destroy,
    };
}

fn main() {
    let vtable = &my_plugin::VTABLE;
    unsafe {
        (vtable.init)();
        let data = b"hello plugin";
        (vtable.process)(data.as_ptr(), data.len());
        (vtable.destroy)();
    }
}
```

---

## Globálny mutable state

Globálny mutable stav je v Ruste zámerné komplikovaný — pretože je to jeden z hlavných zdrojov bugov v C/C++ kóde. `static mut` existuje, ale prístup vyžaduje `unsafe`. V produkčnom kóde by si mal vždy preferovať `AtomicT` alebo `OnceLock`/`LazyLock`.

```rust
// static mut — nebezpečné, nikdy nepoužívaj v multi-thread kóde
static mut COUNTER: u64 = 0;

fn increment() {
    unsafe {
        COUNTER += 1;
        // Ak to volá viac threadov súčasne → data race → UB
    }
}

// Lepšie: AtomicU64 — bez unsafe, thread-safe
use std::sync::atomic::{AtomicU64, Ordering};

static SAFE_COUNTER: AtomicU64 = AtomicU64::new(0);

fn safe_increment() {
    // Relaxed — len atomicita, žiadne ordering garancie
    // SeqCst — total order, najsilnejšie, najpomalšie
    // Acquire/Release — pre synchronizáciu prodducent/konzument
    SAFE_COUNTER.fetch_add(1, Ordering::Relaxed);
}

// Najlepšie: LazyLock pre komplexné typy (od Rust 1.80)
use std::sync::LazyLock;
use std::collections::HashMap;

static CONFIG: LazyLock<HashMap<&str, &str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("host", "localhost");
    m.insert("port", "8080");
    m
});

fn main() {
    increment();
    safe_increment();
    println!("unsafe: {}", unsafe { COUNTER });
    println!("safe:   {}", SAFE_COUNTER.load(Ordering::Relaxed));
    println!("host:   {}", CONFIG["host"]);  // inicializovaný pri prvom prístupe
}
```

### Memory Ordering — pre pokročilých

Atomic operácie nie sú len o atomicite — sú aj o poradí viditeľnosti zmien medzi threadmi. Toto je téma, ktorá zapríčiňuje headache aj skúseným C++ programátorom:

```rust
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

fn producer_consumer_example() {
    let data = Arc::new(AtomicU64::new(0));
    let ready = Arc::new(AtomicBool::new(false));

    let d = Arc::clone(&data);
    let r = Arc::clone(&ready);

    // Producent
    std::thread::spawn(move || {
        d.store(42, Ordering::Relaxed);        // zapíš dáta
        r.store(true, Ordering::Release);       // "publikuj" — garantuje že d.store
                                                // bude viditeľný pred r.store
    });

    // Konzument
    loop {
        if ready.load(Ordering::Acquire) {     // Acquire páruje s Release
            // Tu je garantované, že data.load vidí 42
            println!("data: {}", data.load(Ordering::Relaxed));
            break;
        }
        std::hint::spin_loop();
    }
}
```

Release/Acquire pair je najčastejší pattern. `Relaxed` je vhodné len pre countery kde ti nezáleží na poradí. `SeqCst` je najsilnejšie ale aj najdrahšie — na x86 je to `mfence` inštrukcia.

---

## Inline assembler

Inline assembler je potrebný pre nízkoúrovňové operácie, ktoré nemajú ekvivalent v Ruste: SIMD inštrukcie, CPU control registers, privileged operations pre OS kernel, alebo ultra-optimalizované hot paths.

```rust
fn main() {
    // Jednoduchý príklad — prenositeľný
    let x: u64 = 42;
    let y: u64;
    unsafe {
        std::arch::asm!(
            "mov {0}, {1}",
            "add {0}, 8",
            out(reg) y,   // výstupný register (kompilátor vyberie)
            in(reg) x,    // vstupný register
        );
    }
    println!("42 + 8 = {}", y);  // 50

    // Čítanie TSC (Time Stamp Counter) — len na x86_64
    #[cfg(target_arch = "x86_64")]
    {
        let lo: u32;
        let hi: u32;

        unsafe {
            std::arch::asm!(
                "rdtsc",
                out("eax") lo,
                out("edx") hi,
                options(nostack, nomem),  // kompilátor vie že nemeníme pamäť/stack
            );
        }
        let tsc = ((hi as u64) << 32) | lo as u64;
        println!("CPU cyklov od bootu (approx): {}", tsc);

        // CPUID — čítanie CPU informácií
        let eax_out: u32;
        let brand_eax: u32;
        unsafe {
            std::arch::asm!(
                "cpuid",
                inout("eax") 0x00u32 => eax_out,  // funkcia 0 = max CPUID leaf
                out("ebx") _,
                out("ecx") _,
                out("edx") _,
            );
            std::arch::asm!(
                "cpuid",
                inout("eax") 0x80000002u32 => brand_eax,  // CPU brand string part 1
                out("ebx") _,
                out("ecx") _,
                out("edx") _,
            );
        }
        println!("max CPUID leaf: {:#010X}", eax_out);
        println!("brand EAX: {:#010X}", brand_eax);
    }
}
```

### SIMD intrinsics — vektorové operácie

Pre výkon-kritický kód (kryptografia, kompresia, signálové spracovanie):

```rust
#[cfg(target_arch = "x86_64")]
fn simd_example() {
    use std::arch::x86_64::*;

    // Sčítaj 4 double-precision floaty naraz pomocou AVX
    let a = [1.0f64, 2.0, 3.0, 4.0];
    let b = [5.0f64, 6.0, 7.0, 8.0];
    let mut result = [0.0f64; 4];

    unsafe {
        if is_x86_feature_detected!("avx") {
            let va = _mm256_loadu_pd(a.as_ptr());
            let vb = _mm256_loadu_pd(b.as_ptr());
            let vc = _mm256_add_pd(va, vb);
            _mm256_storeu_pd(result.as_mut_ptr(), vc);
        }
    }

    println!("{:?}", result);  // [6.0, 8.0, 10.0, 12.0]
}
```

V C by si použil `#include <immintrin.h>` a `__m256d` typy — rovnaká operácia, rovnaký výsledok. Rust SIMD API je takmer 1:1 s C intrinsics.

---

## FFI — volanie C kódu

FFI (Foreign Function Interface) je jeden z najlegitímnejších dôvodov na `unsafe`. Rust sa musí rozprávať s existujúcim C kódom — systémovými knižnicami, OpenSSL, SQLite, GPU drivermi, jadrom OS.

### Pod kapotou — C ABI a calling conventions

Keď napíšeš `extern "C"`, Rust prekladač generuje kód, ktorý dodržiava C calling convention (System V AMD64 ABI na Linuxe, MSVC ABI na Windows). To znamená:

- Prvých 6 integer argumentov v `rdi, rsi, rdx, rcx, r8, r9`
- Prvých 8 float argumentov v `xmm0-xmm7`
- Návratová hodnota v `rax` (alebo `xmm0` pre float)
- Stack alignment na 16 bajtov pred `call` inštrukciou
- Caller-saved vs callee-saved registre podľa ABI

```rust
// Deklarácia externej C funkcie — len signatúra, bez tela
extern "C" {
    fn strlen(s: *const u8) -> usize;
    fn abs(x: i32) -> i32;
    fn malloc(size: usize) -> *mut std::ffi::c_void;
    fn free(ptr: *mut std::ffi::c_void);
}

fn main() {
    unsafe {
        // strlen z libc
        let s = b"hello\0";  // null-terminated C string
        let len = strlen(s.as_ptr());
        println!("strlen: {}", len);  // 5

        println!("abs(-42) = {}", abs(-42));  // 42

        // malloc/free — normálne nepoužívaj, len ukážka
        let ptr = malloc(64) as *mut u8;
        if !ptr.is_null() {
            ptr.write(42);
            println!("malloc'd value: {}", *ptr);
            free(ptr as *mut _);
        }
    }
}
```

### Wrapper nad C knižnicou — správny vzor

V praxi nikdy nechceš exposovať `unsafe extern "C"` funkcie priamo. Wrapper pattern skryje unsafe za čistý Rust API:

```rust
// Simulácia GPIO C API (napr. pre Raspberry Pi)
// V reálnom kóde: extern "C" { fn gpio_export(pin: u32) -> i32; }

mod gpio_sys {
    // Toto by bolo generované z C headera pomocou bindgen
    pub unsafe fn gpio_export(_pin: u32) -> i32 { 0 }
    pub unsafe fn gpio_set_direction(_pin: u32, _out: bool) -> i32 { 0 }
    pub unsafe fn gpio_write(_pin: u32, _val: u32) -> i32 { 0 }
    pub unsafe fn gpio_unexport(_pin: u32) -> i32 { 0 }
}

// Bezpečný wrapper — všetok unsafe je skrytý tu
// Verejné API je 100% safe Rust
pub struct GpioPin {
    pin: u32,
}

#[derive(Debug)]
pub struct GpioError(i32);

impl std::fmt::Display for GpioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GPIO error code: {}", self.0)
    }
}

impl GpioPin {
    /// Exportuje GPIO pin a nastaví ho ako výstup.
    /// Automaticky unexportuje pin pri drop.
    pub fn new(pin: u32) -> Result<Self, GpioError> {
        let ret = unsafe { gpio_sys::gpio_export(pin) };
        if ret != 0 { return Err(GpioError(ret)); }

        let ret = unsafe { gpio_sys::gpio_set_direction(pin, true) };
        if ret != 0 {
            // Cleanup pri chybe
            unsafe { gpio_sys::gpio_unexport(pin); }
            return Err(GpioError(ret));
        }

        Ok(GpioPin { pin })
    }

    pub fn set_high(&self) -> Result<(), GpioError> {
        let ret = unsafe { gpio_sys::gpio_write(self.pin, 1) };
        if ret != 0 { Err(GpioError(ret)) } else { Ok(()) }
    }

    pub fn set_low(&self) -> Result<(), GpioError> {
        let ret = unsafe { gpio_sys::gpio_write(self.pin, 0) };
        if ret != 0 { Err(GpioError(ret)) } else { Ok(()) }
    }
}

impl Drop for GpioPin {
    fn drop(&mut self) {
        // RAII — automatický cleanup, aj pri paniku
        unsafe { gpio_sys::gpio_unexport(self.pin); }
    }
}

fn main() {
    // Caller nevidí žiadny unsafe — pracuje s čistým Rust API
    match GpioPin::new(18) {
        Ok(pin) => {
            pin.set_high().unwrap();
            println!("GPIO 18 HIGH");
            pin.set_low().unwrap();
            println!("GPIO 18 LOW");
            // drop(pin) zavolá gpio_unexport automaticky
        }
        Err(e) => eprintln!("GPIO chyba: {}", e),
    }
}
```

### bindgen — automatické generovanie FFI wrapperov

V praxi nepíšeš FFI deklarácie ručne. `bindgen` parsuje C headery a generuje Rust kód:

```bash
# Inštalácia
cargo install bindgen-cli

# Generovanie z C headera
bindgen libgpio.h -o src/gpio_sys.rs

# Prípadne v build.rs:
```

```rust
// build.rs — automaticky generuj FFI pri cargo build
fn main() {
    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Nepodarilo sa vygenerovať bindings");

    let out_path = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Nepodarilo sa zapísať bindings");
}
```

### CStr a String konverzie — bežný zdroj bugov

Prechod medzi Rust `String` a C `char*` je miesto, kde sa dá ľahko urobiť chybu:

```rust
use std::ffi::{CStr, CString, c_char};

extern "C" {
    fn some_c_function(s: *const c_char) -> *const c_char;
}

fn call_c_with_string(input: &str) -> String {
    // Rust &str → CString (pridá \0, môže zlyhať ak input obsahuje \0)
    let c_input = CString::new(input).expect("input nesmie obsahovať null bajt");

    unsafe {
        // Odovzdaj pointer — CString žije cez celý unsafe blok
        let c_output = some_c_function(c_input.as_ptr());

        // C char* → Rust &str → String
        if c_output.is_null() {
            return String::new();
        }

        // CStr::from_ptr — nebezpečné, pointer musí byť validný a null-terminated
        CStr::from_ptr(c_output)
            .to_str()
            .unwrap_or("")
            .to_owned()
        // Pozor: ak C funkcia alokovala string, musíš zavolať free()!
        // Tu predpokladáme, že C vrátil pointer na statický string
    }
}

fn main() {
    // Bezpečný spôsob — CString sa automaticky uvoľní
    let hello = CString::new("hello C").unwrap();
    println!("CString: {:?}", hello);

    // Čítanie C null-terminated stringu — napr. z errno
    unsafe {
        let err_str = libc_strerror(0);  // "Success" alebo podobne
        // V reálnom kóde by si použil libc crate
    }
}
```

---

## `mem::forget` a ManuallyDrop

```rust
use std::mem;

fn main() {
    let v = vec![1u32, 2, 3];

    // forget() — zabraňuje volaniu Drop
    // Použiť keď odovzdávaš vlastníctvo C kódu alebo budujete vlastné abstrakcie
    let ptr = v.as_ptr();
    let len = v.len();
    let cap = v.capacity();
    mem::forget(v); // Vec sa NEzničí — teraz si zodpovedný za free pamäte

    unsafe {
        // Manuálna rekonštrukcia z raw parts
        let v2 = Vec::from_raw_parts(ptr as *mut u32, len, cap);
        println!("{:?}", v2);
        // v2 sa dropne normálne tu a uvoľní pamäť
    }
}
```

```rust
use std::mem::ManuallyDrop;

// ManuallyDrop — wrapper ktorý zabraňuje automatickému Drop
// Lepšia alternatíva k mem::forget v mnohých prípadoch
struct MyBuffer {
    inner: ManuallyDrop<Vec<u8>>,
}

impl MyBuffer {
    fn new(size: usize) -> Self {
        MyBuffer {
            inner: ManuallyDrop::new(vec![0u8; size]),
        }
    }

    fn as_ptr(&self) -> *const u8 {
        self.inner.as_ptr()
    }

    // Explicitný drop — musíme volať manuálne
    unsafe fn free(mut self) {
        ManuallyDrop::drop(&mut self.inner);
    }
}

// Drop pre MyBuffer NIE JE implementovaný — inner sa nikdy automaticky nedroppí
// To je zámer pre interop s C alebo custom alokátormi
```

### Reálny use case — FFI buffer management

```rust
// Scenár: C knižnica alokuje buffer, Rust ho musí uvoľniť cez C funkciu
// (nie cez Rust alokátor)

extern "C" {
    fn c_alloc_buffer(size: usize) -> *mut u8;
    fn c_free_buffer(ptr: *mut u8);
}

struct CBuffer {
    ptr: *mut u8,
    len: usize,
}

impl CBuffer {
    fn new(size: usize) -> Option<Self> {
        let ptr = unsafe { c_alloc_buffer(size) };
        if ptr.is_null() {
            None
        } else {
            Some(CBuffer { ptr, len: size })
        }
    }

    fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

impl Drop for CBuffer {
    fn drop(&mut self) {
        // MUSÍME použiť C free, nie Rust dealloc
        // Buffer bol alokovaný C kódom s C alokátorom
        unsafe { c_free_buffer(self.ptr) }
    }
}

// Teraz je CBuffer bezpečný Rust typ — caller nevidí unsafe
fn main() {
    if let Some(mut buf) = CBuffer::new(1024) {
        buf.as_mut_slice()[0] = 42;
        println!("prvý bajt: {}", buf.as_slice()[0]);
        // drop(buf) → c_free_buffer automaticky
    }
}
```

---

## Unsafe traity — Send a Sync manuálne

Niekedy potrebuješ implementovať `Send` alebo `Sync` pre vlastný typ, ktorý ich nezdedí automaticky — typicky keď typ obsahuje raw pointer:

```rust
use std::ptr::NonNull;

// Vlastný thread-safe smart pointer
pub struct ThreadSafePtr<T> {
    ptr: NonNull<T>,
}

// SAFETY: Garantujeme, že prístup k ptr je synchronizovaný externálne
// (napr. vždy za Mutex). Raw pointer sám osebe nie je Send/Sync,
// ale náš wrapper garantuje bezpečnosť.
unsafe impl<T: Send> Send for ThreadSafePtr<T> {}
unsafe impl<T: Sync> Sync for ThreadSafePtr<T> {}

impl<T> ThreadSafePtr<T> {
    pub fn new(val: T) -> Self {
        let boxed = Box::new(val);
        ThreadSafePtr {
            ptr: NonNull::new(Box::into_raw(boxed)).unwrap(),
        }
    }

    pub unsafe fn get(&self) -> &T {
        self.ptr.as_ref()
    }
}

impl<T> Drop for ThreadSafePtr<T> {
    fn drop(&mut self) {
        unsafe {
            drop(Box::from_raw(self.ptr.as_ptr()));
        }
    }
}
```

Toto je presne to, čo robia `Arc`, `Mutex`, a iné sync primitívy v štandardnej knižnici — obaľujú raw pointer a manuálne deklarujú `Send + Sync` po overení bezpečnostných invariantov.

---

## Miri — nástroj pre detekciu UB

Miri je interpret Rust MIR (Mid-level IR) ktorý dynamicky detekuje undefined behavior v testoch. Je to ekvivalent AddressSanitizer + MemorySanitizer + UBSanitizer kombinovaný do jedného nástroja:

```bash
# Inštalácia Miri
rustup +nightly component add miri

# Spustenie testov pod Miri
cargo +nightly miri test

# Miri odhalí:
# - use-after-free
# - dangling pointer dereference
# - invalid memory access
# - data races (experimental)
# - violation of aliasing rules (Stacked Borrows model)
# - neinicializovaná pamäť
```

```rust
// Príklad: Miri odhalí tento bug, compiler ho prehliadne
fn aliasing_bug() {
    let mut x = 5u32;
    let a = &mut x as *mut u32;
    let b = &mut x as *mut u32;

    unsafe {
        *a = 10;
        *b = 20;
        // Miri: error — a a b aliasujú rovnakú pamäť cez dva *mut
        // Porušenie Stacked Borrows modelu
        println!("{}", *a);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_with_miri() {
        // cargo +nightly miri test -- test_with_miri
        let v = vec![1u32, 2, 3];
        let ptr = v.as_ptr();
        unsafe {
            // Miri overí, že *ptr.add(2) je validný prístup
            assert_eq!(*ptr.add(2), 3);
        }
    }
}
```

---

## Zlaté pravidlá unsafe

1. **Minimalizuj scope** — `unsafe` blok čo najmenší, nie celá funkcia ak to nie je nutné
2. **Dokumentuj invarianty** — `/// # Safety:` komentár s každou `unsafe fn`, popíš čo musí caller garantovať
3. **Wrapper pattern** — schovaj `unsafe` za bezpečné verejné API, caller nikdy nevidí unsafe
4. **Testy a Miri** — `cargo +nightly miri test` detekuje UB v testoch, spúšťaj pravidelne
5. **Nevymýšľaj** — `Vec`, `Box`, `Arc`, `Mutex` riešia 99% problémov bez unsafe
6. **Audit externe** — pri code review grepdni `unsafe {` a každý blok skontroluj osobitne
7. **Clippy unsafe rules** — `#![deny(unsafe_op_in_unsafe_fn)]` vynúti explicitné unsafe bloky aj v unsafe fn

```bash
# Kompletný safety toolchain
cargo clippy -- -W clippy::undocumented_unsafe_blocks
cargo +nightly miri test
cargo audit  # závislostiach s CVE
```

---

## Zhrnutie

| C / C++ | Rust unsafe |
|---|---|
| Vždy implicitne nebezpečné | Explicitný `unsafe` blok — auditovateľné |
| `void*` casted | Raw pointer s typom: `*const T`, `*mut T` |
| Pointer aritmetika bez ochrany | `ptr.add(n)`, `ptr.offset(n)` — jasne pomenované |
| `union` — neoverovaný prístup | `union` + `unsafe` prístup k poliam |
| Inline asm — zabudovaná syntax | `std::arch::asm!` — explicitné vstupy/výstupy |
| `extern "C"` volania | `extern "C"` + FFI + bindgen |
| `reinterpret_cast` | `std::mem::transmute` — overí sizeof |
| `std::atomic` (od C++11) | `std::sync::atomic` — podobné API |
| Virtual dispatch cez vtable | `dyn Trait` — fat pointer + vtable |
| AddressSanitizer (runtime) | Miri (compile-time interpret, testy) |
| Shared ownership bez GC: `shared_ptr` | `Arc<T>` — atómový reference count |

Kľúčový insight: C programátor píše `unsafe` kód každý deň — len bez označenia. Rust ťa núti byť explicitný. Tá explicitnosť nie je bariéra — je to dokumentácia, je to audit trail, je to signál ostatným: "tu sú invarianty, ktoré musíš pochopiť".

Keď vidíš `unsafe` v Ruste, je to varovný prúžok na mieste kde si treba dávať pozor. Keď vidíš `int* ptr` v C, neviete nič.

Ďalšia kapitola: Bevy — ECS herný engine a iný spôsob myslenia o architektúre.

---

## Vizuálny príklad — Memory Visualizer

    cargo run --bin k10_memory

Raw pamäť pohľadom na hex dump — s farebným označením čo kde patrí a čo sa stane keď niečo pokazíš.

Grid zobrazuje 192 bajtov pamäte (12 riadkov × 16 stĺpcov):
- **Zelená** = STACK: `let x: i32 = 42` (vidíš `2A 00 00 00` v little-endian), `let y: f64 = 3.14` (IEEE 754 reprezentácia), raw pointer
- **Modrá** = HEAP: `Vec<u8>` s obsahom "Hello, Rust!" ako ASCII kódy
- **Tmavočervená** = UNMAPPED: oblasti kde prístup spôsobí segfault — zobrazené ako `??`

Šípky presúvajú kurzor — pravý panel ukazuje čo táto konkrétna adresa obsahuje, v ktorom regióne je, a či je prístup bezpečný.

`TAB` prepína scenáre:
1. **Normálny prístup** — cursor len na validnej pamäti
2. **Raw pointer** — žltá šípka ukazuje ako `*mut i32` smeruje zo STACK na HEAP
3. **Use-after-free** — HEAP sčervenie (uvoľnená pamäť), pointer naň bliká červenou; "toto by v C crashlo, Rust to zakazuje v safe kóde"

Toto je vizualizácia toho prečo unsafe blok existuje — nie ako "vypnutie kontroly", ale ako označenie miesta kde ty preberáš zodpovednosť za invarianty ktoré kompilátor nedokáže overiť.

Ovládanie: šípky = pohyb kurzora, `TAB` = scenár, `Q` = koniec.
