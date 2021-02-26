///! Crate for discovering Holochain peers over MDNS
///! Works by broadcasting a service named `HC_SERVICE_NAME`
///! and adding base64 encoded data in a TXT record
///!
///! Uses libmdns crate for broadcasting
///! Uses mdns crate for discovery

use std::time::Duration;
use mdns::RecordKind;
use err_derive::Error;
use tokio::stream::{StreamExt, Stream};

use std::sync::atomic::{AtomicBool, Ordering};

const HC_SERVICE_PROTOCOL: &str   = "._udp";
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
pub fn mdns_kill_thread(can_run: ::std::sync::Arc<AtomicBool>) {
   can_run.store(false, Ordering::Relaxed);
}


/// Create a thread that will broadcast a holochain service over mdns
/// Returns Sender for sending thread termination command
pub fn mdns_create_broadcast_thread(service_type: String, service_name: String, buffer: &[u8]) -> ::std::sync::Arc<AtomicBool> {
   let svc_type = format!("_{}{}", service_type, HC_SERVICE_PROTOCOL);
   assert!(svc_type.len() < 63); // constraint in libmdns
   assert!(service_name.len() < 63); // constraint in libmdns
   // Create Termination command variable
   let can_run  = ::std::sync::Arc::new(AtomicBool::new(true));
   let can_run_clone = can_run.clone();
   // Change buffer to base64 string
   let mut b64 = format!("u{}", base64::encode_config(buffer, base64::URL_SAFE_NO_PAD));
   println!("Broadcasting service type '{}', named '{}' over mdns ({})", svc_type, service_name, b64.len());
   // Create thread
   let _handle = tokio::task::spawn(async move {
      // Split buffer to fix TXT max size
      let mut substrs = Vec::new();
      while b64.len() > MAX_TXT_SIZE {
         let start: String = b64.drain(..MAX_TXT_SIZE).collect();
         substrs.push(start);
      };
      substrs.push(b64);
      let txts: Vec<_> = substrs.iter().map(AsRef::as_ref).collect();
      println!("Entering mdns broadcasting thread...");
      // Create mdns responder

      let responder = libmdns::Responder::new().unwrap();
      let _svc = responder.register(svc_type, service_name, 0, &txts);
      // Loop forever unless termination command received
      loop {
         tokio::time::delay_for(::std::time::Duration::from_secs(BROADCAST_INTERVAL_SEC)).await;
         if !can_run_clone.load(Ordering::Relaxed) {
            println!("Terminating.");
            break;
         }
      }
   });
   // Done
   can_run
}

///
#[derive(Debug, Clone)]
pub struct MdnsResponse {
   /// Service type used
   pub service_type: String,
   /// Service name used
   pub service_name: String,
   /// IP address that responded to the mdns query
   pub addr: std::net::IpAddr,
   /// Data contained in the TXT record
   pub buffer: Vec<u8>,
}

/// Queries the network for the holochain service
/// Returns an iterator over all responses received
pub fn mdns_listen(service_type: String) -> impl Stream<Item = Result<MdnsResponse, MdnsError>> {
   //let service_name = format!("{}.local", HC_SERVICE_TYPE);
   let svc_type = format!("_{}{}.local", service_type, HC_SERVICE_PROTOCOL);
   println!("MDNS query for service type '{}'", svc_type);
   let query = mdns::discover::all(svc_type, Duration::from_secs(QUERY_INTERVAL_SEC))
   .expect("mdns Discover failed");
   // Get Mdns Response stream
   let response_stream = query.listen();
   // Change it into a MdnsResponse stream
   let mdns_stream = response_stream
      // Filtering out Empty responses
      .filter(move |res| {
            match res {
            Ok(response) => !response.is_empty() && !response.ip_addr().is_none(),
            Err(_) => true, // Keep errors
         }
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
         let mut service_type = String::new();
         println!("Response Answer count = {}", response.answers.len());
         for answer in response.answers {
            match  answer.kind {
               RecordKind::TXT(txts) => {
                  //println!("TXT count = {}", txts.len());
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
                  let names: Vec<&str> = answer.name.split("._").collect();
                  //println!("answer.name = {}", answer.name);
                  service_type = names[0][1..].to_string();
               },
               _ => {},
            }
         }
         Ok(MdnsResponse {service_type, service_name, addr, buffer })
      });
   // Done
   mdns_stream
}
