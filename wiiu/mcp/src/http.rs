//! Small, one-request HTTP/1.1 transport for the console's TLS listener.
//! Keeping connections single-use bounds the effects of TLS record buffering
//! and removes ambiguity about pipelined bytes after an accepted request.
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::{self, BufRead, BufReader, Read, Write};

const HEADER_BYTES: usize = 8192;
const BODY_BYTES: usize = 32 * 1024;
const RESPONSE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug)]
pub struct Request {
    pub method: String,
    pub target: String,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(String::as_str)
    }

    pub fn media_type(&self) -> Option<&str> {
        self.header("content-type")?
            .split(';')
            .next()
            .map(str::trim)
    }
}

pub struct Response {
    pub status: u16,
    pub headers: Vec<(&'static str, String)>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn empty(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    pub fn header(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.headers.push((name, value.into()));
        self
    }

    pub fn bytes(status: u16, media_type: &'static str, body: Vec<u8>) -> Self {
        Self {
            body,
            ..Self::empty(status).header("Content-Type", media_type)
        }
    }

    pub fn json(status: u16, body: Value) -> Self {
        Self::bytes(status, "application/json", body.to_string().into_bytes())
    }
}

#[derive(Debug)]
pub enum ReadError {
    Http(u16),
    Io(io::Error),
}

impl From<io::Error> for ReadError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

fn read_line(input: &mut impl BufRead, left: &mut usize) -> Result<String, ReadError> {
    let mut bytes = Vec::new();
    let n = input.take(*left as u64).read_until(b'\n', &mut bytes)?;
    *left -= n;
    if n == 0 {
        return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into());
    }
    if !bytes.ends_with(b"\r\n") {
        return Err(ReadError::Http(if *left == 0 { 431 } else { 400 }));
    }
    bytes.truncate(n - 2);
    if bytes.iter().any(|b| !matches!(b, b'\t' | b' '..=b'~')) {
        return Err(ReadError::Http(400));
    }
    String::from_utf8(bytes).map_err(|_| ReadError::Http(400))
}

fn token(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_|~".contains(&b) || b == 96)
}

pub fn read(input: impl Read) -> Result<Request, ReadError> {
    let mut input = BufReader::with_capacity(4096, input);
    let mut left = HEADER_BYTES;
    let start = read_line(&mut input, &mut left)?;
    let mut fields = start.split(' ');
    let (Some(method), Some(target), Some("HTTP/1.1"), None) =
        (fields.next(), fields.next(), fields.next(), fields.next())
    else {
        return Err(ReadError::Http(400));
    };
    if !token(method)
        || !target.starts_with('/')
        || target.starts_with("//")
        || target.contains(['#', '\\', '\t'])
        || target.len() > 4096
    {
        return Err(ReadError::Http(400));
    }
    let mut headers = BTreeMap::new();
    loop {
        let header = read_line(&mut input, &mut left)?;
        if header.is_empty() {
            break;
        }
        let (name, value) = header.split_once(':').ok_or(ReadError::Http(400))?;
        if !token(name) || headers.len() >= 48 {
            return Err(ReadError::Http(400));
        }
        if headers
            .insert(name.to_ascii_lowercase(), value.trim().to_owned())
            .is_some()
        {
            return Err(ReadError::Http(400));
        }
    }
    // Accept only the framing used by the native CLI clients. Reject alternate
    // framing, duplicate lengths and Expect before trying to read a body.
    if headers.contains_key("transfer-encoding")
        || headers.contains_key("expect")
        || headers.contains_key("content-encoding")
    {
        return Err(ReadError::Http(400));
    }
    let size = match headers.get("content-length") {
        None if method == "POST" => return Err(ReadError::Http(411)),
        None => 0,
        Some(length) if !length.is_empty() && length.bytes().all(|b| b.is_ascii_digit()) => {
            length.parse::<usize>().map_err(|_| ReadError::Http(413))?
        }
        Some(_) => return Err(ReadError::Http(400)),
    };
    if size > BODY_BYTES {
        return Err(ReadError::Http(413));
    }
    if !headers.contains_key("host")
        || (method != "POST" && size != 0)
        || input.buffer().len() > size
    {
        return Err(ReadError::Http(400));
    }
    let mut body = vec![0; size];
    input.read_exact(&mut body)?;
    Ok(Request {
        method: method.into(),
        target: target.into(),
        headers,
        body,
    })
}

pub fn write(mut output: impl Write, response: Response) -> io::Result<()> {
    let response = if response.body.len() > RESPONSE_BYTES {
        Response::empty(500)
    } else {
        response
    };
    let reason = match response.status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        303 => "See Other",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        406 => "Not Acceptable",
        409 => "Conflict",
        411 => "Length Required",
        413 => "Content Too Large",
        415 => "Unsupported Media Type",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        503 => "Service Unavailable",
        _ => "Internal Server Error",
    };
    write!(output, "HTTP/1.1 {} {reason}\r\nConnection: close\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n", response.status, response.body.len())?;
    for (name, value) in &response.headers {
        if !token(name) || value.bytes().any(|b| !matches!(b, b'\t' | b' '..=b'~')) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid response header",
            ));
        }
        write!(output, "{name}: {value}\r\n")?;
    }
    output.write_all(b"\r\n")?;
    output.write_all(&response.body)?;
    output.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_one_request_and_rejects_ambiguous_framing() {
        let post = "POST /mcp HTTP/1.1\r\nHost: console:8443\r\n";
        let request = read(format!("{post}Content-Length: 2\r\n\r\n{{}}").as_bytes()).unwrap();
        assert_eq!(request.body, b"{}");
        assert_eq!(request.header("host"), Some("console:8443"));
        for (tail, expected) in [
            ("Content-Length: 0\r\ncontent-length: 1\r\n\r\n", 400),
            ("Content-Length: +1\r\n\r\nx", 400),
            ("Content-Length: 0\r\n\r\nx", 400),
            (
                "Transfer-Encoding: chunked\r\nContent-Length: 0\r\n\r\n",
                400,
            ),
            ("Content-Length: 32769\r\n\r\n", 413),
            ("Expect: 100-continue\r\n\r\n", 400),
            ("\r\n", 411),
        ] {
            let error = read(format!("{post}{tail}").as_bytes()).unwrap_err();
            assert!(
                matches!(error, ReadError::Http(status) if status == expected),
                "{error:?}"
            );
        }
    }
}
