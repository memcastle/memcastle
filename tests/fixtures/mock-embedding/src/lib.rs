//! Synthetic embedding module for Core's capability and runtime checks.

wit_bindgen::generate!({
    path: "../../../wit/memcastle-embedding.wit",
    world: "embedding-provider",
});

use exports::memcastle::embedding::provider::{Guest, Model};

struct Mock;

impl Guest for Mock {
    fn models() -> Vec<Model> {
        vec![Model {
            id: "synthetic".into(),
            dimensions: vec![3],
        }]
    }

    fn embed(
        model: String,
        dimensions: u32,
        settings: String,
        texts: Vec<String>,
    ) -> Result<Vec<Vec<f32>>, String> {
        if model != "synthetic" || dimensions != 3 {
            return Err("unsupported model or dimension".into());
        }
        if settings == "spin" {
            loop {
                std::hint::spin_loop();
            }
        }
        if settings == "memory" {
            let mut oversized = vec![1u8; 100 * 1024 * 1024];
            oversized[0] = 2;
            std::hint::black_box(oversized);
        }
        if let Some(port) = settings.strip_prefix("tcp:") {
            let port: u16 = port.parse().map_err(|_| "invalid port".to_string())?;
            std::net::TcpStream::connect(("127.0.0.1", port))
                .map_err(|_| "network refused".to_string())?;
        }
        if settings.contains("require_token") {
            let _token = memcastle::embedding::host::access_token()?;
        }
        Ok(texts
            .iter()
            .map(|text| vec![text.len() as f32, 0.0, 1.0])
            .collect())
    }
}

export!(Mock);
