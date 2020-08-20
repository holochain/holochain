/// holds the address to a shared actor that manages all the benchmarking
/// we need to restrict the app to a single actor to ensure that we don't run more than one bench
/// at a time
pub struct AppState {
    pub actor: actix::Addr<super::actor::Actor>,
}

/// bench a specific commit
#[allow(dead_code)]
pub async fn commit(
    data: actix_web::web::Data<AppState>,
    req: actix_web::HttpRequest,
) -> impl actix_web::Responder {
    use actix_web::Responder;

    match req.match_info().get("commit") {
        Some(commit) => {
            data.actor.do_send(super::actor::Commit::from(commit));
            "OK".with_status(actix_web::http::StatusCode::ACCEPTED)
        }
        None => "missing commit to bench".with_status(actix_web::http::StatusCode::BAD_REQUEST),
    }
}
