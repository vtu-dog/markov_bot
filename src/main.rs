mod bot;
mod chain_wrapper;
mod gdrive;
mod utils;

use std::sync::{Arc, Mutex};

use dotenv::dotenv;
use futures::future::select;
use tokio::signal::unix::*;

#[tokio::main]
async fn main() {
    // load environment variables
    dotenv().ok();

    // create a connection to Google Drive
    utils::parse_credentials();
    gdrive::initialize();

    // register a SIGTERM handler
    let mut sigstream =
        signal(SignalKind::terminate()).expect("Failed to register SIGTERM handler");
    let sig = sigstream.recv();

    // create a container for Markov chains
    let chain = Arc::new(Mutex::new(chain_wrapper::ChainWrapper::new()));

    // create and start the bot
    let bot = bot::create(chain.clone());
    let polling = bot.polling().error_handler(|_| async {}).start();

    // await SIGTERM and ensure that polling is stopped
    select(Box::pin(polling), Box::pin(sig)).await;

    // write all changes to Google Drive
    chain.lock().unwrap().drop_all();
}
