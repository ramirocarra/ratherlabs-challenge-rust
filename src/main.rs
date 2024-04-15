use actix_web::{App, HttpServer, web};

mod prices;
mod binance;

pub mod orderbook;

struct AppState {
  binance_client: binance::BinanceClient,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {

    let (binance_client, binance_handle) = binance::BinanceClient::new();

    let app_data = web::Data::new(AppState {
        binance_client,
    });

    HttpServer::new(move || {
        App::new()
            .app_data(app_data.clone())
            .service(web::scope("/prices").configure(prices::price_routes))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
