use axum::http::{HeaderMap, StatusCode};
use minio_rsc::Minio;
use reqwest::Method;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

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


pub fn parse_client_json(json_str: &str, client_id: &str) -> String {
    let clients: ClientMap = match serde_json::from_str(json_str) {
        Ok(c) => c,
        Err(_) => return "Erreur de format JSON".to_string(), 
    };
    if let Some(client_data) = clients.get(client_id) {
        format!("{:?}", client_data)
    }else{
        "".to_string()
    }
}
