use loop_lisp::{HostRequest, HostResponse, HOST_TEXT_LIMIT};
use std::{fs, io::Read, time::Duration};

pub fn handle(request: HostRequest) -> Result<HostResponse, String> {
    let response = match request {
        HostRequest::ReadFile { path } => {
            let mut body = String::new();
            fs::File::open(&path)
                .map_err(|error| format!("{path}: {error}"))?
                .take(HOST_TEXT_LIMIT as u64 + 1)
                .read_to_string(&mut body)
                .map_err(|error| format!("{path}: {error}"))?;
            HostResponse {
                status: None,
                headers: vec![],
                body,
            }
        }
        HostRequest::WriteFile { path, contents } => {
            fs::write(&path, contents).map_err(|error| format!("{path}: {error}"))?;
            HostResponse {
                status: None,
                headers: vec![],
                body: String::new(),
            }
        }
        HostRequest::Http {
            method,
            url,
            headers,
            body,
        } => {
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .http_status_as_error(false)
                .timeout_global(Some(Duration::from_secs(3)))
                .max_redirects(5)
                .build()
                .into();
            let mut request = ureq::http::Request::builder()
                .method(method.as_str())
                .uri(&url);
            for (name, value) in headers {
                request = request.header(name, value);
            }
            let request = request
                .body(body)
                .map_err(|error| format!("HTTP: {error}"))?;
            let mut response = agent
                .run(request)
                .map_err(|error| format!("HTTP: {error}"))?;
            let status = Some(response.status().as_u16());
            let mut headers = Vec::new();
            for (name, value) in response.headers() {
                headers.push((
                    name.to_string(),
                    value
                        .to_str()
                        .map_err(|error| format!("HTTP header: {error}"))?
                        .to_string(),
                ));
            }
            let mut body = String::new();
            response
                .body_mut()
                .as_reader()
                .take(HOST_TEXT_LIMIT as u64 + 1)
                .read_to_string(&mut body)
                .map_err(|error| format!("HTTP body: {error}"))?;
            HostResponse {
                status,
                headers,
                body,
            }
        }
    };
    if response.body.len() > HOST_TEXT_LIMIT {
        return Err("Host response size limit reached (64 KiB)".into());
    }
    Ok(response)
}
