/// we don't have a favicon
pub(crate) async fn favicon(_: actix_web::web::Data<crate::bench::web::AppState>, _: actix_web::HttpRequest) -> impl actix_web::Responder {
    use actix_web::Responder;
    "".with_status(actix_web::http::StatusCode::NO_CONTENT)
}
