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
use charizhard_ota::route::{root, specific_firmware};
use minio_rsc::{provider::StaticProvider, Minio};
use reqwest::Url;
use route::{delete_firmware, fallback, handle_manifest, latest_firmware, post_firmware};
use std::{env, result::Result::Ok};
mod route;

#[derive(Clone)]
pub struct MinioInstance {
    minio: Minio,
}

impl MinioInstance {
    pub fn new() -> Result<MinioInstance, anyhow::Error> {
        // setup database config
        let provider = match StaticProvider::from_env() {
            Some(provider) => provider,
            None => return Err(Error::msg("Env varibles not found")),
        };

        //let ip_addr = env::var("IP_MINIO")?;
        //let port = env::var("PORT_MINIO")?;
        //let endpoint = ip_addr + ":" + &port;

        let minio = Minio::builder()
            .endpoint("minio-service.minio-tenant.svc.cluster.local:9000") //where to look for database
            .provider(provider)
            .secure(false)
            .build()?;
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
                .required_roles(vec![String::from("Admin")])
                .build(),
        )
        .with_state(minstance.get_minio())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // initialize tracing
    tracing_subscriber::fmt::init();

    //let ip_kc = env::var("IP_KC")?;
    //let port_kc = env::var("PORT_KC")?;
    //let url_kc = ip_kc + ":" + &port_kc;

    let keycloak_auth_instance = KeycloakAuthInstance::new(
        KeycloakConfig::builder()
            // a modifier évidemment au deployement
            .server(Url::parse("http://keycloak-service.keycloak.svc.cluster.local:8080").unwrap())
            .realm(String::from("charizhard-ota"))
            .build(),
    );

    let minstance = MinioInstance::new()?;
    let router = public_router(minstance.clone())
        .merge(protected_router(keycloak_auth_instance, minstance.clone()))
        .fallback(fallback);

    // 0.0.0.0 signifie qu'on écoute sur toutes les nci
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8081").await?;
    axum::serve(listener, router.into_make_service()).await?;
    Ok(())
}
