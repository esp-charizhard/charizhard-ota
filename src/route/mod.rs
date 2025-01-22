use std::env::{self, VarError};

//todo clean the the useless unwraps by using anyhow-error response
use async_std::{
    fs::{self, File},
    path::Path,
    stream::{self, StreamExt},
};
use axum::{
    extract::Path as AxumPath,
    http::version,
    response::{IntoResponse, IntoResponseParts},
    Extension, Json,
};
use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
};
use axum_keycloak_auth::decode::KeycloakToken;
use minio_rsc::{provider::StaticProvider, Minio};
use openssl::sign;
use reqwest::{Method, Response};
use serde::Serialize;
use utils::{get_file, stream_to_file};
mod utils;

const FIRMWARE_DIR: &str = "./bin";

#[derive(Serialize)]
struct Manifest {
    version: String,
    error: String,
}

// basic handler that responds with a static string
pub async fn root() -> &'static str {
    "Welcome to Charizhard OTA ! Check /latest/ to get latest firmware"
}

pub async fn handle_manifest() -> impl IntoResponse {
    let entries = match fs::read_dir(FIRMWARE_DIR).await {
        Ok(entries) => entries,
        Err(e) => {
            println!("{}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                HeaderMap::default(),
                Json(Manifest {
                    version: "".to_string(),
                    error: "Failed to read firmware directory".to_string(),
                }),
            );
        }
    };

    let mut firmware_files = Vec::new();
    tokio::pin!(entries); // Pin the stream for iteration
    while let Some(entry_result) = entries.next().await {
        match entry_result {
            Ok(entry) => {
                if let Ok(mut file_name) = entry.file_name().into_string() {
                    if file_name.starts_with("charizhard.V") && file_name.ends_with(".bin") {
                        let mut version_firm = file_name.split_off(12);
                        let _ = version_firm.split_off(3);
                        firmware_files.push(version_firm);
                    }
                }
            }
            Err(err) => {
                println!("{}", err);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    HeaderMap::default(),
                    Json(Manifest {
                        version: "".to_string(),
                        error: "Error reading directory".to_string(),
                    }),
                );
            }
        }
    }

    firmware_files.sort_by(|a, b| a.cmp(b));
    let version = match firmware_files.last() {
        Some(version) => version.to_string(),
        None => {
            return (
                StatusCode::NO_CONTENT,
                HeaderMap::default(),
                Json(Manifest {
                    version: "".to_string(),
                    error: "No firmware files found".to_string(),
                }),
            );
        }
    };

    return (
        StatusCode::OK,
        HeaderMap::default(),
        Json(Manifest {
            version,
            error: "Found".to_string(),
        }),
    );
}

pub async fn latest_firmware() -> (StatusCode, HeaderMap, std::string::String) {
    let entries = match fs::read_dir(FIRMWARE_DIR).await {
        Ok(entries) => entries,
        Err(e) => {
            println!("{}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                HeaderMap::default(),
                "Failed to read firmware directory".to_string(),
            );
        }
    };

    let mut firmware_files = Vec::new();
    tokio::pin!(entries); // Pin the stream for iteration
    while let Some(entry_result) = entries.next().await {
        match entry_result {
            Ok(entry) => {
                if let Ok(file_name) = entry.file_name().into_string() {
                    if file_name.starts_with("charizhard.V") && file_name.ends_with(".bin") {
                        firmware_files.push(file_name);
                    }
                }
            }
            Err(err) => {
                println!("{}", err);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    HeaderMap::default(),
                    "Error reading directory entry".to_string(),
                );
            }
        }
    }

    firmware_files.sort_by(|a, b| a.cmp(b));

    if let Some(latest_firmware) = firmware_files.last() {
        let file_path = Path::new(FIRMWARE_DIR).join(latest_firmware);

        let file = match tokio::fs::File::open(&file_path).await {
            Ok(file) => file,
            Err(e) => {
                println!("{}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    HeaderMap::default(),
                    "Failed to open firmware file".to_string(),
                );
            }
        };
        return get_file(file, latest_firmware).await;
    }

    // If no firmware files are found
    (
        StatusCode::NOT_FOUND,
        HeaderMap::default(),
        "No firmware files found".to_string(),
    )
}
pub async fn specific_firmware(
    AxumPath(file_name): AxumPath<String>,
) -> (StatusCode, HeaderMap, std::string::String) {

    let access_key = match env::var("MINIO_ACCESS_KEY") {
        Ok(key) => key,
        Err(_) => VarError::NotPresent.to_string()
    };

    let signature = match env::var("MINIO_SIGNATURE") {
        Ok(signature) => signature,
        Err(_) => VarError::NotPresent.to_string()
    };
    
     // setup database config
     let provider = StaticProvider::new(access_key, signature, None);
     let minio = Minio::builder()
         .endpoint("10.10.35.70:9000") //where to look for database
         .provider(provider)
         .secure(false)
         .build()
         .unwrap();
    
    let executor = minio.executor(Method::GET);
    let query = executor
        .bucket_name("bin")
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
                    headers.insert("Content-Disposition", format!("attachment; filename=\"{}\"", file_name).parse().unwrap());

                    return (
                        StatusCode::OK,
                        headers,
                        format!("Firmware successfully downloaded: {}", content),
                    )
                }
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        HeaderMap::default(),
                        format!("Failed to read object content: {}", e),
                    )
                }
            }
        }
        Err(e) => {
            return (
                StatusCode::NOT_FOUND,
                HeaderMap::default(),
                e.to_string(),
            )
        }
    }
   
}

//curl -X POST http://localhost:8080/firmware/charizhard.V1.3.bin \
//  -T ./firmware.bin \
//  -H "Authorization: Bearer $JWT_TOKEN"
pub async fn post_firmware(
    AxumPath(file_name): AxumPath<String>,
    Extension(token): Extension<KeycloakToken<String>>,
    request: Request,
) -> Result<(), (StatusCode, std::string::String)> {
    stream_to_file(&file_name, request.into_body().into_data_stream()).await
}

//curl -X DELETE http://localhost:8080/firmware/charizhard.V1.3.bin \
//  -T ./firmware.bin \
//  -H "Authorization: Bearer $JWT_TOKEN"
pub async fn delete_firmware(
    AxumPath(file_name): AxumPath<String>,
) -> (StatusCode, HeaderMap, std::string::String) {
    let result = tokio::fs::remove_file(Path::new(FIRMWARE_DIR).join(file_name)).await;

    match result {
        Ok(_) => (
            StatusCode::OK,
            HeaderMap::default(),
            "Firmware successfully deleted !".to_string(),
        ),
        Err(err) => {
            println!("{}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                HeaderMap::default(),
                "Skill issue".to_string(),
            );
        }
    }
}
