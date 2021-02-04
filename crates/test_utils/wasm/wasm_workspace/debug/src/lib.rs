use hdk3::prelude::*;

#[hdk_extern]
fn debug(_: ()) -> ExternResult<()> {
    trace!("tracing {}", "works!");
    debug!("debug works");
    info!("info works");
    warn!("warn works");
    error!("error works");
    debug!(foo = "fields", bar = "work", "too");

    Ok(())
}
