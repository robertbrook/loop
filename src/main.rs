use loop_lisp::{Evaluation, Interpreter, INPUT_LIMIT};
use std::{
    env, fs,
    io::{self, BufRead, IsTerminal, Read, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

fn read_source(reader: impl Read) -> Result<String, String> {
    let mut source = String::new();
    reader
        .take(INPUT_LIMIT as u64 + 1)
        .read_to_string(&mut source)
        .map_err(|error| error.to_string())?;
    if source.len() > INPUT_LIMIT {
        return Err("Program exceeds the 64 KiB input limit".into());
    }
    Ok(source)
}

fn read_file(path: &Path) -> Result<(String, String), String> {
    let name = fs::canonicalize(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let source =
        read_source(fs::File::open(&name).map_err(|error| format!("{}: {error}", name.display()))?)
            .map_err(|error| format!("{}: {error}", name.display()))?;
    Ok((name.to_string_lossy().into_owned(), source))
}

fn print_results(results: Vec<Evaluation>) -> Result<(), String> {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    for result in results {
        writeln!(output, "{}{}", result.output, result.value())
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn repl(runtime: &mut Interpreter) -> Result<(), String> {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut source = String::new();
    loop {
        print!(
            "{}",
            if source.is_empty() {
                "loop> "
            } else {
                "....> "
            }
        );
        io::stdout().flush().map_err(|error| error.to_string())?;
        let mut line = String::new();
        if input
            .read_line(&mut line)
            .map_err(|error| error.to_string())?
            == 0
        {
            if !source.trim().is_empty() {
                print_results(runtime.execute_named(&source, "<repl>")?)?;
            }
            return Ok(());
        }
        source.push_str(&line);
        if source.len() <= INPUT_LIMIT && Interpreter::is_incomplete(&source) {
            continue;
        }
        match runtime.execute_named(&source, "<repl>") {
            Ok(results) => print_results(results)?,
            Err(error) => eprintln!("{error}"),
        }
        source.clear();
    }
}

fn run() -> Result<(), String> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args
        .first()
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        println!("Usage: loop [-e PROGRAM | FILE ... | -]\n\nNo arguments: REPL on a terminal, otherwise evaluate standard input.\nFiles run left to right in one session. (load \"file.loop\") resolves relative\npaths against the source file containing the load expression.");
        return Ok(());
    }
    if args == ["--version"] {
        println!("Loop {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let mut runtime = Interpreter::new();
    runtime.set_source_loader(|requested, caller| {
        let base = if caller.starts_with('<') {
            PathBuf::from(".")
        } else {
            Path::new(caller)
                .parent()
                .unwrap_or(Path::new("."))
                .to_path_buf()
        };
        read_file(&base.join(requested))
    });
    if args.is_empty() {
        if io::stdin().is_terminal() {
            return repl(&mut runtime);
        }
        let source = read_source(io::stdin().lock())?;
        return print_results(runtime.execute_named(&source, "<stdin>")?);
    }
    if args[0] == "-e" {
        if args.len() != 2 {
            return Err("-e expects exactly one program argument".into());
        }
        return print_results(runtime.execute_named(&args[1], "<command>")?);
    }
    for arg in args {
        let (name, source) = if arg == "-" {
            ("<stdin>".into(), read_source(io::stdin().lock())?)
        } else if arg.starts_with('-') {
            return Err(format!("Unknown option: {arg}. Use --help for usage."));
        } else {
            read_file(Path::new(&arg))?
        };
        print_results(runtime.execute_named(&source, &name)?)?;
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
