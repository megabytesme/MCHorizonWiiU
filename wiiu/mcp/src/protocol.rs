use crate::http::{Request, Response};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};

const MODERN: &str = "2026-07-28";
const LEGACY: &str = "2025-06-18";
const VERSIONS: [&str; 2] = [MODERN, LEGACY];
const VERSION: &str = "io.modelcontextprotocol/protocolVersion";
const CAPABILITIES: &str = "io.modelcontextprotocol/clientCapabilities";
const SERVER_INFO: &str = "io.modelcontextprotocol/serverInfo";

pub struct Fault {
    pub status: u16,
    pub code: i64,
    pub message: &'static str,
    pub data: Option<Value>,
}

impl Fault {
    pub fn new(status: u16, code: i64, message: &'static str) -> Self {
        Self {
            status,
            code,
            message,
            data: None,
        }
    }

    pub fn reply(self, id: Value) -> Response {
        let mut error = json!({"code": self.code, "message": self.message});
        if let Some(data) = self.data {
            error["data"] = data;
        }
        Response::json(
            self.status,
            json!({"jsonrpc": "2.0", "id": id, "error": error}),
        )
    }
}

pub trait Tools {
    fn list(&self) -> Vec<Value>;
    fn call(&self, name: &str, arguments: &Value) -> Result<Value, Fault>;
}

fn server_info() -> Value {
    json!({"name": "horizon-wiiu", "version": env!("CARGO_PKG_VERSION")})
}

fn unsupported(version: &str) -> Fault {
    Fault {
        data: Some(json!({"requested": version, "supported": VERSIONS})),
        ..Fault::new(400, -32022, "Unsupported protocol version")
    }
}

fn header_mismatch(field: &'static str) -> Fault {
    Fault {
        data: Some(json!({"header": field})),
        ..Fault::new(400, -32020, "Header mismatch")
    }
}

fn name_matches(header: Option<&str>, body: Option<&str>) -> bool {
    match (header, body) {
        (Some(header), Some(body)) => {
            if let Some(encoded) = header
                .strip_prefix("=?base64?")
                .and_then(|s| s.strip_suffix("?="))
            {
                STANDARD
                    .decode(encoded)
                    .is_ok_and(|bytes| bytes == body.as_bytes())
            } else {
                header == body
            }
        }
        (None, None) => true,
        _ => false,
    }
}

fn modern_headers(request: &Request, method: &str, params: &Value) -> Result<(), Fault> {
    let header = request.header("mcp-protocol-version");
    let body = params["_meta"][VERSION].as_str();
    let version = match (header, body) {
        (Some(header), Some(body)) if header == body => header,
        _ => return Err(header_mismatch("MCP-Protocol-Version")),
    };
    if version != MODERN {
        return Err(unsupported(version));
    }
    if request.header("mcp-method") != Some(method) {
        return Err(header_mismatch("Mcp-Method"));
    }
    let name = match method {
        "tools/call" | "prompts/get" => params["name"].as_str(),
        "resources/read" => params["uri"].as_str(),
        _ => None,
    };
    if !name_matches(request.header("mcp-name"), name) {
        return Err(header_mismatch("Mcp-Name"));
    }
    if !params["_meta"][CAPABILITIES].is_object() {
        return Err(Fault::new(
            400,
            -32602,
            "Client capabilities must be an object",
        ));
    }
    Ok(())
}

pub fn dispatch(request: &Request, tools: &impl Tools) -> Response {
    if request.method == "GET" || request.method == "DELETE" {
        return Response::empty(405).header("Allow", "POST");
    }
    if request.method != "POST" {
        return Response::empty(405).header("Allow", "POST");
    }
    if !request
        .media_type()
        .is_some_and(|t| t.eq_ignore_ascii_case("application/json"))
    {
        return Response::empty(415);
    }
    if let Some(accept) = request.header("accept") {
        if !accept.split(',').any(|part| {
            let mut values = part.split(';');
            matches!(
                values.next().map(str::trim),
                Some("application/json" | "application/*" | "*/*")
            ) && !values.any(|v| {
                v.trim()
                    .strip_prefix("q=")
                    .is_some_and(|q| q.parse::<f32>() == Ok(0.0))
            })
        }) {
            return Response::empty(406);
        }
    }
    let message = match serde_json::from_slice::<Value>(&request.body) {
        Ok(v) => v,
        Err(_) => return Fault::new(400, -32700, "Parse error").reply(Value::Null),
    };
    let id = message.get("id").cloned();
    if message["jsonrpc"] != "2.0"
        || id
            .as_ref()
            .is_some_and(|id| !id.is_string() && !id.is_i64() && !id.is_u64())
        || !message["method"].is_string()
    {
        return Fault::new(400, -32600, "Invalid Request").reply(Value::Null);
    }
    let method = message["method"].as_str().unwrap_or_default();
    let empty = json!({});
    let params = message.get("params").unwrap_or(&empty);
    if !params.is_object() {
        return Fault::new(400, -32602, "Request params must be an object")
            .reply(id.unwrap_or(Value::Null));
    }
    let header = request.header("mcp-protocol-version");
    let modern = params["_meta"].get(VERSION).is_some()
        || request.header("mcp-method").is_some()
        || header == Some(MODERN)
        || method == "server/discover";
    let version = if modern {
        modern_headers(request, method, params)
    } else if matches!(header, None | Some(LEGACY)) {
        Ok(())
    } else {
        Err(unsupported(header.unwrap_or_default()))
    };
    if let Err(fault) = version {
        return fault.reply(id.unwrap_or(Value::Null));
    }
    let Some(id) = id else {
        return if !modern
            && matches!(
                method,
                "notifications/initialized" | "notifications/cancelled"
            ) {
            Response::empty(202)
        } else {
            Response::empty(400)
        };
    };

    let capabilities = json!({"tools": {}});
    let instructions = "Manage Horizon Night and control a live Horizon game when the player enables Agent Control.";
    let result = match method {
        "server/discover" if modern => Ok(json!({
            "supportedVersions": VERSIONS, "capabilities": capabilities,
            "instructions": instructions, "ttlMs": 3600000, "cacheScope": "public",
        })),
        "initialize"
            if !modern
                && params["protocolVersion"].is_string()
                && params["capabilities"].is_object()
                && params["clientInfo"].is_object() =>
        {
            Ok(
                json!({"protocolVersion": LEGACY, "capabilities": capabilities,
                "serverInfo": server_info(), "instructions": instructions}),
            )
        }
        "initialize" if !modern => {
            Err(Fault::new(400, -32602, "Invalid initialization parameters"))
        }
        "ping" if !modern => Ok(json!({})),
        "tools/list" if !params["cursor"].is_null() => {
            Err(Fault::new(400, -32602, "Invalid list cursor"))
        }
        "tools/list" => {
            let mut result = json!({"tools": tools.list()});
            if modern {
                result["ttlMs"] = json!(0);
                result["cacheScope"] = json!("private");
            }
            Ok(result)
        }
        "tools/call" => match params["name"].as_str() {
            Some(name) if !name.is_empty() => {
                let args = params.get("arguments").unwrap_or(&empty);
                if args.is_object() {
                    tools.call(name, args)
                } else {
                    Err(Fault::new(400, -32602, "Tool arguments must be an object"))
                }
            }
            _ => Err(Fault::new(400, -32602, "Tool name is required")),
        },
        _ => Err(Fault::new(
            if modern { 404 } else { 200 },
            -32601,
            "Method not found",
        )),
    };
    match result {
        Ok(mut result) => {
            if modern {
                result["resultType"] = json!("complete");
                result["_meta"][SERVER_INFO] = server_info();
            }
            Response::json(200, json!({"jsonrpc": "2.0", "id": id, "result": result}))
        }
        Err(error) => error.reply(id),
    }
}

pub fn text_result(text: &str, is_error: bool) -> Value {
    json!({"content": [{"type": "text", "text": text}], "isError": is_error})
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestTools;
    impl Tools for TestTools {
        fn list(&self) -> Vec<Value> {
            vec![json!({"name": "game_state", "inputSchema": {"type": "object"}})]
        }

        fn call(&self, name: &str, _arguments: &Value) -> Result<Value, Fault> {
            assert_eq!(name, "game_state");
            Ok(json!({
                "content": [{"type": "text", "text": "{\"screen\":\"playing\"}"}],
                "structuredContent": {"screen": "playing"},
                "isError": false,
            }))
        }
    }

    fn request(method: &str, mut params: Value, version: Option<&str>) -> Request {
        let mut req = Request {
            method: "POST".into(),
            target: "/mcp".into(),
            headers: [("content-type".into(), "application/json".into())].into(),
            body: Vec::new(),
        };
        if let Some(version) = version {
            req.headers
                .insert("mcp-protocol-version".into(), version.into());
            req.headers.insert("mcp-method".into(), method.into());
            if let Some(name) = params["name"].as_str() {
                req.headers.insert("mcp-name".into(), name.into());
            }
            params["_meta"][VERSION] = json!(version);
            params["_meta"][CAPABILITIES] = json!({});
        }
        req.body = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params})
            .to_string()
            .into_bytes();
        req
    }

    fn answer(req: &Request, status: u16) -> Value {
        let response = dispatch(req, &TestTools);
        assert_eq!(response.status, status);
        serde_json::from_slice(&response.body).unwrap()
    }

    #[test]
    fn lists_and_calls_without_modern_initialization_or_connection_state() {
        let result = answer(&request("tools/list", json!({}), Some(MODERN)), 200);
        assert_eq!(result["result"]["ttlMs"], 0);
        assert_eq!(result["result"]["cacheScope"], "private");
        let result = answer(
            &request("tools/call", json!({"name": "game_state"}), Some(MODERN)),
            200,
        );
        assert_eq!(result["result"]["structuredContent"]["screen"], "playing");
        assert_eq!(result["result"]["resultType"], "complete");
        assert_eq!(
            result["result"]["_meta"][SERVER_INFO]["name"],
            "horizon-wiiu"
        );
        let result = answer(&request("server/discover", json!({}), Some(MODERN)), 200);
        assert_eq!(result["result"]["supportedVersions"], json!(VERSIONS));
        assert_eq!(
            answer(&request("initialize", json!({}), Some(MODERN)), 404)["error"]["code"],
            -32601
        );
    }

    #[test]
    fn validates_modern_header_mirrors_and_version() {
        let mut req = request("tools/call", json!({"name": "game_state"}), Some(MODERN));
        req.headers.remove("mcp-name");
        assert_eq!(answer(&req, 400)["error"]["code"], -32020);
        req.headers
            .insert("mcp-name".into(), "=?base64?Z2FtZV9zdGF0ZQ==?=".into());
        assert_eq!(answer(&req, 200)["result"]["isError"], false);
        assert_eq!(
            answer(
                &request("server/discover", json!({}), Some("1900-01-01")),
                400
            )["error"]["code"],
            -32022
        );
    }

    #[test]
    fn legacy_handshake_and_notifications_have_no_modern_metadata() {
        let response = answer(
            &request(
                "initialize",
                json!({
                    "protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "client", "version": "1"}
                }),
                None,
            ),
            200,
        );
        assert_eq!(response["result"]["protocolVersion"], LEGACY);
        assert!(response["result"]["resultType"].is_null());
        let mut req = request("notifications/initialized", json!({}), None);
        req.body = br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.to_vec();
        let response = dispatch(&req, &TestTools);
        assert_eq!(response.status, 202);
        assert!(response.body.is_empty());
        req.method = "GET".into();
        assert_eq!(dispatch(&req, &TestTools).status, 405);
    }
}
