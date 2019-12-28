use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::prelude::*;
use std::path::Path;
use std::time::{Duration, SystemTime};

use markov::Chain;
use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize)]
struct ChainInfo {
    chain: Chain<String>,
    chain_size: u64,
    chat_id: i64,
    is_learning: bool,
    last_accessed: SystemTime,
}

impl ChainInfo {
    fn get_path (chat_id: i64) -> String {
        format!(
            "{}{}",
            env::var("CHAINDUMP_DIR").expect("CHAINDUMP_DIR not set in .env"),
            chat_id
        )
    }

    fn del_chaindump (&self, path: &str) -> Result<(), std::io::Error> {
        if Path::new(&path).exists() {
            fs::remove_file(&path)
        } else {
            Ok(())
        }
    }

    pub fn serialize_to_file (&self) {
        if self.chain_size == 0 {
            return;
        }

        let path = ChainInfo::get_path(self.chat_id);
        self.del_chaindump(&path)
            .expect("Couldn't remove old files while serializing");

        let mut f = fs::OpenOptions::new()
            .read(false)
            .write(true)
            .create(true)
            .open(&path)
            .unwrap();

        let binc = bincode::serialize(&self).expect("Serialization failed");

        f.write_all(&binc).expect("Write to file failed");
        f.sync_all().expect("Synchronization failed");
    }

    fn deserialize_from_file (chat_id: i64) -> Option<ChainInfo> {
        let path = ChainInfo::get_path(chat_id);

        if Path::new(&path).exists() {
            let mut file = fs::File::open(&path).expect("Failed to load file");
            let mut data = Vec::new();
            file.read_to_end(&mut data).expect("Failed to read file");

            match bincode::deserialize(&data) {
                Ok(c) => Some(c),
                Err(_e) => {
                    fs::remove_file(&path).expect("Couldn't remove corrupt file");
                    None
                }
            }
        } else {
            None
        }
    }

    pub fn new (chat_id: i64) -> ChainInfo {
        match ChainInfo::deserialize_from_file(chat_id) {
            Some(mut chain_info) => {
                chain_info.last_accessed = SystemTime::now();
                chain_info
            }
            None => ChainInfo {
                chain: Chain::<String>::new(),
                chain_size: 0,
                chat_id: chat_id,
                is_learning: true,
                last_accessed: SystemTime::now(),
            },
        }
    }

    pub fn feed (&mut self, s: &str) {
        self.last_accessed = SystemTime::now();

        if self.is_learning {
            self.chain.feed_str(s);
            self.chain_size += 1;
        }
    }

    pub fn generate (&mut self) -> String {
        self.last_accessed = SystemTime::now();

        if self.chain_size != 0 {
            self.chain.generate_str()
        } else {
            String::from("[no phrases learnt]")
        }
    }

    pub fn toggle_learning (&mut self) -> String {
        self.last_accessed = SystemTime::now();

        if self.is_learning {
            self.is_learning = false;
            String::from("Learning disabled.")
        } else {
            self.is_learning = true;
            String::from("Learning enabled.")
        }
    }

    pub fn clear_data (&mut self) {
        self.chain = Chain::<String>::new();
        self.chain_size = 0;
        self.last_accessed = SystemTime::now();

        self.del_chaindump(&ChainInfo::get_path(self.chat_id))
            .expect("Couldn't remove file while clearing data");
    }
}

impl Drop for ChainInfo {
    fn drop (&mut self) {
        self.serialize_to_file();
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

    pub fn generate (&mut self, chat_id: i64) -> String {
        self.get_chain(chat_id).generate()
    }

    pub fn toggle_learning (&mut self, chat_id: i64) -> String {
        self.get_chain(chat_id).toggle_learning()
    }

    pub fn clear_data (&mut self, chat_id: i64) -> String {
        match self.chains.remove(&chat_id) {
            Some(mut c) => {
                c.clear_data();
            }
            None => { /* pass */ }
        };
        String::from("Database cleared.")
    }

    pub fn serialize_all (&self) {
        for (_key, value) in self.chains.iter() {
            value.serialize_to_file();
        }
    }

    pub fn prune (&mut self) {
        self.chains.retain(|_key, value| {
            value.last_accessed.elapsed().unwrap() < ChainWrapper::MAX_TIMEDELTA
        })
    }
}
