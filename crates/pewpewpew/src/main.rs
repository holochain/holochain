pub(crate) mod bench;
pub(crate) mod error;
pub(crate) mod favicon;
pub(crate) mod github;

const PORT_ENV_NAME: &str = "PEWPEWPEW_PORT";

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    // fatal to not have a port to bind to on boot
    let port = std::env::var(PORT_ENV_NAME).unwrap();

    let bench_actor = actix::SyncArbiter::start(1, move || bench::actor::Actor);

    actix_web::HttpServer::new(move || {
        actix_web::App::new()
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
