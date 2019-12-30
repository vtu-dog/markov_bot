use crate::chain_wrapper;

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use tbot::prelude::*;
use tbot::types::chat::Id;
use tbot::types::chat::Kind::*;
use tbot::types::parameters::Text;


pub fn create (chain: Arc<Mutex<chain_wrapper::ChainWrapper>>) -> tbot::EventLoop<impl tbot::connectors::Connector> {
    let mut bot = tbot::Bot::from_env("HTTP_TOKEN").event_loop();

    bot.start(|context| async move {
        let msg = "Hi! Add me to a group as an administrator to begin your \
                   Markov adventure.\nWant to know more? Use /help.";

        let call_result = context.send_message(msg).call().await;

        if let Err(err) = call_result {
            dbg!(err);
        }
    });

    bot.help(|context| async move {
        let msg = "You can use the following commands:\n\n\
                   /speak - generate a new phrase\n\
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
                let is_allowed = if let Private { .. } = &context.chat.kind { true }
                else {
                    match context.from.as_ref() {
                        Some(usr) => {
                            let status = context.get_chat_member(usr.id).call().await.unwrap().status;
                            status.is_administator() || status.is_creator()
                        },
                        None => true
                    }
                };

                let mut msg = String::new();

                if is_allowed {
                    let Id(id) = context.chat.id;
                    msg.push_str(&chain.lock().unwrap().toggle_learning(id));
                } else {
                    msg.push_str("Insufficient permissions! Did you remember to add me as an admin?");
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

        bot.command("clear_data", move |context| {
            let chain = ch.clone();
            async move {
                let is_allowed = if let Private { .. } = &context.chat.kind { true }
                else {
                    match context.from.as_ref() {
                        Some(usr) => {
                            let status = context.get_chat_member(usr.id).call().await.unwrap().status;
                            status.is_creator()
                        },
                        None => true
                    }
                };

                let mut msg = String::new();

                if is_allowed {
                    let Id(id) = context.chat.id;
                    msg.push_str(&chain.lock().unwrap().clear_data(id));
                } else {
                    msg.push_str("Insufficient permissions! Did you remember to add me as an admin?");
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

    bot
}
