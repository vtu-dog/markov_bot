use std::{env, fs, io::prelude::*, path::Path, str, time::Duration};

use base64::decode;
use failure::{format_err, Error};
use futures::future::Future;
use retry::{
    delay::{jitter, Exponential},
    retry,
};

// returns a Vec of 5 durations with a random jitter
fn random_durations() -> Vec<Duration> {
    Exponential::from_millis(2)
        .map(jitter)
        .map(|x| x * 100)
        .take(5)
        .collect()
}

// takes a Fn closure that returns a Result<T, Error>
// calls the closure until it either returns Ok or fails enough times
pub fn exponential_retry<C, T>(closure: C) -> Result<T, Error>
where
    C: Fn() -> Result<T, Error>,
{
    retry(random_durations(), || closure()).map_err(|e| format_err!("{:?}", e))
}

// an asynchronous variation of exponential_retry
pub async fn exponential_retry_async<C, F, T>(closure: C) -> Result<T, Error>
where
    C: Fn() -> F,
    F: Future<Output = Result<T, Error>>,
{
    let mut err = None;
    for duration in random_durations() {
        tokio::time::delay_for(duration).await;
        match closure().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                err = Some(e);
            }
        }
    }

    Err(err.unwrap())
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
