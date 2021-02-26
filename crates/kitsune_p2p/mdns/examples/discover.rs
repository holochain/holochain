use kitsune_mdns::*;
use futures_util::{pin_mut, stream::StreamExt, self};

#[tokio::main(threaded_scheduler)]
async fn main() {
   println!("Starting discovery");
   // Start Stream
   let stream = mdns_listen();
   pin_mut!(stream);
   while let Some(maybe_response) = stream.next().await {
      match maybe_response {
         Ok(response) => {
            println!("Discovered: {:?}", response);
         },
         Err(e) => {
            println!("!!! Discovery Error: {:?}", e);
         }
      }
   }
}
