use libp2p::identity;
use std::{fs::File, io::{Read, Write}, path::PathBuf};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

/// Generate an Ed25519 key pair.
///
/// # Returns
/// A new Ed25519 key pair.
pub fn generate_ed25519_keypair() -> identity::Keypair {
    identity::Keypair::generate_ed25519()
}

/// Save the key pair to a file.
///
/// # Arguments
/// * `keypair` - The key pair to save.
/// * `file_path` - The file path where the key pair should be saved.
///
/// # Returns
/// An empty result if the operation is successful, or an I/O error if it fails.
pub fn save_keypair_to_file(keypair: &identity::Keypair, file_path: String) -> std::io::Result<()> {
    let path = PathBuf::from(file_path);

    let path = if path.exists() && path.is_dir() {
        path.join("node.key")
    } else {
        path
    };

    let keypair_bytes = keypair.to_protobuf_encoding()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    let mut file = File::create(path)?;
    file.write_all(&keypair_bytes)?;
    Ok(())
}

/// Load a key pair from a file.
///
/// # Arguments
/// * `file_path` - The file path from which the key pair should be loaded.
///
/// # Returns
/// The loaded key pair, or an I/O error if it fails.
pub fn load_keypair_from_file(file_path: String) -> std::io::Result<identity::Keypair> {
    let path = PathBuf::from(file_path);
    let mut file = File::open(path)?;
    let mut keypair_bytes = Vec::new();
    file.read_to_end(&mut keypair_bytes)?;
    let keypair = identity::Keypair::from_protobuf_encoding(&keypair_bytes)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    Ok(keypair)
}

/// Sign data with a private key.
///
/// # Arguments
/// * `file_path` - The file path to the private key.
/// * `data` - The data to sign.
///
/// # Returns
/// The signature as a base64 encoded string, or an I/O error if it fails.
pub fn sign_data_with_key(file_path: String, data: Vec<u8>) -> std::io::Result<String> {
    let keypair = load_keypair_from_file(file_path)?;
    let signature = keypair.sign(&data).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    Ok(STANDARD.encode(&signature))
}

/// Serialize the public key to a string.
///
/// # Arguments
/// * `file_path` - The file path to the key pair.
///
/// # Returns
/// The serialized public key as a base64 encoded string, or an I/O error if it fails.
pub fn serialize_public_key_to_string(file_path: String) -> std::io::Result<String> {
    let keypair = load_keypair_from_file(file_path)?;
    let public_key_bytes = keypair.public().encode_protobuf();
    Ok(STANDARD.encode(&public_key_bytes))
}

/// Deserialize a public key from a string.
///
/// # Arguments
/// * `encoded` - The base64 encoded public key string.
///
/// # Returns
/// The deserialized public key, or an I/O error if it fails.
pub fn deserialize_public_key_from_string(encoded: String) -> std::io::Result<identity::PublicKey> {
    let bytes = STANDARD.decode(encoded).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    identity::PublicKey::try_decode_protobuf(&bytes).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// Verify a signature with a public key.
///
/// # Arguments
/// * `public_key` - The public key to verify the signature with.
/// * `data` - The data that was signed.
/// * `signature` - The base64 encoded signature.
///
/// # Returns
/// An empty result if the signature is valid, or an I/O error if it is not.
pub fn verify_signature_with_key(public_key: &identity::PublicKey, data: Vec<u8>, signature: String) -> std::io::Result<()> {
    let signature_bytes = STANDARD.decode(signature).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    if public_key.verify(&data, &signature_bytes) {
        Ok(())
    } else {
        Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Verification failed"))
    }
}
