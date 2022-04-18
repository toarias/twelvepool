use twelvepool::Watcher;

#[tokio::main]
async fn main() {
    println!("-- INIT --");
    let mut watcher = Watcher::new(
        String::from("https://terra.stakesystems.io:2053"),  // RPC address
        String::from("https://terra.stakesystems.io"),   // LCD address
        None,                                    // Optional reqwest client
        None,                                    // Optional interval duration (default to 100ms)
    )
    .run();

    println!("-- WATCHER CREATED --");

    loop {
        if let Some(mempool_item) = watcher.recv().await {
            println!("tx found (tx hash {})", mempool_item.tx_hash);
        }
    }
}