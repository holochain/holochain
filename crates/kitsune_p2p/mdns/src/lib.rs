///! Crate for discovering Holochain peers over MDNS
///! Works by broadcasting a service named `HC_SERVICE_NAME`
///! and adding base64 encoded data in a TXT record
///!
///! Uses libmdns crate for broadcasting
///! Uses mdns crate for discovery

use futures_util::{stream::StreamExt, self};
use std::thread;
use std::time::Duration;
use std::sync::mpsc::{self, TryRecvError, Sender};
use mdns::RecordKind;
use futures_core::Stream;
use err_derive::Error;

const HC_SERVICE_TYPE: &str       = "_holochain._udp";
const BROADCAST_INTERVAL_SEC: u64 = 8;
const QUERY_INTERVAL_SEC: u64     = 5;
const MAX_TXT_SIZE: usize         = 192;

#[derive(Debug, Error)]
pub enum MdnsError {
   #[error(display = "Regular Mdns error {}", _0)]
   Mdns(#[error(source)] mdns::Error),
   #[error(display = "Base64 decoding error {}", _0)]
   Base64(#[error(source)] base64::DecodeError),
}

/// Stop thread created by `mdns_create_broadcast_thread()`
pub fn mdns_kill_thread(tx: Sender<()>) {
   tx.send(()).ok();
}

/// Create a thread that will broadcast a holochain service over mdns
/// Returns Sender for sending thread termination command
pub fn mdns_create_broadcast_thread(service_name: String, buffer: &[u8]) -> Sender<()> {
   assert!(service_name.len() < 63); // constraint in libmdns
   // Create Terminate command channel
   let (tx, rx) = mpsc::channel();
   // Change buffer to base64 string
   let mut b64 = format!("u{}", base64::encode_config(buffer, base64::URL_SAFE_NO_PAD));
   // println!("Broadcasting service '{}' over mdns ({})", service_name, b64.len());
   // Split buffer to fix TXT max size
   let mut substrs = Vec::new();
   while b64.len() > MAX_TXT_SIZE {
      let start: String = b64.drain(..MAX_TXT_SIZE).collect();
      substrs.push(start);
   };
   substrs.push(b64);
   //println!("substrs = {:?}", substrs);
   // Create thread
   let _handle = thread::spawn(move || {
      let txts: Vec<_> = substrs.iter().map(AsRef::as_ref).collect();
      println!("Entering mdns broadcasting thread...");
      // Create mdns responder
      let responder = libmdns::Responder::new().unwrap();
      let _svc = responder.register(
         HC_SERVICE_TYPE.to_owned(),
         service_name,
         0,
         &txts,
      );
      // Loop forever unless termination command recieved
      loop {
         ::std::thread::sleep(::std::time::Duration::from_secs(BROADCAST_INTERVAL_SEC));
         match rx.try_recv() {
            Ok(_) | Err(TryRecvError::Disconnected) => {
               println!("Terminating.");
               break;
            }
            Err(TryRecvError::Empty) => {}
         }
      }
   });
   // Done
   tx
}

///
#[derive(Debug, Clone)]
pub struct MdnsResponse {
   /// Service name used
   pub service_name: String,
   /// IP address that responded to the mdns query
   pub addr: std::net::IpAddr,
   /// Data contained in the TXT record
   pub buffer: Vec<u8>,
}

/// Queries the network for the holochain service
/// Returns an iterator over all responses received
pub fn mdns_listen() -> impl Stream<Item = Result<MdnsResponse, MdnsError>> {
   let service_name = format!("{}.local", HC_SERVICE_TYPE);
   let query = mdns::discover::all(service_name, Duration::from_secs(QUERY_INTERVAL_SEC))
   .expect("mdns Discover failed");
   // Get Mdns Response stream
   let response_stream = query.listen();
   // Change it into a MdnsResponse stream
   let mdns_stream = response_stream
      // Filtering out Empty responses
      .filter(move |res| {
         futures_util::future::ready(match res {
            Ok(response) => !response.is_empty() && !response.ip_addr().is_none(),
            Err(_) => true, // Keep errors
         })
      })
      .map(|maybe_response| {
         if let Err(e) = maybe_response {
            return Err(MdnsError::Mdns(e));
         }
         let response = maybe_response.unwrap();
         // NOTE: if response.ip_addr() is not te right address,
         // we should give all A/AAA records found in the answers instead
         let addr = response.ip_addr().unwrap(); // should have already been filtered out
         let mut buffer = Vec::new();
         let mut service_name = String::new();
         println!("Response Answer count = {}", response.answers.len());
         //println!("Response Answers:  {:?}", response.answers);
         for answer in response.answers {
            match  answer.kind {
               RecordKind::TXT(txts) => {
                  println!("TXT count = {}", txts.len());
                  let mut b64 = String::new();
                  for txt in txts {
                     //println!("Response TXT = {:?}", txt);
                     b64.push_str(&txt);
                  }
                  buffer = match base64::decode_config(&b64[1..], base64::URL_SAFE_NO_PAD) {
                     Err(e) => return Err(MdnsError::Base64(e)),
                     Ok(s) => s,
                  };
               },
               // Retrieve service name stored in PTR record
               RecordKind::PTR(ptr) => {
                  //println!("PTR = {}", ptr);
                  service_name = ptr.split('.')
                     .into_iter()
                     .next()
                     .expect("Found service without a name")
                     .to_string();
               },
               _ => {},
            }
         }
         Ok(MdnsResponse {addr, buffer, service_name })
      });
   // Done
   mdns_stream
}
