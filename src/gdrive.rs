use std::env;
use std::default::Default;
use std::io::{Cursor, Read};
use std::sync::{Arc, Mutex};

use google_drive3::DriveHub;
use hyper::{Client, net::HttpsConnector};
use hyper_rustls::TlsClient;
use lazy_static::lazy_static;
use yup_oauth2::*;


type MyHub = DriveHub<Client, yup_oauth2::ServiceAccountAccess<Client>>;
type MyArcHub = Arc<Mutex<MyHub>>;

lazy_static! {
    static ref HUB: MyArcHub = create_hub();
    static ref PARENT: String = get_or_create_folder();
}

fn create_hub () -> MyArcHub {
    let secret = service_account_key_from_file(&String::from("./credentials.json"))
        .expect("File not found: credentials.json");

    let auth = ServiceAccountAccess::new(
        secret,
        Client::with_connector(HttpsConnector::new(TlsClient::new()))
    );

    let hub = DriveHub::new(Client::with_connector(HttpsConnector::new(TlsClient::new())), auth);
    Arc::new(Mutex::new(hub))
}


fn list_folder_contents (hub: &MyHub, parent_id: &str) -> google_drive3::FileList {
    let (_, res) = hub.files()
        .list()
        .q(&format!("'{}' in parents and trashed = false", parent_id))
        .doit()
        .expect("Failed to list files");

    res
}


fn get_id_by_name (hub: &MyHub, name: &str, parent_id: &str) -> Option<String> {
    match list_folder_contents(&hub, parent_id).files {
        None => None,
        Some(file_v) => {
            let temp = file_v
                .iter()
                .filter(|file| file.name.clone().unwrap_or(String::new()) == name)
                .nth(0);

            match temp {
                Some(headers) => headers.id.clone(),
                None => None
            }
        }
    }
}


fn delete_file_by_id (hub: &MyHub, id: &str) {
    hub.files().delete(id).doit().expect("Failed to delete file");
}


fn upload_file (hub: &MyHub, bytes: &[u8], name: &str) {
    let mut req = google_drive3::File::default();
    req.name = Some(name.to_string());
    req.parents = Some(vec![PARENT.to_string()]);

    hub.files()
        .create(req)
        .upload_resumable(
            Cursor::new(bytes),
            "application/octet-stream".parse().unwrap()
        ).expect("Failed to upload file");
}


fn get_or_create_folder () -> String {
    let hub_arc = HUB.clone();
    let hub = hub_arc.lock().unwrap();

    let chaindump_dir = env::var("CHAINDUMP_DIR").expect("CHAINDUMP_DIR not set");

    let (_, id) = hub.files()
        .list()
        .q("trashed = false")
        .doit()
        .expect("Failed to list files");

    match id.files {
        None => panic!("No files found while searching for chaindump"),
        Some(file_v) => {
            let temp = file_v
                .iter()
                .filter(|file| file.name.clone().unwrap_or(String::new()) == chaindump_dir)
                .nth(0);

            match temp {
                Some(headers) => headers.id.clone().unwrap(),
                None => panic!("Chaindump folder not found")
            }
        }
    }
}


pub fn initialize () {
    lazy_static::initialize(&HUB);
    lazy_static::initialize(&PARENT);
}


pub fn replace_file (bytes: &[u8], name: &str) {
    let hub_arc = HUB.clone();
    let hub = hub_arc.lock().unwrap();

    if let Some(id) = get_id_by_name(&hub, name, &PARENT) {
        delete_file_by_id(&hub, &id);
    };
    upload_file(&hub, bytes, name);
}


pub fn delete_file (name: &str) {
    let hub_arc = HUB.clone();
    let hub = hub_arc.lock().unwrap();

    if let Some(id) = get_id_by_name(&hub, name, &PARENT) {
        delete_file_by_id(&hub, &id);
    };
}


pub fn download_file (name: &str) -> Option<Vec<u8>> {
    let hub_arc = HUB.clone();
    let hub = hub_arc.lock().unwrap();

    match get_id_by_name(&hub, name, &PARENT) {
        Some(file_id) => {
            let (mut response, _) = hub.files().get(&file_id)
                .add_scope(google_drive3::Scope::Full)
                .param("alt", "media")
                .doit()
                .expect("Failed to retrieve file");

            let mut content: Vec<u8> = Vec::new();
            response.read_to_end(&mut content).expect("Failed to write to Vec<u8>");
            Some(content)
        },
        None => None
    }
}
