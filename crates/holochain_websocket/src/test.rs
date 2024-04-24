//! holochain_websocket tests

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};

use crate::*;

#[tokio::test(flavor = "multi_thread")]
async fn sanity() {
    holochain_trace::test_run();

    #[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes, PartialEq)]
    enum TestMsg {
        Hello,
    }

    let (addr_s, addr_r) = tokio::sync::oneshot::channel();

    let l_task = tokio::task::spawn(async move {
        let l = WebsocketListener::bind(Arc::new(WebsocketConfig::LISTENER_DEFAULT), "localhost:0")
            .await
            .unwrap();

        let addr = l.local_addrs().unwrap();
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

    let addr = addr_r.await.unwrap()[0];
    println!("addr: {}", addr);

    let r_task = tokio::task::spawn(async move {
        let (send, mut recv) = connect(Arc::new(WebsocketConfig::CLIENT_DEFAULT), addr)
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

#[tokio::test(flavor = "multi_thread")]
async fn blocks_connect_with_mismatched_origin() {
    holochain_trace::test_run();

    let (addr_s, addr_r) = tokio::sync::oneshot::channel();

    let l_task = tokio::task::spawn(async move {
        let mut config = WebsocketConfig::LISTENER_DEFAULT;
        config.allowed_origins = Some(AllowedOrigins::Origins(
            ["http://example.com".to_string()].into_iter().collect(),
        ));

        let l = WebsocketListener::bind(Arc::new(config), "localhost:0")
            .await
            .unwrap();

        let addr = l.local_addrs().unwrap();
        addr_s.send(addr).unwrap();

        match l.accept().await {
            Ok(_) => panic!("should not have accepted"),
            Err(e) => {
                assert_eq!(e.to_string(), "HTTP error: 400 Bad Request");
            }
        }
    });

    let addr = addr_r.await.unwrap()[0];

    let r_task = tokio::task::spawn(async move {
        match connect(
            Arc::new(WebsocketConfig::CLIENT_DEFAULT),
            ConnectRequest::new(addr)
                .try_set_header("Origin", "http://other.org")
                .unwrap(),
        )
        .await
        {
            Ok(_) => panic!("should not have connected"),
            Err(e) => {
                assert_eq!(e.to_string(), "HTTP error: 400 Bad Request");
            }
        }
    });

    l_task.await.unwrap();
    r_task.await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn blocks_connect_without_origin() {
    holochain_trace::test_run();

    let (addr_s, addr_r) = tokio::sync::oneshot::channel();

    let l_task = tokio::task::spawn(async move {
        let mut config = WebsocketConfig::LISTENER_DEFAULT;
        config.allowed_origins = Some(AllowedOrigins::Origins(
            ["http://example.com".to_string()].into_iter().collect(),
        ));

        let l = WebsocketListener::bind(Arc::new(config), "localhost:0")
            .await
            .unwrap();

        let addr = l.local_addrs().unwrap();
        addr_s.send(addr).unwrap();

        match l.accept().await {
            Ok(_) => panic!("should not have accepted"),
            Err(e) => {
                assert_eq!(e.to_string(), "HTTP error: 400 Bad Request");
            }
        }
    });

    let addr = addr_r.await.unwrap()[0];

    let r_task = tokio::task::spawn(async move {
        match connect(
            Arc::new(WebsocketConfig::CLIENT_DEFAULT),
            ConnectRequest::new(addr).clear_headers(),
        )
        .await
        {
            Ok(_) => panic!("should not have connected"),
            Err(e) => {
                assert_eq!(e.to_string(), "HTTP error: 400 Bad Request");
            }
        }
    });

    l_task.await.unwrap();
    r_task.await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn origin_is_required_on_listener() {
    holochain_trace::test_run();

    let mut config = WebsocketConfig::LISTENER_DEFAULT;
    config.allowed_origins = None;

    match WebsocketListener::bind(Arc::new(config), "localhost:0").await {
        Ok(_) => panic!("should have prevented bind"),
        Err(e) => {
            assert_eq!(
                e.to_string(),
                "WebsocketListener requires allowed_origins to be set in the config"
            );
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn ipv6_or_ipv4_connect() {
    holochain_trace::test_run();

    #[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes, PartialEq)]
    enum TestMsg {
        Hello,
    }

    let (addr_s, addr_r) = tokio::sync::oneshot::channel();

    let l_task = tokio::task::spawn(async move {
        let l = WebsocketListener::dual_bind(
            Arc::new(WebsocketConfig::LISTENER_DEFAULT),
            SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0),
            SocketAddrV6::new(Ipv6Addr::LOCALHOST, 0, 0, 0),
        )
        .await
        .unwrap();

        addr_s.send(l.local_addrs().unwrap()).unwrap();

        for _ in 0..2 {
            let (_send, mut recv) = l.accept().await.unwrap();

            let res = recv.recv::<TestMsg>().await.unwrap();
            match res {
                ReceiveMessage::Request(data, res) => {
                    assert_eq!(TestMsg::Hello, data);
                    res.respond(TestMsg::Hello).await.unwrap();
                }
                oth => panic!("unexpected: {oth:?}"),
            }
        }
    });

    let bound_addr = addr_r.await.unwrap();
    let target_port = bound_addr[0].port();

    let test_addrs: Vec<SocketAddr> = vec![
        (Ipv4Addr::LOCALHOST, target_port).into(),
        (Ipv6Addr::LOCALHOST, target_port).into(),
    ];
    for addr in test_addrs {
        let r_task = tokio::task::spawn(async move {
            let (send, mut recv) = connect(Arc::new(WebsocketConfig::CLIENT_DEFAULT), addr)
                .await
                .unwrap();

            let s_task =
                tokio::task::spawn(
                    async move { while let Ok(_r) = recv.recv::<TestMsg>().await {} },
                );

            let res: TestMsg = send
                .request_timeout(TestMsg::Hello, std::time::Duration::from_secs(5))
                .await
                .unwrap();

            assert_eq!(TestMsg::Hello, res);

            s_task.abort();
        });
        r_task.await.unwrap();
    }

    l_task.await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "Requires a port to be free so should not run on CI"]
async fn ipv6_or_ipv4_connect_on_specific_port() {
    holochain_trace::test_run();

    #[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes, PartialEq)]
    enum TestMsg {
        Hello,
    }

    let (addr_s, addr_r) = tokio::sync::oneshot::channel();

    let l_task = tokio::task::spawn(async move {
        let l = WebsocketListener::dual_bind(
            Arc::new(WebsocketConfig::LISTENER_DEFAULT),
            SocketAddrV4::new(Ipv4Addr::LOCALHOST, 1456),
            SocketAddrV6::new(Ipv6Addr::LOCALHOST, 1456, 0, 0),
        )
            .await
            .unwrap();

        addr_s.send(l.local_addrs().unwrap()).unwrap();

        for _ in 0..2 {
            let (_send, mut recv) = l.accept().await.unwrap();

            let res = recv.recv::<TestMsg>().await.unwrap();
            match res {
                ReceiveMessage::Request(data, res) => {
                    assert_eq!(TestMsg::Hello, data);
                    res.respond(TestMsg::Hello).await.unwrap();
                }
                oth => panic!("unexpected: {oth:?}"),
            }
        }
    });

    let bound_addr = addr_r.await.unwrap();
    let target_port = bound_addr[0].port();

    let test_addrs: Vec<SocketAddr> = vec![
        (Ipv4Addr::LOCALHOST, target_port).into(),
        (Ipv6Addr::LOCALHOST, target_port).into(),
    ];
    for addr in test_addrs {
        let r_task = tokio::task::spawn(async move {
            let (send, mut recv) = connect(Arc::new(WebsocketConfig::CLIENT_DEFAULT), addr)
                .await
                .unwrap();

            let s_task =
                tokio::task::spawn(
                    async move { while let Ok(_r) = recv.recv::<TestMsg>().await {} },
                );

            let res: TestMsg = send
                .request_timeout(TestMsg::Hello, std::time::Duration::from_secs(5))
                .await
                .unwrap();

            assert_eq!(TestMsg::Hello, res);

            s_task.abort();
        });
        r_task.await.unwrap();
    }

    l_task.await.unwrap();
}
