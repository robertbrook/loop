use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    fmt,
    rc::{Rc, Weak},
};

type Env = Rc<RefCell<Scope>>;
type Result<T> = std::result::Result<T, String>;

pub const INPUT_LIMIT: usize = limits::INPUT_BYTES;

mod limits {
    pub const INPUT_BYTES: usize = 65_536;
    pub const OUTPUT_BYTES: usize = 131_072;
    pub const LIST_ITEMS: usize = 10_000;
    pub const EVALUATION_STEPS: usize = 100_000;
    pub const DEPTH: usize = 128;
    pub const SOURCE_FILES: usize = 128;
}

#[derive(Clone)]
enum Value {
    Number(f64),
    Bool(bool),
    Text(String),
    Symbol(String),
    List(Vec<Value>),
    Function(Rc<Procedure>),
    Builtin(String),
    Uninitialized,
    Syntax(Box<Value>, Location),
}

struct Procedure {
    params: Vec<String>,
    body: Vec<Value>,
    env: Env,
}

fn procedure(params: Vec<String>, body: Vec<Value>, env: Env) -> Value {
    Value::Function(Rc::new(Procedure { params, body, env }))
}

#[derive(Clone, Debug)]
struct Location {
    source: Rc<str>,
    line: usize,
    column: usize,
}

impl Location {
    fn error(&self, message: String) -> String {
        self.context(message, "")
    }

    fn context(&self, message: String, context: &str) -> String {
        if message
            .lines()
            .filter(|line| line.starts_with("  at "))
            .count()
            >= 12
        {
            return message;
        }
        format!(
            "{message}\n  at {}:{}:{}{context}",
            self.source, self.line, self.column
        )
    }
}

impl Value {
    fn datum(&self) -> &Self {
        match self {
            Self::Syntax(value, _) => value.datum(),
            _ => self,
        }
    }

    fn quoted(&self) -> Self {
        match self.datum() {
            Self::List(items) => Self::List(items.iter().map(Self::quoted).collect()),
            value => value.clone(),
        }
    }
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
            Self::Uninitialized => write!(f, "#<uninitialized>"),
            Self::Syntax(value, _) => write!(f, "{value}"),
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
        if matches!(value, Value::Uninitialized) {
            return Err(format!("Uninitialized binding: {name}"));
        }
        return Ok(value.clone());
    }
    match &current.parent {
        Some(parent) => lookup(parent, name),
        None => Err(format!("Unknown symbol: {name}")),
    }
}

fn assign(env: &Env, name: &str, value: Value) -> Result<()> {
    let mut current = env.borrow_mut();
    if let Some(binding) = current.values.get_mut(name) {
        if matches!(binding, Value::Uninitialized) {
            return Err(format!("Uninitialized binding: {name}"));
        }
        *binding = value;
        return Ok(());
    }
    let parent = current.parent.clone();
    drop(current);
    match parent {
        Some(parent) => assign(&parent, name, value),
        None => Err(format!("Cannot set! unknown symbol: {name}")),
    }
}

#[derive(Debug)]
enum Token {
    Open,
    Close,
    Quote,
    Quasiquote,
    Unquote,
    UnquoteSplicing,
    Atom(String),
    Text(String),
}

struct LocatedToken {
    token: Token,
    location: Location,
}

fn tokenize(source: &str, name: &str) -> Result<Vec<LocatedToken>> {
    let name: Rc<str> = name.into();
    let mut chars = source.char_indices().peekable();
    let mut tokens = Vec::new();
    let mut line = 1;
    let mut column = 1;
    let mut previous = 0;
    while let Some((offset, c)) = chars.next() {
        for consumed in source[previous..offset].chars() {
            if consumed == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }
        previous = offset;
        let location = Location {
            source: name.clone(),
            line,
            column,
        };
        let token = match c {
            c if c.is_whitespace() => continue,
            ';' => {
                for (_, c) in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
                continue;
            }
            '(' => Token::Open,
            ')' => Token::Close,
            '\'' => Token::Quote,
            '`' => Token::Quasiquote,
            ',' => {
                if chars.peek().is_some_and(|(_, c)| *c == '@') {
                    chars.next();
                    Token::UnquoteSplicing
                } else {
                    Token::Unquote
                }
            }
            '"' => {
                let mut text = String::new();
                let mut closed = false;
                while let Some((_, c)) = chars.next() {
                    match c {
                        '"' => {
                            closed = true;
                            break;
                        }
                        '\\' => text.push(match chars.next() {
                            Some((_, 'n')) => '\n',
                            Some((_, 't')) => '\t',
                            Some((_, '"')) => '"',
                            Some((_, '\\')) => '\\',
                            Some((_, other)) => {
                                return Err(location.error(format!("Unknown escape: \\{other}")))
                            }
                            None => return Err(location.error("Unfinished string escape".into())),
                        }),
                        _ => text.push(c),
                    }
                }
                if !closed {
                    return Err(
                        location.error("Unclosed string: add a closing double quote".into())
                    );
                }
                Token::Text(text)
            }
            _ => {
                let mut atom = c.to_string();
                while let Some(&(_, next)) = chars.peek() {
                    if next.is_whitespace() || "()';\"`,".contains(next) {
                        break;
                    }
                    atom.push(chars.next().unwrap().1);
                }
                Token::Atom(atom)
            }
        };
        tokens.push(LocatedToken { token, location });
    }
    Ok(tokens)
}

fn parse(tokens: &[LocatedToken], pos: &mut usize, depth: usize) -> Result<Value> {
    if depth > limits::DEPTH {
        return Err("Reader nesting limit reached".into());
    }
    let located = tokens.get(*pos).ok_or("Expected an expression")?;
    let token = &located.token;
    *pos += 1;
    let result = (|| match token {
        Token::Open => {
            let mut list = Vec::new();
            loop {
                match tokens.get(*pos).map(|t| &t.token) {
                    None => return Err("Unclosed list: add a closing parenthesis".into()),
                    Some(Token::Close) => {
                        *pos += 1;
                        return Ok(Value::List(list));
                    }
                    _ => {
                        if list.len() >= limits::LIST_ITEMS {
                            return Err("List size limit reached".into());
                        }
                        list.push(parse(tokens, pos, depth + 1)?);
                    }
                }
            }
        }
        Token::Close => Err("Unexpected closing parenthesis".into()),
        Token::Quote | Token::Quasiquote | Token::Unquote | Token::UnquoteSplicing => {
            let name = match token {
                Token::Quote => "quote",
                Token::Quasiquote => "quasiquote",
                Token::Unquote => "unquote",
                _ => "unquote-splicing",
            };
            Ok(Value::List(vec![
                Value::Symbol(name.into()),
                parse(tokens, pos, depth + 1)?,
            ]))
        }
        Token::Text(s) => Ok(Value::Text(s.clone())),
        Token::Atom(s) => Ok(match s.as_str() {
            "#t" => Value::Bool(true),
            "#f" => Value::Bool(false),
            _ => match s.parse::<f64>() {
                Ok(n) if n.is_finite() => Value::Number(n),
                _ => Value::Symbol(s.clone()),
            },
        }),
    })();
    result
        .map(|value| Value::Syntax(Box::new(value), located.location.clone()))
        .map_err(|error: String| {
            if error.contains("\n  at ") {
                error
            } else {
                located.location.error(error)
            }
        })
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
    if let Value::Symbol(s) = value.datum() {
        Ok(s.clone())
    } else {
        Err("Expected a symbol".into())
    }
}

fn parameters(value: &Value) -> Result<Vec<String>> {
    if let Value::List(items) = value.datum() {
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

fn is_symbol(value: &Value, name: &str) -> bool {
    matches!(value.datum(), Value::Symbol(s) if s == name)
}

fn datum_equal(left: &Value, right: &Value) -> bool {
    match (left.datum(), right.datum()) {
        (Value::Number(a), Value::Number(b)) => a == b,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Text(a), Value::Text(b)) | (Value::Symbol(a), Value::Symbol(b)) => a == b,
        (Value::List(a), Value::List(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| datum_equal(a, b))
        }
        (Value::Function(a), Value::Function(b)) => Rc::ptr_eq(a, b),
        (Value::Builtin(a), Value::Builtin(b)) => a == b,
        _ => false,
    }
}

struct Binding<'a> {
    name: String,
    initial: &'a Value,
    step: Option<&'a Value>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BindingForm {
    Let,
    LetStar,
    Letrec,
    Do,
}

impl fmt::Display for BindingForm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Let => "let",
            Self::LetStar => "let*",
            Self::Letrec => "letrec",
            Self::Do => "do",
        })
    }
}

fn bindings(value: &Value, form: BindingForm) -> Result<Vec<Binding<'_>>> {
    let Value::List(items) = value.datum() else {
        return Err(format!("{form} expects a binding list"));
    };
    let mut names = std::collections::HashSet::new();
    let mut result = Vec::new();
    for item in items {
        let Value::List(parts) = item.datum() else {
            return Err(format!("{form} expects (name value) bindings"));
        };
        if parts.len() != 2 && !(form == BindingForm::Do && parts.len() == 3) {
            return Err(format!(
                "{form} binding needs a name, initializer{}",
                if form == BindingForm::Do {
                    ", and optional step"
                } else {
                    ""
                }
            ));
        }
        let name = symbol(&parts[0])?;
        if !names.insert(name.clone()) && form != BindingForm::LetStar {
            return Err(format!("Duplicate {form} binding: {name}"));
        }
        result.push(Binding {
            name,
            initial: &parts[1],
            step: parts.get(2),
        });
    }
    Ok(result)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ClauseForm {
    Cond,
    Case,
}

impl fmt::Display for ClauseForm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Cond => "cond",
            Self::Case => "case",
        })
    }
}

enum ClauseTest<'a> {
    Else,
    Expression(&'a Value),
    Datums(&'a [Value]),
}

enum ClauseBody<'a> {
    TestValue,
    Sequence(&'a [Value]),
    Recipient(&'a Value),
}

struct Clause<'a> {
    test: ClauseTest<'a>,
    body: ClauseBody<'a>,
}

fn clauses(args: &[Value], form: ClauseForm) -> Result<Vec<Clause<'_>>> {
    let mut clauses = Vec::new();
    for (i, arg) in args.iter().enumerate() {
        let Value::List(parts) = arg.datum() else {
            return Err(format!("{form} expects clause lists"));
        };
        let Some(test) = parts.first() else {
            return Err(format!("{form} clause cannot be empty"));
        };
        let otherwise = is_symbol(test, "else");
        if otherwise && (i + 1 != args.len() || parts.len() == 1) {
            return Err(format!("{form} else clause must be last and have a body"));
        }
        if form == ClauseForm::Case
            && !otherwise
            && (!matches!(test.datum(), Value::List(_)) || parts.len() < 2)
        {
            return Err("case clause needs a datum list and a body".into());
        }
        if parts.get(1).is_some_and(|v| is_symbol(v, "=>"))
            && (parts.len() != 3 || (form == ClauseForm::Cond && otherwise))
        {
            return Err(format!("{form} => clause needs a test and one recipient"));
        }
        let test = match (otherwise, form, test.datum()) {
            (true, _, _) => ClauseTest::Else,
            (false, ClauseForm::Case, Value::List(datums)) => ClauseTest::Datums(datums),
            _ => ClauseTest::Expression(test),
        };
        let body = if parts.len() == 1 {
            ClauseBody::TestValue
        } else if is_symbol(&parts[1], "=>") {
            ClauseBody::Recipient(&parts[2])
        } else {
            ClauseBody::Sequence(&parts[1..])
        };
        clauses.push(Clause { test, body });
    }
    Ok(clauses)
}

fn form(name: &str, args: Vec<Value>) -> Value {
    let mut items = vec![Value::Symbol(name.into())];
    items.extend(args);
    Value::List(items)
}

fn expand_let(bindings: &[Binding<'_>], body: &[Value]) -> Value {
    let params = Value::List(
        bindings
            .iter()
            .map(|b| Value::Symbol(b.name.clone()))
            .collect(),
    );
    let mut lambda = vec![params];
    lambda.extend_from_slice(body);
    let mut call = vec![form("lambda", lambda)];
    call.extend(bindings.iter().map(|b| b.initial.clone()));
    Value::List(call)
}

fn expand_let_star(bindings: &[Binding<'_>], body: &[Value]) -> Result<Value> {
    // Bound generated nesting before constructing an owned recursive syntax tree.
    if bindings.len() > limits::DEPTH {
        return Err("Call depth limit reached. Try a smaller input.".into());
    }
    let mut expression = form("begin", body.to_vec());
    for binding in bindings.iter().rev() {
        expression = expand_let(std::slice::from_ref(binding), &[expression]);
    }
    // Initializers belong to let*'s scope, including the first initializer.
    Ok(expand_let(&[], &[expression]))
}

pub struct Evaluation {
    value: Value,
    pub output: String,
}

impl Evaluation {
    pub fn value(&self) -> String {
        self.value.to_string()
    }
}

enum Step {
    Value(Value),
    Eval(Value, Env),
    Call(Value, Vec<Value>),
}

type SourceLoader = dyn FnMut(&str, &str) -> Result<(String, String)>;

pub struct Interpreter {
    env: Env,
    prelude: Env,
    scopes: Vec<Weak<RefCell<Scope>>>,
    modules: HashMap<String, HashMap<String, Value>>,
    sources: HashMap<String, String>,
    loader: Option<Box<SourceLoader>>,
    loading: Vec<String>,
    steps: usize,
    output: String,
    output_bytes: usize,
}

impl Interpreter {
    pub fn new() -> Self {
        let env = scope(None);
        for name in [
            "+",
            "-",
            "*",
            "/",
            "=",
            "<",
            ">",
            "<=",
            ">=",
            "not",
            "list",
            "car",
            "cdr",
            "cons",
            "append",
            "length",
            "null?",
            "number?",
            "list?",
            "display",
            "newline",
            "map",
            "filter",
            "apply",
            "modulo",
            "abs",
            "symbols",
            "equal?",
            "eq?",
            "boolean?",
            "string?",
            "symbol?",
            "procedure?",
            "string-length",
            "string-append",
            "substring",
            "string-ref",
            "string=?",
            "string->symbol",
            "symbol->string",
            "number->string",
            "string->number",
            "error",
        ] {
            env.borrow_mut()
                .values
                .insert(name.into(), Value::Builtin(name.into()));
        }
        let prelude = scope(None);
        prelude.borrow_mut().values = env.borrow().values.clone();
        Self {
            scopes: vec![Rc::downgrade(&env), Rc::downgrade(&prelude)],
            env,
            prelude,
            modules: HashMap::new(),
            sources: HashMap::new(),
            loader: None,
            loading: Vec::new(),
            steps: 0,
            output: String::new(),
            output_bytes: 0,
        }
    }

    pub fn register_source(&mut self, name: &str, source: &str) -> Result<()> {
        if name.is_empty() || name.len() > 1024 || name.chars().any(char::is_control) {
            return Err(
                "Source filename must contain 1-1024 bytes and no control characters".into(),
            );
        }
        if source.len() > limits::INPUT_BYTES {
            return Err("Program exceeds the 64 KiB input limit".into());
        }
        if !self.sources.contains_key(name) && self.sources.len() >= limits::SOURCE_FILES {
            return Err("Source file limit reached (128 files)".into());
        }
        self.sources.insert(name.into(), source.into());
        Ok(())
    }

    pub fn set_source_loader(
        &mut self,
        loader: impl FnMut(&str, &str) -> Result<(String, String)> + 'static,
    ) {
        self.loader = Some(Box::new(loader));
    }

    fn scope(&mut self, parent: Option<Env>) -> Env {
        let env = scope(parent);
        self.scopes.push(Rc::downgrade(&env));
        env
    }

    fn collect(&mut self, roots: &[Evaluation]) {
        fn trace_value(value: &Value, pending: &mut Vec<Env>) {
            match value {
                Value::Function(function) => pending.push(function.env.clone()),
                Value::List(items) => {
                    for item in items {
                        trace_value(item, pending);
                    }
                }
                Value::Syntax(value, _) => trace_value(value, pending),
                _ => {}
            }
        }
        let mut pending = vec![self.env.clone(), self.prelude.clone()];
        for exports in self.modules.values() {
            for value in exports.values() {
                trace_value(value, &mut pending);
            }
        }
        for root in roots {
            trace_value(&root.value, &mut pending);
        }
        let mut marked = HashSet::new();
        while let Some(env) = pending.pop() {
            if !marked.insert(Rc::as_ptr(&env) as usize) {
                continue;
            }
            let current = env.borrow();
            if let Some(parent) = &current.parent {
                pending.push(parent.clone());
            }
            for value in current.values.values() {
                trace_value(value, &mut pending);
            }
        }
        // Hold every candidate while severing cycles, so weak entries cannot disappear mid-sweep.
        let candidates: Vec<_> = self.scopes.iter().filter_map(Weak::upgrade).collect();
        for env in &candidates {
            if !marked.contains(&(Rc::as_ptr(env) as usize)) {
                let mut scope = env.borrow_mut();
                scope.values.clear();
                scope.parent = None;
            }
        }
        drop(candidates);
        self.scopes.retain(|env| env.strong_count() > 0);
    }

    pub fn execute(&mut self, source: &str) -> Result<Vec<Evaluation>> {
        self.execute_named(source, "<input>")
    }

    pub fn is_incomplete(source: &str) -> bool {
        Self::read(source, "<input>").err().is_some_and(|error| {
            error.starts_with("Unclosed list")
                || error.starts_with("Unclosed string")
                || error.starts_with("Expected an expression")
                || error.starts_with("Unfinished string escape")
        })
    }

    pub fn execute_named(&mut self, source: &str, name: &str) -> Result<Vec<Evaluation>> {
        if source.len() > limits::INPUT_BYTES {
            return Err("Program exceeds the 64 KiB input limit".into());
        }
        self.steps = 0;
        self.output.clear();
        self.output_bytes = 0;
        let result = self.execute_source(source, name);
        self.loading.clear();
        match &result {
            Ok(values) => self.collect(values),
            Err(_) => self.collect(&[]),
        }
        result
    }

    fn read(source: &str, name: &str) -> Result<Vec<Value>> {
        if source.len() > limits::INPUT_BYTES {
            return Err("Program exceeds the 64 KiB input limit".into());
        }
        let tokens = tokenize(source, name)?;
        let mut pos = 0;
        let mut expressions = Vec::new();
        while pos < tokens.len() {
            expressions.push(parse(&tokens, &mut pos, 0)?);
        }
        Ok(expressions)
    }

    fn execute_source(&mut self, source: &str, name: &str) -> Result<Vec<Evaluation>> {
        let expressions = Self::read(source, name)?;
        let mut evaluations = Vec::new();
        for expression in expressions {
            let value = self.eval(&expression, self.env.clone(), 0)?;
            self.reserve_output(value.to_string().len() + 1)?;
            evaluations.push(Evaluation {
                value,
                output: std::mem::take(&mut self.output),
            });
        }
        Ok(evaluations)
    }

    fn reserve_output(&mut self, bytes: usize) -> Result<()> {
        if bytes > limits::OUTPUT_BYTES.saturating_sub(self.output_bytes) {
            return Err("Output limit reached".into());
        }
        self.output_bytes += bytes;
        Ok(())
    }

    fn write_output(&mut self, text: &str) -> Result<()> {
        self.reserve_output(text.len())?;
        self.output.push_str(text);
        Ok(())
    }

    fn tick(&mut self, depth: usize) -> Result<()> {
        self.steps += 1;
        if self.steps > limits::EVALUATION_STEPS {
            return Err("Evaluation limit reached. Try a smaller program.".into());
        }
        if depth > limits::DEPTH {
            return Err("Call depth limit reached. Try a smaller input.".into());
        }
        Ok(())
    }

    fn sequence(&mut self, body: &[Value], env: Env, depth: usize) -> Result<Step> {
        let Some((last, before)) = body.split_last() else {
            return Ok(Step::Value(Value::List(vec![])));
        };
        for expression in before {
            self.eval(expression, env.clone(), depth + 1)?;
        }
        Ok(Step::Eval(last.clone(), env))
    }

    fn quasiquote(&mut self, value: &Value, level: usize, env: Env, depth: usize) -> Result<Value> {
        self.tick(depth)?;
        let Value::List(items) = value.datum() else {
            return Ok(value.quoted());
        };
        if let Some(Value::Symbol(name)) = items.first().map(Value::datum) {
            if ["quasiquote", "unquote", "unquote-splicing"].contains(&name.as_str()) {
                arity(name, &items[1..], 1)?;
                if name == "unquote" && level == 1 {
                    return self.eval(&items[1], env, depth + 1);
                }
                if name == "unquote-splicing" && level == 1 {
                    return Err("unquote-splicing is only valid inside a quasiquoted list".into());
                }
                let next_level = if name == "quasiquote" {
                    level + 1
                } else {
                    level - 1
                };
                return Ok(Value::List(vec![
                    items[0].quoted(),
                    self.quasiquote(&items[1], next_level, env, depth + 1)?,
                ]));
            }
        }
        let mut result = Vec::new();
        for item in items {
            if let Value::List(parts) = item.datum() {
                if level == 1
                    && parts
                        .first()
                        .is_some_and(|v| is_symbol(v, "unquote-splicing"))
                {
                    arity("unquote-splicing", &parts[1..], 1)?;
                    let Value::List(values) = self.eval(&parts[1], env.clone(), depth + 1)? else {
                        return Err("unquote-splicing expects a list".into());
                    };
                    if result.len() + values.len() > limits::LIST_ITEMS {
                        return Err("List size limit reached".into());
                    }
                    result.extend(values);
                    continue;
                }
            }
            if result.len() >= limits::LIST_ITEMS {
                return Err("List size limit reached".into());
            }
            result.push(self.quasiquote(item, level, env.clone(), depth + 1)?);
        }
        Ok(Value::List(result))
    }

    fn eval_let(
        &mut self,
        form: BindingForm,
        args: &[Value],
        env: Env,
        depth: usize,
    ) -> Result<Step> {
        if args.len() < 2 {
            return Err(format!("{form} needs bindings and a body"));
        }
        let named = form == BindingForm::Let && matches!(args[0].datum(), Value::Symbol(_));
        let offset = usize::from(named);
        if args.len() < offset + 2 {
            return Err("Named let needs a name, bindings, and a body".into());
        }
        let bindings = bindings(&args[offset], form)?;
        let body = &args[offset + 1..];
        if !named {
            match form {
                BindingForm::Let => return Ok(Step::Eval(expand_let(&bindings, body), env)),
                BindingForm::LetStar => {
                    return Ok(Step::Eval(expand_let_star(&bindings, body)?, env))
                }
                BindingForm::Letrec => {}
                BindingForm::Do => unreachable!("do uses eval_do"),
            }
        }
        let local = self.scope(Some(env.clone()));
        if named {
            let values = bindings
                .iter()
                .map(|b| self.eval(b.initial, env.clone(), depth + 1))
                .collect::<Result<Vec<_>>>()?;
            let function = procedure(
                bindings.iter().map(|b| b.name.clone()).collect(),
                body.to_vec(),
                local.clone(),
            );
            local
                .borrow_mut()
                .values
                .insert(symbol(&args[0])?, function.clone());
            return Ok(Step::Call(function, values));
        }
        for binding in &bindings {
            local
                .borrow_mut()
                .values
                .insert(binding.name.clone(), Value::Uninitialized);
        }
        let mut values = Vec::new();
        for binding in &bindings {
            let value = self.eval(binding.initial, local.clone(), depth + 1)?;
            values.push((binding.name.clone(), value));
        }
        local.borrow_mut().values.extend(values);
        self.sequence(body, local, depth)
    }

    fn eval_clauses(
        &mut self,
        form: ClauseForm,
        args: &[Value],
        env: Env,
        depth: usize,
    ) -> Result<Step> {
        let (clauses, key) = match form {
            ClauseForm::Cond => (clauses(args, form)?, None),
            ClauseForm::Case => {
                let Some((key, rest)) = args.split_first() else {
                    return Err("case needs a key expression".into());
                };
                let clauses = clauses(rest, form)?;
                (clauses, Some(self.eval(key, env.clone(), depth + 1)?))
            }
        };
        for clause in clauses {
            let value = match clause.test {
                ClauseTest::Else => key.clone().unwrap_or(Value::Bool(true)),
                ClauseTest::Expression(test) => {
                    let value = self.eval(test, env.clone(), depth + 1)?;
                    if !truthy(&value) {
                        continue;
                    }
                    value
                }
                ClauseTest::Datums(datums) => {
                    let key = key.as_ref().expect("case clauses have a key");
                    if !datums.iter().any(|datum| datum_equal(datum, key)) {
                        continue;
                    }
                    key.clone()
                }
            };
            return match clause.body {
                ClauseBody::TestValue => Ok(Step::Value(value)),
                ClauseBody::Sequence(body) => self.sequence(body, env, depth),
                ClauseBody::Recipient(recipient) => {
                    let recipient = self.eval(recipient, env, depth + 1)?;
                    Ok(Step::Call(recipient, vec![value]))
                }
            };
        }
        Ok(Step::Value(Value::List(vec![])))
    }

    fn eval_do(&mut self, args: &[Value], env: Env, depth: usize) -> Result<Step> {
        if args.len() < 2 {
            return Err("do needs bindings and a termination clause".into());
        }
        let bindings = bindings(&args[0], BindingForm::Do)?;
        let Value::List(termination) = args[1].datum() else {
            return Err("do expects a termination clause".into());
        };
        if termination.is_empty() {
            return Err("do termination clause needs a test".into());
        }
        let mut local = self.scope(Some(env.clone()));
        for binding in &bindings {
            let value = self.eval(binding.initial, env.clone(), depth + 1)?;
            local
                .borrow_mut()
                .values
                .insert(binding.name.clone(), value);
        }
        loop {
            self.tick(depth)?;
            if truthy(&self.eval(&termination[0], local.clone(), depth + 1)?) {
                return self.sequence(&termination[1..], local, depth);
            }
            for expression in &args[2..] {
                self.eval(expression, local.clone(), depth + 1)?;
            }
            let mut next = Vec::new();
            for binding in &bindings {
                let value = match binding.step {
                    Some(step) => self.eval(step, local.clone(), depth + 1)?,
                    None => lookup(&local, &binding.name)?,
                };
                next.push((binding.name.clone(), value));
            }
            local = self.scope(Some(env.clone()));
            local.borrow_mut().values.extend(next);
        }
    }

    fn eval_module(&mut self, args: &[Value], _env: Env, depth: usize) -> Result<Step> {
        if args.len() < 2 {
            return Err("module needs a name, (export name ...), and optional body".into());
        }
        let name = symbol(&args[0])?;
        if self.modules.contains_key(&name) {
            return Err(format!("Module already defined: {name}"));
        }
        let Value::List(exports) = args[1].datum() else {
            return Err("module needs an (export name ...) declaration".into());
        };
        if !exports.first().is_some_and(|v| is_symbol(v, "export")) {
            return Err("module needs an (export name ...) declaration".into());
        }
        let exports = parameters(&Value::List(exports[1..].to_vec()))?;
        let local = self.scope(Some(self.prelude.clone()));
        for expression in &args[2..] {
            self.eval(expression, local.clone(), depth + 1)?;
        }
        // A nested declaration may have claimed this name during body evaluation.
        if self.modules.contains_key(&name) {
            return Err(format!("Module already defined: {name}"));
        }
        let mut values = HashMap::new();
        for export in exports {
            let value = local
                .borrow()
                .values
                .get(&export)
                .cloned()
                .ok_or_else(|| format!("Module {name} exports undefined binding: {export}"))?;
            values.insert(export, value);
        }
        self.modules.insert(name.clone(), values);
        Ok(Step::Value(Value::Symbol(name)))
    }

    fn eval_import(&mut self, args: &[Value], env: Env) -> Result<Step> {
        if args.len() < 2 {
            return Err("import needs a module name and explicit binding names".into());
        }
        let name = symbol(&args[0])?;
        let exports = self
            .modules
            .get(&name)
            .ok_or_else(|| format!("Unknown module: {name}. Load its source first."))?;
        let names = parameters(&Value::List(args[1..].to_vec()))?;
        let mut values = Vec::new();
        for key in names {
            if env.borrow().values.contains_key(&key) {
                return Err(format!("Import would overwrite binding: {key}"));
            }
            let value = exports
                .get(&key)
                .ok_or_else(|| format!("Module {name} does not export: {key}"))?;
            values.push((key, value.clone()));
        }
        env.borrow_mut().values.extend(values);
        Ok(Step::Value(Value::List(vec![])))
    }

    fn eval(&mut self, expression: &Value, env: Env, depth: usize) -> Result<Value> {
        self.finish(Step::Eval(expression.clone(), env), depth)
    }

    fn finish(&mut self, mut step: Step, depth: usize) -> Result<Value> {
        let mut origin = None;
        let mut current = None;
        let mut origin_context = String::new();
        let mut current_context = String::new();
        loop {
            let result = match step {
                Step::Value(value) => return Ok(value),
                Step::Eval(expression, env) => {
                    if let Value::Syntax(_, location) = &expression {
                        current_context = match expression.datum() {
                            Value::List(items) => items
                                .first()
                                .and_then(|head| match head.datum() {
                                    Value::Symbol(name) => Some(format!(" in {name}")),
                                    _ => None,
                                })
                                .unwrap_or_default(),
                            _ => String::new(),
                        };
                        if origin.is_none() {
                            origin = Some(location.clone());
                            origin_context = current_context.clone();
                        }
                        current = Some(location.clone());
                    }
                    self.eval_step(&expression, env, depth)
                }
                Step::Call(function, args) => self.call_step(function, args, depth),
            };
            step = result.map_err(|mut error| {
                if let Some(location) = &current {
                    error = location.context(error, &current_context);
                }
                if let Some(location) = &origin {
                    if current.as_ref().is_some_and(|last| {
                        last.source != location.source
                            || last.line != location.line
                            || last.column != location.column
                    }) {
                        error = location.context(error, &origin_context);
                    }
                }
                error
            })?;
        }
    }

    fn eval_step(&mut self, expression: &Value, env: Env, depth: usize) -> Result<Step> {
        self.tick(depth)?;
        let items = match expression.datum() {
            Value::Symbol(name) => return lookup(&env, name).map(Step::Value),
            Value::List(items) if !items.is_empty() => items,
            _ => return Ok(Step::Value(expression.quoted())),
        };
        let args = &items[1..];
        if let Value::Symbol(name) = items[0].datum() {
            match name.as_str() {
                "quote" => {
                    arity(name, args, 1)?;
                    return Ok(Step::Value(args[0].quoted()));
                }
                "quasiquote" => {
                    arity(name, args, 1)?;
                    return self
                        .quasiquote(&args[0], 1, env, depth + 1)
                        .map(Step::Value);
                }
                "unquote" | "unquote-splicing" => {
                    return Err(format!("{name} is only valid inside quasiquote"));
                }
                "set!" => {
                    arity(name, args, 2)?;
                    let key = symbol(&args[0])?;
                    let value = self.eval(&args[1], env.clone(), depth + 1)?;
                    assign(&env, &key, value)?;
                    return Ok(Step::Value(Value::List(vec![])));
                }
                "module" => return self.eval_module(args, env, depth),
                "import" => return self.eval_import(args, env),
                "load" => {
                    arity(name, args, 1)?;
                    let Value::Text(path) = self.eval(&args[0], env.clone(), depth + 1)? else {
                        return Err("load expects a source filename string".into());
                    };
                    let caller = match expression {
                        Value::Syntax(_, location) => location.source.as_ref(),
                        _ => "<input>",
                    };
                    let (name, source) = if let Some(source) = self.sources.get(&path) {
                        (path.clone(), source.clone())
                    } else if let Some(loader) = &mut self.loader {
                        loader(&path, caller)?
                    } else {
                        return Err(format!(
                            "Source not registered: {path}. Open the file before loading it."
                        ));
                    };
                    if self.loading.len() >= 32 || self.loading.contains(&name) {
                        return Err(format!("Cyclic or excessively nested load: {name}"));
                    }
                    let expressions = Self::read(&source, &name)?;
                    self.loading.push(name);
                    let result = (|| {
                        let mut value = Value::List(vec![]);
                        for expression in expressions {
                            value = self.eval(&expression, env.clone(), depth + 1)?;
                        }
                        Ok(Step::Value(value))
                    })();
                    self.loading.pop();
                    return result;
                }
                "cond" => return self.eval_clauses(ClauseForm::Cond, args, env, depth),
                "case" => return self.eval_clauses(ClauseForm::Case, args, env, depth),
                "do" => return self.eval_do(args, env, depth),
                "when" | "unless" => {
                    if args.is_empty() {
                        return Err(format!("{name} needs a test"));
                    }
                    let body = form("begin", args[1..].to_vec());
                    let empty = Value::List(vec![]);
                    let branches = if name == "when" {
                        vec![body, empty]
                    } else {
                        vec![empty, body]
                    };
                    let mut conditional = vec![args[0].clone()];
                    conditional.extend(branches);
                    return Ok(Step::Eval(form("if", conditional), env));
                }
                "if" => {
                    arity(name, args, 3)?;
                    let condition = self.eval(&args[0], env.clone(), depth + 1)?;
                    return Ok(Step::Eval(
                        args[if truthy(&condition) { 1 } else { 2 }].clone(),
                        env,
                    ));
                }
                "define" => {
                    if args.len() < 2 {
                        return Err("define needs a name and a value or body".into());
                    }
                    let (key, value) = match args[0].datum() {
                        Value::Symbol(key) => {
                            arity(name, args, 2)?;
                            (key.clone(), self.eval(&args[1], env.clone(), depth + 1)?)
                        }
                        Value::List(signature) if !signature.is_empty() => (
                            symbol(&signature[0])?,
                            procedure(
                                parameters(&Value::List(signature[1..].to_vec()))?,
                                args[1..].to_vec(),
                                env.clone(),
                            ),
                        ),
                        _ => return Err("define needs a symbol or function signature".into()),
                    };
                    env.borrow_mut().values.insert(key.clone(), value);
                    return Ok(Step::Value(Value::Symbol(key)));
                }
                "lambda" => {
                    if args.len() < 2 {
                        return Err("lambda needs parameters and a body".into());
                    }
                    return Ok(Step::Value(procedure(
                        parameters(&args[0])?,
                        args[1..].to_vec(),
                        env,
                    )));
                }
                "begin" => return self.sequence(args, env, depth),
                "let" => return self.eval_let(BindingForm::Let, args, env, depth),
                "let*" => return self.eval_let(BindingForm::LetStar, args, env, depth),
                "letrec" => return self.eval_let(BindingForm::Letrec, args, env, depth),
                "and" | "or" => {
                    let Some((last, before)) = args.split_last() else {
                        return Ok(Step::Value(Value::Bool(name == "and")));
                    };
                    for arg in before {
                        let result = self.eval(arg, env.clone(), depth + 1)?;
                        if truthy(&result) == (name == "or") {
                            return Ok(Step::Value(result));
                        }
                    }
                    return Ok(Step::Eval(last.clone(), env));
                }
                _ => {}
            }
        }
        let function = self.eval(&items[0], env.clone(), depth + 1)?;
        let values = args
            .iter()
            .map(|arg| self.eval(arg, env.clone(), depth + 1))
            .collect::<Result<Vec<_>>>()?;
        Ok(Step::Call(function, values))
    }

    fn call(&mut self, function: Value, args: Vec<Value>, depth: usize) -> Result<Value> {
        self.finish(Step::Call(function, args), depth)
    }

    fn call_step(&mut self, function: Value, args: Vec<Value>, depth: usize) -> Result<Step> {
        self.tick(depth)?;
        let name = match function {
            Value::Function(function) => {
                if function.params.len() != args.len() {
                    return Err(format!(
                        "Procedure expects {} arguments, got {}",
                        function.params.len(),
                        args.len()
                    ));
                }
                let local = self.scope(Some(function.env.clone()));
                local
                    .borrow_mut()
                    .values
                    .extend(function.params.iter().cloned().zip(args));
                return self.sequence(&function.body, local, depth);
            }
            Value::Builtin(name) => name,
            _ => return Err("First item in a call must be a procedure".into()),
        };
        if name == "apply" {
            arity(&name, &args, 2)?;
            let Value::List(list) = &args[1] else {
                return Err("apply expects a list".into());
            };
            return Ok(Step::Call(args[0].clone(), list.clone()));
        }
        self.builtin(&name, args, depth).map(Step::Value)
    }

    fn builtin(&mut self, name: &str, args: Vec<Value>, depth: usize) -> Result<Value> {
        let n = name;
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
            "symbols" => {
                arity(n, &args, 0)?;
                let mut names: Vec<_> = self.env.borrow().values.keys().cloned().collect();
                if names.len() > limits::LIST_ITEMS {
                    return Err("List size limit reached".into());
                }
                names.sort();
                Ok(Value::List(names.into_iter().map(Value::Symbol).collect()))
            }
            "list" => {
                if args.len() > limits::LIST_ITEMS {
                    return Err("List size limit reached".into());
                }
                Ok(Value::List(args))
            }
            "equal?" | "eq?" => {
                arity(n, &args, 2)?;
                // Lists and strings are immutable values; both predicates compare them by value.
                let equal = datum_equal(&args[0], &args[1]);
                Ok(Value::Bool(equal))
            }
            "not" | "null?" | "number?" | "list?" | "boolean?" | "string?" | "symbol?"
            | "procedure?" => {
                arity(n, &args, 1)?;
                Ok(Value::Bool(match n {
                    "not" => !truthy(&args[0]),
                    "null?" => matches!(&args[0], Value::List(l) if l.is_empty()),
                    "number?" => matches!(args[0], Value::Number(_)),
                    "list?" => matches!(args[0], Value::List(_)),
                    "boolean?" => matches!(args[0], Value::Bool(_)),
                    "string?" => matches!(args[0], Value::Text(_)),
                    "symbol?" => matches!(args[0], Value::Symbol(_)),
                    _ => matches!(args[0], Value::Function(..) | Value::Builtin(_)),
                }))
            }
            "string-length" | "string-append" | "substring" | "string-ref" | "string=?"
            | "string->symbol" | "symbol->string" | "number->string" | "string->number" => {
                self.string_builtin(n, &args)
            }
            "error" => {
                if args.is_empty() {
                    return Err("error needs a message string and optional values".into());
                }
                let Value::Text(message) = &args[0] else {
                    return Err("error expects a message string".into());
                };
                let details = args[1..]
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" ");
                Err(if details.is_empty() {
                    message.clone()
                } else {
                    format!("{message}: {details}")
                })
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
                if result.len() > limits::LIST_ITEMS {
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
                self.write_output(&text)?;
                Ok(Value::List(vec![]))
            }
            "newline" => {
                arity(n, &args, 0)?;
                self.write_output("\n")?;
                Ok(Value::List(vec![]))
            }
            "map" | "filter" => {
                arity(n, &args, 2)?;
                let Value::List(list) = &args[1] else {
                    return Err(format!("{n} expects a list"));
                };
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

    fn string_builtin(&mut self, name: &str, args: &[Value]) -> Result<Value> {
        fn text(value: &Value) -> Result<&str> {
            if let Value::Text(text) = value {
                Ok(text)
            } else {
                Err("Expected a string".into())
            }
        }
        fn index(value: &Value) -> Result<usize> {
            match value {
                Value::Number(n)
                    if n.is_finite()
                        && *n >= 0.0
                        && n.fract() == 0.0
                        && *n <= limits::OUTPUT_BYTES as f64 =>
                {
                    Ok(*n as usize)
                }
                _ => Err("String index must be a nonnegative integer within the string".into()),
            }
        }
        match name {
            "string-append" => {
                let mut result = String::new();
                for arg in args {
                    let part = text(arg)?;
                    if result.len() + part.len() > limits::OUTPUT_BYTES {
                        return Err("String size limit reached (128 KiB)".into());
                    }
                    result.push_str(part);
                }
                Ok(Value::Text(result))
            }
            "string=?" => {
                if args.len() < 2 {
                    return Err("string=? needs at least two strings".into());
                }
                let strings = args.iter().map(text).collect::<Result<Vec<_>>>()?;
                Ok(Value::Bool(
                    strings.windows(2).all(|pair| pair[0] == pair[1]),
                ))
            }
            "substring" | "string-ref" => {
                arity(name, args, if name == "substring" { 3 } else { 2 })?;
                let chars: Vec<_> = text(&args[0])?.chars().collect();
                let start = index(&args[1])?;
                let end = if name == "substring" {
                    index(&args[2])?
                } else {
                    start + 1
                };
                if start > end || end > chars.len() {
                    return Err("String index out of bounds".into());
                }
                Ok(Value::Text(chars[start..end].iter().collect()))
            }
            _ => {
                arity(name, args, 1)?;
                match name {
                    "string-length" => Ok(Value::Number(text(&args[0])?.chars().count() as f64)),
                    "string->symbol" => {
                        let name = text(&args[0])?;
                        let parsed = Self::read(name, "<symbol>")?;
                        if parsed.len() != 1
                            || !matches!(parsed[0].datum(), Value::Symbol(symbol) if symbol == name)
                        {
                            return Err("string->symbol expects one readable symbol name".into());
                        }
                        Ok(Value::Symbol(name.into()))
                    }
                    "symbol->string" => Ok(Value::Text(symbol(&args[0])?)),
                    "number->string" => match &args[0] {
                        Value::Number(number) => Ok(Value::Text(number.to_string())),
                        _ => Err("number->string expects a number".into()),
                    },
                    "string->number" => {
                        let number = text(&args[0])?
                            .parse::<f64>()
                            .map_err(|_| "string->number expects a valid number".to_string())?;
                        if !number.is_finite() {
                            return Err("Numeric result is not finite".into());
                        }
                        Ok(Value::Number(number))
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Interpreter {
    fn drop(&mut self) {
        let candidates: Vec<_> = self.scopes.iter().filter_map(Weak::upgrade).collect();
        for env in &candidates {
            let mut scope = env.borrow_mut();
            scope.values.clear();
            scope.parent = None;
        }
    }
}

thread_local! {
    static RUNTIME: RefCell<Interpreter> = RefCell::new(Interpreter::new());
    static INPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static SOURCE_NAME: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static OUTPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

fn json_string(text: &str) -> String {
    use fmt::Write;
    let mut json = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => json.push_str("\\\""),
            '\\' => json.push_str("\\\\"),
            '\n' => json.push_str("\\n"),
            '\r' => json.push_str("\\r"),
            '\t' => json.push_str("\\t"),
            c if c < '\u{20}' => write!(json, "\\u{:04x}", c as u32).unwrap(),
            _ => json.push(c),
        }
    }
    json.push('"');
    json
}

fn serialize_evaluations(result: Result<Vec<Evaluation>>) -> String {
    match result {
        Ok(evaluations) => {
            let entries: Vec<_> = evaluations
                .iter()
                .map(|evaluation| {
                    format!(
                        "{{\"value\":{},\"output\":{}}}",
                        json_string(&evaluation.value.to_string()),
                        json_string(&evaluation.output),
                    )
                })
                .collect();
            format!("{{\"evaluations\":[{}],\"error\":null}}", entries.join(","))
        }
        Err(error) => format!("{{\"evaluations\":[],\"error\":{}}}", json_string(&error)),
    }
}

#[no_mangle]
pub extern "C" fn input_limit() -> usize {
    limits::INPUT_BYTES
}

#[no_mangle]
pub extern "C" fn source_limit() -> usize {
    limits::SOURCE_FILES
}

#[no_mangle]
pub extern "C" fn input_buffer(len: usize) -> *mut u8 {
    if len > limits::INPUT_BYTES {
        return std::ptr::null_mut();
    }
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        input.resize(len, 0);
        input.as_mut_ptr()
    })
}

#[no_mangle]
pub extern "C" fn evaluate() -> i32 {
    let result = INPUT.with(|input| match std::str::from_utf8(&input.borrow()) {
        Ok(source) => RUNTIME.with(|runtime| runtime.borrow_mut().execute(source)),
        Err(_) => Err("Input must be UTF-8".into()),
    });
    let failed = result.is_err();
    let text = serialize_evaluations(result);
    OUTPUT.with(|output| *output.borrow_mut() = text.into_bytes());
    i32::from(failed)
}

#[no_mangle]
pub extern "C" fn source_name_buffer(len: usize) -> *mut u8 {
    if len > 1024 {
        return std::ptr::null_mut();
    }
    SOURCE_NAME.with(|name| {
        let mut name = name.borrow_mut();
        name.resize(len, 0);
        name.as_mut_ptr()
    })
}

fn named_request(register: bool) -> i32 {
    let result = SOURCE_NAME.with(|name| {
        INPUT.with(|input| {
            let name = name.borrow();
            let name = std::str::from_utf8(&name)
                .map_err(|_| "Source filename must be UTF-8".to_string())?;
            if name.is_empty() || name.chars().any(char::is_control) {
                return Err(
                    "Source filename must not be empty or contain control characters".into(),
                );
            }
            let input = input.borrow();
            let source =
                std::str::from_utf8(&input).map_err(|_| "Input must be UTF-8".to_string())?;
            RUNTIME.with(|runtime| {
                let mut runtime = runtime.borrow_mut();
                if register {
                    runtime.register_source(name, source)?;
                    Ok(vec![])
                } else {
                    runtime.execute_named(source, name)
                }
            })
        })
    });
    let failed = result.is_err();
    OUTPUT.with(|output| *output.borrow_mut() = serialize_evaluations(result).into_bytes());
    i32::from(failed)
}

#[no_mangle]
pub extern "C" fn evaluate_named() -> i32 {
    named_request(false)
}

#[no_mangle]
pub extern "C" fn register_source() -> i32 {
    named_request(true)
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

    impl Interpreter {
        // Legacy presentation assertions remain fixtures; formatting is no longer part of execution.
        fn run(&mut self, source: &str) -> Result<String> {
            let evaluations = self
                .execute(source)
                .map_err(|error| error.lines().next().unwrap_or("").to_string())?;
            let text: String = evaluations
                .iter()
                .map(|evaluation| format!("{}{}\n", evaluation.output, evaluation.value))
                .collect();
            Ok(text.trim_end().to_string())
        }
    }

    fn run(source: &str) -> String {
        Interpreter::new().run(source).unwrap()
    }

    #[test]
    fn execution_returns_values_and_output_separately() {
        let mut runtime = Interpreter::new();
        let evaluations = runtime
            .execute("(display \"hello\") (+ 1 2) (begin (display \"bye\") (newline) 4)")
            .unwrap();
        assert_eq!(evaluations.len(), 3);
        assert!(matches!(&evaluations[0].value, Value::List(items) if items.is_empty()));
        assert_eq!(evaluations[0].output, "hello");
        assert!(matches!(evaluations[1].value, Value::Number(3.0)));
        assert!(evaluations[1].output.is_empty());
        assert!(matches!(evaluations[2].value, Value::Number(4.0)));
        assert_eq!(evaluations[2].output, "bye\n");
        assert!(runtime.execute("; empty").unwrap().is_empty());
        assert!(runtime.execute("(display \"discard\") missing").is_err());
        let recovered = runtime.execute("7").unwrap();
        assert!(recovered[0].output.is_empty());
    }

    #[test]
    fn derived_forms_preserve_order_scope_and_keyword_names() {
        assert_eq!(
            run("(let ((n 0))
               (let ((a (begin (set! n (+ n 1)) n))
                     (b (begin (set! n (+ n 1)) n)))
                 (list a b n)))
             (let* ((x (define hidden 7)) (y hidden)) y)
             (let ((if 1) (begin 2) (lambda 3))
               (when #t (list if begin lambda)))
             (unless #t missing)"),
            "(1 2 2)\n7\n(1 2 3)\n()"
        );
        let mut runtime = Interpreter::new();
        runtime
            .run("(let* ((x (define private 7))) private)")
            .unwrap();
        assert!(runtime.run("private").is_err());
        assert!(runtime
            .run("(let ((n 0)) (let* ((a (set! n 1)) invalid) n))")
            .is_err());
    }

    #[test]
    fn centralized_limits_cover_input_output_lists_and_depth() {
        let mut runtime = Interpreter::new();
        assert!(runtime
            .execute(&" ".repeat(limits::INPUT_BYTES))
            .unwrap()
            .is_empty());
        assert_eq!(
            runtime
                .execute(&" ".repeat(limits::INPUT_BYTES + 1))
                .err()
                .unwrap(),
            "Program exceeds the 64 KiB input limit"
        );
        assert!(runtime
            .execute(&"é".repeat(limits::INPUT_BYTES / 2 + 1))
            .err()
            .unwrap()
            .contains("input limit"));
        let mut value = Value::Number(1.0);
        for _ in 0..=limits::DEPTH {
            value = Value::List(vec![value]);
        }
        assert_eq!(
            runtime
                .execute(&value.to_string())
                .err()
                .unwrap()
                .lines()
                .next()
                .unwrap(),
            "Reader nesting limit reached"
        );
        let list = Value::List(vec![Value::Number(1.0); limits::LIST_ITEMS]);
        assert!(runtime
            .call(
                Value::Builtin("list".into()),
                if let Value::List(items) = list.clone() {
                    items
                } else {
                    unreachable!()
                },
                0
            )
            .is_ok());
        assert_eq!(
            runtime
                .call(
                    Value::Builtin("cons".into()),
                    vec![Value::Number(0.0), list],
                    0
                )
                .err()
                .unwrap(),
            "List size limit reached"
        );

        runtime.output.clear();
        runtime.output_bytes = 0;
        runtime
            .write_output(&"x".repeat(limits::OUTPUT_BYTES - 1))
            .unwrap();
        runtime
            .call(Value::Builtin("newline".into()), vec![], 0)
            .unwrap();
        assert_eq!(runtime.output_bytes, limits::OUTPUT_BYTES);
        assert_eq!(
            runtime
                .call(Value::Builtin("newline".into()), vec![], 0)
                .err()
                .unwrap(),
            "Output limit reached"
        );
        assert!(runtime.execute("1").is_ok());
        runtime.steps = limits::EVALUATION_STEPS - 1;
        assert!(runtime.tick(0).is_ok());
        assert!(runtime.tick(0).is_err());
        runtime.steps = 0;
        assert!(runtime.tick(limits::DEPTH).is_ok());
        assert!(runtime.tick(limits::DEPTH + 1).is_err());
    }

    #[test]
    fn output_limit_counts_all_top_level_values_and_printed_output() {
        let mut runtime = Interpreter::new();
        runtime.env.borrow_mut().values.insert(
            "chunk".into(),
            Value::Text("x".repeat(limits::OUTPUT_BYTES / 2)),
        );
        assert_eq!(
            runtime
                .execute("(begin (display chunk) (display chunk) 1)")
                .err()
                .unwrap()
                .lines()
                .next()
                .unwrap(),
            "Output limit reached"
        );
        assert_eq!(
            runtime
                .execute("(display chunk) (display chunk)")
                .err()
                .unwrap()
                .lines()
                .next()
                .unwrap(),
            "Output limit reached"
        );
        assert!(runtime.execute("2").is_ok());
    }

    #[test]
    fn generated_nesting_is_bounded_before_building_large_trees() {
        let source = format!("(let* ({}) 1)", vec!["(x 1)"; limits::DEPTH + 1].join(" "));
        assert_eq!(
            Interpreter::new()
                .execute(&source)
                .err()
                .unwrap()
                .lines()
                .next()
                .unwrap(),
            "Call depth limit reached. Try a smaller input."
        );
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
    fn assignment_updates_nearest_lexical_binding() {
        assert_eq!(
            run("(define x 1)
                 (define (global-x) x)
                 (let ((x 10)) (set! x 11) (list x (global-x)))
                 x
                 (set! x 2)
                 (global-x)"),
            "x\nglobal-x\n(11 1)\n1\n()\n2"
        );
        assert_eq!(
            run("(define counter
                   (let ((n 0)) (lambda () (set! n (+ n 1)) n)))
                 (list (counter) (counter) (counter))"),
            "counter\n(1 2 3)"
        );
        assert_eq!(
            Interpreter::new().run("(set! missing 1)").unwrap_err(),
            "Cannot set! unknown symbol: missing"
        );
    }

    #[test]
    fn binding_forms_preserve_scope_and_initialization_rules() {
        assert_eq!(
            run("(let ((x 10)) (let ((x 1) (y x)) (list x y)))
                 (let* ((x 1) (y (+ x 2))) (list x y))
                 (let* ((x 1) (f (lambda () x)) (x 2)) (list (f) x))
                 (let () 3)
                 (let* () 4)
                 (letrec () 5)"),
            "(1 10)\n(1 3)\n(1 2)\n3\n4\n5"
        );
        assert_eq!(
            run("(letrec ((even (lambda (n) (if (= n 0) #t (odd (- n 1)))))
                           (odd (lambda (n) (if (= n 0) #f (even (- n 1))))))
                   (list (even 10) (odd 9)))"),
            "(#t #t)"
        );
        for source in [
            "(letrec ((x x)) x)",
            "(let ((x 10)) (letrec ((x 1) (y x)) y))",
            "(letrec ((x (set! x 1))) x)",
        ] {
            assert!(
                Interpreter::new()
                    .run(source)
                    .unwrap_err()
                    .contains("Uninitialized binding"),
                "{source}"
            );
        }
    }

    #[test]
    fn named_let_is_local_and_initializers_use_outer_scope() {
        assert_eq!(
            run("(let loop ((n 6) (total 1))
                   (if (= n 0) total (loop (- n 1) (* total n))))
                 (let ((loop 7)) (let loop ((x loop)) x))
                 (let loop () 9)"),
            "720\n7\n9"
        );
        let mut runtime = Interpreter::new();
        runtime.run("(let loop ((n 0)) n)").unwrap();
        assert!(runtime.run("loop").is_err());
    }

    #[test]
    fn cond_short_circuits_and_supports_test_values_and_recipients() {
        assert_eq!(
            run("(cond (#f missing) ((+ 2 3)) (else missing))
                 (cond (#f missing) ((list 1 2) => length) (else missing))
                 (cond (#f missing) (else 1 7))
                 (cond)
                 (cond (#f missing))
                 (let ((count 0))
                   (list (cond ((begin (set! count (+ count 1)) 8) => (lambda (n) (+ n 1))))
                         count))"),
            "5\n2\n7\n()\n()\n(9 1)"
        );
    }

    #[test]
    fn case_evaluates_key_once_and_compares_literal_data() {
        assert_eq!(
            run("(case (+ 1 1) ((1 3) missing) ((2 4) 'even) (else missing))
                 (case 'blue ((red) missing) ((blue green) 'color))
                 (case '(1 2) (((1 2)) 'list))
                 (case \"text\" ((\"text\") 'string))
                 (case #f ((#t) missing) ((#f) 'false))
                 (case 99 ((1) missing) (else 'other))
                 (case 99 ((1) missing))
                 (case 99)
                 (case 4 ((4) => (lambda (key) (+ key 1))))
                 (case 4 (else => (lambda (key) (+ key 2))))
                 (let ((count 0))
                   (list (case (begin (set! count (+ count 1)) 2)
                           ((1) missing) ((2) 'found))
                         count))"),
            "even\ncolor\nlist\nstring\nfalse\nother\n()\n()\n5\n6\n(found 1)"
        );
    }

    #[test]
    fn when_and_unless_evaluate_only_the_selected_body() {
        assert_eq!(
            run("(when #t 1 2) (unless #f 3 4)
                 (when #f missing) (unless #t missing)
                 (when #t) (unless #f)
                 (let ((n 0))
                   (when (begin (set! n (+ n 1)) #t) (set! n (+ n 1)))
                   n)"),
            "2\n4\n()\n()\n()\n()\n2"
        );
    }

    #[test]
    fn quasiquote_reads_evaluates_splices_and_tracks_nesting() {
        assert_eq!(
            run("(let ((x 3) (xs '(4 5))) `(a ,x ,@xs ,(+ x 3)))
                 (quasiquote (a (unquote (+ 1 2)) (unquote-splicing (list 4 5))))
                 `()
                 `atom
                 `,(+ 2 3)
                 `(a ,@'() b)
                 (let ((x 10)) `(outer `(inner ,x ,,x)))
                 '(a,b)
                 \",@`\""),
            "(a 3 4 5 6)\n(a 3 4 5)\n()\natom\n5\n(a b)\n\
             (outer (quasiquote (inner (unquote x) (unquote 10))))\n\
             (a (unquote b))\n\",@`\""
        );
        assert_eq!(
            run("(let ((xs '(1 2))) `(a `(b ,@xs ,@,xs)))"),
            "(a (quasiquote (b (unquote-splicing xs) (unquote-splicing (1 2)))))"
        );
    }

    #[test]
    fn do_uses_parallel_initializers_steps_and_fresh_iteration_bindings() {
        assert_eq!(
            run("(do ((i 0 (+ i 1)) (total 0 (+ total i))) ((= i 5) total))
                 (do ((a 1 b) (b 2 a) (i 0 (+ i 1))) ((= i 3) (list a b)))
                 (do ((i 0 (+ i 1)) (fixed 9)) ((= i 2) fixed))
                 (do () (#t 1 2) missing)
                 (do () (#t))
                 (let ((n 0)) (do () ((= n 3) n) (set! n (+ n 1))))
                 (let ((x 7)) (do ((x 1) (y x)) (#t (list x y))))
                 (let ((fs '()))
                   (do ((i 0 (+ i 1))) ((= i 3) (map (lambda (f) (f)) fs))
                     (set! fs (cons (lambda () i) fs))))"),
            "10\n(2 1)\n9\n2\n()\n3\n(1 7)\n(2 1 0)"
        );
        let mut runtime = Interpreter::new();
        assert_eq!(
            runtime.run("(do () (#f))").unwrap_err(),
            "Evaluation limit reached. Try a smaller program."
        );
        assert_eq!(runtime.run("(+ 1 2)").unwrap(), "3");
    }

    #[test]
    fn new_forms_reject_malformed_syntax_and_invalid_unquotes() {
        for source in [
            "(set!)",
            "(set! x)",
            "(set! 1 2)",
            "(set! x 1 2)",
            "(let*)",
            "(let* ())",
            "(let* 1 2)",
            "(letrec)",
            "(letrec ())",
            "(let ((x 1) (x 2)) x)",
            "(letrec ((x 1) (x 2)) x)",
            "(let loop ())",
            "(let loop ((1 2)) 3)",
            "(let ((x)) x)",
            "(let* ((x 1 2)) x)",
            "(letrec (x) x)",
            "(when)",
            "(unless)",
            "(cond ())",
            "(cond #t)",
            "(cond (else 1) (#t 2))",
            "(cond (else))",
            "(cond (#t =>))",
            "(cond (#t => list 1))",
            "(cond (else => list))",
            "(case)",
            "(case 1 ())",
            "(case 1 (1 2))",
            "(case 1 ((1)))",
            "(case 1 (else 1) ((2) 2))",
            "(case 1 ((1) =>))",
            "(do)",
            "(do ())",
            "(do () ())",
            "(do () #t)",
            "(do ((x 1 2 3)) (#t))",
            "(do ((x 1) (x 2)) (#t))",
            "(quasiquote)",
            "(quasiquote 1 2)",
            "`",
            ",",
            ",@",
            ",x",
            ",@x",
            "(unquote 1)",
            "(unquote-splicing '(1))",
            "`,@'(1 2)",
            "`(a ,@1)",
            "`(a (unquote 1 2))",
            "`(a (unquote-splicing))",
        ] {
            assert!(Interpreter::new().run(source).is_err(), "{source}");
        }
    }

    #[test]
    fn malformed_clauses_and_bindings_are_validated_before_side_effects() {
        let mut runtime = Interpreter::new();
        runtime.run("(define n 0)").unwrap();
        for source in [
            "(let ((x (set! n 1)) (x 2)) x)",
            "(letrec ((x (set! n 1)) (x 2)) x)",
            "(let* ((x (set! n 1)) invalid) x)",
            "(cond ((begin (set! n 1) #t) 1) (else))",
            "(case (set! n 1) ((1)))",
            "(do ((x (set! n 1))) ())",
        ] {
            assert!(runtime.run(source).is_err(), "{source}");
            assert_eq!(runtime.run("n").unwrap(), "0", "{source}");
        }
    }

    #[test]
    fn quasiquote_obeys_list_and_evaluation_limits() {
        let mut runtime = Interpreter::new();
        let source = format!("(define xs '({}))", vec!["1"; 6_000].join(" "));
        runtime.run(&source).unwrap();
        assert_eq!(
            runtime.run("`(,@xs ,@xs)").unwrap_err(),
            "List size limit reached"
        );
        assert_eq!(runtime.run("(length xs)").unwrap(), "6000");
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
    fn symbols_lists_sorted_global_bindings() {
        let mut runtime = Interpreter::new();
        runtime.run("(define zebra 1) (define apple 2)").unwrap();
        let mut expected: Vec<_> = runtime.env.borrow().values.keys().cloned().collect();
        expected.sort();
        let result = runtime.run("(symbols)").unwrap();
        assert_eq!(result, format!("({})", expected.join(" ")));
        assert!(expected.contains(&"+".to_string()));
        assert!(expected.contains(&"symbols".to_string()));
        assert!(expected.contains(&"apple".to_string()));
        assert!(!expected.contains(&"define".to_string()));
        assert_eq!(
            runtime
                .run("(let ((local 1)) ((lambda (parameter) (symbols)) 2))")
                .unwrap(),
            result
        );
        assert_eq!(runtime.run("(apply symbols '())").unwrap(), result);
        assert_eq!(
            runtime.run("(symbols 1)").unwrap_err(),
            "symbols expects 0 argument(s), got 1"
        );
        runtime.run("(define apple 3)").unwrap();
        assert_eq!(runtime.run("(symbols)").unwrap(), result);
        let fresh = Interpreter::new().run("(symbols)").unwrap();
        assert!(!fresh.split_whitespace().any(|name| name == "apple"));
    }

    #[test]
    fn persistent_repl() {
        let mut runtime = Interpreter::new();
        runtime.run("(define x 42)").unwrap();
        assert_eq!(runtime.run("(+ x 1)").unwrap(), "43");
        assert!(runtime.run("(+ 1").is_err());
        assert_eq!(runtime.run("x").unwrap(), "42");
    }

    #[test]
    fn proper_tail_calls_cover_every_tail_position() {
        for wrapper in [
            "TAIL",
            "(if #t TAIL missing)",
            "(begin 1 TAIL)",
            "(and #t TAIL)",
            "(or #f TAIL)",
            "(when #t TAIL)",
            "(unless #f TAIL)",
            "(let ((x 1)) TAIL)",
            "(let* ((x 1) (y x)) TAIL)",
            "(letrec ((x 1)) TAIL)",
            "(cond (#f missing) (#t TAIL))",
            "(case 1 ((1) TAIL))",
            "(cond (#t => (lambda (value) TAIL)))",
            "(case 1 ((1) => (lambda (value) TAIL)))",
            "(do () (#t TAIL))",
        ] {
            let body = wrapper.replace("TAIL", "(loop (- n 1))");
            let source = format!("(let loop ((n 500)) (if (= n 0) 7 {body}))");
            assert_eq!(run(&source), "7", "{wrapper}");
        }
        assert_eq!(
            run("(let loop ((n 3000) (sum 0))
            (if (= n 0) sum (apply loop (list (- n 1) (+ sum 1)))))"),
            "3000"
        );
        assert_eq!(
            run("(letrec
            ((even (lambda (n) (if (= n 0) #t (odd (- n 1)))))
             (odd (lambda (n) (if (= n 0) #f (even (- n 1))))))
            (even 3000))"),
            "#t"
        );
        let error = Interpreter::new()
            .execute("(define (grow n) (+ 1 (grow (+ n 1)))) (grow 0)")
            .err()
            .unwrap();
        assert!(error.starts_with("Call depth limit"), "{error}");
        assert!(error.lines().count() <= 14, "{error}");
    }

    #[test]
    fn tracing_collection_reclaims_cycles_and_preserves_live_closures() {
        let mut runtime = Interpreter::new();
        for _ in 0..100 {
            runtime
                .execute("(letrec ((f (lambda () f))) 'done)")
                .unwrap();
            assert_eq!(runtime.scopes.len(), 2);
        }
        runtime
            .execute(
                "(define saved
            (letrec ((count 0) (f (lambda () (set! count (+ count 1)) count)))
                (list f)))",
            )
            .unwrap();
        let live = runtime.scopes.len();
        assert_eq!(runtime.run("((car saved))").unwrap(), "1");
        assert_eq!(runtime.run("((car saved))").unwrap(), "2");
        assert_eq!(runtime.scopes.len(), live);
        assert!(runtime
            .execute("(letrec ((lost (lambda () lost))) (error \"stop\"))")
            .is_err());
        assert_eq!(runtime.scopes.len(), live);
        runtime.execute("(set! saved #f)").unwrap();
        assert_eq!(runtime.scopes.len(), 2);
        runtime.execute("(define (recursive) recursive)").unwrap();
        let scopes = runtime.scopes.clone();
        drop(runtime);
        assert!(scopes.iter().all(|env| env.upgrade().is_none()));
    }

    #[test]
    fn diagnostics_retain_source_locations_across_requests_and_calls() {
        let mut runtime = Interpreter::new();
        runtime
            .execute_named("(define (fail n)\n  (/ n 0))", "library.loop")
            .unwrap();
        let error = runtime
            .execute_named("\n(+ 1 (fail 4))", "main.loop")
            .err()
            .unwrap();
        assert!(error.starts_with("Division by zero"), "{error}");
        assert!(error.contains("library.loop:2:3 in /"), "{error}");
        assert!(error.contains("main.loop:2:6 in fail"), "{error}");
        assert!(error.contains("main.loop:2:1 in +"), "{error}");
        let error = runtime
            .execute_named("(list 'λ missing)", "unicode.loop")
            .err()
            .unwrap();
        assert!(error.contains("unicode.loop:1:10"), "{error}");
        let error = runtime.execute_named("\n (", "broken.loop").err().unwrap();
        assert!(error.contains("broken.loop:2:2"), "{error}");
        assert!(Interpreter::is_incomplete("(list\n"));
        assert!(Interpreter::is_incomplete("'"));
        assert!(Interpreter::is_incomplete("\"unfinished"));
        assert!(!Interpreter::is_incomplete(")"));
    }

    #[test]
    fn named_modules_have_explicit_exports_and_isolated_lexical_scopes() {
        let mut runtime = Interpreter::new();
        assert_eq!(
            runtime
                .run(
                    "(define private 99)
            (module math (export square counter)
                (define private 2)
                (define count 0)
                (define (square n) (* n n))
                (define (counter) (set! count (+ count private)) count))
            (import math square counter)
            (list (square 9) (counter) (counter) private)"
                )
                .unwrap(),
            "private\nmath\n()\n(81 2 4 99)"
        );
        assert_eq!(runtime.run("(counter)").unwrap(), "6");
        for source in [
            "(import math private)",
            "(import math square)",
            "(import absent x)",
            "(module math (export))",
            "(module missing (export unknown))",
            "(module bad (export a a))",
            "(module bad ())",
            "(module bad (export answer) (define answer private))",
        ] {
            assert!(runtime.execute(source).is_err(), "{source}");
        }
        runtime
            .execute("(module other (export b square) (define b 1) (define square 2))")
            .unwrap();
        assert!(runtime.execute("(import other b square)").is_err());
        assert!(
            runtime.execute("b").is_err(),
            "imports must validate atomically"
        );
        assert_eq!(runtime.run("(square 2)").unwrap(), "4");
    }

    #[test]
    fn loading_is_host_controlled_and_shares_budgets_and_diagnostics() {
        let mut runtime = Interpreter::new();
        runtime
            .register_source(
                "math.loop",
                "(module math (export double) (define (double x) (+ x x)))",
            )
            .unwrap();
        assert_eq!(
            runtime
                .run("(load \"math.loop\") (import math double) (double 9)")
                .unwrap(),
            "math\n()\n18"
        );
        assert!(runtime
            .execute("(load \"https://example.com/code.loop\")")
            .is_err());
        runtime
            .register_source("cycle.loop", "(load \"cycle.loop\")")
            .unwrap();
        assert!(runtime
            .execute("(load \"cycle.loop\")")
            .err()
            .unwrap()
            .contains("Cyclic"));
        assert!(runtime.loading.is_empty());
        runtime.register_source("broken.loop", "\n(/ 1 0)").unwrap();
        let error = runtime.execute("(load \"broken.loop\")").err().unwrap();
        assert!(error.contains("broken.loop:2:1 in /"), "{error}");
        assert!(runtime.register_source("bad\nname", "1").is_err());
        assert!(runtime
            .register_source("huge", &" ".repeat(INPUT_LIMIT + 1))
            .is_err());
        runtime.steps = 0;
        runtime
            .register_source("forever", "(let loop () (loop))")
            .unwrap();
        assert!(runtime
            .execute("(load \"forever\")")
            .err()
            .unwrap()
            .starts_with("Evaluation limit"));
        assert_eq!(runtime.run("(double 2)").unwrap(), "4");
    }

    #[test]
    fn equality_predicates_and_unicode_string_library() {
        assert_eq!(run("(equal? '(1 (\"x\" #t)) '(1 (\"x\" #t)))
            (eq? 'a 'a)
            (equal? 1 \"1\")
            (let ((f (lambda () 1))) (list (eq? f f) (eq? f (lambda () 1))))
            (map procedure? (list + (lambda () 1) 3))
            (list (boolean? #f) (string? \"x\") (symbol? 'x) (number? 2) (list? '()))
            (string-length \"aλ😀\")
            (substring \"aλ😀z\" 1 3)
            (string-ref \"aλ😀\" 2)
            (string-append \"a\" \"λ\" \"😀\")
            (string-append)
            (string=? \"λ\" \"λ\" \"λ\")
            (string=? \"a\" \"b\")
            (symbol->string (string->symbol \"hello-world\"))
            (number->string (string->number \"12.5\"))"),
            "#t\n#t\n#f\n(#t #f)\n(#t #t #f)\n(#t #t #t #t #t)\n3\n\"λ😀\"\n\"😀\"\n\"aλ😀\"\n\"\"\n#t\n#f\n\"hello-world\"\n\"12.5\"");
        for source in [
            "(equal? 1)",
            "(string-ref \"a\" 1)",
            "(substring \"a\" 1 0)",
            "(substring \"a\" 0 2)",
            "(string-ref \"a\" -1)",
            "(string-ref \"a\" 0.5)",
            "(string-length 1)",
            "(string-append 1)",
            "(string=? \"a\")",
            "(string->number \"nan\")",
            "(string->number \"no\")",
            "(string->symbol \"two names\")",
            "(string->symbol \"1\")",
            "(number->string #t)",
            "(symbol->string \"a\")",
        ] {
            assert!(Interpreter::new().execute(source).is_err(), "{source}");
        }
        let mut runtime = Interpreter::new();
        let error = runtime.execute("(error \"bad value\" 3 'x)").err().unwrap();
        assert!(error.starts_with("bad value: 3 x"), "{error}");
        assert!(error.contains("in error"), "{error}");
        assert_eq!(runtime.run("(+ 1 2)").unwrap(), "3");
    }
}
