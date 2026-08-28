use std::marker::PhantomData;
use std::sync::Arc;

pub enum Static {}
pub enum Dynamic {}

pub struct RequestBuilder<T> {
    method: &'static str,
    service: &'static str,
    path: &'static str,
    headers: Vec<(&'static str, &'static str)>,
    query: Vec<(&'static str, &'static str)>,
    body: Option<serde_json::Value>,
    extract: Option<&'static str>,
    name: Option<&'static str>,
    raw_lua: &'static str,
    _markers: PhantomData<T>,
}

pub const DEFAULT_TEST_METHOD: &str = "GET";
pub const DEFAULT_TEST_SERVICE: &str = "api";
pub const DEFAULT_TEST_PATH: &str = "/test";

impl Default for RequestBuilder<Static> {
    fn default() -> Self {
        Self {
            method: DEFAULT_TEST_METHOD,
            service: DEFAULT_TEST_SERVICE,
            path: DEFAULT_TEST_PATH,
            headers: Vec::new(),
            query: Vec::new(),
            body: None,
            extract: None,
            name: None,
            raw_lua: "",
            _markers: PhantomData,
        }
    }
}

impl RequestBuilder<Static> {
    pub fn r#static(method: &'static str, service: &'static str, path: &'static str) -> Self {
        RequestBuilder {
            method,
            service,
            path,
            headers: Vec::new(),
            query: Vec::new(),
            body: None,
            extract: None,
            name: None,
            raw_lua: "",
            _markers: PhantomData,
        }
    }
}

impl RequestBuilder<Dynamic> {
    pub fn dynamic(raw: &'static str) -> Self {
        RequestBuilder {
            method: "",
            service: "",
            path: "",
            headers: Vec::new(),
            query: Vec::new(),
            body: None,
            extract: None,
            name: None,
            raw_lua: raw,
            _markers: PhantomData,
        }
    }
}

impl<T> RequestBuilder<T> {
    pub fn with_name(self, name: &'static str) -> RequestBuilder<T> {
        RequestBuilder {
            name: Some(name),
            ..self
        }
    }
}

fn json_to_lua_literal(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "nil".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => format!(r#""{s}""#),
        serde_json::Value::Array(items) => {
            let items = items
                .iter()
                .map(json_to_lua_literal)
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{ {items} }}")
        }
        serde_json::Value::Object(map) => {
            let entries = map
                .iter()
                .map(|(key, value)| format!(r#"["{key}"] = {}"#, json_to_lua_literal(value)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{ {entries} }}")
        }
    }
}

impl RequestBuilder<Static> {
    pub fn with_method(self, method: &'static str) -> Self {
        RequestBuilder { method, ..self }
    }
    pub fn with_service(self, service: &'static str) -> Self {
        RequestBuilder { service, ..self }
    }
    pub fn with_path(self, path: &'static str) -> Self {
        RequestBuilder { path, ..self }
    }
    pub fn with_headers(self, h: Vec<(&'static str, &'static str)>) -> Self {
        RequestBuilder { headers: h, ..self }
    }

    pub fn with_query(self, q: Vec<(&'static str, &'static str)>) -> Self {
        RequestBuilder { query: q, ..self }
    }

    pub fn with_body(self, b: serde_json::Value) -> Self {
        RequestBuilder {
            body: Some(b),
            ..self
        }
    }

    pub fn with_extract(self, lua: &'static str) -> Self {
        RequestBuilder {
            extract: Some(lua),
            ..self
        }
    }
}

impl RequestBuilder<Static> {
    /// The HTTP method of this request.
    pub fn method(&self) -> &'static str {
        self.method
    }

    /// The service this request targets.
    pub fn service(&self) -> &'static str {
        self.service
    }

    /// The path this request targets.
    pub fn path(&self) -> &'static str {
        self.path
    }

    /// The body this request sends
    pub fn body(&self) -> Option<&serde_json::Value> {
        self.body.as_ref()
    }

    /// The headers this request sends
    pub fn headers(&self) -> &[(&'static str, &'static str)] {
        &self.headers
    }

    /// The query this request sends
    pub fn query(&self) -> &[(&'static str, &'static str)] {
        &self.query
    }

    pub fn build(self) -> String {
        let mut parts = Vec::new();

        parts.push("protocol = \"http\"".to_string());
        parts.push(format!("method = \"{}\"", self.method));
        parts.push(format!("service = \"{}\"", self.service));
        parts.push(format!("path = \"{}\"", self.path));

        if !self.headers.is_empty() {
            let headers = self
                .headers
                .into_iter()
                .map(|(k, v)| format!("[\"{}\"] = \"{}\"", k, v))
                .collect::<Vec<_>>()
                .join(", ");
            parts.push(format!("headers = {{ {} }}", headers));
        }

        if !self.query.is_empty() {
            let query = self
                .query
                .into_iter()
                .map(|(k, v)| format!("[\"{}\"] = \"{}\"", k, v))
                .collect::<Vec<_>>()
                .join(", ");
            parts.push(format!("query = {{ {} }}", query));
        }

        if let Some(body) = self.body {
            parts.push(format!("body = {}", json_to_lua_literal(&body)));
        }

        if let Some(extract) = self.extract {
            parts.push(format!("extract = {}", extract));
        }

        let spec = format!("{{ {} }}", parts.join(", "));

        if let Some(name) = self.name {
            format!("{{ name = {:?}, spec = {} }}", name, spec)
        } else {
            spec
        }
    }
}

impl RequestBuilder<Dynamic> {
    pub fn build(self) -> String {
        let raw = self.raw_lua;

        if let Some(name) = self.name {
            format!("{{ name = {:?}, spec = {} }}", name, raw)
        } else {
            raw.to_string()
        }
    }
}

pub struct ScriptBuilder {
    setup: Vec<String>,
    requests: Vec<String>,
}

impl Default for ScriptBuilder {
    fn default() -> Self {
        Self {
            setup: Default::default(),
            requests: vec![RequestBuilder::default().build()],
        }
    }
}

impl ScriptBuilder {
    pub fn new() -> Self {
        Self {
            setup: Vec::new(),
            requests: Vec::new(),
        }
    }

    pub fn with_setup(mut self, entries: &[String]) -> Self {
        self.setup = entries.to_vec();
        self
    }

    pub fn with_requests(mut self, entries: &[String]) -> Self {
        self.requests = entries.to_vec();
        self
    }

    pub fn build(self) -> Arc<str> {
        let setup_entries = self.setup.join(", ");

        let request_entries = self.requests.join(",");

        let script = format!(
            r#"
local function setup()
    return {{{setup_entries}}}
end

local function requests()
    return {{{request_entries}}}
end

return {{ setup = setup, requests = requests }}
"#
        );

        Arc::from(script.trim())
    }
}
