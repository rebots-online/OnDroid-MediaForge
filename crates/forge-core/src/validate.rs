//! Why a graph cannot run, and the full pre-run validation that finds every
//! reason at once.
//!
//! `validate_graph` never short-circuits. The editor annotates every offending
//! node in a single pass, so a user fixing a pipeline is not led through one
//! error at a time.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::capability::{exclusive_families, required_tier, DeviceTier, SocProfile, StageFamily};
use crate::graph::{ports_for, Edge, Graph, NodeId, PortType};

/// Why a graph cannot run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum ValidationError {
    /// An edge joins two ports whose types differ.
    #[error("type mismatch on {}.{} -> {}.{}: expected {expected:?}, found {found:?}",
        edge.from.0, edge.from.1, edge.to.0, edge.to.1)]
    TypeMismatch {
        edge: Edge,
        expected: PortType,
        found: PortType,
    },
    /// The graph is not a DAG; these node ids remain in the cycle.
    #[error("cycle through {0:?}")]
    Cycle(Vec<NodeId>),
    /// A declared input port has no incoming edge.
    #[error("node {node} has no input connected to port {port}")]
    MissingInput { node: NodeId, port: String },
    /// The node needs silicon this device does not have.
    #[error("node {node} requires tier {required:?}")]
    TierUnavailable { node: NodeId, required: DeviceTier },
    /// Two nodes hold model families that must not be resident together at
    /// this device's tier.
    #[error("nodes {a} and {b} hold {families:?}, which cannot be co-resident at this tier")]
    ExclusiveFamilies {
        a: NodeId,
        b: NodeId,
        families: (StageFamily, StageFamily),
    },
}

/// Full pre-run validation. Returns **every** error found, in a deterministic
/// order: tier, then co-residency, then cycles, then edge typing, then missing
/// inputs.
///
/// Edges naming a node id or a port name that `ports_for` does not declare are
/// skipped by the typing pass — the editor authors edges from `ports_for` and
/// cannot produce one. A port left dangling that way still surfaces through the
/// missing-input pass.
pub fn validate_graph(g: &Graph, caps: &SocProfile) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    // 1. Tier. Physics first, exactly as `resolve_availability` orders it.
    for node in &g.nodes {
        let required = required_tier(node.kind);
        if required > caps.tier {
            errors.push(ValidationError::TierUnavailable {
                node: node.id.clone(),
                required,
            });
        }
    }

    // 2. Co-residency. One report per forbidden family pair, naming the first
    //    two nodes that trigger it.
    for (fa, fb) in exclusive_families(caps.tier) {
        let first_of = |family: StageFamily| {
            g.nodes
                .iter()
                .find(|n| StageFamily::of(n.kind) == Some(family))
                .map(|n| n.id.clone())
        };
        if let (Some(a), Some(b)) = (first_of(fa), first_of(fb)) {
            if a != b {
                errors.push(ValidationError::ExclusiveFamilies {
                    a,
                    b,
                    families: (fa, fb),
                });
            }
        }
    }

    // 3. Cycles.
    if let Err(cycle) = g.topological_order() {
        errors.push(cycle);
    }

    // 4. Edge typing.
    for edge in &g.edges {
        let (Some(from), Some(to)) = (g.node(&edge.from.0), g.node(&edge.to.0)) else {
            continue;
        };
        let (_, from_outputs) = ports_for(from.kind);
        let Some(found) = from_outputs
            .iter()
            .find(|p| p.name == edge.from.1)
            .map(|p| p.ty)
        else {
            continue;
        };
        let (to_inputs, _) = ports_for(to.kind);
        let accepted: Vec<PortType> = to_inputs
            .iter()
            .filter(|p| p.name == edge.to.1)
            .map(|p| p.ty)
            .collect();
        let Some(&expected) = accepted.first() else {
            continue;
        };
        if !accepted.contains(&found) {
            errors.push(ValidationError::TypeMismatch {
                edge: edge.clone(),
                expected,
                found,
            });
        }
    }

    // 5. Required inputs. Every distinct declared input port name must have at
    //    least one incoming edge.
    for node in &g.nodes {
        let (inputs, _) = ports_for(node.kind);
        let names: BTreeSet<&str> = inputs.iter().map(|p| p.name.as_str()).collect();
        for name in names {
            let connected = g
                .edges
                .iter()
                .any(|e| e.to.0 == node.id && e.to.1 == name);
            if !connected {
                errors.push(ValidationError::MissingInput {
                    node: node.id.clone(),
                    port: name.to_string(),
                });
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::{Backend, PROBE_SCHEMA_VERSION};
    use crate::graph::{NodeKind, NodeSpec};

    fn profile(tier: DeviceTier) -> SocProfile {
        SocProfile {
            soc_id: "test".to_string(),
            soc_name: "test".to_string(),
            tier,
            ram_bytes: 8 * 1024 * 1024 * 1024,
            model_budget_bytes: 1024 * 1024 * 1024,
            backends: vec![Backend::Cpu],
            npu_experimental: false,
            probe_schema_version: PROBE_SCHEMA_VERSION,
        }
    }

    fn node(id: &str, kind: NodeKind) -> NodeSpec {
        NodeSpec {
            id: NodeId::from(id),
            kind,
            model: None,
            params: serde_json::Map::new(),
        }
    }

    fn edge(from: (&str, &str), to: (&str, &str)) -> Edge {
        Edge {
            from: (NodeId::from(from.0), from.1.to_string()),
            to: (NodeId::from(to.0), to.1.to_string()),
        }
    }

    #[test]
    fn valid_audio_chain_passes() {
        let g = Graph {
            nodes: vec![
                node("src", NodeKind::SourceAudio),
                node("dnz", NodeKind::AudioDenoise),
                node("out", NodeKind::SinkFiles),
            ],
            edges: vec![
                edge(("src", "audio"), ("dnz", "in")),
                edge(("dnz", "out"), ("out", "in")),
            ],
        };
        assert_eq!(validate_graph(&g, &profile(DeviceTier::T0)), Ok(()));
    }

    #[test]
    fn audio_into_image_port_is_a_type_mismatch() {
        let g = Graph {
            nodes: vec![
                node("src", NodeKind::SourceAudio),
                node("up", NodeKind::ImageUpscale),
            ],
            edges: vec![edge(("src", "audio"), ("up", "in"))],
        };
        let errors = validate_graph(&g, &profile(DeviceTier::T0)).unwrap_err();
        assert_eq!(
            errors,
            vec![ValidationError::TypeMismatch {
                edge: edge(("src", "audio"), ("up", "in")),
                expected: PortType::Image,
                found: PortType::Audio,
            }]
        );
    }

    #[test]
    fn two_node_cycle_is_reported_with_both_ids() {
        let g = Graph {
            nodes: vec![
                node("a", NodeKind::AudioDenoise),
                node("b", NodeKind::AudioIsolateVoice),
            ],
            edges: vec![
                edge(("a", "out"), ("b", "in")),
                edge(("b", "out"), ("a", "in")),
            ],
        };
        let errors = validate_graph(&g, &profile(DeviceTier::T0)).unwrap_err();
        let cycle = errors
            .iter()
            .find_map(|e| match e {
                ValidationError::Cycle(ids) => Some(ids.clone()),
                _ => None,
            })
            .expect("cycle reported");
        assert_eq!(cycle, vec![NodeId::from("a"), NodeId::from("b")]);
    }

    #[test]
    fn t2_only_node_on_a_t0_profile_is_tier_unavailable() {
        let g = Graph {
            nodes: vec![node("fill", NodeKind::GenerativeFill)],
            edges: vec![],
        };
        let errors = validate_graph(&g, &profile(DeviceTier::T0)).unwrap_err();
        assert!(errors.contains(&ValidationError::TierUnavailable {
            node: NodeId::from("fill"),
            required: DeviceTier::T2,
        }));

        // The same graph on a T2 profile raises no tier error.
        let t2 = validate_graph(&g, &profile(DeviceTier::T2)).unwrap_err();
        assert!(!t2
            .iter()
            .any(|e| matches!(e, ValidationError::TierUnavailable { .. })));
    }

    #[test]
    fn generative_fill_with_llm_metadata_is_rejected_at_t0() {
        let g = Graph {
            nodes: vec![
                node("fill", NodeKind::GenerativeFill),
                node("meta", NodeKind::MetadataGen),
            ],
            edges: vec![],
        };
        let errors = validate_graph(&g, &profile(DeviceTier::T0)).unwrap_err();
        assert!(errors.contains(&ValidationError::ExclusiveFamilies {
            a: NodeId::from("fill"),
            b: NodeId::from("meta"),
            families: (StageFamily::Diffusion, StageFamily::LargeLanguage),
        }));

        // T2 has the budget to hold both, so the pair is not excluded there.
        let t2 = validate_graph(&g, &profile(DeviceTier::T2)).unwrap_err();
        assert!(!t2
            .iter()
            .any(|e| matches!(e, ValidationError::ExclusiveFamilies { .. })));
    }
}
