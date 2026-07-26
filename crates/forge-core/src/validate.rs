//! Why a graph cannot run, and the full pre-run validation that finds every
//! reason at once.

use serde::{Deserialize, Serialize};

use crate::capability::DeviceTier;
use crate::graph::{Edge, NodeId, PortType};

/// Why a graph cannot run. `validate_graph` returns all of these, never just
/// the first, so the editor can annotate every offending node in one pass.
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
}
