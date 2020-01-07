use std::{env, fs, io::prelude::*, path::Path, str};

use base64::decode;
use failure::{format_err, Error};
use retry::{
    delay::{jitter, Exponential},
    retry,
};

// takes a Fn closure that returns a Result<U, Error>
// calls the closure until it returns Ok or fails 5 times
pub fn exponential_retry<T, U>(closure: T) -> Result<U, Error>
where
    T: Fn() -> Result<U, Error>,
{
    retry(
        Exponential::from_millis(2)
            .map(jitter)
            .map(|x| x * 100)
            .take(5),
        || closure(),
    )
    .map_err(|e| format_err!("{:?}", e))
}

// deletes a file from a filesystem
pub fn delete_file(path: &str) {
    if Path::new(path).exists() {
        fs::remove_file(&path).expect("Couldn't remove file");
    }
}

// writes a file to a filesystem
pub fn bytes_to_file(bytes: &[u8], path: &str) {
    delete_file(path);

    let mut f = fs::OpenOptions::new()
        .read(false)
        .write(true)
        .create(true)
        .open(path)
        .unwrap();

    f.write_all(bytes).expect("Write to file failed");
    f.sync_all().expect("Synchronization failed");
}

// decodes base64 string from std::env and saves the result to a filesystem
pub fn parse_credentials() {
    let cred_b64 = env::var("GDRIVE_CREDENTIALS").expect("GDRIVE_CREDENTIALS not set");
    let v_u8_b64 = decode(&cred_b64).expect("Failed to decode base64 credentials");
    bytes_to_file(&v_u8_b64, "./credentials.json");
}
