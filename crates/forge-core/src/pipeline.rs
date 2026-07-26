//! The shareable pipeline file.
//!
//! Deterministic, JSON-serialisable, and the unit of virality: the same
//! document runs on the desktop harness and on the phone.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::graph::Graph;
use crate::CoreError;

/// Serde form of a shareable pipeline file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineDoc {
    pub version: u32,
    pub name: String,
    pub graph: Graph,
}

impl PipelineDoc {
    /// Parse a document from JSON text.
    pub fn from_json(text: &str) -> Result<Self, CoreError> {
        Ok(serde_json::from_str(text)?)
    }

    /// Render the document as pretty JSON.
    pub fn to_json(&self) -> Result<String, CoreError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Read a document from a file.
    pub fn from_path(path: &Path) -> Result<Self, CoreError> {
        Self::from_json(&std::fs::read_to_string(path)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::probe_device;
    use crate::graph::{NodeId, NodeKind};
    use crate::validate::validate_graph;
    use std::path::PathBuf;

    fn fixture_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("tests")
            .join("fixtures")
            .join("podcast-cleanup.json")
    }

    fn fixture() -> PipelineDoc {
        PipelineDoc::from_path(&fixture_path()).expect("read the podcast cleanup fixture")
    }

    #[test]
    fn the_fixture_parses_to_the_documented_pipeline() {
        let doc = fixture();
        assert_eq!(doc.version, 1);
        assert_eq!(doc.name, "Podcast cleanup");

        let kinds: Vec<NodeKind> = doc.graph.nodes.iter().map(|n| n.kind).collect();
        assert_eq!(
            kinds,
            vec![
                NodeKind::SourceVideo,
                NodeKind::AudioSplit,
                NodeKind::AudioDenoise,
                NodeKind::Transcribe,
                NodeKind::SinkFiles,
            ]
        );

        let denoise = doc
            .graph
            .node(&NodeId::from("dnz"))
            .expect("the denoise node");
        assert_eq!(denoise.model.as_deref(), Some("gtcrn"));
        assert_eq!(
            denoise.params.get("strength").and_then(|v| v.as_f64()),
            Some(0.8)
        );
        assert_eq!(doc.graph.edges.len(), 4);
    }

    #[test]
    fn the_fixture_round_trips_to_an_identical_graph() {
        let doc = fixture();
        let text = doc.to_json().expect("serialise");
        let again = PipelineDoc::from_json(&text).expect("re-parse");

        assert_eq!(again.graph, doc.graph, "the graph must survive a round trip");
        assert_eq!(again, doc);

        // And once more, so the serialised form is itself a fixed point.
        let third = PipelineDoc::from_json(&again.to_json().expect("serialise")).expect("re-parse");
        assert_eq!(third, doc);
    }

    #[test]
    fn the_fixture_validates_on_a_desktop_profile() {
        let doc = fixture();
        let caps = probe_device().expect("desktop probe");
        assert_eq!(validate_graph(&doc.graph, &caps), Ok(()));
    }

    #[test]
    fn the_fixture_orders_source_before_sink() {
        let order = fixture()
            .graph
            .topological_order()
            .expect("the fixture is acyclic");
        let ids: Vec<&str> = order.iter().map(|id| id.as_str()).collect();
        assert_eq!(ids, vec!["in", "aud", "dnz", "txt", "out"]);
    }
}
