use std::{
    env,
    fs::{self, File},
    io::Write,
};

use axum::{
    extract::{DefaultBodyLimit, Multipart},
    routing::post,
    Router,
};
use object_store::{
    aws::{AmazonS3, AmazonS3Builder},
    path::Path,
    ObjectStore,
};
use sha2::{Digest, Sha256};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    // initialize tracing
    tracing_subscriber::fmt::init();

    dotenvy::dotenv().unwrap();

    // build our application with a route
    let app = Router::new()
        .nest_service("/", ServeDir::new("assets"))
        .route("/meme", post(upload))
        .layer(DefaultBodyLimit::disable())
        .layer(RequestBodyLimitLayer::new(
            250 * 1024 * 1024, /* 250mb */
        ));

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn upload(mut multipart: Multipart) -> String {
    while let Some(field) = multipart.next_field().await.unwrap() {
        let _name = field.name().unwrap().to_string();
        let data = field.bytes().await.unwrap();

        let digest = Sha256::digest(&data);
        let hash: String = digest.iter().map(|byte| format!("{:02x}", byte)).collect();
        let temp_dir = env::temp_dir();
        let temp_file_path = temp_dir.join(&hash);

        let mut temp_file = File::create(&temp_file_path).unwrap();
        temp_file.write_all(&data).unwrap();
        temp_file.flush().unwrap();

        let store: AmazonS3 = AmazonS3Builder::from_env()
            .with_endpoint(dotenvy::var("AWS_ENDPOINT_URL_S3").unwrap())
            .with_bucket_name(dotenvy::var("BUCKET_NAME").unwrap())
            .build()
            .unwrap();

        let path_str = temp_file_path.to_string_lossy().into_owned();
        println!("File created at: {}", path_str);
        _ = store.put_multipart(&Path::from(path_str)).await.unwrap();
        fs::remove_file(temp_file_path).unwrap();
    }

    format!("https://meme.crabenjoyer.xyz/meme/{}", "abcde12345")
}
