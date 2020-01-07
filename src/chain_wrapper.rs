use crate::gdrive;

use std::{
    collections::HashMap,
    env,
    time::{Duration, SystemTime},
};

use lazy_static::lazy_static;
use markov::Chain;
use serde::{Deserialize, Serialize};

// a Markov chain wrapper
// holds the information for each chat
#[derive(Serialize, Deserialize)]
struct ChainInfo {
    chain: Chain<String>,
    chat_id: i64,
    is_learning: bool,
    last_accessed: SystemTime,
}

impl ChainInfo {
    // serializes the current object to a binary blob
    fn get_bincode(&self) -> Vec<u8> {
        bincode::serialize(&self).expect("Serialization failed")
    }

    // sends a binary blob of the current object to Google Drive
    fn serialize_to_gdrive(&self) -> Option<String> {
        if !self.chain.is_empty() {
            let binc = self.get_bincode();
            gdrive::update_or_create_file(&binc, &self.chat_id.to_string())
        } else {
            None
        }
    }

    // downloads a binary blob from Google Drive and populates the current object
    fn deserialize_from_gdrive(chat_id: i64) -> Result<Option<ChainInfo>, String> {
        match gdrive::download_file(&chat_id.to_string()) {
            Err(e) => Err(e),
            Ok(buf) => match buf {
                None => Ok(None),
                Some(v_u8) => match bincode::deserialize(&v_u8) {
                    Ok(c) => Ok(Some(c)),
                    Err(e) => Err(format!("Deserialization failed for {}: {}", chat_id, e)),
                },
            },
        }
    }

    // creates a new ChainInfo
    pub fn new(chat_id: i64) -> Result<ChainInfo, String> {
        match ChainInfo::deserialize_from_gdrive(chat_id) {
            Err(e) => Err(e),
            Ok(obj) => match obj {
                // ChainInfo exists for the given chat
                Some(mut chain_info) => {
                    chain_info.last_accessed = SystemTime::now();
                    Ok(chain_info)
                }
                // ChainInfo does not exist
                None => Ok(ChainInfo {
                    chain: Chain::<String>::new(),
                    chat_id: chat_id,
                    is_learning: true,
                    last_accessed: SystemTime::now(),
                }),
            },
        }
    }

    // updates the last_accessed property
    fn touch(&mut self) {
        self.last_accessed = SystemTime::now();
    }

    // feeds the Markov chain a new string
    pub fn feed(&mut self, msg: &str) {
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

    // generates messages from a Markov chain until one is non-empty
    // chain-generated messages can be of length 0
    // fails after 10 tries - highly improbable, but possible
    fn gen_loop(&self) -> Option<String> {
        let mut res = None;
        for _ in 0..10 {
            let sth = self.chain.generate_str();
            if sth.trim().is_empty() {
                continue;
            } else {
                res = Some(sth);
                break;
            }
        }

        res
    }

    // generates a message from a Markov chain
    pub fn generate(&mut self, token: &str) -> Option<String> {
        self.touch();

        if !self.chain.is_empty() {
            if token.trim().is_empty() {
                // no words were provided after /speak
                self.gen_loop()
            } else {
                // some words were provided after /speak
                let sth = self.chain.generate_str_from_token(token);
                if sth.trim().is_empty() {
                    // no message beginning with the given word can be generated
                    self.gen_loop()
                } else {
                    Some(sth)
                }
            }
        } else {
            Some(String::from("[no phrases learnt]"))
        }
    }

    // toggles learning of new words
    pub fn toggle_learning(&mut self) -> String {
        self.touch();

        if self.is_learning {
            self.is_learning = false;
            String::from("[learning disabled]")
        } else {
            self.is_learning = true;
            String::from("[learning enabled]")
        }
    }

    // deletes the Markov chain data
    pub fn clear_data(&mut self) -> Option<String> {
        self.chain = Chain::<String>::new();
        self.is_learning = true;
        self.touch();

        // clear the binary blob
        let binc = self.get_bincode();
        gdrive::update_or_create_file(&binc, &self.chat_id.to_string())
    }
}

// serializes the object to Google Drive on drop
impl Drop for ChainInfo {
    fn drop(&mut self) {
        if let Some(err) = self.serialize_to_gdrive() {
            dbg!(err);
        }
    }
}

// extracts MAX_TIMEDELTA from std::env and returns a Duration
fn get_max_timedelta() -> Duration {
    let minutes = env::var("MAX_TIMEDELTA")
        .expect("MAX_TIMEDELTA not set")
        .parse::<u64>()
        .unwrap();

    Duration::from_secs(minutes * 60)
}

lazy_static! {
    // the maximum duration a chat can stay idle without getting dropped from memory
    static ref MAX_TIMEDELTA: Duration = get_max_timedelta();
    static ref COMMAND_FAILED: &'static str = "[command failed, please try again later]";
}

// a wrapper for ChainInfo
pub struct ChainWrapper {
    chains: HashMap<i64, ChainInfo>,
}

impl ChainWrapper {
    // creates a new ChainWrapper
    pub fn new() -> ChainWrapper {
        let chains = HashMap::new();
        ChainWrapper { chains: chains }
    }

    // returns an error message string
    fn err_msg() -> String {
        COMMAND_FAILED.to_string()
    }

    // returns the specified ChainInfo object, creating a new one if necessary
    fn get_chain(&mut self, chat_id: i64) -> Result<&mut ChainInfo, String> {
        if self.chains.contains_key(&chat_id) {
            Ok(self
                .chains
                .entry(chat_id)
                .or_insert_with(|| panic!("HashMap changed mid-extraction")))
        } else {
            match ChainInfo::new(chat_id) {
                Ok(chain) => {
                    self.chains.insert(chat_id, chain);
                    self.get_chain(chat_id)
                }
                Err(e) => Err(e),
            }
        }
    }

    // feeds the specified Markov chain a new string
    pub fn feed(&mut self, chat_id: i64, s: &str) {
        match self.get_chain(chat_id) {
            Ok(chain) => chain.feed(s),
            Err(e) => {
                dbg!(e);
            }
        }
    }

    // generates a message from a specified Markov chain
    pub fn generate(&mut self, chat_id: i64, token: &str) -> String {
        match self.get_chain(chat_id) {
            Ok(chain) => match chain.generate(token) {
                Some(s) => s,
                None => ChainWrapper::err_msg(),
            },
            Err(e) => {
                dbg!(e);
                ChainWrapper::err_msg()
            }
        }
    }

    // toggles learning of new words for a specified Markov chain
    pub fn toggle_learning(&mut self, chat_id: i64) -> String {
        match self.get_chain(chat_id) {
            Ok(chain) => chain.toggle_learning(),
            Err(e) => {
                dbg!(e);
                ChainWrapper::err_msg()
            }
        }
    }

    // deletes the specified Markov chain data
    pub fn clear_data(&mut self, chat_id: i64) -> String {
        match self.chains.remove(&chat_id) {
            Some(mut c) => match c.clear_data() {
                Some(err) => {
                    dbg!(err);
                    ChainWrapper::err_msg()
                }
                None => String::from("[database cleared]"),
            },
            None => String::from("[database cleared]"),
        }
    }

    // drops all the ChainInfo objects
    pub fn drop_all(&mut self) {
        self.chains.retain(|_, _| false);
    }

    // checks if the ChainInfo is old enough to be dropped
    fn is_old(elem: &ChainInfo) -> bool {
        elem.last_accessed.elapsed().unwrap() > *MAX_TIMEDELTA
    }

    // prunes all the old ChainInfo objects from memory
    pub fn prune(&mut self) {
        self.chains.retain(|_, x| !ChainWrapper::is_old(x));
    }
}
