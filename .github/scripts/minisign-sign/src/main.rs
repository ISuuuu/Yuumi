use base64::{engine::general_purpose::STANDARD, Engine};
use minisign::{SecretKey, SecretKeyBox};
use std::env;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;

fn decode_key_box(raw: &str) -> Result<SecretKeyBox, String> {
    let trimmed = raw.trim();
    if let Ok(decoded) = STANDARD.decode(trimmed) {
        if let Ok(text) = String::from_utf8(decoded) {
            if let Ok(sk_box) = SecretKeyBox::from_string(&text) {
                return Ok(sk_box);
            }
        }
    }
    SecretKeyBox::from_string(trimmed).map_err(|e| format!("解析签名私钥失败: {e}"))
}

fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        return Err("用法: minisign-sign <file-path> [sig-path]".into());
    }
    let file_path = Path::new(&args[1]);
    let sig_path = if args.len() >= 3 {
        args[2].clone()
    } else {
        format!("{}.sig", file_path.display())
    };

    let raw_key = env::var("TAURI_SIGNING_PRIVATE_KEY")
        .map_err(|_| "缺少环境变量 TAURI_SIGNING_PRIVATE_KEY".to_string())?;
    let password = env::var("TAURI_SIGNING_PRIVATE_KEY_PASSWORD").ok();

    let sk_box = decode_key_box(&raw_key)?;
    let sk: SecretKey = sk_box
        .into_secret_key(password)
        .map_err(|e| format!("解密签名私钥失败: {e}"))?;

    let file = File::open(file_path).map_err(|e| format!("打开待签名文件失败: {e}"))?;
    let mut reader = BufReader::new(file);
    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("artifact");
    let trusted_comment = format!(
        "timestamp:{}\tfile:{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        file_name
    );

    let signature_box =
        minisign::sign(None, &sk, &mut reader, Some(trusted_comment.as_str()), None)
            .map_err(|e| format!("生成签名失败: {e}"))?;

    // Tauri updater expects .sig files to contain base64-encoded minisign signature box text.
    let encoded_sig = STANDARD.encode(signature_box.into_string().as_bytes());
    fs::write(&sig_path, encoded_sig).map_err(|e| format!("写入签名文件失败: {e}"))?;
    println!("Signed {} -> {}", file_path.display(), sig_path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_and_verify_roundtrip() {
        use minisign::KeyPair;

        let mut pk_bytes = Vec::new();
        let mut sk_bytes = Vec::new();
        KeyPair::generate_and_write_encrypted_keypair(
            &mut pk_bytes,
            &mut sk_bytes,
            Some("test comment"),
            Some("password123".into()),
        )
        .unwrap();

        let sk_box_str = String::from_utf8(sk_bytes).unwrap();
        let pk_box_str = String::from_utf8(pk_bytes).unwrap();

        let data = b"Hello Yuumi Release Artifact!";
        let mut reader = std::io::Cursor::new(data);

        let sk = decode_key_box(&STANDARD.encode(&sk_box_str))
            .unwrap()
            .into_secret_key(Some("password123".into()))
            .unwrap();

        let sig_box = minisign::sign(None, &sk, &mut reader, Some("test comment"), None).unwrap();
        let encoded_sig = STANDARD.encode(sig_box.into_string().as_bytes());

        // Verify with minisign-verify using update.rs's exact logic
        let key_text = pk_box_str;
        let key_line = key_text.lines().nth(1).unwrap();
        let pk = minisign_verify::PublicKey::from_base64(key_line).unwrap();

        let sig_text = String::from_utf8(STANDARD.decode(&encoded_sig).unwrap()).unwrap();
        let sig = minisign_verify::Signature::decode(&sig_text).unwrap();

        assert!(pk.verify(data, &sig, false).is_ok());
    }
}
