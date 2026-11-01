use std::{
    fs,
    io::{Read, Write},
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

#[test]
fn native_text_files_and_data_formats_compose() {
    let root = std::env::temp_dir().join(format!("loop-data-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let source = r#"
        (write-file "data.json" (json-stringify '(object ("answer" 42))))
        (json-get (json-parse (read-file "data.json")) "answer")
        (write-file "data.xml" (xml-stringify '(element "root" () "λ & text")))
        (node-text (xml-parse (read-file "data.xml")))
        (node-text (html-parse "<p>one<p>two"))
        (defmacro double (x) `(+ ,x ,x))
        (double (eval '(+ 1 2)))
    "#;
    let output = Command::new(env!("CARGO_BIN_EXE_loop"))
        .current_dir(&root)
        .args(["-e", source])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "()\n42\n()\n\"λ & text\"\n\"onetwo\"\ndouble\n6\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("data.json")).unwrap(),
        "{\"answer\":42}"
    );
    let missing = Command::new(env!("CARGO_BIN_EXE_loop"))
        .current_dir(&root)
        .args(["-e", "(read-file \"missing.txt\")"])
        .output()
        .unwrap();
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("missing.txt"));
    fs::write(root.join("binary.txt"), [255]).unwrap();
    let binary = Command::new(env!("CARGO_BIN_EXE_loop"))
        .current_dir(&root)
        .args(["-e", "(read-file \"binary.txt\")"])
        .output()
        .unwrap();
    assert!(
        !binary.status.success(),
        "files are UTF-8 text, not silently decoded binary"
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn native_http_transmits_method_headers_body_and_returns_error_statuses() {
    use std::{
        net::TcpListener,
        thread,
        time::{Duration, Instant},
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(8);
        let mut connection = loop {
            match listener.accept() {
                Ok((connection, _)) => break connection,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "HTTP client did not connect");
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("{error}"),
            }
        };
        connection
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut request = Vec::new();
        let mut buffer = [0; 1024];
        loop {
            let count = connection.read(&mut buffer).unwrap();
            assert_ne!(count, 0, "client closed before sending the request");
            request.extend_from_slice(&buffer[..count]);
            if request.windows(4).any(|window| window == b"\r\n\r\n")
                && request.ends_with(b"{\"a\":1}")
            {
                break;
            }
        }
        let request = String::from_utf8(request).unwrap();
        assert!(request.starts_with("POST /data HTTP/1.1\r\n"), "{request}");
        assert!(
            request
                .to_lowercase()
                .contains("content-type: application/json"),
            "{request}"
        );
        connection.write_all(b"HTTP/1.1 422 Unprocessable Content\r\nContent-Type: application/json\r\nContent-Length: 14\r\nConnection: close\r\n\r\n{\"error\":true}").unwrap();
    });
    let source = format!(
        r#"
        (define response (http-request "POST" "http://{address}/data" '(("Content-Type" "application/json")) "{{\"a\":1}}"))
        (list (http-status response) (json-get (json-parse (http-body response)) "error"))
    "#
    );
    let output = cli(&["-e", &source]);
    server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"response\n(422 #t)\n");
}

fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_loop"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn expressions_stdin_and_exit_codes() {
    let result = cli(&["-e", "(+ 1 2)"]);
    assert!(result.status.success());
    assert_eq!(result.stdout, b"3\n");
    let result = cli(&["-e", "(/ 1 0)"]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("<command>:1:1 in /"));
    assert!(!cli(&["-e"]).status.success());
    assert!(!cli(&["--unknown"]).status.success());
    assert!(cli(&["--help"]).status.success());
    assert!(cli(&["--version"]).status.success());
    let mut child = Command::new(env!("CARGO_BIN_EXE_loop"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"(define x 4)\n(+ x 3)\n")
        .unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(result.status.success());
    assert_eq!(result.stdout, b"x\n7\n");
}

#[test]
fn committed_example_runs_with_the_native_host() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/main.loop");
    let result = Command::new(env!("CARGO_BIN_EXE_loop"))
        .arg(path)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"math\n()\n(1 4 9 16 25)\n500500\n");
}

#[test]
fn files_modules_and_nested_relative_loads() {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "loop-cli-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    fs::create_dir(root.join("lib")).unwrap();
    fs::write(
        root.join("lib/math.loop"),
        "(module math (export double) (define (double x) (+ x x)))",
    )
    .unwrap();
    fs::write(
        root.join("lib/import.loop"),
        "(load \"math.loop\") (import math double)",
    )
    .unwrap();
    fs::write(
        root.join("main.loop"),
        "(load \"lib/import.loop\") (double 21)",
    )
    .unwrap();
    fs::write(root.join("bad.loop"), "(load \"missing.loop\")").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_loop"))
        .arg(root.join("main.loop"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"()\n42\n");
    let bad = Command::new(env!("CARGO_BIN_EXE_loop"))
        .arg(root.join("bad.loop"))
        .output()
        .unwrap();
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("missing.loop"));
    let multiple = Command::new(env!("CARGO_BIN_EXE_loop"))
        .arg(root.join("lib/math.loop"))
        .arg(root.join("main.loop"))
        .output()
        .unwrap();
    assert!(
        !multiple.status.success(),
        "duplicate module declarations must fail"
    );
    fs::remove_dir_all(&root).unwrap();
}
