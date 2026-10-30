use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Command, Stdio},
    thread,
};

fn receive(stream: &mut TcpStream) -> Value {
    let mut buf = [0_u8; 8192];
    let mut data = Vec::new();
    let end = loop {
        let count = stream.read(&mut buf).unwrap();
        assert!(count > 0);
        data.extend_from_slice(&buf[..count]);
        if let Some(index) = data.windows(4).position(|v| v == b"\r\n\r\n") {
            break index + 4;
        }
    };
    let headers = String::from_utf8_lossy(&data[..end]);
    let length = headers
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().unwrap())
        })
        .unwrap();
    while data.len() - end < length {
        let count = stream.read(&mut buf).unwrap();
        assert!(count > 0);
        data.extend_from_slice(&buf[..count]);
    }
    serde_json::from_slice(&data[end..end + length]).unwrap()
}

#[test]
fn sdk_initialize_schema_is_sent_in_native_provider_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let request = receive(&mut stream);
        let result = json!({"choices":[{"message":{"role":"assistant","content":"{\"answer\":\"ok\"}"},"finish_reason":"stop"}]}).to_string();
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{result}", result.len()).unwrap();
        stream.flush().unwrap();
        request
    });
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({"default_provider":"mock","providers":[{
            "id":"mock","kind":"openai-compatible","model":"test-model",
            "base_url":format!("http://{addr}/v1"),"stream":false
        }]})
        .to_string(),
    )
    .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
        .args(["--workspace", root.path().to_str().unwrap(), "sdk"])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut input = child.stdin.take().unwrap();
        writeln!(
            input,
            "{}",
            json!({"type":"control_request","request_id":"init", "request":{
            "subtype":"initialize","jsonSchema":{"type":"object","properties":{
                "answer":{"type":"string"}},"required":["answer"],"additionalProperties":false}}})
        )
        .unwrap();
        writeln!(
            input,
            "{}",
            json!({"type":"user","content":"Return an answer"})
        )
        .unwrap();
    }
    let output = child.wait_with_output().unwrap();
    let request = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(request["response_format"]["type"], "json_schema");
    assert_eq!(
        request["response_format"]["json_schema"]["schema"]["properties"]["answer"]["type"],
        "string"
    );
    let output = String::from_utf8_lossy(&output.stdout);
    assert!(output.contains("\"request_id\":\"init\""), "{output}");
    assert!(output.contains("\"subtype\":\"success\""), "{output}");
}
