use anyhow::Error;
use axum::{
    routing::{get, post},
    Router,
};
use axum_keycloak_auth::{
    instance::{KeycloakAuthInstance, KeycloakConfig},
    layer::KeycloakAuthLayer,
    PassthroughMode,
};
use charizhard_ota::route::{configure_server_tls, root, specific_firmware};
use minio_rsc::{provider::StaticProvider, Minio};
use reqwest::Url;
use route::{delete_firmware, fallback, handle_manifest, latest_firmware, post_firmware,config_wg};
use std::{net::SocketAddr, result::Result::Ok, sync::Arc};
use axum_server::tls_rustls::RustlsConfig;
mod route;
#[derive(Clone)]
pub struct MinioInstance {
    minio: Minio,
}

impl MinioInstance {
    pub fn new() -> Result<MinioInstance, anyhow::Error> {
        // setup database config
        println!("Minio: Attempting to load configuration from environment");
        // let access_key = env::var("MINIO_SECRET_KEY").unwrap_or_else(|_| "Not Set".to_string());
        // let secret_key = env::var("MINIO_ACCESS_KEY").unwrap_or_else(|_| "Not Set".to_string());
        // println!("Debug - MINIO_ACCESS_KEY: {}", access_key);
        // println!("Debug - MINIO_SECRET_KEY: {}", secret_key);
        let provider = match StaticProvider::from_env() {
            Some(provider) => provider,
            None => return Err(Error::msg("Env varibles not found")),
        };

        println!("Minio: Successfully loaded configuration from environment");
        //let ip_addr = env::var("IP_MINIO")?;
        //let port = env::var("PORT_MINIO")?;
        //let endpoint = ip_addr + ":" + &port;

        println!("Minio: Attempting to connect to database at localhost:9000");
        let minio = Minio::builder()
            .endpoint("localhost:9000") //where to look for database
            .provider(provider)
            .secure(false)
            .build()?;
        println!("Minio: Successfully connected to database at localhost:9000");
        Ok(MinioInstance { minio })
    }

    pub fn get_minio(self) -> Minio {
        self.minio
    }
}


pub fn public_router(instance: MinioInstance) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/latest", get(latest_firmware))
        .route("/firmware/{file_name}", get(specific_firmware))
        .route("/manifest", get(handle_manifest))
        .route("/configwg", get(config_wg))//TODO : mettre dans le eprotected_router pour tester
        .with_state(instance.get_minio())
}
pub fn mtls_router(instance: MinioInstance) -> Router {
    Router::new()
        .route("/configwg", get(config_wg))//TODO : mettre dans le eprotected_router pour tester
        .with_state(instance.get_minio())
}

pub fn protected_router(instance: KeycloakAuthInstance, minstance: MinioInstance) -> Router {
    Router::new()
        .route(
            "/firmware/{file_name}",
            post(post_firmware).delete(delete_firmware),
        )
        .layer(
            KeycloakAuthLayer::<String>::builder()
                .instance(instance)
                .passthrough_mode(PassthroughMode::Block)
                .persist_raw_claims(false)
                .expected_audiences(vec![String::from("account")])
                .required_roles(vec![String::from("admin")])
                .build()
                
        )
        .with_state(minstance.get_minio())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // initialize tracing
    tracing_subscriber::fmt::init();
    let tls_config2 = configure_server_tls("temp_certif/server.crt","temp_certif/server.key","temp_certif/ca.crt");
    let tls_config3 = RustlsConfig::from_config(Arc::clone(&tls_config2));
    //let ip_kc = env::var("IP_KC")?;
    //let port_kc = env::var("PORT_KC")?;
    //let url_kc = ip_kc + ":" + &port_kc;

    let keycloak_auth_instance = KeycloakAuthInstance::new(
        KeycloakConfig::builder()
            // a modifier évidemment au deployement
            .server(Url::parse("http://localhost:8080").unwrap())
            .realm(String::from("charizhard-ota"))
            .build(),
    );

    let minstance = MinioInstance::new()?;
    let router = public_router(minstance.clone())
    // let router = public_router();
        .merge(protected_router(keycloak_auth_instance, minstance.clone()))
        .fallback(fallback);
    let https_app = mtls_router(minstance.clone())
        .fallback(fallback);
    // 0.0.0.0 signifie qu'on écoute sur toutes les nci
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8082").await?;
    tokio::spawn(async move {
        println!("HTTP listening on 127.0.0.1:8082");
        axum::serve(listener, router.into_make_service())
            .await
            .unwrap();
    });

    let listener2 = SocketAddr::from(([127, 0, 0, 1], 8083));
    println!("https listening on {}", listener2);
    axum_server::bind_rustls(listener2, tls_config3)
    .serve(https_app.into_make_service())
    .await?;

    Ok(())
}
