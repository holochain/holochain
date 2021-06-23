use super::*;

pub(crate) async fn run(_opt: KdOptBootstrap) -> KdResult<()> {
    let (addr, complete, _) = new_quick_bootstrap_v1().await?;
    println!("{}", addr);
    complete.await;
    Ok(())
}
