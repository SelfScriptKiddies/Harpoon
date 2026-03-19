use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use harpoon_core::config::CoreConfig;
use harpoon_core::types::endpoint::Endpoint;
use harpoon_core::types::rule::Rule;

#[tokio::test]
async fn test_control_socket_full_workflow() {
    // Start echo server
    let echo_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let echo_addr = echo_listener.local_addr().unwrap();

    tokio::spawn(async move {
        loop {
            if let Ok((mut stream, _)) = echo_listener.accept().await {
                tokio::spawn(async move {
                    let mut buf = [0u8; 1024];
                    loop {
                        let n = stream.read(&mut buf).await.unwrap();
                        if n == 0 {
                            break;
                        }
                        stream.write_all(&buf[..n]).await.unwrap();
                    }
                });
            }
        }
    });

    let proxy_tmp = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_tmp.local_addr().unwrap();
    drop(proxy_tmp);

    let config = CoreConfig {
        rules: vec![Rule {
            name: "test-rule".into(),
            listen: Endpoint::tcp(proxy_addr),
            target: Endpoint::tcp(echo_addr),
            filters: vec![],
            duplicate: None,
            exporter: None,
            idle_timeout_secs: 30,
        }],
        ..CoreConfig::default()
    };

    let engine_handle = harpoon_core::run(config).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Verify proxy works
    let mut client = tokio::net::TcpStream::connect(proxy_addr).await.unwrap();
    client.write_all(b"test control").await.unwrap();
    let mut buf = [0u8; 64];
    let n = client.read(&mut buf).await.unwrap();
    assert_eq!(&buf[..n], b"test control");

    // Verify stats
    let stats = engine_handle.stats_snapshot();
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].rule_name, "test-rule");
    assert!(stats[0].bytes_client_to_server > 0);

    engine_handle.stop();
    engine_handle.shutdown().await;
}

#[tokio::test]
async fn test_cli_help_output() {
    let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_harpoon"))
        .arg("--help")
        .output()
        .await
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("MITM/traffic-analysis proxy"));
    assert!(stdout.contains("run"));
    assert!(stdout.contains("stop"));
    assert!(stdout.contains("status"));
    assert!(stdout.contains("stats"));
    assert!(stdout.contains("rules"));
    assert!(stdout.contains("events"));
    assert!(stdout.contains("reload"));
}
