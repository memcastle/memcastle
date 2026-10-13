//! Synthetic source used only to exercise Core's component and plugin lifecycle.

use serde_json::{Value, json};

wit_bindgen::generate!({
    path: "../../../wit",
    world: "source",
});

use exports::memcastle::source::adapter::Guest;
use memcastle::source::types::{
    Candidate, CanonicalDocument, Discovery, RawDocument, Segment, SourceError, SourceKind,
    SourceRef,
};

struct Mock;

const NAME: &str = "mock-wasm";
const DOCUMENTS: [&str; 2] = ["gazebos", "lighthouses"];

impl Guest for Mock {
    fn identify(
        locator: Option<String>,
        options: Vec<(String, String)>,
    ) -> Result<SourceRef, SourceError> {
        if let Some((key, _)) = options.first() {
            return Err(SourceError::InvalidInput(format!(
                "unknown mock option `{key}`"
            )));
        }
        let locator =
            locator.ok_or_else(|| SourceError::InvalidInput("provide a locator".into()))?;
        // Multi-module tests use the same component with independent manifest names, without recompiling a guest.
        let source = match locator.split_once(':') {
            Some((name @ ("first" | "second"), _)) => name,
            _ => NAME,
        };
        Ok(SourceRef {
            source: source.into(),
            account: None,
            locator,
            options: Vec::new(),
        })
    }

    fn default_wing(_source: SourceRef) -> String {
        "mock".into()
    }

    fn default_room() -> String {
        "synthetic".into()
    }

    fn discover(_source: SourceRef, cursor: String, limit: u32) -> Result<Discovery, SourceError> {
        let cursor: Value = serde_json::from_str(&cursor)
            .map_err(|_| SourceError::CursorInvalid("the mock cursor is not JSON".into()))?;
        let after = if cursor.is_null() {
            0
        } else {
            cursor
                .get("after")
                .and_then(Value::as_u64)
                .filter(|value| *value <= DOCUMENTS.len() as u64)
                .ok_or_else(|| {
                    SourceError::CursorInvalid("the mock cursor is not an index".into())
                })? as usize
        };
        let candidates = DOCUMENTS
            .iter()
            .enumerate()
            .skip(after)
            .take(limit as usize)
            .map(|(index, name)| Candidate {
                external_id: (*name).into(),
                cursor_after: json!({ "after": index + 1 }).to_string(),
                handle: (*name).into(),
            })
            .collect::<Vec<_>>();
        let exhausted = after + candidates.len() == DOCUMENTS.len();
        Ok(Discovery {
            candidates,
            exhausted,
        })
    }

    fn read(source: SourceRef, candidate: Candidate) -> Result<Option<RawDocument>, SourceError> {
        // The locator supplies distinct search text without consulting a provider or the host filesystem.
        let body = format!("{} {}", source.locator, candidate.handle);
        Ok(Some(RawDocument {
            external_id: candidate.external_id,
            revision: "1".into(),
            body,
            metadata: "{}".into(),
            occurred_at: None,
        }))
    }

    fn normalize(raw: RawDocument) -> Result<CanonicalDocument, SourceError> {
        Ok(CanonicalDocument {
            title: Some(raw.external_id),
            room: None,
            name: None,
            kind: SourceKind::Other,
            uri: None,
            tags: Vec::new(),
            segments: vec![Segment { text: raw.body }],
        })
    }
}

export!(Mock);
