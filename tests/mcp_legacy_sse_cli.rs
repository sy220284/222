//! Source mcp/addCommand.ts: SSE configuration must connect to a legacy
//! MCP event stream, negotiate its endpoint, then execute discovery.
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::Command,
    thread,
    time::Duration,
};

fn request(socket: &mut TcpStream) -> Value {
    socket
        .set_read_timeout(Some(Duration::from_secs(15)))
        .unwrap();
    let mut body = Vec::new();
    let mut buf = [0_u8; 4096];
    let header_end = loop {
        let count = socket.read(&mut buf).unwrap();
        assert!(count > 0, "MCP client closed before headers");
        body.extend_from_slice(&buf[..count]);
        if let Some(p) = body.windows(4).position(|s| s == b"\r\n\r\n") {
            break p + 4;
        }
    };
    let header = String::from_utf8_lossy(&body[..header_end]);
    let length = header
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().unwrap())
        })
        .unwrap_or(0);
    while body.len() - header_end < length {
        let count = socket.read(&mut buf).unwrap();
        assert!(count > 0, "MCP client closed before body");
        body.extend_from_slice(&buf[..count]);
    }
    if length == 0 {
        Value::Null
    } else {
        serde_json::from_slice(&body[header_end..header_end + length]).unwrap()
    }
}

fn run(root: &std::path::Path, config: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
        .env("ATLAS_CONFIG", config)
        .arg("--workspace")
        .arg(root)
        .arg("mcp")
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn legacy_sse_server_connects_and_discovers_tools() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut sse, _) = listener.accept().unwrap();
        sse.set_read_timeout(Some(Duration::from_secs(15))).unwrap();
        let start = request(&mut sse);
        assert_eq!(start, Value::Null);
        write!(sse, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\n\r\nevent: endpoint\ndata: /messages?session=test\n\n").unwrap();
        sse.flush().unwrap();
        let mut methods = Vec::new();
        for _ in 0..3 {
            let (mut post, _) = listener.accept().unwrap();
            let data = request(&mut post);
            let method = data["method"].as_str().unwrap().to_owned();
            methods.push(method.clone());
            write!(
                post,
                "HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )
            .unwrap();
            post.flush().unwrap();
            if data.get("id").is_some() {
                let result = match method.as_str() {
                    "initialize" => {
                        json!({"protocolVersion":"2024-11-05","serverInfo":{"name":"legacy","version":"1.0"},"capabilities":{"tools":{}}})
                    }
                    "tools/list" => {
                        json!({"tools":[{"name":"ping","description":"Returns pong","inputSchema":{"type":"object"}}]})
                    }
                    _ => panic!("Unexpected MCP method: {method}"),
                };
                let reply = json!({"jsonrpc":"2.0","id":data["id"],"result":result});
                write!(sse, "event: message\ndata: {reply}\n\n").unwrap();
                sse.flush().unwrap();
            }
        }
        methods
    });
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("settings.json");
    let url = format!("http://{addr}/sse");
    let added = run(
        root.path(),
        &config,
        &[
            "add",
            "--transport",
            "sse",
            "--scope",
            "user",
            "legacy",
            &url,
        ],
    );
    assert!(
        added.status.success(),
        "{}",
        String::from_utf8_lossy(&added.stderr)
    );
    let get = run(root.path(), &config, &["get", "legacy"]);
    assert!(
        get.status.success(),
        "{}",
        String::from_utf8_lossy(&get.stderr)
    );
    let reply: Value = serde_json::from_slice(&get.stdout).unwrap();
    assert_eq!(reply["health"]["status"], "connected", "{reply}");
    assert_eq!(reply["type"], "sse");
    assert_eq!(
        server.join().unwrap(),
        ["initialize", "notifications/initialized", "tools/list"]
    );
    let saved: Value = serde_json::from_slice(&fs::read(config).unwrap()).unwrap();
    assert_eq!(saved["mcp_servers"][0]["type"], "sse");
}

#[test]
fn legacy_sse_rejects_credential_exposure_before_connecting() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("settings.json");
    let output = run(
        root.path(),
        &config,
        &[
            "add",
            "--transport",
            "sse",
            "--scope",
            "user",
            "--header",
            "Authorization: Bearer secret",
            "unsafe",
            "http://example.org/sse",
        ],
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("HTTPS"));
}
