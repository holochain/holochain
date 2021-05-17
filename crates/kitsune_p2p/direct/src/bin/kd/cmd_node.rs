use super::*;
use futures::stream::StreamExt;

pub(crate) async fn run(opt: KdOptNode) -> KitsuneResult<()> {
    let persist = new_persist_mem();

    let conf = KitsuneDirectV1Config {
        persist,
        proxy: opt.proxy_url.into(),
        ui_port: 0,
    };

    let (kd, mut evt) = new_kitsune_direct_v1(conf).await?;
    let node_addrs = kd.list_transport_bindings().await?;
    for addr in node_addrs {
        println!("{}", addr);
    }
    let ui_addr = kd.get_ui_addr()?;
    println!("http://{}", ui_addr);

    while evt.next().await.is_some() {}

    Ok(())
}
