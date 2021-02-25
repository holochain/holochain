///! Use libmdns crate for broadcasting
///! Une mdns crate for discovery

use futures_util::{pin_mut, stream::StreamExt, self, future::ready};
use std::thread;
use std::time::Duration;
use regex::Regex;
use std::sync::mpsc::{self, TryRecvError, Sender};
use mdns::{discover::Discovery, RecordKind};
use futures_core::Stream;
use err_derive::Error;

const HC_SERVICE_NAME: &str       = "_holochain._udp";
const BROADCAST_INTERVAL_SEC: u64 = 8;
const QUERY_INTERVAL_SEC: u64     = 5;
const BASE64_REGEX:& str          = "[^-A-Za-z0-9+/=]|=[^=]|={3,}";



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


/// Create a thread that will broadcast the holochain service over mdns
/// Returns Sender for sending thread termination command
pub fn mdns_create_broadcast_thread(name: &str, buffer: &[u8]) -> Sender<()> {
   // Create Terminate command channel
   let (tx, rx) = mpsc::channel();
   // Change buffer to base64 string
   let b64 =format!("u{}", base64::encode_config(buffer, base64::URL_SAFE_NO_PAD));
   // Format into a TXT attribute
   let txt = format!("\"{}={}\"", name, b64);
   println!("TXT = {} ({})", txt, txt.len());
   // Create thread
   let _handle = thread::spawn(move || {
      // debug
      println!("Entering mdns broadcasting thread...");
      // Create mdns responder
      let responder = libmdns::Responder::new().unwrap();
      let _svc = responder.register(
         HC_SERVICE_NAME.to_owned(),
         "holonode".to_owned(),
         80,
         &[&b64],
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


pub struct MdnsResponse {
   pub addr: std::net::IpAddr,
   pub buffer: Vec<u8>,
}

/// Gets an iterator over all responses for the holochain service
pub fn mdns_listen() -> impl Stream<Item = Result<MdnsResponse, MdnsError>> {
   let service_name = format!("{}.local", HC_SERVICE_NAME);
   let query = mdns::discover::all(service_name, Duration::from_secs(QUERY_INTERVAL_SEC))
   .expect("mdns Discover failed");
   // Get Mdns Response stream
   let response_stream = query.listen();
   // Change it into a MdnsResponse stream
   let mdns_stream = response_stream
      // Filtering out Empty responses
      .filter(move |res| {
         ready(match res {
            Ok(response) => !response.is_empty() && !response.ip_addr().is_none(),
            Err(_) => true, // Keep errors
         })
      })
      .map(|maybe_response| {
         if let Err(e) = maybe_response {
            return Err(MdnsError::Mdns(e));
         }
         let response = maybe_response.unwrap();
         let addr = response.ip_addr().unwrap(); // should have already been filtered out
         let mut buffer = Vec::new();
         for answer in response.answers {
            if let RecordKind::TXT(txts) = answer.kind {
               for txt in txts {
                  println!("Response TXT = {:?}", txt);

                  buffer = match base64::decode_config(&txt[1..], base64::URL_SAFE_NO_PAD) {
                     Err(e) => return Err(MdnsError::Base64(e)),
                     Ok(s) => s,
                  };
                  // Expecting only one valid response
                  return Ok(MdnsResponse {addr, buffer });
               }
            }
         }
         Ok(MdnsResponse {addr, buffer })
      });
   // Done
   mdns_stream
}

// -- Deprecated -- //

type MdnsCallback = fn(addr: std::net::IpAddr, /*name: &str,*/ buffer: &[u8]);

///
///
pub async fn mdns_create_discovery_thread(callback: MdnsCallback) /*-> Sender<()>*/ {
   // Create Terminate command channel
   //let (tx, rx) = mpsc::channel();

   // Create thread
   println!("Launching discovery thread...");

   //let _ = thread::spawn(move || async move {

      println!("Entered mdns discovery thread...");
      let service_name = format!("{}.local", HC_SERVICE_NAME);
      let query = mdns::discover::all(service_name, Duration::from_secs(QUERY_INTERVAL_SEC))
         .expect("mdns discover failed");
      let stream = query.listen();
      pin_mut!(stream);
      //let re = Regex::new(r"(?P<name>.*)=(?P<base64>.*)").unwrap();

      //loop {
         // match rx.try_recv() {
         //    Ok(_) | Err(TryRecvError::Disconnected) => {
         //       println!("Terminating.");
         //       break;
         //    }
         //    Err(TryRecvError::Empty) => {}
         // }

         while let Some(Ok(response)) = stream.next().await {
            //println!("Found response {:?}", response);
            // Must have IP address
            let maybe_addr = response.ip_addr();
            if maybe_addr.is_none() {
               println!("Device does not advertise address");
               continue;
            }
            // Must have TXT field with base64 attribute
            for answer in response.answers.clone() {
               if let RecordKind::TXT(txts) = answer.kind {
                  for txt in txts {
                     println!("Response TXT = {:?}", txt);

                     let buffer = match base64::decode_config(&txt[1..], base64::URL_SAFE_NO_PAD) {
                        Err(_) => panic!("Err(HoloHashError::BadBase64)"),
                        Ok(s) => s,
                     };

                     // let caps = re.captures(txt).expect("Regex capture failed");
                     // let maybe_name = caps.get(1);
                     // let maybe_b64 = caps.get(2);
                     // if maybe_name.is_none() || maybe_b64.is_none() {
                     //    continue;
                     // }
                     callback(maybe_addr.unwrap(), &buffer /*maybe_name.unwrap(), maybe_b64.unwrap()*/);
                  }
               }
            }
         }
      //}
      println!("EXITING discovery thread.");
   //});
   // Done
   //tx
}