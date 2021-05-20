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

    let _root = mk_demo(&kd).await?;

    println!("http://{}", ui_addr);

    while evt.next().await.is_some() {}

    Ok(())
}

const ICON: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" width="256" height="256">
    <path d="M 24 16 L 24 240 L 48 240 L 48 152 L 104 240 L 192 240 L 240 128 L 192 16 L 104 16 L 48 104 L 48 16 L 24 16 z M 128 32 L 128 224 L 64 128 L 128 32 z M 152 32 L 176 32 L 216 128 L 176 224 L 152 224 L 152 32 z " />
</svg>"#;

const INDEX: &[u8] = br#"<!DOCTYPE html>
<html>
  <head>
    <meta charset="UTF-8" />
    <link rel="icon" type="image/svg+xml" href="/favicon.svg" />
  </head>
  <body>
    <img src="/favicon.svg" />
  </body>
</html>"#;

async fn mk_demo(kd: &KitsuneDirect) -> KitsuneResult<KdHash> {
    let persist = kd.get_persist();
    let root = persist.generate_signing_keypair().await?;

    let mk_entry = |t: &'static str, p: &KdHash, d: serde_json::Value, b: &[u8]| {
        let e = KdEntryContent {
            kind: t.to_string(),
            parent: p.clone(),
            author: root.clone(),
            verify: "".to_string(),
            data: d,
        };
        let fut = KdEntrySigned::from_content_with_binary(&persist, e, b);
        async {
            let e = fut.await.map_err(KitsuneError::other)?;
            let e_hash = e.hash().clone();
            kd.publish_entry(root.clone(), root.clone(), e).await?;
            KitsuneResult::Ok(e_hash)
        }
    };

    let app = mk_entry("s.app", &root, serde_json::json!({}), &[]).await?;
    let ui = mk_entry("s.ui", &app, serde_json::json!({}), &[]).await?;
    let _favicon = mk_entry(
        "s.file",
        &ui,
        serde_json::json!({
            "name": "favicon.svg",
            "mime": "image/svg+xml",
        }),
        ICON,
    )
    .await?;
    let _index_html = mk_entry(
        "s.file",
        &ui,
        serde_json::json!({
            "name": "index.html",
            "mime": "text/html",
        }),
        INDEX,
    )
    .await?;
    let _index = mk_entry(
        "s.index",
        &ui,
        serde_json::json!({
            "path": "/index.html",
        }),
        &[],
    )
    .await?;

    Ok(root)
}
