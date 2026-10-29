use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

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
