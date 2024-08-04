use sha2::{Digest, Sha256};

#[derive(Debug)]
pub enum HashOutputSize {
    Full = 32,
    Half = 16,
    Short32 = 8,
    Short16 = 4,
}

pub fn get_truncated_sha256(data: impl AsRef<[u8]>, size: HashOutputSize) -> String {
    let digest = Sha256::digest(data);

    println!("Digest before processing: {:?}", digest);
    println!("Size: {:?}", size);

    let digest_iter = digest.iter();

    dbg!(&digest_iter);

    let hash = digest_iter
        .skip(32 - (size as usize))
        .map(|byte| format!("{:02x}", byte))
        .collect::<String>();

    hash
}
