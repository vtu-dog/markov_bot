use crate::gdrive;

use std::{collections::HashMap, time::{Duration, SystemTime}, thread};

use markov::Chain;
use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize)]
struct ChainInfo {
    chain: Chain<String>,
    chat_id: i64,
    is_learning: bool,
    last_accessed: SystemTime,
}

impl ChainInfo {
    pub fn serialize_to_gdrive (&self) {
        if !self.chain.is_empty() {
            let binc = bincode::serialize(&self).expect("Serialization failed");
            gdrive::replace_file(&binc, &self.chat_id.to_string());
        }
    }

    fn deserialize_from_gdrive (chat_id: i64) -> Option<ChainInfo> {
        let name = chat_id.to_string();

        match gdrive::download_file(&name) {
            Some(v_u8) => match bincode::deserialize(&v_u8) {
                Ok(c) => Some(c),
                Err(_e) => {
                    gdrive::delete_file(&name);
                    None
                }
            },
            None => None
        }
    }

    fn touch (&mut self) {
        self.last_accessed = SystemTime::now();
    }

    pub fn new (chat_id: i64) -> ChainInfo {
        match ChainInfo::deserialize_from_gdrive(chat_id) {
            Some(mut chain_info) => {
                chain_info.last_accessed = SystemTime::now();
                chain_info
            },
            None => ChainInfo {
                chain: Chain::<String>::new(),
                chat_id: chat_id,
                is_learning: true,
                last_accessed: SystemTime::now(),
            }
        }
    }

    pub fn feed (&mut self, msg: &str) {
        self.touch();

        if self.is_learning {
            msg.lines().for_each(|line| {
                let ln = line.trim();
                if ln != "" {
                    self.chain.feed_str(ln);
                }
            });
        }
    }

    fn gen_loop (&self) -> String {
        loop {
            let sth = self.chain.generate_str();
            if sth.trim().is_empty() { continue; }
            else { break sth; }
        }
    }

    pub fn generate (&mut self, token: &str) -> String {
        self.touch();

        if !self.chain.is_empty() {
            if token.trim().is_empty() {
                self.gen_loop()
            } else {
                let sth = self.chain.generate_str_from_token(token);
                if sth.trim().is_empty() {
                    self.gen_loop()
                } else {
                    sth
                }
            }
        } else {
            String::from("[no phrases learnt]")
        }
    }

    pub fn toggle_learning (&mut self) -> String {
        self.touch();

        if self.is_learning {
            self.is_learning = false;
            String::from("Learning disabled.")
        } else {
            self.is_learning = true;
            String::from("Learning enabled.")
        }
    }

    pub fn clear_data (&mut self) {
        self.touch();
        self.chain = Chain::<String>::new();
        gdrive::delete_file(&self.chat_id.to_string());
    }
}

impl Drop for ChainInfo {
    fn drop (&mut self) {
        self.serialize_to_gdrive();
    }
}


pub struct ChainWrapper {
    chains: HashMap<i64, ChainInfo>,
}

impl ChainWrapper {
    const MAX_TIMEDELTA: Duration = Duration::from_secs(30 * 60);

    pub fn new () -> ChainWrapper {
        let chains = HashMap::new();
        ChainWrapper { chains: chains }
    }

    fn get_chain (&mut self, chat_id: i64) -> &mut ChainInfo {
        self.chains
            .entry(chat_id)
            .or_insert_with(|| ChainInfo::new(chat_id))
    }

    pub fn feed (&mut self, chat_id: i64, s: &str) {
        self.get_chain(chat_id).feed(s);
    }

    pub fn generate (&mut self, chat_id: i64, token: &str) -> String {
        self.get_chain(chat_id).generate(token)
    }

    pub fn toggle_learning (&mut self, chat_id: i64) -> String {
        self.get_chain(chat_id).toggle_learning()
    }

    pub fn clear_data (&mut self, chat_id: i64) -> String {
        if let Some(mut c) = self.chains.remove(&chat_id) {
            c.clear_data();
        };
        String::from("Database cleared.")
    }

    pub fn drop_all (&mut self) {
        self.chains.retain(|_, _| false);
    }

    fn is_old (elem: &ChainInfo) -> bool {
        elem.last_accessed.elapsed().unwrap() > ChainWrapper::MAX_TIMEDELTA
    }

    pub fn prune (&mut self) {
        if self.chains.iter().filter(|(_, x)| ChainWrapper::is_old(x)).count() != 0 {
            self.chains.retain(|_, x| { !ChainWrapper::is_old(x) });
            thread::sleep(Duration::from_secs(5));
        }
    }
}
