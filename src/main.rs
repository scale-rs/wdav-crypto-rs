use std::env;
use std::net::{IpAddr, SocketAddr};

use dav_server::warp::dav_dir;

#[tokio::main]
async fn main() {
    let port = env::var("PORT").unwrap_or("8080".to_string());
    let port = port.parse::<u16>().unwrap();

    let cd = std::env::current_dir().unwrap();
    let ip: IpAddr = "127.0.0.1".parse().unwrap();
    let addr = SocketAddr::new(ip, port);
    println!("current dir : {}", cd.to_string_lossy().to_string());
    println!("listening on {}.", addr);
    let warp_dav = dav_dir("/tmp".to_owned(), true, true);
    warp::serve(warp_dav).run(addr).await;
}
