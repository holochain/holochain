pub(crate) mod bench;
pub(crate) mod error;
pub(crate) mod favicon;
pub(crate) mod github;

// use std::convert::TryFrom;
// use actix_service::Service;
// use futures::future::FutureExt;

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    let port = std::env::var("PEWPEWPEW_PORT").unwrap();

    let bench_actor = actix::SyncArbiter::start(1, move || bench::actor::Actor);

    actix_web::HttpServer::new(move || {
        actix_web::App::new()
            // // .wrap(github::middleware::SayHiMiddleware)
            // .wrap_fn(|req, srv| {
            //
            //     match crate::github::signature::ValidSignature::try_from(&req) {
            //         Ok(_) => { },
            //         Err(_) => { },
            //     }
            //
            //     srv.call(req).map(|res| {
            //         println!("Hi from response");
            //         res
            //     })
            // })
            .data(bench::web::AppState {
                actor: bench_actor.clone(),
            })
            .data(github::web::AppState {
                actor: bench_actor.clone(),
            })
            // short circuit favicon requests
            .route(
                "/favicon.ico",
                actix_web::web::get().to(favicon::web::favicon),
            )
            // "raw" benchmark URL
            // useful for testing but github events is a better route because it has security
            // to be clear THERE IS NO SECURITY ON THIS ENDPOINT
            // .route("/bench/{commit}", actix_web::web::get().to(bench::web::commit))
            .route("/github/push", actix_web::web::post().to(github::web::push))
    })
    .bind(format!("127.0.0.1:{}", port))?
    .run()
    .await
}
