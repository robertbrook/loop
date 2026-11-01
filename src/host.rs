use super::*;
use serde::{Deserialize, Serialize};

pub const HOST_TEXT_LIMIT: usize = limits::INPUT_BYTES;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum HostRequest {
    ReadFile {
        path: String,
    },
    WriteFile {
        path: String,
        contents: String,
    },
    Http {
        method: String,
        url: String,
        headers: Vec<(String, String)>,
        body: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HostResponse {
    pub status: Option<u16>,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

type HostHandler = dyn FnMut(HostRequest) -> std::result::Result<HostResponse, String>;

pub(super) type Handler = Box<HostHandler>;

impl Interpreter {
    pub fn set_host_handler(
        &mut self,
        handler: impl FnMut(HostRequest) -> std::result::Result<HostResponse, String> + 'static,
    ) {
        self.host = Some(Box::new(handler));
    }

    pub(super) fn host_builtin(
        &mut self,
        builtin: Builtin,
        args: Vec<Value>,
        _depth: usize,
    ) -> Result<Step> {
        fn text(value: &Value) -> Result<&str> {
            match value {
                Value::Text(value) => Ok(value),
                _ => Err("Expected a string".into()),
            }
        }
        builtin.arguments().check(builtin.name(), &args)?;
        let request = match builtin {
            ReadFile | WriteFile => {
                let path = text(&args[0])?;
                if path.is_empty() || path.len() > 1024 || path.chars().any(char::is_control) {
                    return Err(
                        "File path must contain 1-1024 bytes and no control characters".into(),
                    );
                }
                if builtin == ReadFile {
                    HostRequest::ReadFile { path: path.into() }
                } else {
                    let contents = text(&args[1])?;
                    if contents.len() > HOST_TEXT_LIMIT {
                        return Err("File size limit reached (64 KiB)".into());
                    }
                    HostRequest::WriteFile {
                        path: path.into(),
                        contents: contents.into(),
                    }
                }
            }
            HttpRequest => {
                let method = text(&args[0])?;
                if !["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"].contains(&method) {
                    return Err("Unsupported HTTP method; use uppercase GET, POST, PUT, PATCH, DELETE, HEAD, or OPTIONS".into());
                }
                let url = text(&args[1])?;
                if url.len() > 8192 {
                    return Err("HTTP URL size limit reached (8192 bytes)".into());
                }
                if !(url.starts_with("https://") || url.starts_with("http://"))
                    || url.chars().any(char::is_control)
                {
                    return Err("HTTP URL must use http:// or https://".into());
                }
                let Value::List(pairs) = &args[2] else {
                    return Err("HTTP headers must be a list of (name value) string pairs".into());
                };
                if pairs.len() > 128 {
                    return Err("HTTP request header limit reached".into());
                }
                let mut headers = Vec::new();
                let mut header_bytes = 0;
                for pair in pairs {
                    let Value::List(pair) = pair else {
                        return Err("HTTP headers need (name value) pairs".into());
                    };
                    if pair.len() != 2 {
                        return Err("HTTP headers need (name value) pairs".into());
                    }
                    let name = text(&pair[0])?;
                    let value = text(&pair[1])?;
                    header_bytes += name.len() + value.len();
                    if header_bytes > HOST_TEXT_LIMIT {
                        return Err("HTTP request header size limit reached (64 KiB)".into());
                    }
                    if name.is_empty()
                        || !name
                            .bytes()
                            .all(|c| c.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&c))
                        || value.chars().any(|c| c.is_control() && c != '\t')
                    {
                        return Err("Invalid HTTP header".into());
                    }
                    headers.push((name.into(), value.into()));
                }
                let body = text(&args[3])?;
                if body.len() > HOST_TEXT_LIMIT {
                    return Err("HTTP body size limit reached (64 KiB)".into());
                }
                if ["GET", "HEAD"].contains(&method) && !body.is_empty() {
                    return Err("GET and HEAD requests must have an empty body".into());
                }
                HostRequest::Http {
                    method: method.into(),
                    url: url.into(),
                    headers,
                    body: body.into(),
                }
            }
            _ => unreachable!(),
        };
        let written = match &request {
            HostRequest::WriteFile { path, contents } => Some((path.clone(), contents.clone())),
            _ => None,
        };
        let response = self
            .host
            .as_mut()
            .ok_or("Host file and HTTP access is not configured")?(request)?;
        if let Some((path, contents)) = written {
            if self.sources.contains_key(&path) {
                self.sources.insert(path, contents);
            }
        }
        if response.body.len() > HOST_TEXT_LIMIT {
            return Err("Host response size limit reached (64 KiB)".into());
        }
        let value = if builtin == HttpRequest {
            let status = response
                .status
                .filter(|status| (100..=599).contains(status))
                .ok_or("Host returned an invalid HTTP status")?;
            if response.headers.len() > 128 {
                return Err("HTTP response header limit reached".into());
            }
            if response
                .headers
                .iter()
                .map(|(key, value)| key.len() + value.len())
                .sum::<usize>()
                > HOST_TEXT_LIMIT
            {
                return Err("HTTP response header size limit reached (64 KiB)".into());
            }
            let headers = response
                .headers
                .into_iter()
                .map(|(key, value)| Value::List(vec![Value::Text(key), Value::Text(value)]))
                .collect();
            Value::List(vec![
                Value::Symbol("response".into()),
                Value::Number(status as f64),
                Value::List(headers),
                Value::Text(response.body),
            ])
        } else if builtin == WriteFile {
            Value::List(vec![])
        } else {
            Value::Text(response.body)
        };
        Ok(Step::Value(value))
    }
}

#[cfg(target_arch = "wasm32")]
pub(super) fn default_handler() -> Option<Handler> {
    Some(Box::new(wasm_request))
}

#[cfg(target_arch = "wasm32")]
pub(super) fn wasm_request(request: HostRequest) -> std::result::Result<HostResponse, String> {
    #[link(wasm_import_module = "env")]
    unsafe extern "C" {
        fn loop_host_request(pointer: *const u8, length: usize) -> usize;
        fn loop_host_response(pointer: *mut u8, length: usize) -> usize;
    }
    let request = serde_json::to_vec(&request).map_err(|error| error.to_string())?;
    let length = unsafe { loop_host_request(request.as_ptr(), request.len()) };
    if length == 0 || length > limits::OUTPUT_BYTES * 8 {
        return Err("Invalid or oversized host response".into());
    }
    let mut bytes = vec![0; length];
    let copied = unsafe { loop_host_response(bytes.as_mut_ptr(), bytes.len()) };
    if copied != length {
        return Err("Incomplete host response".into());
    }
    #[derive(Deserialize)]
    struct Reply {
        response: Option<HostResponse>,
        error: Option<String>,
    }
    let reply: Reply = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Invalid host response: {error}"))?;
    match (reply.response, reply.error) {
        (Some(response), None) => Ok(response),
        (None, Some(error)) => Err(error),
        _ => Err("Host response must contain either a response or an error".into()),
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn default_handler() -> Option<Handler> {
    None
}
