use kitsune_mdns::*;

#[tokio::main(threaded_scheduler)]
async fn main() {
    println!("Starting broadcast");
    // Create buffer
    // let buffer = [0, 1, 2];
    // let buffer: [u8; 190] = [42; 190];
    let mut buffer: Vec<u8> = Vec::new();
    for i in 0..12 as u32 {
        buffer.push((i % 255) as u8);
    }
    // Launch thread
    let service_type = "bobby".to_owned();
    let service_name = (0..62).map(|_| "X").collect::<String>();
    let tx = mdns_create_broadcast_thread(service_type, service_name, &buffer);
    // Kill thread after a minute
    tokio::time::delay_for(::std::time::Duration::from_secs(60)).await;
    mdns_kill_thread(tx);
}
