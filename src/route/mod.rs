use std::{
    env::{self, VarError},
    usize,
};

//todo clean the the useless unwraps by using anyhow-error response
use async_std::{
    fs::{self, File},
    path::Path,
    stream::{self, StreamExt},
};
use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
};
use axum::{
    extract::{Path as AxumPath, State},
    http::version,
    response::IntoResponse,
    Extension, Json,
};
use axum_keycloak_auth::decode::KeycloakToken;
use minio_rsc::{client::ListObjectsArgs, Minio};
use reqwest::{get, Method, Response};
use serde::Serialize;
use utils::get_file;
mod utils;

const FIRMWARE_DIR: &str = "bin";

#[derive(Serialize)]
struct Manifest {
    version: String,
    error: String,
}

// basic handler that responds with a static string
pub async fn root() -> &'static str {
    "Welcome to Charizhard OTA ! Check /latest to get latest firmware"
}

pub async fn handle_manifest(State(instance): State<Minio>) -> impl IntoResponse {
    let args = ListObjectsArgs::default();
    let query = instance.list_objects("bin", args).await;
    match query {
        Ok(res) => {
            let mut version_files: Vec<String> = res
                .contents
                .iter()
                .filter_map(|object| {
                    if object.key.starts_with("charizhard") || object.key.ends_with(".bin") {
                        // on transforme charizhard.Vx.x.bin en x.x
                        Some(object.key.clone().split_off(12).split_off(3))
                    } else {
                        None
                    }
                })
                .collect();

            version_files.sort_by(|a, b| a.cmp(b));
            let latest_version = match version_files.last() {
                Some(vers) => vers,
                None => {
                    return (
                        StatusCode::NO_CONTENT,
                        Json(Manifest {
                            version: "".to_string(),
                            error: format!("No firmware files found"),
                        }),
                    )
                }
            };
            return (
                StatusCode::OK,
                Json(Manifest {
                    version: latest_version.to_string(),
                    error: "Found".to_string(),
                }),
            );
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(Manifest {
                    version: "".to_string(),
                    error: format!("Error querying bucket {}", e),
                }),
            )
        }
    }
}

pub async fn latest_firmware(
    State(instance): State<Minio>,
) -> (StatusCode, HeaderMap, std::string::String) {
    let args = ListObjectsArgs::default();
    let query = instance.list_objects("bin", args).await;
    match query {
        Ok(res) => {
            let mut firmware_files: Vec<String> = res
                .contents
                .iter()
                .filter_map(|object| {
                    if object.key.starts_with("charizhard") || object.key.ends_with(".bin") {
                        Some(object.key.clone())
                    } else {
                        None
                    }
                })
                .collect();

            firmware_files.sort_by(|a, b| a.cmp(b));

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
pub async fn specific_firmware(
    AxumPath(file_name): AxumPath<String>,
    State(instance): State<Minio>,
) -> (StatusCode, HeaderMap, std::string::String) {
    return get_file(instance, &file_name).await;
}

//curl -X POST http://localhost:8080/firmware/charizhard.V1.3.bin \
//  -T ./firmware.bin \
//  -H "Authorization: Bearer $JWT_TOKEN"
pub async fn post_firmware(
    AxumPath(file_name): AxumPath<String>,
    Extension(token): Extension<KeycloakToken<String>>,
    State(instance): State<Minio>,
    request: Request,
) -> (StatusCode, std::string::String) {
    let executor = instance.executor(Method::POST);
    let body = request.into_body();
    let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    let query = executor
        .bucket_name(FIRMWARE_DIR)
        .object_name(file_name)
        .body(bytes)
        .send_ok()
        .await;
    match query {
        Ok(_) => return (StatusCode::OK, format!("Firmware successfully uploaded !")),
        Err(e) => {
            return (
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
        Ok(_) => return (StatusCode::OK, format!("Firmware successfully deleted !")),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Error deleting firmware {}", e),
            )
        }
    }
}

pub async fn fallback() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "Not Found")
}
