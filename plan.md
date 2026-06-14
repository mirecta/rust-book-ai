# Rust Pre Systémových Programátorov — PLÁN PROJEKTU

## Cieľová skupina

Čitateľ: elektrotechnik/telekomunikačný inžinier s C/C++, Python, ASM, OS internals, sieťovými protokolmi.
Žiadne vysvetľovanie čo je pointer, stack, heap, register, interrupt. Rovno na vec.

---

## Technologický stack

```
Zdroj:     Markdown (.md) — jeden súbor na kapitolu
Konverzia: Pandoc → EPUB / PDF
Build:     Makefile (jednoduchý, bez závislostí navyše)
Kód:       Cargo workspace — každý príklad je spustiteľný crate
Editor:    Zed + Claude Code (agent mode)
Verzia:    Git
```

### Adresárová štruktúra projektu

```
rust-kniha/
├── plan.md                  ← tento súbor
├── Makefile
├── metadata.yaml            ← Pandoc metadáta (autor, jazyk, obálka)
├── cover.png
├── kapitoly/
│   ├── 00-preco-rust.md
│   ├── 01-toolchain.md
│   ├── 02-ownership.md
│   ├── 03-types-enums.md
│   ├── 04-pattern-matching.md
│   ├── 05-traits.md
│   ├── 06-lifetimes.md
│   ├── 07-error-handling.md
│   ├── 08-closures-iterators.md
│   ├── 09-concurrency.md
│   ├── 10-unsafe.md
│   └── 11-bevy-projekt.md
├── priklady/                ← Cargo workspace
│   ├── Cargo.toml           ← workspace root
│   ├── ownership/
│   ├── types/
│   ├── traits/
│   ├── lifetimes/
│   ├── concurrency/
│   ├── unsafe-demo/
│   └── bevy-hra/
└── build/
    ├── kniha.epub
    └── kniha.pdf
```

---

## Obsah knihy — Diel 1

### Kapitola 0 — Prečo Rust? (motivácia)
- C/C++ problémy ktoré Rust rieši: use-after-free, data races, NULL
- Rust v roku 2025: kde sa používa (Linux kernel, Android, Windows, embedded)
- Filozofia: "zero-cost abstractions", "fearless concurrency"
- Porovnanie: `malloc/free` vs ownership, `pthread` vs `std::thread`
- Čo Rust **nie je**: GC jazyk, pomalý, len pre sistemas

### Kapitola 1 — Toolchain & Cargo
- `rustup`, `rustc`, `cargo` — inštalácia, kanály (stable/nightly)
- `cargo new`, `cargo build`, `cargo run`, `cargo test`
- `Cargo.toml` — závislosti, features, profiles (`[profile.release]`)
- `rustfmt`, `clippy` — enforce štýlu, zachytenie bugov
- Cargo workspace — pre väčšie projekty
- **Príklad:** jednoduchý CLI nástroj čo parsuje argumenty

### Kapitola 2 — Ownership & Borrowing ⚠️ THE THING
- Stack vs Heap v Ruste — čo poznaš z C, čo je iné
- Move semantics: `let a = b` nie je kópia (vs C++)
- Borrow checker: `&` (shared) vs `&mut` (exclusive) — mutex v compile time
- `Clone` vs `Copy` — kedy a prečo
- Dangling pointers: prečo to nejde skompilovať (a prečo je to dobre)
- **"Toto by v C explodovalo"** — sekcia s konkrétnymi príkladmi
- **Príklad:** implementácia jednoduchého zásobníka (stack) bez unsafe

### Kapitola 3 — Typy, Štruktúry, Enums
- Primitívne typy: `u8/i32/f64/usize` — žiadne implicit conversions
- `struct` — podobné C, ale s metódami (`impl`)
- `enum` — nie C enum, ale algebraické dátové typy (tagged union)
- `Option<T>` namiesto NULL — koniec null pointer exception
- `Result<T, E>` namiesto errno — koniec ignorovaných chýb
- **Príklad:** parser jednoduchého protokolu (podobné čo poznáš z telekomunikácií)

### Kapitola 4 — Pattern Matching
- `match` — switch na steroidoch
- Destructuring: struct, enum, tuple, slice
- Guards: `if` v `match` vetve
- `if let` a `while let` — skrátená syntax
- Exhaustiveness checking — kompilátor vie či si zabudol prípad
- **Príklad:** stavový automat (FSM) pre jednoduchý protokol

### Kapitola 5 — Traits
- Traits nie sú interfaces (ale sú podobné)
- Definícia a implementácia traitu
- `Display`, `Debug`, `Clone`, `Iterator` — štandardné traity
- Trait objects: `dyn Trait` vs generics `<T: Trait>` — vtable vs monomorphization
- Blanket implementations
- **Príklad:** vlastný `Iterator` pre binárny protokol

### Kapitola 6 — Lifetimes
- Prečo existujú — borrow checker potrebuje pomoc pri funkciách
- Lifetime anotácie `'a` — čo znamenajú, čo neznamenajú
- Lifetime elision — kedy ich nemusíš písať
- `'static` — žije celý program (statická pamäť)
- Lifetimes v štruktúrach
- **Príklad:** parser čo vracia referencie do vstupného bufferu (zero-copy)

### Kapitola 7 — Error Handling
- `panic!` vs `Result` — kedy čo použiť
- `?` operátor — propagácia chýb bez boilerplate
- Vlastné error typy (`impl std::error::Error`)
- `thiserror` crate — derive macro pre error typy
- `anyhow` crate — rýchly error handling v aplikáciách
- **Príklad:** čítanie konfiguračného súboru s poriadnym error handlingom

### Kapitola 8 — Closures & Iterators
- Closures — `Fn`, `FnMut`, `FnOnce` a prečo sú tri
- `move` closures — capture by value
- Iterator trait — `map`, `filter`, `fold`, `collect`
- Iterator adapters — lazy evaluation (nič sa nevykoná kým nepotrebuješ)
- `collect()` do rôznych kolekcií
- **Príklad:** spracovanie CSV/log súboru funkcionálnym štýlom

### Kapitola 9 — Concurrency
- `std::thread` — ako `pthread` ale s ownership
- Message passing: `std::sync::mpsc` — channels
- Shared state: `Mutex<T>`, `Arc<T>` — thread-safe reference counting
- `Send` a `Sync` traity — compile-time thread safety
- Rayon — data parallelism jedným `par_iter()`
- Async/Await základy — `tokio` runtime
- **Príklad:** paralelné sťahovanie URL (async + tokio)

### Kapitola 10 — Unsafe Rust
- Čo unsafe dovolí: raw pointery, extern C, mutovať global state
- `unsafe` blok vs `unsafe` funkcia
- FFI — volanie C kódu z Rustu a naopak
- Raw pointery: `*const T` a `*mut T`
- `transmute`, `mem::forget` — keď naozaj vieš čo robíš
- Inline assembly (`asm!` macro) — tu sa stretneme s tvojím ASM
- **Príklad:** wraper nad C knižnicou (simulácia GPIO)

### Kapitola 11 — Bevy: ECS a herná architektúra
- Čo je Bevy a prečo ECS (Entity Component System) — iný spôsob myslenia ako OOP
- Porovnanie s klasickým herným objektovým modelom
- `App`, `Plugin`, systémy (`System`), zdroje (`Resource`)
- Startup systémy vs Update systémy
- Príklad: spawning entít, komponenty, dotazy (`Query`)

### Kapitola 12 — Bevy: Grafika, vstup, pohyb
- `Camera2d` a súradnicový systém
- `Sprite`, `Transform`, `Mesh`
- Vstup: `ButtonInput<KeyCode>`, `ButtonInput<MouseButton>`
- Pohyb hráča, delta time (`time.delta_secs()`)
- Príklad: hráč pohybujúci sa po obrazovke

### Kapitola 13 — Bevy: Kolízie, herná logika, stavy
- `bevy_rapier2d` alebo jednoduchá AABB kolízia
- Herné stavy: `States` — menu, gameplay, game over
- Events: vlastné udalosti medzi systémami
- Príklad: asteroidový shooter — lopty, kolízie, skóre

### Kapitola 14 — Bevy: Audio, UI, záver projektu
- `bevy_audio` — pozadie a zvukové efekty
- `bevy_ui` — HUD, skóre na obrazovke
- Asset loading: obrázky, zvuky
- Finálna hra: kompletný asteroid shooter
- Záver: čo ďalej (embedded v Diele 2!)

---

## Štýl písania

- **Tón:** kolega čo ti vysvetľuje pri káve — nie učebnica
- **C/C++ porovnania:** v každej kapitole kde to dáva zmysel
- **"Toto by v C explodovalo"** sekcia — konkrétne UB príklady
- **"Kompilátor hovorí nie"** — čitateľné error messages s vysvetlením
- **Bez zbytočného teórizovania** — príklad najprv, teória potom
- Slovenčina pre text, anglické názvy pre kód a technické termíny

---

## Makefile (build systém)

```makefile
KAPITOLY := $(sort $(wildcard kapitoly/*.md))

epub:
	pandoc metadata.yaml $(KAPITOLY) \
		--output build/kniha.epub \
		--toc \
		--toc-depth=2 \
		--highlight-style=kate \
		--epub-cover-image=cover.png

pdf:
	pandoc metadata.yaml $(KAPITOLY) \
		--output build/kniha.pdf \
		--toc \
		--highlight-style=kate \
		--pdf-engine=xelatex

check:
	cd priklady && cargo check --workspace

test:
	cd priklady && cargo test --workspace

all: epub pdf

.PHONY: epub pdf check test all
```

---

## metadata.yaml

```yaml
---
title: "Rust pre Systémových Programátorov"
author: "tvoje meno"
lang: sk
date: 2025
description: |
  Praktický sprievodca jazykom Rust pre programátorov
  so skúsenosťami s C/C++, assemblerom a systémovým
  programovaním. Od ownership po Bevy hry.
rights: "CC BY-SA 4.0"
---
```

---

## Poradie práce pre Claude Code agenta

1. `init` — vytvor adresárovú štruktúru, `Makefile`, `metadata.yaml`, Cargo workspace
2. `kap-00` — napíš kapitolu 0 s príkladmi
3. `kap-01` — kapitola 1, otestuj `cargo check`
4. `kap-02` — kapitola 2 (najdôležitejšia, najdlhšia)
5. ... iteruj kapitolu po kapitole
6. `bevy` — Bevy projekt — funkčná hra
7. `epub-test` — prvý build EPUB, skontroluj formátovanie
8. `polish` — jazyk, príklady, cross-references medzi kapitolami

---

## Prompt pre Claude Code na štart

```
Vytvor projekt Rust knihy podľa plan.md.
Začni s:
1. Celou adresárovou štruktúrou
2. Makefile a metadata.yaml
3. Cargo workspace v priklady/
4. Kapitolou 00-preco-rust.md — plná verzia s príkladmi

Štýl: čitateľ je elektrotechnik s C/C++/ASM skúsenosťami.
Žiadne vysvetľovanie základov. Priamo na vec s porovnaniami na C.
Slovenčina pre text, angličtina pre kód a technické termíny.
Každý príklad musí byť spustiteľný cez cargo run.
```

---

*Diel 2: Embedded Rust — neskôr 🙂*
*(`no_std`, HAL, RTIC, ESP32, defmt, probe-rs)*
