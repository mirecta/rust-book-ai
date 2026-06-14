// Kapitola 10: Unsafe Rust — raw pointery a FFI

fn main() {
    demo_raw_pointers();
    demo_inline_asm();
}

fn demo_raw_pointers() {
    let mut x = 42u32;
    let ptr: *mut u32 = &mut x;

    unsafe {
        *ptr += 1;
        println!("cez raw pointer: {}", *ptr);
    }
    println!("normálne: {}", x);
}

fn demo_inline_asm() {
    let result: u64;
    unsafe {
        std::arch::asm!(
            "mov {0}, 0xDEAD",
            out(reg) result,
        );
    }
    println!("z asm!: 0x{:X}", result);
}

// Simulácia GPIO C API — v reálnom kóde by bolo: extern "C" { fn gpio_write(...); }
unsafe fn gpio_write_stub(_pin: u32, _val: u32) {}

pub fn gpio_write(pin: u32, high: bool) {
    unsafe { gpio_write_stub(pin, high as u32) }
}
