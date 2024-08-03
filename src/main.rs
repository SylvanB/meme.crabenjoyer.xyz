use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{HeaderMap, Response, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use minijinja::render;
use object_store::{
    aws::{AmazonS3, AmazonS3Builder},
    path::Path as ObjStorePath,
    Attribute, Attributes, ObjectStore, PutOptions, PutPayload,
};
use sha2::{Digest, Sha256};
use std::{borrow::Cow, fs::File, io::Read, path::Path as StdPath, sync::Arc};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::services::fs::ServeDir;

type ObjStore = Arc<AmazonS3>;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    match dotenvy::dotenv() {
        Ok(_) => {}
        Err(_) => {}
    }

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
        .route("/meme/:obj_hash", get(get_meme))
        .layer(DefaultBodyLimit::disable())
        .layer(RequestBodyLimitLayer::new(
            10 * 1024 * 1024, /* 250mb */
        ))
        .with_state(store);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn get_meme(
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

    let data = resp.bytes().await.unwrap();

    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", "application/octet-stream".parse().unwrap());
    headers.insert("Content-Disposition", content_disposition.parse().unwrap());
    (StatusCode::OK, headers, data)
}

async fn upload(State(store): State<ObjStore>, mut multipart: Multipart) -> impl IntoResponse {
    let mut hashes = Vec::new();
    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.file_name().unwrap().to_string();
        let data = field.bytes().await.unwrap();

        let digest = Sha256::digest(&data);
        let hash: String = digest.iter().map(|byte| format!("{:02x}", byte)).collect();

        let mut attributes = Attributes::new();
        attributes.insert(Attribute::Metadata("filename".into()), name.into());

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

    let base_url = dotenvy::var("BASE_SITE_URL").unwrap();
    let mut urls = Vec::new();
    for hash in hashes {
        urls.push(format!("{0}/meme/{1}", base_url, &hash));
    }

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
