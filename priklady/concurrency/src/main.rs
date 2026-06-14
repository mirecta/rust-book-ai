// Kapitola 9: Concurrency — tokio príklady

use std::sync::{Arc, Mutex};
use std::time::Duration;

#[tokio::main]
async fn main() {
    println!("=== std::thread + Mutex ===");
    demo_threads_mutex();

    println!("\n=== tokio tasks + select! ===");
    demo_tokio_tasks().await;

    println!("\n=== async TCP server (skrátený príklad) ===");
    println!("Pozri src/tcp_server.rs pre plný príklad s tokio::net::TcpListener");
}

fn demo_threads_mutex() {
    let counter = Arc::new(Mutex::new(0u32));
    let mut handles = vec![];

    for _ in 0..4 {
        let c = Arc::clone(&counter);
        handles.push(std::thread::spawn(move || {
            let mut val = c.lock().unwrap();
            *val += 1;
        }));
    }
    for h in handles { h.join().unwrap(); }
    println!("counter = {}", *counter.lock().unwrap()); // vždy 4
}

async fn demo_tokio_tasks() {
    let task_a = tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        "A hotovo"
    });

    let task_b = tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(50)).await;
        "B hotovo"
    });

    // select! — čakaj na prvého víťaza (ako epoll)
    tokio::select! {
        res = task_a => println!("select: {}", res.unwrap()),
        res = task_b => println!("select: {}", res.unwrap()),
    }

    // join — čakaj na oboch
    let (a, b) = tokio::join!(
        async { "join A" },
        async { "join B" },
    );
    println!("join: {} + {}", a, b);
}
