use crate::utils::exponential_retry;

use std::{
    default::Default,
    env,
    io::{Cursor, Read},
    sync::{Arc, Mutex},
};

use failure::format_err;
use google_drive3::DriveHub;
use hyper::{net::HttpsConnector, Client};
use hyper_rustls::TlsClient;
use lazy_static::lazy_static;
use yup_oauth2::*;

// shorthands for complex types
type MyHub = DriveHub<Client, yup_oauth2::ServiceAccountAccess<Client>>;
type MyArcHub = Arc<Mutex<MyHub>>;

lazy_static! {
    // a Google Drive hub service worker object
    static ref HUB: MyArcHub = create_hub();
    // the ID of a chaindump folder
    static ref PARENT: String = get_or_create_folder();
}

// creates a Google Drive hub
fn create_hub() -> MyArcHub {
    let secret = service_account_key_from_file(&String::from("./credentials.json"))
        .expect("File not found: credentials.json");

    let auth = ServiceAccountAccess::new(
        secret,
        Client::with_connector(HttpsConnector::new(TlsClient::new())),
    );

    let hub = DriveHub::new(
        Client::with_connector(HttpsConnector::new(TlsClient::new())),
        auth,
    );

    Arc::new(Mutex::new(hub))
}

// returns contents of a specified Google Drive folder
fn list_folder_contents(hub: &MyHub, parent_id: &str) -> Result<google_drive3::FileList, String> {
    let req = exponential_retry(|| {
        let (_, res) = hub
            .files()
            .list()
            .q(&format!("'{}' in parents and trashed = false", parent_id))
            .doit()
            .map_err(|e| format_err!("{}", e))?;

        Ok(res)
    });

    match req {
        Ok(elem) => Ok(elem),
        Err(e) => Err(format!("list_folder_contents failed: {}", e)),
    }
}

// returns Google Drive folder ID from the name of a directory
fn get_id_by_name(hub: &MyHub, name: &str, parent_id: &str) -> Result<Option<String>, String> {
    match list_folder_contents(&hub, parent_id) {
        Err(e) => Err(e),
        Ok(contents) => match contents.files {
            None => Err("list_folder_contents.files is None".to_string()),
            Some(file_v) => {
                let temp = file_v
                    .iter()
                    .filter(|file| file.name.clone().unwrap_or(String::new()) == name)
                    .nth(0);

                match temp {
                    Some(headers) => Ok(headers.id.clone()),
                    None => Ok(None),
                }
            }
        },
    }
}

// replaces contents of a specified Google Drive file
fn replace_file_by_id(hub: &MyHub, bytes: &[u8], id: &str) -> Option<String> {
    let req = exponential_retry(|| {
        let res = hub
            .files()
            .update(google_drive3::File::default(), id)
            .upload_resumable(
                Cursor::new(bytes),
                "application/octet-stream".parse().unwrap(),
            )
            .map_err(|e| format_err!("{}", e))?;

        Ok(res)
    });

    match req {
        Ok(_) => None,
        Err(e) => Some(format!("replace_file_by_id failed: {}", e)),
    }
}

// uploads a file to Google Drive
fn upload_file(hub: &MyHub, bytes: &[u8], name: &str) -> Option<String> {
    let mut req = google_drive3::File::default();
    req.name = Some(name.to_string());
    req.parents = Some(vec![PARENT.to_string()]);

    let req = exponential_retry(|| {
        let res = hub
            .files()
            .create(req.clone())
            .upload_resumable(
                Cursor::new(bytes),
                "application/octet-stream".parse().unwrap(),
            )
            .map_err(|e| format_err!("{}", e))?;

        Ok(res)
    });

    match req {
        Ok(_) => None,
        Err(e) => Some(format!("upload_file failed: {}", e)),
    }
}

// returns Google Drive folder ID of a chaindump directory
fn get_or_create_folder() -> String {
    let hub_arc = HUB.clone();
    let hub = hub_arc.lock().unwrap();

    let chaindump_dir = env::var("CHAINDUMP_DIR").expect("CHAINDUMP_DIR not set");

    let req = exponential_retry(|| {
        let (_, res) = hub
            .files()
            .list()
            .q("trashed = false")
            .doit()
            .map_err(|e| format_err!("{}", e))?;

        Ok(res)
    });

    let id = match req {
        Ok(elem) => elem,
        Err(e) => panic!("Failed to search for folders: {}", e),
    };

    match id.files {
        None => panic!("No files found while searching for chaindump"),
        Some(file_v) => {
            let temp = file_v
                .iter()
                .filter(|file| file.name.clone().unwrap_or(String::new()) == chaindump_dir)
                .nth(0);

            match temp {
                Some(headers) => headers.id.clone().unwrap(),
                None => panic!("Chaindump folder not found"),
            }
        }
    }
}

// initializes lazy_static fields
pub fn initialize() {
    lazy_static::initialize(&HUB);
    lazy_static::initialize(&PARENT);
}

// replaces contents of a specified Google Drive file
// creates a new file if one does not exist
pub fn update_or_create_file(bytes: &[u8], name: &str) -> Option<String> {
    let hub_arc = HUB.clone();
    let hub = hub_arc.lock().unwrap();

    if let Ok(Some(id)) = get_id_by_name(&hub, name, &PARENT) {
        replace_file_by_id(&hub, bytes, &id)
    } else {
        upload_file(&hub, bytes, name)
    }
}

// downloads a specified Google Drive file
pub fn download_file(name: &str) -> Result<Option<Vec<u8>>, String> {
    let hub_arc = HUB.clone();
    let hub = hub_arc.lock().unwrap();

    match get_id_by_name(&hub, name, &PARENT) {
        Err(e) => Err(e),
        Ok(value) => match value {
            // no file found
            None => Ok(None),
            // some file found
            Some(file_id) => {
                let req = exponential_retry(|| {
                    let (res, _) = hub
                        .files()
                        .get(&file_id)
                        .add_scope(google_drive3::Scope::Full)
                        .param("alt", "media")
                        .doit()
                        .map_err(|e| format_err!("{}", e))?;

                    Ok(res)
                });

                match req {
                    Ok(mut response) => {
                        let mut content: Vec<u8> = Vec::new();
                        response
                            .read_to_end(&mut content)
                            .expect("Failed to write to Vec<u8>");
                        Ok(Some(content))
                    }
                    Err(e) => Err(format!("Failed to download file: {}", e)),
                }
            }
        },
    }
}
