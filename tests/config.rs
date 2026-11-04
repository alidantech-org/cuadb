//! Standalone config tests that never open a network socket.

use cuadb::config::Config;

#[test]
fn new_install_can_start_without_database_credentials() {
    let config = Config::from_values(&[]).expect("default configuration must be valid");
    assert_eq!(config.port, 4848);
    assert!(config.database_url().is_none());
}

#[test]
fn rejects_invalid_values_without_exposing_passwords() {
    let secret = "do-not-echo-this";
    let malformed_port = format!("postgresql://user:{secret}@postgis:5432/db");
    let error = Config::from_values(&[("CUADB_PORT", &malformed_port)]).unwrap_err();
    assert_eq!(error.name(), "CUADB_PORT");
    assert!(!error.to_string().contains(secret));
}

#[test]
fn process_rejects_invalid_config_before_opening_a_listener() {
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_cuadb"))
        .env("CUADB_PORT", "0")
        .env("CUADB_HOST", "127.0.0.1")
        .env_remove("CUADB_DATABASE_URL")
        .output()
        .expect("binary should launch");

    assert!(!result.status.success());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("CUADB_PORT"));
    assert!(!stderr.contains("DATABASE_URL"));
}
