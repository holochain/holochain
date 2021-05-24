use futures::stream::StreamExt;
use kitsune_p2p_direct::prelude::*;

fn print(json: &serde_json::Value) {
    println!("{}", serde_json::to_string_pretty(&json).unwrap());
}

fn to_api(json: serde_json::Value) -> KdApi {
    KdApi::User { user: json }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let (srv, mut evt) = new_srv(Default::default(), 0).await.unwrap();
    print(&serde_json::json!({
        "event": "KdSrv.listening",
        "local_addr": format!("http://{}", srv.local_addr().unwrap()),
    }));

    while let Some(evt) = evt.next().await {
        match evt {
            KdSrvEvt::HttpRequest {
                uri,
                method,
                headers: _,
                body,
                respond_cb,
            } => {
                let data = serde_json::json!({
                    "event": "KdSrv.incoming_http",
                    "uri": uri,
                    "method": method,
                    "body": String::from_utf8_lossy(&body),
                });
                print(&data);
                let mut resp = HttpResponse::default();
                resp.body = serde_json::to_string_pretty(&data).unwrap().into_bytes();
                respond_cb(Ok(resp)).await.unwrap();
            }
            KdSrvEvt::WebsocketConnected { .. } => (),
            KdSrvEvt::WebsocketMessage { con, data } => {
                let data = serde_json::json!({
                    "event": "KdSrv.incoming_websocket",
                    "data": data,
                });
                print(&data);
                srv.websocket_broadcast(to_api(serde_json::json!({
                    "event": "KdSrv.websocket_broadcast",
                    "data": &data,
                })))
                .await
                .unwrap();
                srv.websocket_send(
                    con,
                    to_api(serde_json::json!({
                        "event": "KdSrv.websocket_send",
                        "data": &data,
                    })),
                )
                .await
                .unwrap();
            }
        }
    }
}
