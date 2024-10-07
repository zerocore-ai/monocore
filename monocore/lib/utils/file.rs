use std::path::Path;

use oci_spec::image::DigestAlgorithm;
use sha2::{Digest, Sha256, Sha384, Sha512};
use tokio::{fs::File, io::AsyncReadExt};

use crate::{MonocoreError, MonocoreResult};

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Gets the hash of a file.
pub async fn get_file_hash(path: &Path, algorithm: &DigestAlgorithm) -> MonocoreResult<Vec<u8>> {
    let mut file = File::open(path).await?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).await?;

    let hash = match algorithm {
        DigestAlgorithm::Sha256 => Sha256::digest(&buffer).to_vec(),
        DigestAlgorithm::Sha384 => Sha384::digest(&buffer).to_vec(),
        DigestAlgorithm::Sha512 => Sha512::digest(&buffer).to_vec(),
        _ => {
            return Err(MonocoreError::UnsupportedImageHashAlgorithm(format!(
                "Unsupported algorithm: {}",
                algorithm
            )));
        }
    };

    Ok(hash)
}
