//! holochain_websocket tests

use crate::*;

#[tokio::test(flavor = "multi_thread")]
async fn sanity() {
    holochain_trace::test_run().ok();

    #[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes, PartialEq)]
    enum TestMsg {
        Hello,
    }

    let (addr_s, addr_r) = tokio::sync::oneshot::channel();

    let l_task = tokio::task::spawn(async move {
        let l = WebsocketListener::bind(Arc::new(WebsocketConfig::default()), "localhost:0")
            .await
            .unwrap();

        let addr = l.local_addr().unwrap();
        addr_s.send(addr).unwrap();

        let (_send, mut recv) = l.accept().await.unwrap();

        let res = recv.recv::<TestMsg>().await.unwrap();
        assert_eq!(
            ReceiveMessage::Signal(encode(&TestMsg::Hello).unwrap()),
            res
        );

        let res = recv.recv::<TestMsg>().await.unwrap();
        match res {
            ReceiveMessage::Request(data, res) => {
                assert_eq!(TestMsg::Hello, data);
                res.respond(TestMsg::Hello).await.unwrap();
            }
            oth => panic!("unexpected: {oth:?}"),
        }
    });

    let addr = addr_r.await.unwrap();
    println!("addr: {}", addr);

    let r_task = tokio::task::spawn(async move {
        let (send, mut recv) = connect(Arc::new(WebsocketConfig::default()), addr)
            .await
            .unwrap();

        send.signal_timeout(TestMsg::Hello, std::time::Duration::from_secs(5))
            .await
            .unwrap();

        let s_task =
            tokio::task::spawn(async move { while let Ok(_r) = recv.recv::<TestMsg>().await {} });

        let res: TestMsg = send
            .request_timeout(TestMsg::Hello, std::time::Duration::from_secs(5))
            .await
            .unwrap();

        assert_eq!(TestMsg::Hello, res);

        s_task.abort();
    });

    l_task.await.unwrap();
    r_task.await.unwrap();
}
