use serde_json::Value;
use std::process::{Command, Output};

fn molt(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_molt"))
        .args(args)
        .output()
        .expect("molt binary must run")
}

#[test]
fn analyze_and_query_contracts_with_bounded_json_output() -> Result<(), Box<dyn std::error::Error>>
{
    let dir = tempfile::tempdir()?;
    let client_dir = dir.path().join("another/layout");
    std::fs::create_dir_all(&client_dir)?;
    std::fs::write(
        client_dir.join("ExampleClient.ts"),
        "class ExampleClient { create() { return this.post('/items/new'); } }",
    )?;
    std::fs::write(
        dir.path().join("ItemController.java"),
        "@RestController @RequestMapping(\"/items\") class ItemController { @PostMapping(\"/new\") void create() {} }",
    )?;
    let source = dir.path().to_str().ok_or("temporary path is not UTF-8")?;
    let analyzed = molt(&["analyze", "--source", source]);
    assert!(
        analyzed.status.success(),
        "{}",
        String::from_utf8_lossy(&analyzed.stderr)
    );
    let result: Value = serde_json::from_slice(&analyzed.stdout)?;
    assert_eq!(result["outbound_http_candidates"], 1);
    assert_eq!(result["http_routes"], 1);

    let linked = molt(&[
        "query",
        "--source",
        source,
        "http-links",
        "--route",
        "/items/new",
    ]);
    assert!(
        linked.status.success(),
        "{}",
        String::from_utf8_lossy(&linked.stderr)
    );
    let links: Value = serde_json::from_slice(&linked.stdout)?;
    assert_eq!(links[0]["status"], "path_match_unverified");
    assert_eq!(links[0]["backend_path"], "ItemController.java");

    let issues = molt(&["query", "--source", source, "contract-issues"]);
    assert!(issues.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&issues.stdout)?,
        serde_json::json!([])
    );
    let invalid = molt(&[
        "query",
        "--source",
        source,
        "contract-issues",
        "--limit",
        "101",
    ]);
    assert!(!invalid.status.success());
    let error: Value = serde_json::from_slice(&invalid.stderr)?;
    assert_eq!(error["status"], "failed");
    Ok(())
}
