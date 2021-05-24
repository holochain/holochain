use super::*;

pub(crate) async fn run(_opt: KdOptProxy) -> KdResult<()> {
    let (addr, complete) = new_quick_proxy_v1().await?;
    println!("{}", addr);
    complete.await;
    Ok(())
}
