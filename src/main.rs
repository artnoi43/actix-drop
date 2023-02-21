//! soydrop is a simple web app for sharing texts across different computers.
//!
//! # Features
//!
//! soydrop writes text to file or in-memory clipboard store, with a timer.
//! The clipboard is later accessed by referencing the first 4 characters of
//! hex-encoded representation of its SHA2 hash.
//!
//! For security reason, host it behind a firewall and VPN, or use modern reverse proxy
//! like NGINX to enable HTTP Basic Authentication.
//!
//! - In-memory or file storage
//!
//! - Multiple endpoints for different HTTP content type: HTML, JSON, and plain text
//!
//! - Expiration timer (can be reset/extended)
//!
//! - Configuation via files or environment
//!
//! ## Planned features (not yet implemented)
//!
//! - Expandable hash keys using trie nodes for clipboard hashes (see branch `dev/trie`)
//!
//! - AES or RSA encryption,
//!
//! - File upload (probably with multiform)
//!
//! - TCP support

mod config; // soydrop config, not extern crate `config`
mod http_server;
mod resp;
mod store;

#[cfg(unix)] // Our code currently uses UNIX file paths
#[actix_web::main]
async fn main() {
    use std::time::Duration;

    use actix_web::{middleware, web, App, HttpServer};
    use colored::Colorize;

    use crate::config::AppConfig;
    use crate::http_server;
    use crate::resp::http_resp;
    use crate::store::tracker::Tracker;

    let conf = AppConfig::init();
    println!(
        "\n{}\n{}\n",
        "Starting soydrop: current configuration".yellow(),
        serde_json::to_string(&conf).unwrap()
    );

    // Ensure that ./${DIR} is a directory
    store::persist::assert_dir(conf.dir);

    let http_addr = format!(
        "{}:{}",
        conf.http_addr.expect(&"http_addr is None".red()),
        conf.http_port.expect(&"http_port is None".red()),
    );

    println!(
        "{} {}",
        "Starting actix-web on".yellow(),
        format!("http://{}", http_addr).cyan()
    );

    HttpServer::new(move || {
        App::new()
            .wrap(middleware::NormalizePath::new(
                middleware::TrailingSlash::Trim,
            ))
            .app_data(web::Data::new(Duration::from_secs(
                conf.timeout.expect("timeout is None"),
            )))
            .app_data(web::Data::new(String::from(http_server::CSS)))
            .app_data(web::Data::new(Tracker::new()))
            .service(web::resource("/style.css").route(web::get().to(http_server::serve_css)))
            .service(http_server::routes::<http_resp::ResponseHtml>("/app"))
            .service(http_server::routes::<http_resp::ResponseJson>("/api"))
            .service(http_server::routes::<http_resp::ResponseText>("/txt"))
    })
    .bind(http_addr)
    .expect(&"error binding server to address".red())
    .run()
    .await
    .expect(&"error running server".red());
}
