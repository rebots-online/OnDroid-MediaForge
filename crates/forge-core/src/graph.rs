//! The pipeline graph model: typed ports, nodes, edges, and the cycle-detecting
//! execution order.
//!
//! [`ports_for`] is the single source of port typing for the whole product —
//! the validator, the editor palette and the scheduler all read node shape from
//! it and nowhere else.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

use crate::validate::ValidationError;

/// Typed port discriminant. Six types, each carrying its own hue and geometry
/// in the frozen design system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PortType {
    Audio,
    Video,
    Image,
    Mask,
    Text,
    Tensor,
}

/// Stable node identity within a graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl NodeId {
    /// Borrow the identity as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for NodeId {
    fn from(s: &str) -> Self {
        NodeId(s.to_string())
    }
}

impl From<String> for NodeId {
    fn from(s: String) -> Self {
        NodeId(s)
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Which of the v1 node types a [`NodeSpec`] instantiates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum NodeKind {
    SourceVideo,
    SourceImage,
    SourceAudio,
    AudioSplit,
    AudioDenoise,
    AudioIsolateVoice,
    AudioStems,
    Transcribe,
    Diarize,
    ImageUpscale,
    ImageObjectRemove,
    GenerativeFill,
    ImageCutout,
    VideoUpscale,
    VideoRemoveBg,
    VideoInterpolate,
    MetadataGen,
    CaptionFrames,
    MaskHelper,
    AvMux,
    SinkGallery,
    SinkFiles,
}

impl NodeKind {
    /// Every variant, in declaration order. The palette and `cmd_availability`
    /// enumerate the node set from here so a new kind cannot be forgotten.
    pub const ALL: [NodeKind; 22] = [
        NodeKind::SourceVideo,
        NodeKind::SourceImage,
        NodeKind::SourceAudio,
        NodeKind::AudioSplit,
        NodeKind::AudioDenoise,
        NodeKind::AudioIsolateVoice,
        NodeKind::AudioStems,
        NodeKind::Transcribe,
        NodeKind::Diarize,
        NodeKind::ImageUpscale,
        NodeKind::ImageObjectRemove,
        NodeKind::GenerativeFill,
        NodeKind::ImageCutout,
        NodeKind::VideoUpscale,
        NodeKind::VideoRemoveBg,
        NodeKind::VideoInterpolate,
        NodeKind::MetadataGen,
        NodeKind::CaptionFrames,
        NodeKind::MaskHelper,
        NodeKind::AvMux,
        NodeKind::SinkGallery,
        NodeKind::SinkFiles,
    ];
}

/// One input or output socket on a node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Port {
    pub name: String,
    pub ty: PortType,
}

/// A node instance with its parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeSpec {
    pub id: NodeId,
    pub kind: NodeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default)]
    pub params: serde_json::Map<String, serde_json::Value>,
}

/// A typed connection from one node's output port to another's input port.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub from: (NodeId, String),
    pub to: (NodeId, String),
}

/// The pipeline itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Graph {
    pub nodes: Vec<NodeSpec>,
    pub edges: Vec<Edge>,
}

impl Graph {
    /// Look a node up by identity.
    pub fn node(&self, id: &NodeId) -> Option<&NodeSpec> {
        self.nodes.iter().find(|n| &n.id == id)
    }

    /// Execution order by Kahn's algorithm.
    ///
    /// Returns [`ValidationError::Cycle`] carrying every node id still holding a
    /// non-zero in-degree when the queue drains — that residue is exactly the
    /// set of nodes participating in, or fed by, the cycle.
    ///
    /// Edges naming a node id that is not in `nodes` cannot constrain the order
    /// of nodes that are, so they are skipped here; the editor cannot author
    /// one.
    pub fn topological_order(&self) -> Result<Vec<NodeId>, ValidationError> {
        let known: HashSet<&NodeId> = self.nodes.iter().map(|n| &n.id).collect();

        let mut indegree: HashMap<&NodeId, usize> =
            self.nodes.iter().map(|n| (&n.id, 0usize)).collect();
        let mut successors: HashMap<&NodeId, Vec<&NodeId>> =
            self.nodes.iter().map(|n| (&n.id, Vec::new())).collect();

        for e in &self.edges {
            if !known.contains(&e.from.0) || !known.contains(&e.to.0) {
                continue;
            }
            successors.get_mut(&e.from.0).expect("known node").push(&e.to.0);
            *indegree.get_mut(&e.to.0).expect("known node") += 1;
        }

        let mut queue: VecDeque<&NodeId> = self
            .nodes
            .iter()
            .map(|n| &n.id)
            .filter(|id| indegree[id] == 0)
            .collect();

        let mut order: Vec<NodeId> = Vec::with_capacity(self.nodes.len());
        while let Some(id) = queue.pop_front() {
            order.push(id.clone());
            for succ in &successors[id] {
                let d = indegree.get_mut(succ).expect("known node");
                *d -= 1;
                if *d == 0 {
                    queue.push_back(succ);
                }
            }
        }

        if order.len() != self.nodes.len() {
            let stuck: Vec<NodeId> = self
                .nodes
                .iter()
                .filter(|n| indegree[&n.id] > 0)
                .map(|n| n.id.clone())
                .collect();
            return Err(ValidationError::Cycle(stuck));
        }
        Ok(order)
    }
}

fn p(name: &str, ty: PortType) -> Port {
    Port {
        name: name.to_string(),
        ty,
    }
}

/// Every [`PortType`], used to declare a sink socket that accepts any media
/// class. A sink port is polymorphic: it appears once per accepted type under a
/// single name, so an edge type-checks when *some* declaration under that name
/// matches the producing port.
fn any_port(name: &str) -> Vec<Port> {
    [
        PortType::Audio,
        PortType::Video,
        PortType::Image,
        PortType::Mask,
        PortType::Text,
        PortType::Tensor,
    ]
    .into_iter()
    .map(|ty| p(name, ty))
    .collect()
}

/// The declared input and output ports of a node kind, as `(inputs, outputs)`.
///
/// This is the single source of port typing. Every declared input port *name*
/// is required: a graph that leaves one unconnected is rejected by
/// `validate_graph`.
pub fn ports_for(kind: NodeKind) -> (Vec<Port>, Vec<Port>) {
    use NodeKind::*;
    use PortType::*;
    match kind {
        SourceVideo => (vec![], vec![p("video", Video), p("audio", Audio)]),
        SourceImage => (vec![], vec![p("image", Image)]),
        SourceAudio => (vec![], vec![p("audio", Audio)]),

        AudioSplit => (vec![p("in", Audio)], vec![p("voice", Audio), p("music", Audio)]),
        AudioDenoise => (vec![p("in", Audio)], vec![p("out", Audio)]),
        AudioIsolateVoice => (vec![p("in", Audio)], vec![p("out", Audio)]),
        AudioStems => (
            vec![p("in", Audio)],
            vec![
                p("vocals", Audio),
                p("drums", Audio),
                p("bass", Audio),
                p("other", Audio),
            ],
        ),

        Transcribe => (vec![p("in", Audio)], vec![p("out", Text)]),
        Diarize => (vec![p("in", Audio)], vec![p("out", Text)]),

        ImageUpscale => (vec![p("in", Image)], vec![p("out", Image)]),
        ImageObjectRemove => (vec![p("in", Image), p("mask", Mask)], vec![p("out", Image)]),
        GenerativeFill => (
            vec![p("in", Image), p("mask", Mask), p("prompt", Text)],
            vec![p("out", Image)],
        ),
        ImageCutout => (vec![p("in", Image)], vec![p("out", Image), p("mask", Mask)]),

        VideoUpscale => (vec![p("in", Video)], vec![p("out", Video)]),
        VideoRemoveBg => (vec![p("in", Video)], vec![p("out", Video), p("mask", Mask)]),
        VideoInterpolate => (vec![p("in", Video)], vec![p("out", Video)]),

        MetadataGen => (vec![p("in", Text)], vec![p("out", Text)]),
        CaptionFrames => (vec![p("in", Video)], vec![p("out", Text)]),
        MaskHelper => (vec![p("in", Image)], vec![p("mask", Mask)]),

        AvMux => (
            vec![p("video", Video), p("audio", Audio)],
            vec![p("out", Video)],
        ),

        SinkGallery => (any_port("in"), vec![]),
        SinkFiles => (any_port("in"), vec![]),
    }
}

#[cfg(test)]
mod port_table_tests {
    use super::*;

    // -- Source nodes: no inputs, typed outputs --

    #[test]
    fn port_table_source_video() {
        let (i, o) = ports_for(NodeKind::SourceVideo);
        assert!(i.is_empty(), "SourceVideo has no inputs");
        assert_eq!(o.len(), 2);
        assert_eq!(o[0], p("video", PortType::Video));
        assert_eq!(o[1], p("audio", PortType::Audio));
    }

    #[test]
    fn port_table_source_image() {
        let (i, o) = ports_for(NodeKind::SourceImage);
        assert!(i.is_empty());
        assert_eq!(o, vec![p("image", PortType::Image)]);
    }

    #[test]
    fn port_table_source_audio() {
        let (i, o) = ports_for(NodeKind::SourceAudio);
        assert!(i.is_empty());
        assert_eq!(o, vec![p("audio", PortType::Audio)]);
    }

    // -- Audio processing nodes --

    #[test]
    fn port_table_audio_split() {
        let (i, o) = ports_for(NodeKind::AudioSplit);
        assert_eq!(i, vec![p("in", PortType::Audio)]);
        assert_eq!(o.len(), 2);
        assert_eq!(o[0], p("voice", PortType::Audio));
        assert_eq!(o[1], p("music", PortType::Audio));
    }

    #[test]
    fn port_table_audio_denoise() {
        let (i, o) = ports_for(NodeKind::AudioDenoise);
        assert_eq!(i, vec![p("in", PortType::Audio)]);
        assert_eq!(o, vec![p("out", PortType::Audio)]);
    }

    #[test]
    fn port_table_audio_isolate_voice() {
        let (i, o) = ports_for(NodeKind::AudioIsolateVoice);
        assert_eq!(i, vec![p("in", PortType::Audio)]);
        assert_eq!(o, vec![p("out", PortType::Audio)]);
    }

    #[test]
    fn port_table_audio_stems() {
        let (i, o) = ports_for(NodeKind::AudioStems);
        assert_eq!(i, vec![p("in", PortType::Audio)]);
        assert_eq!(o.len(), 4);
        assert_eq!(o[0], p("vocals", PortType::Audio));
        assert_eq!(o[1], p("drums", PortType::Audio));
        assert_eq!(o[2], p("bass", PortType::Audio));
        assert_eq!(o[3], p("other", PortType::Audio));
    }

    // -- Language nodes: audio in, text out --

    #[test]
    fn port_table_transcribe() {
        let (i, o) = ports_for(NodeKind::Transcribe);
        assert_eq!(i, vec![p("in", PortType::Audio)]);
        assert_eq!(o, vec![p("out", PortType::Text)]);
    }

    #[test]
    fn port_table_diarize() {
        let (i, o) = ports_for(NodeKind::Diarize);
        assert_eq!(i, vec![p("in", PortType::Audio)]);
        assert_eq!(o, vec![p("out", PortType::Text)]);
    }

    // -- Image processing nodes --

    #[test]
    fn port_table_image_upscale() {
        let (i, o) = ports_for(NodeKind::ImageUpscale);
        assert_eq!(i, vec![p("in", PortType::Image)]);
        assert_eq!(o, vec![p("out", PortType::Image)]);
    }

    #[test]
    fn port_table_image_object_remove() {
        let (i, o) = ports_for(NodeKind::ImageObjectRemove);
        assert_eq!(i, vec![p("in", PortType::Image), p("mask", PortType::Mask)]);
        assert_eq!(o, vec![p("out", PortType::Image)]);
    }

    #[test]
    fn port_table_generative_fill() {
        let (i, o) = ports_for(NodeKind::GenerativeFill);
        assert_eq!(
            i,
            vec![
                p("in", PortType::Image),
                p("mask", PortType::Mask),
                p("prompt", PortType::Text),
            ]
        );
        assert_eq!(o, vec![p("out", PortType::Image)]);
    }

    #[test]
    fn port_table_image_cutout() {
        let (i, o) = ports_for(NodeKind::ImageCutout);
        assert_eq!(i, vec![p("in", PortType::Image)]);
        assert_eq!(o.len(), 2);
        assert_eq!(o[0], p("out", PortType::Image));
        assert_eq!(o[1], p("mask", PortType::Mask));
    }

    // -- Video processing nodes --

    #[test]
    fn port_table_video_upscale() {
        let (i, o) = ports_for(NodeKind::VideoUpscale);
        assert_eq!(i, vec![p("in", PortType::Video)]);
        assert_eq!(o, vec![p("out", PortType::Video)]);
    }

    #[test]
    fn port_table_video_remove_bg() {
        let (i, o) = ports_for(NodeKind::VideoRemoveBg);
        assert_eq!(i, vec![p("in", PortType::Video)]);
        assert_eq!(o.len(), 2);
        assert_eq!(o[0], p("out", PortType::Video));
        assert_eq!(o[1], p("mask", PortType::Mask));
    }

    #[test]
    fn port_table_video_interpolate() {
        let (i, o) = ports_for(NodeKind::VideoInterpolate);
        assert_eq!(i, vec![p("in", PortType::Video)]);
        assert_eq!(o, vec![p("out", PortType::Video)]);
    }

    // -- Language nodes: text/video in, text out --

    #[test]
    fn port_table_metadata_gen() {
        let (i, o) = ports_for(NodeKind::MetadataGen);
        assert_eq!(i, vec![p("in", PortType::Text)]);
        assert_eq!(o, vec![p("out", PortType::Text)]);
    }

    #[test]
    fn port_table_caption_frames() {
        let (i, o) = ports_for(NodeKind::CaptionFrames);
        assert_eq!(i, vec![p("in", PortType::Video)]);
        assert_eq!(o, vec![p("out", PortType::Text)]);
    }

    // -- Utility nodes --

    #[test]
    fn port_table_mask_helper() {
        let (i, o) = ports_for(NodeKind::MaskHelper);
        assert_eq!(i, vec![p("in", PortType::Image)]);
        assert_eq!(o, vec![p("mask", PortType::Mask)]);
    }

    #[test]
    fn port_table_av_mux() {
        let (i, o) = ports_for(NodeKind::AvMux);
        assert_eq!(
            i,
            vec![p("video", PortType::Video), p("audio", PortType::Audio)]
        );
        assert_eq!(o, vec![p("out", PortType::Video)]);
    }

    // -- Sinks: polymorphic input, no outputs --

    #[test]
    fn port_table_sink_gallery() {
        let (i, o) = ports_for(NodeKind::SinkGallery);
        assert!(o.is_empty());
        assert_eq!(i.len(), 6, "sink accepts all six port types");
        let types: Vec<_> = i.iter().map(|p| p.ty).collect();
        assert!(types.contains(&PortType::Audio));
        assert!(types.contains(&PortType::Video));
        assert!(types.contains(&PortType::Image));
        assert!(types.contains(&PortType::Mask));
        assert!(types.contains(&PortType::Text));
        assert!(types.contains(&PortType::Tensor));
        assert!(i.iter().all(|p| p.name == "in"));
    }

    #[test]
    fn port_table_sink_files() {
        let (i, o) = ports_for(NodeKind::SinkFiles);
        assert!(o.is_empty());
        assert_eq!(i.len(), 6, "sink accepts all six port types");
        let types: Vec<_> = i.iter().map(|p| p.ty).collect();
        assert!(types.contains(&PortType::Audio));
        assert!(types.contains(&PortType::Video));
        assert!(types.contains(&PortType::Image));
        assert!(types.contains(&PortType::Mask));
        assert!(types.contains(&PortType::Text));
        assert!(types.contains(&PortType::Tensor));
        assert!(i.iter().all(|p| p.name == "in"));
    }

    // -- Exhaustiveness: every variant in NodeKind::ALL is covered --

    #[test]
    fn port_table_covers_all_22_variants() {
        for kind in NodeKind::ALL {
            let (_i, _o) = ports_for(kind);
        }
    }

    /// No two input ports on the same node share a name, and no two output
    /// ports on the same node share a name — except sinks, whose polymorphic
    /// `in` port is declared once per accepted type under a single name. The
    /// validator relies on this to resolve edge endpoints.
    #[test]
    fn port_table_no_duplicate_port_names() {
        for kind in NodeKind::ALL {
            let (inputs, outputs) = ports_for(kind);
            let out_names: Vec<&str> = outputs.iter().map(|p| p.name.as_str()).collect();
            let out_set: HashSet<&str> = out_names.iter().copied().collect();
            assert_eq!(
                out_names.len(),
                out_set.len(),
                "{kind:?} has duplicate output port names: {out_names:?}"
            );

            // Sinks use any_port("in") — 6 declarations under the same name,
            // one per port type. That is the only allowed duplication.
            let is_sink = matches!(kind, NodeKind::SinkGallery | NodeKind::SinkFiles);
            if !is_sink {
                let in_names: Vec<&str> = inputs.iter().map(|p| p.name.as_str()).collect();
                let in_set: HashSet<&str> = in_names.iter().copied().collect();
                assert_eq!(
                    in_names.len(),
                    in_set.len(),
                    "{kind:?} has duplicate input port names: {in_names:?}"
                );
            }
        }
    }
}
