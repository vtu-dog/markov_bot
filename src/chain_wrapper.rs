use crate::gdrive;

use std::{collections::HashMap, time::{Duration, SystemTime}};

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
    fn get_bincode (&self) -> Vec<u8> {
        bincode::serialize(&self).expect("Serialization failed")
    }

    fn serialize_to_gdrive (&self) -> Option<String> {
        if !self.chain.is_empty() {
            let binc = self.get_bincode();
            gdrive::update_or_create_file(&binc, &self.chat_id.to_string())
        } else {
            None
        }
    }

    fn deserialize_from_gdrive (chat_id: i64) -> Result<Option<ChainInfo>, String> {
        let name = chat_id.to_string();

        match gdrive::download_file(&name) {
            Err(e) => Err(e),
            Ok(buf) => match buf {
                None => Ok(None),
                Some(v_u8) => match bincode::deserialize(&v_u8) {
                    Ok(c) => Ok(Some(c)),
                    Err(e) => Err(format!("Deserialization failed for {}: {}", chat_id, e))
                }
            }
        }
    }

    pub fn new (chat_id: i64) -> Result<ChainInfo, String> {
        match ChainInfo::deserialize_from_gdrive(chat_id) {
            Err(e) => Err(e),
            Ok(obj) => match obj {
                Some(mut chain_info) => {
                    chain_info.last_accessed = SystemTime::now();
                    Ok(chain_info)
                },
                None => Ok(ChainInfo {
                    chain: Chain::<String>::new(),
                    chat_id: chat_id,
                    is_learning: true,
                    last_accessed: SystemTime::now(),
                })
            }
        }
    }

    fn touch (&mut self) {
        self.last_accessed = SystemTime::now();
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
            String::from("[learning disabled]")
        } else {
            self.is_learning = true;
            String::from("[learning enabled]")
        }
    }

    pub fn clear_data (&mut self) -> Option<String> {
        self.chain = Chain::<String>::new();
        self.is_learning = true;
        self.touch();

        let binc = self.get_bincode();
        gdrive::update_or_create_file(&binc, &self.chat_id.to_string())
    }
}

impl Drop for ChainInfo {
    fn drop (&mut self) {
        if let Some(err) = self.serialize_to_gdrive() {
            dbg!(err);
        }
    }
}


pub struct ChainWrapper {
    chains: HashMap<i64, ChainInfo>,
}

impl ChainWrapper {
    const MAX_TIMEDELTA: Duration = Duration::from_secs(30 * 60);
    const COMMAND_FAILED: &'static str = "[command failed, please try again later]";

    pub fn new () -> ChainWrapper {
        let chains = HashMap::new();
        ChainWrapper { chains: chains }
    }

    fn err_msg () -> String {
        String::from(ChainWrapper::COMMAND_FAILED)
    }

    fn get_chain (&mut self, chat_id: i64) -> Result<&mut ChainInfo, String> {
        if self.chains.contains_key(&chat_id) {
            Ok(self.chains.entry(chat_id).or_insert_with(|| panic!("HashMap changed mid-extraction")))
        } else {
            match ChainInfo::new(chat_id) {
                Ok(chain) => {
                    self.chains.insert(chat_id, chain);
                    self.get_chain(chat_id)
                },
                Err(e) => Err(e)
            }
        }
    }

    pub fn feed (&mut self, chat_id: i64, s: &str) {
        match self.get_chain(chat_id) {
            Ok(chain) => chain.feed(s),
            Err(e) => { dbg!(e); }
        }
    }

    pub fn generate (&mut self, chat_id: i64, token: &str) -> String {
        match self.get_chain(chat_id) {
            Ok(chain) => chain.generate(token),
            Err(e) => {
                dbg!(e);
                ChainWrapper::err_msg()
            }
        }
    }

    pub fn toggle_learning (&mut self, chat_id: i64) -> String {
        match self.get_chain(chat_id) {
            Ok(chain) => chain.toggle_learning(),
            Err(e) => {
                dbg!(e);
                ChainWrapper::err_msg()
            }
        }
    }

    pub fn clear_data (&mut self, chat_id: i64) -> String {
        match self.chains.remove(&chat_id) {
            Some(mut c) => match c.clear_data() {
                Some(err) => {
                    dbg!(err);
                    ChainWrapper::err_msg()
                },
                None => String::from("[database cleared]")
            },
            None => String::from("[database cleared]")
        }
    }

    pub fn drop_all (&mut self) {
        self.chains.retain(|_, _| false);
    }

    fn is_old (elem: &ChainInfo) -> bool {
        elem.last_accessed.elapsed().unwrap() > ChainWrapper::MAX_TIMEDELTA
    }

    pub fn prune (&mut self) {
        self.chains.retain(|_, x| { !ChainWrapper::is_old(x) });
    }
}
