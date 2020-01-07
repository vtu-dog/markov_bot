use crate::chain_wrapper;

use std::{
    env,
    sync::{Arc, Mutex},
    time,
};

use tbot::prelude::*;
use tbot::types::{
    chat::{Id, Kind::*},
    parameters::Text,
};

// creates and returns an event loop for the bot
pub fn create(
    chain: Arc<Mutex<chain_wrapper::ChainWrapper>>,
) -> tbot::EventLoop<impl tbot::connectors::Connector> {
    // create an empty event loop
    let mut bot = tbot::Bot::from_env("HTTP_TOKEN").event_loop();

    // add a callback for /start
    bot.start(|context| async move {
        let msg = "Hi! Add me to a group as an administrator to begin your \
                   Markov adventure.\nWant to know more? Use /help.";

        let call_result = context.send_message(msg).call().await;

        if let Err(err) = call_result {
            dbg!(err);
        }
    });

    // add a callback for /help
    bot.help(|context| async move {
        let msg = "You can use the following commands:\n\n\
                   /speak msg - generate a new phrase (starting from msg if possible)\n\
                   /toggle_learning - enable / disable learning\n\
                   /clear_data - delete ALL data (irreversible!)\n\n\
                   Any more questions? Feature suggestions? Contact @Vyaatu or visit \
                   <a href=\"https://github.com/vyatu/markov_bot\">project's GitHub page</a>";

        let call_result = context.send_message(Text::html(msg)).call().await;

        if let Err(err) = call_result {
            dbg!(err);
        }
    });

    {
        let ch = Arc::clone(&chain);
        // add a callback for /speak msg
        bot.command("speak", move |context| {
            let chain = ch.clone();
            async move {
                let Id(id) = context.chat.id;
                let msg = chain.lock().unwrap().generate(id, &context.text.value);
                let call_result = context.send_message(&msg).call().await;

                if let Err(err) = call_result {
                    dbg!(err);
                }
            }
        });
    }

    {
        let ch = Arc::clone(&chain);
        // add a callback for /toggle_learning
        bot.command("toggle_learning", move |context| {
            let chain = ch.clone();
            async move {
                let is_allowed = if let Private { .. } = &context.chat.kind {
                    // the command was received from a private chat
                    true
                } else {
                    // the command was received from an admin or a group creator
                    match context.from.as_ref() {
                        Some(usr) => {
                            let status =
                                context.get_chat_member(usr.id).call().await.unwrap().status;
                            status.is_administrator() || status.is_creator()
                        }
                        None => true,
                    }
                };

                let mut msg = String::new();

                // execute or refuse the command
                if is_allowed {
                    let Id(id) = context.chat.id;
                    msg.push_str(&chain.lock().unwrap().toggle_learning(id));
                } else {
                    msg.push_str("[only the chat owner and admins can do that]");
                }

                let call_result = context.send_message(&msg).call().await;

                if let Err(err) = call_result {
                    dbg!(err);
                }
            }
        });
    }

    {
        let ch = Arc::clone(&chain);
        // add a callback for /clear_data
        bot.command("clear_data", move |context| {
            let chain = ch.clone();
            async move {
                let is_allowed = if let Private { .. } = &context.chat.kind {
                    // the command was received from a private chat
                    true
                } else {
                    // the command was received from a group creator
                    match context.from.as_ref() {
                        Some(usr) => {
                            let status =
                                context.get_chat_member(usr.id).call().await.unwrap().status;
                            status.is_creator()
                        }
                        None => true,
                    }
                };

                let mut msg = String::new();

                // execute or refuse the command
                if is_allowed {
                    let Id(id) = context.chat.id;
                    msg.push_str(&chain.lock().unwrap().clear_data(id));
                } else {
                    msg.push_str("[only the chat owner can do that]");
                }

                let call_result = context.send_message(&msg).call().await;

                if let Err(err) = call_result {
                    dbg!(err);
                }
            }
        });
    }

    {
        let ch = Arc::clone(&chain);
        // add a callback for non-command messages
        bot.text(move |context| {
            let chain = ch.clone();
            async move {
                if let Some(from) = &context.from {
                    if let Some(_) = from.username {
                        let Id(id) = context.chat.id;
                        chain.lock().unwrap().feed(id, &context.text.value);
                    }
                }
            }
        });
    }

    {
        // set update frequency
        let upd_freq = env::var("UPDATE_FREQUENCY")
            .expect("UPDATE_FREQUENCY not set")
            .parse::<u64>()
            .unwrap();

        let ch = Arc::clone(&chain);
        let dur = time::Duration::from_secs(upd_freq * 60);
        let now = Arc::new(Mutex::new(time::SystemTime::now()));

        // add a callback for periodic serialization
        bot.before_update(move |_| {
            let chain = ch.clone();
            let now = now.clone();
            async move {
                let mut now = now.lock().unwrap();
                // executes only if the last update was performed sufficiently long ago
                if now.elapsed().unwrap() > dur {
                    *now = time::SystemTime::now();
                    chain.lock().unwrap().prune();
                }
            }
        });
    }

    // return the event loop
    bot
}
