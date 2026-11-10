use std::path::PathBuf;
use std::process::Command;

/// macOS: the key patch library loaded into the game copy (app/macos/keypatch.c), universal so that it fits the
/// game whether it runs native or under Rosetta. Embedded in the launcher with include_bytes!.
fn build_keypatch() {
    let src = "../macos/keypatch.c";
    println!("cargo:rerun-if-changed={src}");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("libforever_keypatch.dylib");
    let status = Command::new("clang")
        .args(["-dynamiclib", "-O2", "-Wall", "-arch", "arm64", "-arch", "x86_64", "-mmacosx-version-min=11.0", "-o"])
        .arg(&out)
        .arg(src)
        .status()
        .expect("clang not found (xcode-select --install)");
    assert!(status.success(), "building {src} failed");
}

/// The public certificate of your server's CA (scripts/gen-certs.sh writes it to certs/ca/forever_root_ca.crt):
/// the FOREVER_CA_CERT variable (its PEM text, as GitHub Actions passes it), else resources/forever_root_ca.crt.
fn embed_ca() {
    let file = "resources/forever_root_ca.crt";
    println!("cargo:rerun-if-env-changed=FOREVER_CA_CERT");
    println!("cargo:rerun-if-changed={file}");
    let pem = match std::env::var("FOREVER_CA_CERT") {
        Ok(text) if !text.trim().is_empty() => text,
        _ => std::fs::read_to_string(file).unwrap_or_else(|_| {
            panic!("no server CA: set FOREVER_CA_CERT or copy certs/ca/forever_root_ca.crt (never the .key) to app/src-tauri/{file}")
        }),
    };
    assert!(pem.contains("-----BEGIN CERTIFICATE-----"), "the server CA is not a PEM certificate");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("forever_root_ca.crt");
    std::fs::write(out, pem.trim().to_string() + "\n").unwrap();
}

fn main() {
    embed_ca();
    seal_server();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        build_keypatch();
    }
    // FOREVER_CHECK_ONLY=1: lets `cargo check --target x86_64-pc-windows-gnu` run on Linux without the Windows
    // resource compiler (real builds never set it)
    if std::env::var_os("FOREVER_CHECK_ONLY").is_some() {
        println!("cargo:rustc-cfg=desktop");
        println!("cargo:rustc-check-cfg=cfg(desktop, mobile)");
        return;
    }
    tauri_build::build()
}

/// FOREVER_SERVER_ADDRESS / FOREVER_SERVER_LAN_ADDRESS -> AES-256-GCM blobs in OUT_DIR/server_blob.rs (fresh random
/// key and nonces each build). The plain address never reaches the source or the exe. Never printed.
fn seal_server() {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Nonce};
    for key in [
        "AETERNUM_SERVER_ADDRESS",
        "FOREVER_SERVER_ADDRESS",
        "AETERNUM_SERVER_LAN_ADDRESS",
        "FOREVER_SERVER_LAN_ADDRESS",
        "AETERNUM_PORTAL",
        "FOREVER_PORTAL",
        "AETERNUM_BNET_PORT",
        "FOREVER_BNET_PORT",
        "AETERNUM_REST_PORT",
        "FOREVER_REST_PORT",
        "AETERNUM_WORLD_PORT",
        "FOREVER_WORLD_PORT",
        "AETERNUM_WEBSITE",
        "FOREVER_WEBSITE",
        "AETERNUM_SERVER_NAME",
        "FOREVER_SERVER_NAME",
        "AETERNUM_UPDATE_URL",
    ] {
        println!("cargo:rerun-if-env-changed={key}");
    }
    let get = |k: &str| std::env::var(k).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    // Empty unless the operator sets it. Never a production address in the source.
    let public = get("AETERNUM_SERVER_ADDRESS").or_else(|| get("FOREVER_SERVER_ADDRESS")).unwrap_or_default();
    let lan = get("AETERNUM_SERVER_LAN_ADDRESS").or_else(|| get("FOREVER_SERVER_LAN_ADDRESS")).unwrap_or_default();
    let mut key = [0u8; 32];
    getrandom::getrandom(&mut key).expect("random key");
    let cipher = Aes256Gcm::new_from_slice(&key).unwrap();
    let seal = |plain: &str| -> ([u8; 12], Vec<u8>) {
        let mut n = [0u8; 12];
        getrandom::getrandom(&mut n).expect("random nonce");
        if plain.is_empty() {
            return (n, Vec::new());
        }
        (n, cipher.encrypt(Nonce::from_slice(&n), plain.as_bytes()).expect("encrypt"))
    };
    let (na, sa) = seal(&public);
    let (nl, sl) = seal(&lan);
    let code = format!(
        "const KEY: [u8; 32] = {key:?};\nconst NONCE_A: [u8; 12] = {na:?};\nconst SEALED_A: &[u8] = &{sa:?};\nconst NONCE_L: [u8; 12] = {nl:?};\nconst SEALED_L: &[u8] = &{sl:?};\n"
    );
    std::fs::write(PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("server_blob.rs"), code).unwrap();
}
