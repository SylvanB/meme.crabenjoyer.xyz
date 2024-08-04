mod crypto;
mod objects;

use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{HeaderMap, Response, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::{Duration, Utc};
use crypto::{get_truncated_sha256, HashOutputSize};
use futures::stream::StreamExt;
use minijinja::render;
use object_store::{
    aws::{AmazonS3, AmazonS3Builder},
    path::Path as ObjStorePath,
    Attribute, Attributes, ObjectStore, PutOptions, PutPayload,
};
use objects::get_urls_from_hashes;
use std::{fs::File, io::Read, path::Path as StdPath, sync::Arc};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::services::fs::ServeDir;

type ObjStore = Arc<AmazonS3>;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    _ = dotenvy::dotenv();

    let store = Arc::new(
        AmazonS3Builder::new()
            .with_access_key_id(dotenvy::var("AWS_ACCESS_KEY_ID").unwrap())
            .with_endpoint(dotenvy::var("AWS_ENDPOINT_URL_S3").unwrap())
            .with_region(dotenvy::var("AWS_REGION").unwrap())
            .with_secret_access_key(dotenvy::var("AWS_SECRET_ACCESS_KEY").unwrap())
            .with_bucket_name(dotenvy::var("BUCKET_NAME").unwrap())
            .build()
            .unwrap(),
    );

    let app = Router::new()
        .nest_service("/", ServeDir::new("assets"))
        .route("/meme", post(upload))
        .route("/meme", get(get_recent_memes))
        .route("/meme/:obj_hash", get(get_meme_by_id))
        .layer(DefaultBodyLimit::disable())
        .layer(RequestBodyLimitLayer::new(10 * 1024 * 1024 /* 10mb */))
        .with_state(store);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn get_recent_memes(State(store): State<ObjStore>) -> impl IntoResponse {
    let object_stream = store.list(None);

    let twelve_hours_ago = Utc::now() - Duration::hours(12);
    let mut recent_objects = Vec::new();
    let mut stream = object_stream;
    while let Some(result) = stream.next().await {
        match result {
            Ok(meta) if meta.last_modified > twelve_hours_ago => recent_objects.push(meta),
            Err(e) => eprintln!("Error fetching object metadata: {}", e),
            _ => (),
        }
    }

    dbg!(&recent_objects);

    let hashes = recent_objects
        .into_iter()
        .map(|om| om.location.to_string())
        .collect::<Vec<String>>();

    let urls = get_urls_from_hashes(hashes);
    Json(urls)
}

async fn get_meme_by_id(
    State(store): State<ObjStore>,
    Path(obj_hash): Path<String>,
) -> impl IntoResponse {
    let resp = store.get(&obj_hash.into()).await.unwrap();
    dbg!(&resp);

    let attributes = resp.attributes.clone();

    let filename = attributes
        .get(&Attribute::Metadata("filename".into()))
        .unwrap();
    let content_disposition = format!("attachment; filename=\"{}\"", filename.to_string());
    dbg!(&content_disposition);

    let content_type = attributes
        .get(&Attribute::Metadata("content_type".into()))
        .unwrap();
    let content_type = content_type.to_string();
    dbg!(&content_type);

    let data = resp.bytes().await.unwrap();

    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", content_type.parse().unwrap());
    (StatusCode::OK, headers, data)
}

async fn upload(State(store): State<ObjStore>, mut multipart: Multipart) -> impl IntoResponse {
    let mut hashes = Vec::new();
    while let Some(field) = multipart.next_field().await.unwrap() {
        let filename = field.file_name().unwrap().to_string();
        let content_type = field.content_type().unwrap().to_string();
        dbg!(&content_type);
        let data = field.bytes().await.unwrap();

        let hash = get_truncated_sha256(&data, HashOutputSize::Short32);
        dbg!(&hash);

        let mut attributes = Attributes::new();
        attributes.insert(Attribute::Metadata("filename".into()), filename.into());
        attributes.insert(
            Attribute::Metadata("content_type".into()),
            content_type.into(),
        );

        dbg!(&attributes);

        let mut put_opts = PutOptions::default();
        put_opts.attributes = attributes;

        let payload = PutPayload::from_bytes(data);
        _ = store
            .put_opts(&ObjStorePath::from(hash.clone()), payload, put_opts)
            .await
            .unwrap();

        hashes.push(hash.clone());
    }

    let urls = get_urls_from_hashes(hashes);

    let file_path = format!(
        "{}/upload_success.html",
        dotenvy::var("STATIC_ASSETS").unwrap()
    );
    let path = StdPath::new(&file_path);

    if path.exists() {
        let mut file = File::open(path)
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read file"))?;

        let mut contents = String::new();
        file.read_to_string(&mut contents).map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to read file".into(),
            )
        })?;

        let r = render!(&contents, upload_urls => urls);

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", mime_guess::mime::TEXT_HTML.as_ref())
            .body(Body::from(r))
            .unwrap())
    } else {
        Err((StatusCode::NOT_FOUND, "File not found test".into()))
    }
}
