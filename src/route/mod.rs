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
use utils::parse_client_json;
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


/// Handles the WireGuard configuration request by retrieving the configuration
/// file from the "config-wg" bucket in Minio and parsing it based on the
/// `id_client_x` header from the request. Returns the parsed configuration or
/// an error message if the operation fails.
/// 
/// # Arguments
/// 
/// * `State(instance)`: Minio instance used to interact with the object storage.
/// * `req`: The HTTP request containing headers with client information.
/// 
/// # Returns
/// 
/// An HTTP response indicating the status of the operation:
/// * `StatusCode::OK` with "LOADING CONFIG" if the configuration is successfully loaded.
/// * `StatusCode::NO_CONTENT` if no files are found in the bucket.
/// * `StatusCode::INTERNAL_SERVER_ERROR` with an error message if an error occurs.
/// 
/// # Errors
/// 
/// Returns an error if:
/// * The `id_client_x` header is missing or invalid.
/// * There is an error querying the bucket or retrieving the file.
/// * The configuration cannot be parsed successfully.

#[axum::debug_handler]
#[allow(unused_assignments)]
pub async fn config_wg(State(instance): State<Minio>,req: Request) -> impl IntoResponse {
    // println!("hello endpoint reached");
    let args = ListObjectsArgs::default();
    let query = instance.list_objects("config-wg", args).await;
    let headers = req.headers();
    // println!("Headers: {:#?}", headers.get("id_client_x"));
    let mut id_client_x_value = "";
    let mut config = None;
    if let Some(id_client_x) = headers.get("id_client_x") {
        id_client_x_value = id_client_x.to_str().unwrap();
        // println!("{:?}",id_client_x_value);
    } else {
        println!("Bad header");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Bad header".to_string(),
        )
    }
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
                    // println!("Contenu du fichier : {}", contents);
                    config = parse_client_json(&contents, &id_client_x_value);
                    println!("Config : {:?}", config);
                }
                Err(e) => {
                    println!("Erreur lors de la récupération du contenu du fichier : {}", e);
                }
            }
            // for file in &file_list {
            //     println!("Fichier trouvé : {}", file);
            // }

            if file_list.is_empty() {
                return (
                    StatusCode::NO_CONTENT,
                    "No Files Found in Bucket".to_string(),
                );
            }

            (
                StatusCode::OK,
                "LOADING CONFIG".to_string(),//TODO à modif pour renvoyer la config une fois le tls/mtls ok
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Error deleting firmware {}", e)
        ),
    }
}

