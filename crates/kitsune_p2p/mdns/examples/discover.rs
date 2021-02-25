use kitsune_mdns::*;
use futures_util::{pin_mut, stream::StreamExt, self};

#[async_std::main]
async fn main() {
   // Debug info
   let mut builder = env_logger::Builder::new();
   builder.parse_filters("libmdns=debug");
   builder.init();
   println!("Starting discovery");
   // Start Stream
   let stream = mdns_listen();
   pin_mut!(stream);
   while let Some(maybe_response) = stream.next().await {
      match maybe_response {
         Ok(response) => {
            println!("Discovered: {:?} = {:?} ({})", response.addr, response.buffer, response.buffer.len());
         },
         Err(e) => {
            println!("!!! Discovery Error: {:?}", e);
         }
      }
   }
}
