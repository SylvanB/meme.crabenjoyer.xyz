pub fn get_urls_from_hashes(hashes: Vec<String>) -> Vec<String> {
    let base_url = dotenvy::var("BASE_SITE_URL").unwrap();
    let mut urls = Vec::new();
    for hash in hashes {
        urls.push(format!("{0}/meme/{1}", base_url, &hash));
    }

    urls
}
