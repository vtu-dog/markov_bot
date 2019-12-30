mod bot;
mod chain_wrapper;
mod gdrive;
mod utils;

use std::sync::{Arc, Mutex};

use dotenv::dotenv;
use futures::future::select;
use tokio::signal::unix::*;


#[tokio::main]
async fn main () {
    dotenv().ok();

    utils::parse_credentials();
    gdrive::initialize();

    let mut sigstream = signal(SignalKind::terminate()).expect("Failed to register SIGTERM handler");
    let sig = sigstream.recv();

    let chain = Arc::new(Mutex::new(chain_wrapper::ChainWrapper::new()));
    let bot = bot::create(chain.clone());
    let polling = bot.polling().start();

    select(Box::pin(polling), Box::pin(sig)).await;
    chain.lock().unwrap().serialize_all();
}
