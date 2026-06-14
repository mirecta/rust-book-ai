// Kapitola 2: Ownership & Borrowing — spustiteľné príklady

fn main() {
    demo_move_semantics();
    demo_borrow();
    demo_stack_vs_heap();
}

fn demo_move_semantics() {
    let s1 = String::from("hello");
    let s2 = s1; // move — s1 je invalidovaný
    // println!("{}", s1); // compile error: use of moved value
    println!("po move: {}", s2);
}

fn demo_borrow() {
    let mut data = vec![1u32, 2, 3];
    let len = calculate_len(&data); // shared borrow
    println!("dĺžka: {}", len);
    data.push(4); // ok, borrow skončil
    println!("po push: {:?}", data);
}

fn calculate_len(v: &Vec<u32>) -> usize {
    v.len()
}

fn demo_stack_vs_heap() {
    let stack_val: u32 = 42; // Copy — na stacku
    let heap_val = Box::new(42u32); // na heape
    let stack_copy = stack_val; // kópia, nie move
    println!("stack: {} {}", stack_val, stack_copy);
    println!("heap: {}", *heap_val);
}
