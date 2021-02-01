use crate::*;
use futures::stream::StreamExt;
use ghost_actor::dependencies::tracing;

fn init_tracing() {
    let _ = tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .finish(),
    );
}

#[tokio::test(threaded_scheduler)]
async fn tls_server_and_client() {
    init_tracing();
    if let Err(e) = tls_server_and_client_inner().await {
        panic!("{:?}", e);
    }
}

async fn tls_server_and_client_inner() -> TransportResult<()> {
    tracing::warn!("start test");

    let tls_config_1 = TlsConfig::new_ephemeral().await?;
    let tls_config_2 = TlsConfig::new_ephemeral().await?;

    let (tls_srv_conf, _tls_cli_conf) = gen_tls_configs(&tls_config_1)?;
    let (_tls_srv_conf, tls_cli_conf) = gen_tls_configs(&tls_config_2)?;

    let (in_con_send, mut in_con_recv) = futures::channel::mpsc::channel::<TransportEvent>(10);

    metric_task(async move {
        while let Some(evt) = in_con_recv.next().await {
            match evt {
                TransportEvent::IncomingChannel(_url, mut send, recv) => {
                    tracing::warn!("incoming channel - reading...");
                    let data = recv.read_to_end().await;
                    let data = String::from_utf8_lossy(&data);
                    let data = format!("echo: {}", data);
                    tracing::warn!("incoming channel - responding...");
                    send.write_and_close(data.into_bytes()).await?;
                    tracing::warn!("incoming channel - responding complete.");
                }
            }
        }
        TransportResult::Ok(())
    });

    let (srv_proxy_send, cli_proxy_recv) = futures::channel::mpsc::channel(10);
    let (cli_proxy_send, srv_proxy_recv) = futures::channel::mpsc::channel(10);

    tls_srv::spawn_tls_server(
        "srv".to_string(),
        url2::url2!("srv://srv.srv"),
        tls_srv_conf,
        in_con_send,
        srv_proxy_send,
        srv_proxy_recv,
    );

    let ((cli_data_send1, cli_data_recv1), (mut cli_data_send2, cli_data_recv2)) =
        kitsune_p2p_types::transport::create_transport_channel_pair();

    let expected_proxy_url = ProxyUrl::new("srv://srv.srv", tls_config_1.cert_digest)?;
    tls_cli::spawn_tls_client(
        "cli".to_string(),
        expected_proxy_url,
        tls_cli_conf,
        cli_data_send1,
        cli_data_recv1,
        cli_proxy_send,
        cli_proxy_recv,
    );

    tracing::warn!("about to write");
    let large_msg = std::iter::repeat(b"a"[0]).take(70_400).collect::<Vec<_>>();
    cli_data_send2.write_and_close(large_msg.clone()).await?;

    tracing::warn!("about to recv");
    let res = cli_data_recv2.collect::<Vec<_>>().await;
    let res = res.into_iter().flat_map(|a| a).collect::<Vec<_>>();
    let data = String::from_utf8_lossy(&res);
    assert_eq!(data.len(), 70_406);
    assert_eq!(
        format!("echo: {}", String::from_utf8_lossy(&large_msg)),
        data
    );

    tracing::warn!("end test");

    Ok(())
}
