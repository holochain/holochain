use kitsune_mdns::*;

pub fn main() {
   /// Debug info
   let mut builder = env_logger::Builder::new();
   builder.parse_filters("libmdns=debug");
   builder.init();
   println!("Starting broadcast");
   /// Create buffer
   // let buffer = [0, 1, 2];
   let buffer: [u8; 190] = [42; 190];
   /// Launch thread
   let tx = mdns_create_broadcast_thread(/*"AgentInfos",*/ &buffer);
   ::std::thread::sleep(::std::time::Duration::from_secs(60));
   /// Kill thread
   mdns_kill_thread(tx);
}
