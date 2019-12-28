mod chain_wrapper;

use std::env;
use std::fs;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use dotenv::dotenv;
use futures::future::select;
use tbot::prelude::*;
use tbot::types::chat::Id;
use tokio::signal::unix::*;

#[tokio::main]
async fn main () {
    dotenv().ok();

    fs::create_dir_all(&env::var("CHAINDUMP_DIR").expect("CHAINDUMP_DIR not set in .env"))
        .expect("Failed to create CHAINDUMP_DIR");

    let mut bot = tbot::Bot::from_env("HTTP_TOKEN").event_loop();
    let mut stream = signal(SignalKind::terminate()).expect("Failed to register SIGTERM handler");
    let sig = stream.recv();

    let cw = chain_wrapper::ChainWrapper::new();
    let chain = Arc::new(Mutex::new(cw));

    bot.start(|context| async move {
        let msg = "Hi! Add me to a group to begin your Markov \
                       adventure.\nWant to know more? Use /help.";

        let call_result = context.send_message(msg).call().await;

        if let Err(err) = call_result {
            dbg!(err);
        }
    });

    bot.help(|context| async move {
        let msg = "You can use the following commands:\n\
                       /speak - generate a new phrase from already learnt sentences\n\
                       /toggle_learning - enable / disable learning new sentences\n\
                       /clear_data - delete ALL data learnt in this group (irreversible!)";

        let call_result = context.send_message(msg).call().await;

        if let Err(err) = call_result {
            dbg!(err);
        }
    });

    {
        let ch = Arc::clone(&chain);

        bot.command("speak", move |context| {
            let chain = ch.clone();
            async move {
                let Id(id) = context.chat.id;
                let msg = chain.lock().unwrap().generate(id);
                let call_result = context.send_message(&msg).call().await;

                if let Err(err) = call_result {
                    dbg!(err);
                }
            }
        });
    }

    {
        let ch = Arc::clone(&chain);

        bot.command("toggle_learning", move |context| {
            let chain = ch.clone();
            async move {
                let Id(id) = context.chat.id;
                let msg = chain.lock().unwrap().toggle_learning(id);
                let call_result = context.send_message(&msg).call().await;

                if let Err(err) = call_result {
                    dbg!(err);
                }
            }
        });
    }

    {
        let ch = Arc::clone(&chain);

        bot.command("clear_data", move |context| {
            let chain = ch.clone();
            async move {
                let Id(id) = context.chat.id;
                let msg = chain.lock().unwrap().clear_data(id);
                let call_result = context.send_message(&msg).call().await;

                if let Err(err) = call_result {
                    dbg!(err);
                }
            }
        });
    }

    {
        let ch = Arc::clone(&chain);

        bot.text(move |context| {
            let chain = ch.clone();
            async move {
                let Id(id) = context.chat.id;
                chain.lock().unwrap().feed(id, &context.text.value);
            }
        });
    }

    {
        let ch = Arc::clone(&chain);
        let dur = Duration::from_secs(15 * 60);
        let now = Arc::new(Mutex::new(SystemTime::now()));

        bot.after_update(move |_| {
            let chain = ch.clone();
            let now = now.clone();
            async move {
                let mut now = now.lock().unwrap();
                if now.elapsed().unwrap() > dur {
                    *now = SystemTime::now();
                    chain.lock().unwrap().prune();
                }
            }
        });
    }

    let polling = bot.polling().start();
    select(Box::pin(polling), Box::pin(sig)).await;

    chain.lock().unwrap().serialize_all();
}
