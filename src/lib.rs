use std::{cell::RefCell, collections::HashMap, fmt, rc::Rc};

type Env = Rc<RefCell<Scope>>;
type Result<T> = std::result::Result<T, String>;

#[derive(Clone)]
enum Value {
    Number(f64),
    Bool(bool),
    Text(String),
    Symbol(String),
    List(Vec<Value>),
    Function(Vec<String>, Vec<Value>, Env),
    Builtin(String),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(n) => write!(f, "{n}"),
            Self::Bool(b) => write!(f, "{}", if *b { "#t" } else { "#f" }),
            Self::Text(s) => write!(f, "{s:?}"),
            Self::Symbol(s) => write!(f, "{s}"),
            Self::List(items) => {
                write!(f, "(")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, ")")
            }
            Self::Function(..) => write!(f, "#<procedure>"),
            Self::Builtin(name) => write!(f, "#<builtin:{name}>"),
        }
    }
}

struct Scope {
    values: HashMap<String, Value>,
    parent: Option<Env>,
}

fn scope(parent: Option<Env>) -> Env {
    Rc::new(RefCell::new(Scope {
        values: HashMap::new(),
        parent,
    }))
}

fn lookup(env: &Env, name: &str) -> Result<Value> {
    let current = env.borrow();
    if let Some(value) = current.values.get(name) {
        return Ok(value.clone());
    }
    match &current.parent {
        Some(parent) => lookup(parent, name),
        None => Err(format!("Unknown symbol: {name}")),
    }
}

#[derive(Debug)]
enum Token {
    Open,
    Close,
    Quote,
    Atom(String),
    Text(String),
}

fn tokenize(source: &str) -> Result<Vec<Token>> {
    let mut chars = source.chars().peekable();
    let mut tokens = Vec::new();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {}
            ';' => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            '(' => tokens.push(Token::Open),
            ')' => tokens.push(Token::Close),
            '\'' => tokens.push(Token::Quote),
            '"' => {
                let mut text = String::new();
                let mut closed = false;
                while let Some(c) = chars.next() {
                    match c {
                        '"' => {
                            closed = true;
                            break;
                        }
                        '\\' => text.push(match chars.next() {
                            Some('n') => '\n',
                            Some('t') => '\t',
                            Some('"') => '"',
                            Some('\\') => '\\',
                            Some(other) => return Err(format!("Unknown escape: \\{other}")),
                            None => return Err("Unfinished string escape".into()),
                        }),
                        _ => text.push(c),
                    }
                }
                if !closed {
                    return Err("Unclosed string: add a closing double quote".into());
                }
                tokens.push(Token::Text(text));
            }
            _ => {
                let mut atom = c.to_string();
                while let Some(&next) = chars.peek() {
                    if next.is_whitespace() || "()';\"".contains(next) {
                        break;
                    }
                    atom.push(chars.next().unwrap());
                }
                tokens.push(Token::Atom(atom));
            }
        }
    }
    Ok(tokens)
}

fn parse(tokens: &[Token], pos: &mut usize, depth: usize) -> Result<Value> {
    if depth > 128 {
        return Err("Reader nesting limit reached".into());
    }
    let token = tokens.get(*pos).ok_or("Expected an expression")?;
    *pos += 1;
    match token {
        Token::Open => {
            let mut list = Vec::new();
            loop {
                match tokens.get(*pos) {
                    None => return Err("Unclosed list: add a closing parenthesis".into()),
                    Some(Token::Close) => {
                        *pos += 1;
                        return Ok(Value::List(list));
                    }
                    _ => {
                        if list.len() >= 10_000 {
                            return Err("List size limit reached".into());
                        }
                        list.push(parse(tokens, pos, depth + 1)?);
                    }
                }
            }
        }
        Token::Close => Err("Unexpected closing parenthesis".into()),
        Token::Quote => Ok(Value::List(vec![
            Value::Symbol("quote".into()),
            parse(tokens, pos, depth + 1)?,
        ])),
        Token::Text(s) => Ok(Value::Text(s.clone())),
        Token::Atom(s) => Ok(match s.as_str() {
            "#t" => Value::Bool(true),
            "#f" => Value::Bool(false),
            _ => match s.parse::<f64>() {
                Ok(n) if n.is_finite() => Value::Number(n),
                _ => Value::Symbol(s.clone()),
            },
        }),
    }
}

fn arity(name: &str, values: &[Value], count: usize) -> Result<()> {
    if values.len() == count {
        Ok(())
    } else {
        Err(format!(
            "{name} expects {count} argument(s), got {}",
            values.len()
        ))
    }
}

fn symbol(value: &Value) -> Result<String> {
    if let Value::Symbol(s) = value {
        Ok(s.clone())
    } else {
        Err("Expected a symbol".into())
    }
}

fn parameters(value: &Value) -> Result<Vec<String>> {
    if let Value::List(items) = value {
        let names = items.iter().map(symbol).collect::<Result<Vec<_>>>()?;
        let mut unique = std::collections::HashSet::new();
        if names.iter().any(|name| !unique.insert(name)) {
            return Err("Duplicate parameter".into());
        }
        Ok(names)
    } else {
        Err("Expected a parameter list".into())
    }
}

fn truthy(value: &Value) -> bool {
    !matches!(value, Value::Bool(false))
}

struct Interpreter {
    env: Env,
    steps: usize,
    output: String,
}

impl Interpreter {
    fn new() -> Self {
        let env = scope(None);
        for name in [
            "+", "-", "*", "/", "=", "<", ">", "<=", ">=", "not", "list", "car", "cdr", "cons",
            "append", "length", "null?", "number?", "list?", "display", "newline", "map", "filter",
            "apply", "modulo", "abs",
        ] {
            env.borrow_mut()
                .values
                .insert(name.into(), Value::Builtin(name.into()));
        }
        Self {
            env,
            steps: 0,
            output: String::new(),
        }
    }

    fn run(&mut self, source: &str) -> Result<String> {
        if source.len() > 65_536 {
            return Err("Program exceeds the 64 KiB input limit".into());
        }
        self.steps = 0;
        self.output.clear();
        let tokens = tokenize(source)?;
        let mut pos = 0;
        let mut expressions = Vec::new();
        while pos < tokens.len() {
            expressions.push(parse(&tokens, &mut pos, 0)?);
        }
        for expression in expressions {
            let value = self.eval(&expression, self.env.clone(), 0)?;
            self.output.push_str(&format!("{value}\n"));
            if self.output.len() > 131_072 {
                return Err("Output limit reached".into());
            }
        }
        Ok(self.output.trim_end().to_string())
    }

    fn tick(&mut self, depth: usize) -> Result<()> {
        self.steps += 1;
        if self.steps > 100_000 {
            return Err("Evaluation limit reached. Try a smaller program.".into());
        }
        if depth > 128 {
            return Err("Call depth limit reached. Try a smaller input.".into());
        }
        Ok(())
    }

    fn sequence(&mut self, body: &[Value], env: Env, depth: usize) -> Result<Value> {
        let mut result = Value::List(vec![]);
        for expression in body {
            result = self.eval(expression, env.clone(), depth + 1)?;
        }
        Ok(result)
    }

    fn eval(&mut self, expression: &Value, env: Env, depth: usize) -> Result<Value> {
        self.tick(depth)?;
        let items = match expression {
            Value::Symbol(name) => return lookup(&env, name),
            Value::List(items) if !items.is_empty() => items,
            _ => return Ok(expression.clone()),
        };
        let args = &items[1..];
        if let Value::Symbol(name) = &items[0] {
            match name.as_str() {
                "quote" => {
                    arity(name, args, 1)?;
                    return Ok(args[0].clone());
                }
                "if" => {
                    arity(name, args, 3)?;
                    let condition = self.eval(&args[0], env.clone(), depth + 1)?;
                    return self.eval(
                        &args[if truthy(&condition) { 1 } else { 2 }],
                        env,
                        depth + 1,
                    );
                }
                "define" => {
                    if args.len() < 2 {
                        return Err("define needs a name and a value or body".into());
                    }
                    let (key, value) = match &args[0] {
                        Value::Symbol(key) => {
                            arity(name, args, 2)?;
                            (key.clone(), self.eval(&args[1], env.clone(), depth + 1)?)
                        }
                        Value::List(signature) if !signature.is_empty() => (
                            symbol(&signature[0])?,
                            Value::Function(
                                parameters(&Value::List(signature[1..].to_vec()))?,
                                args[1..].to_vec(),
                                env.clone(),
                            ),
                        ),
                        _ => return Err("define needs a symbol or function signature".into()),
                    };
                    env.borrow_mut().values.insert(key.clone(), value);
                    return Ok(Value::Symbol(key));
                }
                "lambda" => {
                    if args.len() < 2 {
                        return Err("lambda needs parameters and a body".into());
                    }
                    return Ok(Value::Function(
                        parameters(&args[0])?,
                        args[1..].to_vec(),
                        env,
                    ));
                }
                "begin" => return self.sequence(args, env, depth),
                "let" => {
                    if args.len() < 2 {
                        return Err("let needs bindings and a body".into());
                    }
                    let Value::List(bindings) = &args[0] else {
                        return Err("Expected let bindings".into());
                    };
                    let local = scope(Some(env.clone()));
                    for binding in bindings {
                        let Value::List(pair) = binding else {
                            return Err("Expected (name value) binding".into());
                        };
                        arity("binding", pair, 2)?;
                        let key = symbol(&pair[0])?;
                        let value = self.eval(&pair[1], env.clone(), depth + 1)?;
                        if local.borrow_mut().values.insert(key, value).is_some() {
                            return Err("Duplicate let binding".into());
                        }
                    }
                    return self.sequence(&args[1..], local, depth);
                }
                "and" | "or" => {
                    let mut result = Value::Bool(name == "and");
                    for arg in args {
                        result = self.eval(arg, env.clone(), depth + 1)?;
                        if truthy(&result) == (name == "or") {
                            break;
                        }
                    }
                    return Ok(result);
                }
                _ => {}
            }
        }
        let function = self.eval(&items[0], env.clone(), depth + 1)?;
        let values = args
            .iter()
            .map(|arg| self.eval(arg, env.clone(), depth + 1))
            .collect::<Result<Vec<_>>>()?;
        self.call(function, values, depth + 1)
    }

    fn call(&mut self, function: Value, args: Vec<Value>, depth: usize) -> Result<Value> {
        self.tick(depth)?;
        let name = match function {
            Value::Function(params, body, closure) => {
                if params.len() != args.len() {
                    return Err(format!(
                        "Procedure expects {} arguments, got {}",
                        params.len(),
                        args.len()
                    ));
                }
                let local = scope(Some(closure));
                local
                    .borrow_mut()
                    .values
                    .extend(params.into_iter().zip(args));
                return self.sequence(&body, local, depth);
            }
            Value::Builtin(name) => name,
            _ => return Err("First item in a call must be a procedure".into()),
        };
        let n = name.as_str();
        match n {
            "+" | "-" | "*" | "/" | "=" | "<" | ">" | "<=" | ">=" | "modulo" | "abs" => {
                let nums = args
                    .iter()
                    .map(|v| {
                        if let Value::Number(n) = v {
                            Ok(*n)
                        } else {
                            Err(format!("{name} expects numbers"))
                        }
                    })
                    .collect::<Result<Vec<_>>>()?;
                if n == "abs" {
                    arity(n, &args, 1)?;
                    return Ok(Value::Number(nums[0].abs()));
                }
                if n == "modulo" {
                    arity(n, &args, 2)?;
                    if nums[1] == 0.0 {
                        return Err("Division by zero".into());
                    }
                    return Ok(Value::Number(((nums[0] % nums[1]) + nums[1]) % nums[1]));
                }
                if ["=", "<", ">", "<=", ">="].contains(&n) {
                    if nums.len() < 2 {
                        return Err(format!("{n} needs at least two numbers"));
                    }
                    return Ok(Value::Bool(nums.windows(2).all(|p| match n {
                        "=" => p[0] == p[1],
                        "<" => p[0] < p[1],
                        ">" => p[0] > p[1],
                        "<=" => p[0] <= p[1],
                        _ => p[0] >= p[1],
                    })));
                }
                if nums.is_empty() && (n == "-" || n == "/") {
                    return Err(format!("{n} needs at least one number"));
                }
                let result = match n {
                    "+" => nums.iter().fold(0.0, |a, b| a + b),
                    "*" => nums.iter().product(),
                    "-" if nums.len() == 1 => -nums[0],
                    "-" => nums[1..].iter().fold(nums[0], |a, b| a - b),
                    "/" => {
                        let (start, divisors): (f64, &[f64]) = if nums.len() == 1 {
                            (1.0, &nums)
                        } else {
                            (nums[0], &nums[1..])
                        };
                        if divisors.contains(&0.0) {
                            return Err("Division by zero".into());
                        }
                        divisors.iter().fold(start, |a, b| a / b)
                    }
                    _ => unreachable!(),
                };
                if !result.is_finite() {
                    return Err("Numeric result is not finite".into());
                }
                Ok(Value::Number(result))
            }
            "list" => Ok(Value::List(args)),
            "not" | "null?" | "number?" | "list?" => {
                arity(n, &args, 1)?;
                Ok(Value::Bool(match n {
                    "not" => !truthy(&args[0]),
                    "null?" => matches!(&args[0], Value::List(l) if l.is_empty()),
                    "number?" => matches!(args[0], Value::Number(_)),
                    _ => matches!(args[0], Value::List(_)),
                }))
            }
            "car" | "cdr" | "length" => {
                arity(n, &args, 1)?;
                let Value::List(list) = &args[0] else {
                    return Err(format!("{n} expects a list"));
                };
                match n {
                    "length" => Ok(Value::Number(list.len() as f64)),
                    _ if list.is_empty() => Err(format!("{n} needs a nonempty list")),
                    "car" => Ok(list[0].clone()),
                    _ => Ok(Value::List(list[1..].to_vec())),
                }
            }
            "cons" | "append" => {
                arity(n, &args, 2)?;
                let Value::List(tail) = &args[1] else {
                    return Err(format!("{n} expects a list as its second argument"));
                };
                let mut result = if n == "cons" {
                    vec![args[0].clone()]
                } else if let Value::List(head) = &args[0] {
                    head.clone()
                } else {
                    return Err("append expects lists".into());
                };
                result.extend(tail.clone());
                if result.len() > 10_000 {
                    return Err("List size limit reached".into());
                }
                Ok(Value::List(result))
            }
            "display" => {
                arity(n, &args, 1)?;
                let text = if let Value::Text(s) = &args[0] {
                    s.clone()
                } else {
                    args[0].to_string()
                };
                if self.output.len() + text.len() > 131_072 {
                    return Err("Output limit reached".into());
                }
                self.output.push_str(&text);
                Ok(Value::List(vec![]))
            }
            "newline" => {
                arity(n, &args, 0)?;
                self.output.push('\n');
                Ok(Value::List(vec![]))
            }
            "map" | "filter" | "apply" => {
                arity(n, &args, 2)?;
                let Value::List(list) = &args[1] else {
                    return Err(format!("{n} expects a list"));
                };
                if n == "apply" {
                    return self.call(args[0].clone(), list.clone(), depth + 1);
                }
                let mut result = Vec::new();
                for item in list {
                    let value = self.call(args[0].clone(), vec![item.clone()], depth + 1)?;
                    if n == "map" {
                        result.push(value);
                    } else if truthy(&value) {
                        result.push(item.clone());
                    }
                }
                Ok(Value::List(result))
            }
            _ => Err(format!("Unknown builtin: {n}")),
        }
    }
}

thread_local! {
    static RUNTIME: RefCell<Interpreter> = RefCell::new(Interpreter::new());
    static INPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static OUTPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

#[no_mangle]
pub extern "C" fn input_buffer(len: usize) -> *mut u8 {
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        input.resize(len, 0);
        input.as_mut_ptr()
    })
}

#[no_mangle]
pub extern "C" fn evaluate() -> i32 {
    let result = INPUT.with(|input| match std::str::from_utf8(&input.borrow()) {
        Ok(source) => RUNTIME.with(|runtime| runtime.borrow_mut().run(source)),
        Err(_) => Err("Input must be UTF-8".into()),
    });
    let failed = result.is_err();
    let text = result.unwrap_or_else(|error| error);
    OUTPUT.with(|output| *output.borrow_mut() = text.into_bytes());
    i32::from(failed)
}

#[no_mangle]
pub extern "C" fn output_pointer() -> *const u8 {
    OUTPUT.with(|output| output.borrow().as_ptr())
}
#[no_mangle]
pub extern "C" fn output_length() -> usize {
    OUTPUT.with(|output| output.borrow().len())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn run(source: &str) -> String {
        Interpreter::new().run(source).unwrap()
    }
    #[test]
    fn arithmetic() {
        assert_eq!(
            run("(+ 1 (* 2 3)) (/ 4) (- 9 2 1) (+) (*)"),
            "7\n0.25\n6\n0\n1"
        );
    }
    #[test]
    fn recursion_and_closures() {
        assert_eq!(
            run("(define (fact n) (if (= n 0) 1 (* n (fact (- n 1))))) (fact 8)"),
            "fact\n40320"
        );
        assert_eq!(
            run("(define (adder x) (lambda (y) (+ x y))) ((adder 4) 8)"),
            "adder\n12"
        );
    }
    #[test]
    fn list_operations() {
        assert_eq!(run("(map (lambda (x) (* x x)) '(1 2 3)) (filter (lambda (x) (> x 2)) '(1 2 3 4)) (apply + '(1 2 3))"), "(1 4 9)\n(3 4)\n6");
        assert_eq!(
            run("(cons 1 '(2 3)) (car '(4 5)) (cdr '(4 5)) (append '(1) '(2))"),
            "(1 2 3)\n4\n(5)\n(1 2)"
        );
    }
    #[test]
    fn reader_and_special_forms() {
        assert_eq!(run("; hello\n(let ((x 3) (y 4)) (+ x y)) (if #f missing 'ok) (and #f missing) (or 7 missing)"), "7\nok\n#f\n7");
        assert_eq!(run("\"hello\\nworld\" '()"), "\"hello\\nworld\"\n()");
    }
    #[test]
    fn errors_and_limits() {
        for source in [
            "(",
            ")",
            "'",
            "\"oops",
            "(/ 1 0)",
            "(car '())",
            "(+ #t 1)",
            "(if 1 2)",
            "missing",
            "(lambda (x x) x)",
            "(define (f) (f)) (f)",
        ] {
            assert!(Interpreter::new().run(source).is_err(), "{source}");
        }
    }
    #[test]
    fn persistent_repl() {
        let mut runtime = Interpreter::new();
        runtime.run("(define x 42)").unwrap();
        assert_eq!(runtime.run("(+ x 1)").unwrap(), "43");
        assert!(runtime.run("(+ 1").is_err());
        assert_eq!(runtime.run("x").unwrap(), "42");
    }
}
