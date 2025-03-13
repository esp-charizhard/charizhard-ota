use std::fs::File;

use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
};
use axum::{
    extract::{Path as AxumPath, State},
    response::IntoResponse,
    Json,
};
use minio_rsc::{client::ListObjectsArgs, Minio};
use crate::route::utils::get_file;
use regex::Regex;
use reqwest::Method;
use serde::Serialize;
use utils::{parse_client_json, create_urlencoded_data};
mod utils;

const FIRMWARE_DIR: &str = "bin";

#[derive(Serialize)]
struct Manifest {
    version: String,
    error: String,
}

// basic handler that responds with a static string
#[allow(dead_code)]
pub async fn root() -> &'static str {
    "Welcome to Charizhard OTA ! Check /latest to get latest firmware"
}

pub async fn handle_manifest(State(instance): State<Minio>) -> impl IntoResponse {
    let args = ListObjectsArgs::default();
    let query = instance.list_objects("bin", args).await;
    let re = Regex::new(r"charizhard\.V(\d+\.\d+)\.bin").unwrap();

    match query {
        Ok(res) => {
            let mut version_files: Vec<String> = res
                .contents
                .iter()
                .filter_map(|object| {
                    re.captures(&object.key)
                        .and_then(|caps| caps.get(1).map(|version| version.as_str().to_string()))
                })
                .collect();

            version_files.sort();
            let latest_version = match version_files.last() {
                Some(vers) => vers,
                None => {
                    return (
                        StatusCode::NO_CONTENT,
                        Json(Manifest {
                            version: "".to_string(),
                            error: "No firmware files found".to_string(),
                        }),
                    )
                }
            };
            (
                StatusCode::OK,
                Json(Manifest {
                    version: latest_version.to_string(),
                    error: "Found".to_string(),
                }),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(Manifest {
                version: "".to_string(),
                error: format!("Error querying bucket {}", e),
            }),
        ),
    }
}

pub async fn latest_firmware(
    State(instance): State<Minio>,
) -> (StatusCode, HeaderMap, std::string::String) {
    let args = ListObjectsArgs::default();
    let query = instance.list_objects("bin", args).await;
    let re = match Regex::new(r"^(charizhard\.V\d+\.\d+\.bin)$") {
        Ok(re) => re,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                HeaderMap::default(),
                format!("Error compiling regex: {}", e),
            )
        }
    };

    match query {
        Ok(res) => {
            let mut firmware_files: Vec<String> = res
                .contents
                .iter()
                .filter_map(|object| re.captures(&object.key).map(|caps| caps[1].to_string()))
                .collect();

            eprintln!("{:?}", firmware_files);
            firmware_files.sort();

            if let Some(latest_firmware) = firmware_files.last() {
                return get_file(instance, latest_firmware).await;
            } else {
                (
                    StatusCode::NOT_FOUND,
                    HeaderMap::default(),
                    "No firmware files found.".to_string(),
                )
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            HeaderMap::default(),
            format!("Error querying bucket: {}", e),
        ),
    }
}

#[allow(dead_code)]
pub async fn specific_firmware(
    AxumPath(file_name): AxumPath<String>,
    State(instance): State<Minio>,
) -> (StatusCode, HeaderMap, std::string::String) {
    return get_file(instance, &file_name).await;
}

// //curl -X POST http://localhost:8080/firmware/charizhard.V1.3.bin \
// //  -T ./firmware.bin \
// //  -H "Authorization: Bearer $JWT_TOKEN"
pub async fn post_firmware(
    AxumPath(file_name): AxumPath<String>,
    State(instance): State<Minio>,
    request: Request,
) -> (StatusCode, std::string::String) {
    let executor = instance.executor(Method::PUT);
    let body = request.into_body();
    let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap(); //cant fail, usize::max is never reached
    let query = executor
        .bucket_name(FIRMWARE_DIR)
        .object_name(file_name)
        .body(bytes)
        .send_ok()
        .await;
    match query {
        Ok(_) => (
            StatusCode::OK,
            "Firmware successfully uploaded !".to_string(),
        ),
        Err(e) => {
            eprintln!("Upload error: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Error uploading firmware {}", e),
            )
        }
    }
}

//curl -X DELETE http://localhost:8080/firmware/charizhard.V1.3.bin \
//  -T ./firmware.bin \
//  -H "Authorization: Bearer $JWT_TOKEN"
pub async fn delete_firmware(
    AxumPath(file_name): AxumPath<String>,
    State(instance): State<Minio>,
) -> (StatusCode, std::string::String) {
    let executor = instance.executor(Method::DELETE);
    let query = executor
        .bucket_name(FIRMWARE_DIR)
        .object_name(file_name)
        .send_ok()
        .await;
    match query {
        Ok(_) => (
            StatusCode::OK,
            "Firmware successfully deleted !".to_string(),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Error deleting firmware {}", e),
        ),
    }
}

pub async fn fallback() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "Not Found")
}





/// Handles the configuration retrieval for a specific client.
/// 
/// This function processes an incoming HTTP request to obtain a WireGuard configuration
/// for a specific client identified by the "id_client_x" header. It interacts with a Minio
/// storage instance to list and retrieve configuration files from the "config-wg" bucket.
/// 
/// # Arguments
/// 
/// * `State(instance)`: A Minio storage instance used for listing and retrieving objects.
/// * `req`: The incoming HTTP request containing headers used to identify the client.
/// 
/// # Returns
/// 
/// An HTTP response indicating the success or failure of the operation. It returns:
/// - `StatusCode::OK` with the configuration if retrieval is successful.
/// - `StatusCode::BAD_REQUEST` if the "id_client_x" header is missing or invalid.
/// - `StatusCode::SERVICE_UNAVAILABLE` if the configuration cannot be sent.
/// - `StatusCode::INTERNAL_SERVER_ERROR` if there is an error querying the bucket or retrieving file content.

#[axum::debug_handler]
#[allow(unused_assignments)]
pub async fn config_wg(State(instance): State<Minio>,req: Request) -> impl IntoResponse {
    let headers = req.headers();
    let id_client_x_value = match headers.get("id_client_x") {
        Some(value) => match value.to_str() {
            Ok(v) => v.to_string(),
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    "Invalid 'id_client_x' header".to_string(),
                );
            }
        },
        None => {
            return(
                StatusCode::BAD_REQUEST,
                "Missing 'id_client_x' header".to_string(),
            );
        }
    };

    let args = ListObjectsArgs::default();
    let query = instance.list_objects("config-wg", args).await;


    match query {
        Ok(res) => {
            let file_list: Vec<String> = res
                .contents
                .iter()
                .map(|object| object.key.clone()) 
                .collect();
            let file = instance.get_object("config-wg",&file_list[0]).await;
            match file {
                Ok(response) => {
                    let contents = response.text().await.unwrap();
                    match parse_client_json(&contents, &id_client_x_value) {
                        Ok(client_data) => {
                            println!("Config trouvée : {:?}", client_data);
                            let encoded_data = create_urlencoded_data(&client_data);
                            println!("Encoded: {}", encoded_data);
                            return (StatusCode::OK, encoded_data);
                        }
                        Err(e) => {
                            println!("Erreur : {}", e);
                    
                            // Retourne une erreur avec le statut SERVICE_UNAVAILABLE
                            return (
                                StatusCode::SERVICE_UNAVAILABLE,
                                "Cannot send you the config".to_string(),
                            );
                        }
                    }
                    
                }
                Err(e) => {
                    println!("Erreur lors de la récupération du contenu du fichier : {}", e);
                    return(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Cannot send you the config".to_string(),
                    )
                }
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Error querying bucket {}", e)
        ),
    }
}


pub async fn configure_tls() -> () {
    println!("Test configure mTLS");
    let cert_file = File::open("temp_certif/server.crt");
    let key_file = File::open("temp_certif/server.key");
    // let certs = pemfile::certs(&mut BufReader::new(cert_file))?;
    // let key = pemfile::pkcs8_private_keys(&mut BufReader::new(key_file))?.remove(0);
    // let mut root_store = RootCertStore::empty();
    // let ca_file = File::open("temp_certif/ca.crt")?;
    // let mut ca_reader = BufReader::new(ca_file);
    // root_store.add_pem_file(&mut ca_reader)?;
    // let mut config = ServerConfig::new(NoClientAuth::new()); 
    // config.set_single_cert(certs, key)?;
    // config.set_client_cert_verifier(rustls::server::ClientCertVerifier::from(root_store));
    ()
}