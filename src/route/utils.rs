use axum::http::{HeaderMap, StatusCode};
use minio_rsc::Minio;
use reqwest::Method;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use urlencoding::encode;
use rustls_pemfile::{certs, pkcs8_private_keys};
use std::fs::File;
use std::io::{self, BufReader};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use super::FIRMWARE_DIR;

#[derive(Serialize, Deserialize, Debug,Clone)]
pub struct ClientData {
    endpoint: String,
    public_key: String,
    private_key: String,
    alloweds_ips: String,
}
type ClientMap = HashMap<String, ClientData>;

pub async fn get_file(
    instance: Minio,
    file_name: &String,
) -> (StatusCode, HeaderMap, std::string::String) {
    let executor = instance.executor(Method::GET);
    let query = executor
  
        .bucket_name(FIRMWARE_DIR)
        .object_name(file_name.clone())
        .send_ok()
        .await;
    match query {
        Ok(res) => {
            let body = res.bytes().await;
            match body {
                Ok(bytes) => {
                    let content = String::from_utf8_lossy(&bytes).to_string();
                    let mut headers = HeaderMap::new();
                    headers.insert("Content-Type", "application/octet-stream".parse().unwrap());
                    headers.insert(
                        "Content-Disposition",
                        format!("attachment; filename=\"{}\"", file_name)
                            .parse()
                            .unwrap(),
                    );
                    (
                        StatusCode::OK,
                        headers,
                        format!("Firmware successfully downloaded: {}", content),
                    )
                }
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    HeaderMap::default(),
                    format!("Failed to read object content: {}", e),
                ),
            }
        }
        Err(e) => (StatusCode::NOT_FOUND, HeaderMap::default(), e.to_string()),
    }
}


/// Parse a JSON string to extract a client configuration given by `client_id`
///
/// Returns a string representation of the client configuration if found, otherwise an empty string.
///
/// # Errors
///
/// Returns an error string if the JSON string is not valid.
///
/// # Examples
///
/// 
pub fn parse_client_json(json_str: &str, client_id: &str) -> Result<ClientData, String> {
    let clients: ClientMap = match serde_json::from_str(json_str) {
        Ok(c) => c,
        Err(e) => return Err(format!("Erreur de format JSON : {}", e)),
    };
    match clients.get(client_id) {
        Some(client_data) => Ok(client_data.clone()), // Clone pour retourner une copie de ClientData
        None => Err(format!("Client ID '{}' non trouvé", client_id)),
    }
}

/// Create a URL encoded string from a ClientData structure
///
/// It creates a HashMap with the ClientData fields and then uses the `urlencoding` crate
/// to encode the key-value pairs. The encoded pairs are then joined with "&" to form a single string.
///
/// # Examples
///
/// let client_data = ClientData {
///     endpoint: "1.1.1.1:51820".to_string(),
///     public_key: "INSERT_PUB_KEY".to_string(),
///     private_key: "INSERT_PRV_KEY".to_string(),
///     allowed_ips: "10.200.200.200/32".to_string(),
/// };
///
/// let encoded_data = create_urlencoded_data(&client_data);
/// assert_eq!(encoded_data, "endpoint=1.1.1.1%3A51820&public_key=INSERT_PUB_KEY&private_key=INSERT_PRV_KEY&allowed_ips=10.200.200.200%2F32");
pub fn create_urlencoded_data(client_data: &ClientData) -> String {
    let mut data = HashMap::new();
    data.insert("endpoint", &client_data.endpoint);
    data.insert("public_key", &client_data.public_key);
    data.insert("private_key", &client_data.private_key);
    data.insert("allowed_ips", &client_data.alloweds_ips);
    let encoded_data: String = data
        .iter()
        .map(|(key, value)| format!("{}={}", encode(key), encode(value)))
        .collect::<Vec<String>>()
        .join("&");

    encoded_data
}

pub fn load_certs(path: &str) -> io::Result<Vec<CertificateDer<'static>>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let certs: Vec<_> = certs(&mut reader).collect();
    let certs = certs.into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid cert"))?;
    Ok(certs.into_iter()
        .map(CertificateDer::from)
        .collect())
}

pub fn load_private_key(path: &str) -> io::Result<PrivateKeyDer<'static>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let keys: Vec<_> = pkcs8_private_keys(&mut reader).collect();
    let keys = keys.into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid key"))?;
    let keys = keys.into_iter()
        .map(PrivateKeyDer::from)
        .collect::<Vec<_>>();
    keys.into_iter()
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no private key found"))
}