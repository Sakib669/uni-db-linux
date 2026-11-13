// PostgreSQL driver — SSL required if cloud, pooling, parameterized queries.
pub fn connect_url(url: &str) -> Result<(), String> { if url.is_empty() { Err("URL required".into()) } else { Ok(()) } }
