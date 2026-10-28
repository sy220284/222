use serde_json::{json, Value};
use std::{fs, io::{Read, Write}, net::{TcpListener, TcpStream}, process::Command, thread, time::Duration};

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
}

#[test]
fn cli_initialization_and_model_listing_work_offline() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.json");
    let init = binary().args(["init"]).env("ATLAS_CONFIG", &config).output().unwrap();
    assert!(init.status.success(), "{:?}", init);
    assert!(config.is_file());
    let list = binary().args(["models"]).env("ATLAS_CONFIG", &config).output().unwrap();
    assert!(list.status.success());
    let stdout = String::from_utf8(list.stdout).unwrap();
    assert!(stdout.contains("local (default)"));
    assert!(stdout.contains("gemini"));
}

fn read_request(stream: &mut TcpStream) -> String {
    stream.set_read_timeout(Some(Duration::from_secs(15))).unwrap();
    let mut data = Vec::new();
    loop {
        let mut chunk = [0u8; 4096];
        let count = stream.read(&mut chunk).unwrap();
        if count == 0 { break; }
        data.extend_from_slice(&chunk[..count]);
        if let Some(index) = data.windows(4).position(|v| v == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&data[..index]);
            let length = headers.lines().find_map(|line| {
                line.to_ascii_lowercase().strip_prefix("content-length:")
                    .and_then(|v| v.trim().parse::<usize>().ok())
            }).unwrap_or(0);
            if data.len() >= index + 4 + length { break; }
        }
    }
    String::from_utf8(data).unwrap()
}

#[test]
fn real_http_tool_roundtrip_and_session_persistence() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("example.txt"), "hello from workspace\n").unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for index in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_request(&mut stream);
            requests.push(request);
            let payload = if index == 0 {
                json!({"choices":[{"message":{"role":"assistant","content":null,"tool_calls":[
                    {"id":"call_1","type":"function","function":{
                        "name":"read_file","arguments":"{\"path\":\"example.txt\"}"
                    }}
                ]},"finish_reason":"tool_calls"}]})
            } else {
                json!({"choices":[{"message":{"role":"assistant","content":"Read verified."},
                    "finish_reason":"stop"}]})
            };
            let body = payload.to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
            stream.flush().unwrap();
        }
        requests
    });

    let config = dir.path().join("test-config.json");
    fs::write(&config, json!({
        "default_provider":"mock",
        "providers":[{"id":"mock","kind":"openai-compatible",
            "base_url":format!("http://{addr}/v1"),"model":"test-model"}]
    }).to_string()).unwrap();
    let output = binary()
        .args(["--workspace", dir.path().to_str().unwrap(), "run", "Inspect example.txt"])
        .env("ATLAS_CONFIG", &config)
        .output().unwrap();
    let requests = server.join().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stdout).contains("Read verified."));
    assert!(requests[0].contains("Inspect example.txt"));
    assert!(requests[1].contains("hello from workspace"));
    let session_dir = dir.path().join(".atlas/sessions");
    let sessions: Vec<_> = fs::read_dir(session_dir).unwrap().collect();
    assert_eq!(sessions.len(), 1);
    let saved = fs::read_to_string(sessions[0].as_ref().unwrap().path()).unwrap();
    assert!(saved.contains("call_1"));
    assert!(saved.contains("Read verified."));
}

#[test]
fn gemini_native_adapter_completes_tool_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("example.txt"), "gemini test file\n").unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut received = Vec::new();
        for i in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            received.push(read_request(&mut stream));
            let payload = if i == 0 {
                json!({"candidates":[{"content":{"parts":[
                    {"functionCall":{"name":"read_file","args":{"path":"example.txt"}}}
                ]},"finishReason":"STOP"}]})
            } else {
                json!({"candidates":[{"content":{"parts":[{"text":"Gemini read verified."}]},"finishReason":"STOP"}]})
            };
            let body = payload.to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
            stream.flush().unwrap();
        }
        received
    });
    let config = dir.path().join("gemini-config.json");
    fs::write(&config, json!({
        "default_provider":"gm",
        "providers":[{"id":"gm","kind":"gemini",
            "base_url":format!("http://{address}/v1beta"),
            "model":"example","api_key_env":"ATLAS_TEST_GEMINI_KEY"}]
    }).to_string()).unwrap();
    let output = binary().args([
        "--workspace",dir.path().to_str().unwrap(),"run","Read example.txt"
    ]).env("ATLAS_CONFIG", &config).env("ATLAS_TEST_GEMINI_KEY","fake-local-key")
        .output().unwrap();
    let requests = server.join().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stdout).contains("Gemini read verified."));
    assert!(requests[0].contains("functionDeclarations"));
    assert!(requests[1].contains("functionResponse"));
    assert!(requests[1].contains("gemini test file"));
}

#[test]
fn subagent_runs_a_separate_model_conversation_and_returns_to_parent() {
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for step in 0..3 {
            let (mut stream, _) = listener.accept().unwrap();
            let data = read_request(&mut stream);
            requests.push(data);
            let reply = match step {
                0 => json!({"choices":[{"message":{
                    "role":"assistant","content":null,
                    "tool_calls":[{"id":"parent_spawn","type":"function",
                        "function":{"name":"spawn_agent",
                            "arguments":json!({"prompt":"Analyze workspace","isolation":"none"}).to_string()
                        }}]
                },"finish_reason":"tool_calls"}]}),
                1 => json!({"choices":[{"message":{
                    "role":"assistant","content":"Subagent completed analysis."
                },"finish_reason":"stop"}]}),
                _ => json!({"choices":[{"message":{
                    "role":"assistant","content":"Delegation verified."
                },"finish_reason":"stop"}]}),
            };
            let body = reply.to_string();
            write!(stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(), body).unwrap();
            stream.flush().unwrap();
        }
        requests
    });
    let config = root.path().join("provider.json");
    fs::write(&config, json!({"default_provider":"test","providers":[{
        "id":"test","kind":"openai-compatible","model":"mock",
        "base_url":format!("http://{address}/v1")
    }]}).to_string()).unwrap();
    let output = binary().args([
        "--workspace",root.path().to_str().unwrap(),
        "--approval","read-only","run","Delegate analysis"
    ]).env("ATLAS_CONFIG", &config).output().unwrap();
    let requests = server.join().unwrap();
    assert!(output.status.success(),
        "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stdout).contains("Delegation verified."));
    assert!(requests[1].contains("Analyze workspace"));
    assert!(!requests[1].contains("spawn_agent"));
    assert!(requests[2].contains("Subagent completed analysis."));
    assert_eq!(fs::read_dir(root.path().join(".atlas/sessions")).unwrap().count(), 2);
}

#[test]
fn stop_hook_block_causes_one_additional_model_round() {
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut messages = Vec::new();
        for i in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            messages.push(read_request(&mut stream));
            let text = if i == 0 { "Premature answer" } else { "Verified final answer" };
            let body = json!({"choices":[{
                "message":{"role":"assistant","content":text},"finish_reason":"stop"
            }]}).to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
            stream.flush().unwrap();
        }
        messages
    });
    let config = root.path().join("config.json");
    fs::write(&config, json!({
        "default_provider":"mock",
        "providers":[{"id":"mock","kind":"openai-compatible",
            "base_url":format!("http://{address}/v1"),"model":"mock"}],
        "hooks":[{"event":"Stop","command":"echo 'Confirm tests' >&2; exit 2",
            "timeout_seconds":5}]
    }).to_string()).unwrap();
    let output = binary()
        .args(["--workspace",root.path().to_str().unwrap(),"run","Do work"])
        .env("ATLAS_CONFIG",&config).output().unwrap();
    let requests = server.join().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stdout).contains("Verified final answer"));
    assert!(requests[1].contains("Confirm tests"));
}

#[test]
fn sdk_mode_emits_json_lines_and_preserves_context() {
    use std::process::Stdio;
    let dir=tempfile::tempdir().unwrap();
    let listener=TcpListener::bind("127.0.0.1:0").unwrap();
    let addr=listener.local_addr().unwrap();
    let server=thread::spawn(move||{
        let mut requests=Vec::new();
        for step in 0..2 {
            let (mut stream,_)=listener.accept().unwrap();
            requests.push(read_request(&mut stream));
            let body=json!({"choices":[{"message":{"role":"assistant",
                "content":if step==0{"First reply"}else{"Second reply"}},"finish_reason":"stop"}]})
                .to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
            stream.flush().unwrap();
        }
        requests
    });
    let conf=dir.path().join("config.json");
    fs::write(&conf,json!({"default_provider":"mock","providers":[{
        "id":"mock","kind":"openai-compatible","base_url":format!("http://{addr}/v1"),
        "model":"fake"}]}).to_string()).unwrap();
    let mut process=binary().args(["--workspace",dir.path().to_str().unwrap(),
        "--approval","read-only","sdk"]).env("ATLAS_CONFIG",&conf)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().unwrap();
    {
        let mut writer=process.stdin.take().unwrap();
        writeln!(writer,"{}",json!({"type":"user","content":"First"})).unwrap();
        writeln!(writer,"{}",json!({"type":"user","content":"Second"})).unwrap();
    }
    let output=process.wait_with_output().unwrap();
    let requests=server.join().unwrap();
    assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stderr));
    assert!(requests[1].contains("First reply"));
    let events=String::from_utf8(output.stdout).unwrap();
    let records:Vec<Value>=events.lines().map(|s|serde_json::from_str(s).unwrap()).collect();
    assert_eq!(records.iter().filter(|v|v["type"]=="result").count(),2);
    assert!(records.iter().any(|v|v["type"]=="system"));
    assert!(records.iter().any(|v|v["type"]=="assistant"));
}

#[test]
fn team_runs_two_members_with_separate_contexts_and_stable_result_order() {
    let root=tempfile::tempdir().unwrap();
    let listener=TcpListener::bind("127.0.0.1:0").unwrap();
    let addr=listener.local_addr().unwrap();
    let server=thread::spawn(move||{
        let mut requests=Vec::new();
        for step in 0..4{
            let (mut conn,_)=listener.accept().unwrap();
            let raw=read_request(&mut conn);
            let request:Value=serde_json::from_str(raw.split("\r\n\r\n").nth(1).unwrap()).unwrap();
            let messages=request["messages"].as_array().unwrap();
            let serialized=serde_json::to_string(messages).unwrap();
            let payload=if step==0{
                json!({"choices":[{"message":{"role":"assistant","content":null,
                    "tool_calls":[{"id":"team-1","type":"function","function":{
                        "name":"spawn_team","arguments":json!({"tasks":[
                            {"name":"one","prompt":"Analyze A"},
                            {"name":"two","prompt":"Analyze B"}
                        ]}).to_string()}}
                    ]},"finish_reason":"tool_calls"}]})
            }else if messages.last().is_some_and(|m|m["role"]=="tool"){
                json!({"choices":[{"message":{"role":"assistant","content":"Team complete"},
                    "finish_reason":"stop"}]})
            }else if serialized.contains("Analyze A"){
                json!({"choices":[{"message":{"role":"assistant","content":"Result A"},
                    "finish_reason":"stop"}]})
            }else if serialized.contains("Analyze B"){
                json!({"choices":[{"message":{"role":"assistant","content":"Result B"},
                    "finish_reason":"stop"}]})
            }else{
                json!({"choices":[{"message":{"role":"assistant","content":"Team complete"},
                    "finish_reason":"stop"}]})
            };
            let body=payload.to_string();
            write!(conn,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
            conn.flush().unwrap();
            requests.push(serialized);
        }
        requests
    });
    let config=root.path().join("config.json");
    fs::write(&config,json!({"default_provider":"mock","providers":[{
        "id":"mock","kind":"openai-compatible","model":"mock",
        "base_url":format!("http://{addr}/v1")
    }]}).to_string()).unwrap();
    let output=binary().args(["--workspace",root.path().to_str().unwrap(),
        "--approval","read-only","run","Team tasks"]).env("ATLAS_CONFIG",&config)
        .output().unwrap();
    let requests=server.join().unwrap();
    assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stdout).contains("Team complete"));
    assert!(requests.iter().any(|q|q.contains("Analyze A")));
    assert!(requests.iter().any(|q|q.contains("Analyze B")));
    assert!(requests.iter().any(|q|q.contains("Result A")&&q.contains("Result B")));
    assert_eq!(fs::read_dir(root.path().join(".atlas/sessions")).unwrap().count(),3);
}

#[test]
fn sdk_streaming_emits_incremental_chunks_and_final_result() {
    use std::process::Stdio;
    let root=tempfile::tempdir().unwrap();
    let listener=TcpListener::bind("127.0.0.1:0").unwrap();
    let addr=listener.local_addr().unwrap();
    let server=thread::spawn(move||{
        let (mut stream,_)=listener.accept().unwrap();
        let request=read_request(&mut stream);
        assert!(request.contains("\"stream\":true"));
        let sse=concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hel\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"lo\"},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n"
        );
        write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",sse.len(),sse).unwrap();
        stream.flush().unwrap();
    });
    let config=root.path().join("config.json");
    fs::write(&config,json!({"default_provider":"mock","providers":[{
        "id":"mock","kind":"openai-compatible",
        "base_url":format!("http://{addr}/v1"),"model":"mock","stream":true
    }]}).to_string()).unwrap();
    let mut process=binary().args(["--workspace",root.path().to_str().unwrap(),
        "--approval","read-only","sdk"]).env("ATLAS_CONFIG",&config)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().unwrap();
    {
        let mut input=process.stdin.take().unwrap();
        writeln!(input,"{}",json!({"type":"user","content":"Say hello"})).unwrap();
    }
    let output=process.wait_with_output().unwrap();
    server.join().unwrap();
    assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stderr));
    let result=String::from_utf8(output.stdout).unwrap();
    let events=result.lines().map(|s|serde_json::from_str::<Value>(s).unwrap())
        .collect::<Vec<_>>();
    let deltas=events.iter().filter(|v|v["type"]=="assistant_delta")
        .filter_map(|v|v["delta"].as_str()).collect::<String>();
    assert_eq!(deltas,"Hello");
    assert!(events.iter().any(|v|v["type"]=="result"&&v["content"]=="Hello"));
}

#[cfg(unix)]
#[test]
fn background_shell_model_tool_roundtrip_runs_and_reads_real_process() {
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut log = Vec::new();
        for turn in 0..3 {
            let (mut stream, _) = listener.accept().unwrap();
            let raw = read_request(&mut stream);
            let request: Value = serde_json::from_str(raw.split("\r\n\r\n").nth(1).unwrap()).unwrap();
            let messages = request["messages"].as_array().unwrap();
            let reply = if turn == 0 {
                json!({"choices":[{"message":{"role":"assistant","content":null,
                    "tool_calls":[{"id":"start1","type":"function","function":{
                        "name":"run_command","arguments":json!({
                            "command":"printf background-ready",
                            "run_in_background":true,"description":"Test shell"
                        }).to_string()
                    }}]},"finish_reason":"tool_calls"}]})
            } else if turn == 1 {
                let last: Value = serde_json::from_str(
                    messages.last().unwrap()["content"].as_str().unwrap()
                ).unwrap();
                let task_id = last["task_id"].as_str().unwrap().to_owned();
                log.push(last);
                json!({"choices":[{"message":{"role":"assistant","content":null,
                    "tool_calls":[{"id":"read1","type":"function","function":{
                        "name":"task_output","arguments":json!({
                            "task_id":task_id,"block":true,"timeout":5000
                        }).to_string()
                    }}]},"finish_reason":"tool_calls"}]})
            } else {
                let last: Value = serde_json::from_str(
                    messages.last().unwrap()["content"].as_str().unwrap()
                ).unwrap();
                log.push(last);
                json!({"choices":[{"message":{"role":"assistant",
                    "content":"Background shell verified."},"finish_reason":"stop"}]})
            };
            let body = reply.to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(), body).unwrap();
            stream.flush().unwrap();
        }
        log
    });
    let config = root.path().join("config.json");
    fs::write(&config, json!({"default_provider":"mock","providers":[{
        "id":"mock","kind":"openai-compatible",
        "base_url":format!("http://{addr}/v1"),"model":"mock"
    }]}).to_string()).unwrap();
    let output = binary().args(["--workspace", root.path().to_str().unwrap(),
        "--approval", "full", "run", "Run a background test"])
        .env("ATLAS_CONFIG", &config).output().unwrap();
    let logs = server.join().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stdout).contains("Background shell verified."));
    assert_eq!(logs[0]["status"], "running");
    assert_eq!(logs[1]["retrieval_status"], "success");
    assert_eq!(logs[1]["task"]["status"], "completed");
    assert!(logs[1]["task"]["output"].as_str().unwrap().contains("background-ready"));
}

#[test]
fn sdk_json_lines_never_contain_javascript_unicode_line_separators() {
    use std::process::Stdio;
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let _request = read_request(&mut stream);
        let message = "first\u{2028}middle\u{2029}last";
        let body = json!({"choices":[{"message":{
            "role":"assistant","content":message
        },"finish_reason":"stop"}]}).to_string();
        write!(stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),body).unwrap();
        stream.flush().unwrap();
    });
    let config = root.path().join("config.json");
    fs::write(&config,json!({"default_provider":"mock","providers":[{
        "id":"mock","kind":"openai-compatible",
        "base_url":format!("http://{addr}/v1"),"model":"mock"
    }]}).to_string()).unwrap();
    let mut child = binary().args([
        "--workspace",root.path().to_str().unwrap(),"--approval","read-only","sdk"
    ]).env("ATLAS_CONFIG",&config)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().unwrap();
    {
        let mut input=child.stdin.take().unwrap();
        writeln!(input,"{}",json!({"type":"user","content":"Reply with separators"})).unwrap();
    }
    let output=child.wait_with_output().unwrap();
    server.join().unwrap();
    assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stderr));
    let wire=String::from_utf8(output.stdout).unwrap();
    assert!(!wire.contains('\u{2028}'));
    assert!(!wire.contains('\u{2029}'));
    let events=wire.lines().map(|line|serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    let result=events.iter().find(|e|e["type"]=="result").unwrap();
    assert_eq!(result["content"],"first\u{2028}middle\u{2029}last");
}

#[test]
fn original_task_tools_end_to_end_model_calls_and_disk_graph() {
    let root=tempfile::tempdir().unwrap();
    let listener=TcpListener::bind("127.0.0.1:0").unwrap();
    let addr=listener.local_addr().unwrap();
    let server=thread::spawn(move||{
        let mut seen=Vec::<String>::new();
        let script=[
            ("TaskCreate",json!({"subject":"Implement","description":"Implement feature"})),
            ("TaskCreate",json!({"subject":"Verify","description":"Run integration tests"})),
            ("TaskUpdate",json!({"taskId":"2","addBlockedBy":["1"],"owner":"tester"})),
            ("TaskGet",json!({"taskId":"2"})),
            ("TaskList",json!({})),
            ("TaskUpdate",json!({"taskId":"1","status":"completed"})),
            ("TaskList",json!({})),
        ];
        for index in 0..=script.len(){
            let (mut stream,_)=listener.accept().unwrap();
            let raw=read_request(&mut stream);
            let req:Value=serde_json::from_str(raw.split("\r\n\r\n").nth(1).unwrap()).unwrap();
            if let Some(last)=req["messages"].as_array().unwrap().last(){
                if last["role"]=="tool"{
                    seen.push(last["content"].as_str().unwrap().to_owned());
                }
            }
            let response=if let Some((name,args))=script.get(index){
                json!({"choices":[{"message":{"role":"assistant","content":null,
                    "tool_calls":[{"id":format!("task-{index}"),"type":"function",
                        "function":{"name":name,"arguments":args.to_string()}}]},
                    "finish_reason":"tool_calls"}]})
            }else{
                json!({"choices":[{"message":{"role":"assistant",
                    "content":"Task workflow verified."},"finish_reason":"stop"}]})
            };
            let data=response.to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                data.len(),data).unwrap();
            stream.flush().unwrap();
        }
        seen
    });
    let config=root.path().join("config.json");
    fs::write(&config,json!({"default_provider":"mock","providers":[{
        "id":"mock","kind":"openai-compatible","model":"mock",
        "base_url":format!("http://{addr}/v1")
    }]}).to_string()).unwrap();
    let response=binary().args(["--workspace",root.path().to_str().unwrap(),
        "--approval","read-only","run","Create two tasks, link dependencies and finish"])
        .env("ATLAS_CONFIG",&config).output().unwrap();
    let results=server.join().unwrap();
    assert!(response.status.success(),"{}",String::from_utf8_lossy(&response.stderr));
    assert!(String::from_utf8_lossy(&response.stdout).contains("Task workflow verified."));
    assert_eq!(results.len(),7);
    assert!(results[0].contains("Task #1 created successfully: Implement"));
    assert!(results[1].contains("Task #2 created successfully: Verify"));
    assert!(results[2].contains("blockedBy"));
    assert!(results[3].contains("Blocked by: #1"));
    assert!(results[4].contains("[blocked by #1]"));
    assert!(results[5].contains("status"));
    assert!(!results[6].contains("[blocked by #1]"));
    let list_dir=root.path().join(".atlas/task-lists");
    let entries=fs::read_dir(&list_dir).unwrap().collect::<Vec<_>>();
    assert_eq!(entries.len(),1);
    let tasks=fs::read_dir(entries[0].as_ref().unwrap().path()).unwrap()
        .filter_map(Result::ok).filter(|e|e.path().extension().is_some_and(|x|x=="json"))
        .count();
    assert_eq!(tasks,2);
}

#[test]
fn model_driven_team_create_message_receive_and_cleanup() {
    let dir=tempfile::tempdir().unwrap();
    let listener=TcpListener::bind("127.0.0.1:0").unwrap();
    let addr=listener.local_addr().unwrap();
    let server=thread::spawn(move||{
        let steps=[
            ("TeamCreate",json!({"team_name":"review","description":"Review code"})),
            ("SendMessage",json!({"to":"team-lead","summary":"Review note","message":"Check CI output"})),
            ("TeamInbox",json!({"mark_read":true})),
            ("TeamDelete",json!({})),
        ];
        let mut outputs=Vec::new();
        let mut saw_auto_delivery=false;
        for index in 0..=steps.len() {
            let (mut stream,_)=listener.accept().unwrap();
            let request=read_request(&mut stream);
            let payload:Value=serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
            let messages=payload["messages"].as_array().unwrap();
            if messages.iter().any(|m|m["role"]=="user" &&
                m["content"].as_str().unwrap_or("").contains(
                    "<teammate-message teammate_id=\"team-lead\""")){
                saw_auto_delivery=true;
            }
            if let Some(last)=messages.iter().rev().find(|m|m["role"]=="tool"){
                outputs.push(last["content"].as_str().unwrap().to_owned());
            }
            let reply=if let Some((name,args))=steps.get(index){
                json!({"choices":[{"message":{"role":"assistant","content":null,
                    "tool_calls":[{"id":format!("team-{index}"),"type":"function",
                        "function":{"name":name,"arguments":args.to_string()}}]},
                    "finish_reason":"tool_calls"}]})
            }else{
                json!({"choices":[{"message":{"role":"assistant",
                    "content":"Team lifecycle complete."},"finish_reason":"stop"}]})
            };
            let body=reply.to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),body).unwrap();
            stream.flush().unwrap();
        }
        (outputs,saw_auto_delivery)
    });
    let config=dir.path().join("config.json");
    fs::write(&config,json!({"default_provider":"mock","providers":[{
        "id":"mock","kind":"openai-compatible","model":"test",
        "base_url":format!("http://{addr}/v1")
    }]}).to_string()).unwrap();
    let output=binary().args(["--workspace",dir.path().to_str().unwrap(),
        "--approval","full","run","Run team lifecycle"])
        .env("ATLAS_CONFIG",&config).output().unwrap();
    let (results,saw_auto_delivery)=server.join().unwrap();
    assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stdout).contains("Team lifecycle complete."));
    assert_eq!(results.len(),4);
    assert!(results[0].contains("review"));
    assert!(results[1].contains("success"));
    assert!(saw_auto_delivery,"Teammate messages must reach the next model round");
    assert_eq!(results[2],"[]");
    assert!(results[3].contains("Cleaned up directories"));
    assert!(!dir.path().join(".atlas/teams/review").exists());
}
