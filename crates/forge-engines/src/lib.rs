//! `forge-engines` — inference adapters behind one uniform interface.
//!
//! Inputs are **named and plural** because the models this product depends on
//! are: a diffusion UNet step takes latents, a timestep and encoder hidden
//! states, and Whisper takes mel features plus decoder state. Inputs are also
//! **borrowed** — a 1080p RGB frame is roughly 6 MB against a 2.2 ms per-frame
//! super-resolution budget, so an owned copy per stage per frame would dominate
//! that budget. Outputs are owned, allocated from a [`TensorPool`] so steady
//! state does not churn the heap.

pub mod registry;

use std::path::PathBuf;

use forge_core::capability::Backend;
use forge_core::graph::NodeKind;

/// Element type of a tensor payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DType {
    F32,
    F16,
    I64,
    I32,
    I8,
    U8,
}

impl DType {
    /// Bytes per element.
    pub fn size(self) -> usize {
        match self {
            DType::F32 | DType::I32 => 4,
            DType::F16 => 2,
            DType::I64 => 8,
            DType::I8 | DType::U8 => 1,
        }
    }
}

/// Borrowed input tensor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TensorRef<'a> {
    pub shape: &'a [usize],
    pub dtype: DType,
    pub data: &'a [u8],
}

impl<'a> TensorRef<'a> {
    /// A borrowed tensor over `data`.
    pub fn new(shape: &'a [usize], dtype: DType, data: &'a [u8]) -> Self {
        TensorRef { shape, dtype, data }
    }

    /// Elements implied by the shape.
    pub fn elements(&self) -> usize {
        self.shape.iter().product()
    }

    /// Whether the payload length matches the shape and element type.
    pub fn is_consistent(&self) -> bool {
        self.data.len() == self.elements() * self.dtype.size()
    }
}

/// Owned output tensor, allocated from [`TensorPool`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TensorIo {
    pub shape: Vec<usize>,
    pub dtype: DType,
    pub data: Vec<u8>,
}

impl TensorIo {
    /// An owned tensor over `data`.
    pub fn new(shape: Vec<usize>, dtype: DType, data: Vec<u8>) -> Self {
        TensorIo { shape, dtype, data }
    }

    /// Borrow this tensor as an input to the next stage, with no copy.
    pub fn as_ref(&self) -> TensorRef<'_> {
        TensorRef {
            shape: &self.shape,
            dtype: self.dtype,
            data: &self.data,
        }
    }
}

/// Reuses output buffers across segments so steady-state inference does not
/// churn the heap.
#[derive(Debug, Default)]
pub struct TensorPool {
    pub buffers: Vec<Vec<u8>>,
}

impl TensorPool {
    /// An empty pool.
    pub fn new() -> Self {
        TensorPool::default()
    }

    /// A zeroed buffer of exactly `bytes` length, reusing a retired allocation
    /// large enough to hold it.
    pub fn take(&mut self, bytes: usize) -> Vec<u8> {
        if let Some(index) = self
            .buffers
            .iter()
            .position(|buffer| buffer.capacity() >= bytes)
        {
            let mut buffer = self.buffers.swap_remove(index);
            buffer.clear();
            buffer.resize(bytes, 0);
            return buffer;
        }
        vec![0u8; bytes]
    }

    /// Return a buffer for reuse.
    pub fn give(&mut self, buffer: Vec<u8>) {
        self.buffers.push(buffer);
    }
}

/// The model an engine is asked to load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRef {
    pub name: String,
    pub path: PathBuf,
}

impl ModelRef {
    /// A model named `name` living at `path`.
    pub fn new(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        ModelRef {
            name: name.into(),
            path: path.into(),
        }
    }
}

/// Why an engine refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EngineError {
    /// The model could not be loaded on this backend. Not fatal on its own:
    /// the registry falls back to the next backend in the chain.
    #[error("loading {model} on {backend:?}: {reason}")]
    Load {
        model: String,
        backend: Backend,
        reason: String,
    },
    /// Inference failed.
    #[error("running inference: {0}")]
    Run(String),
    /// A required input was not supplied.
    #[error("missing input tensor {0}")]
    MissingInput(String),
    /// No backend in the chain could load. This is the only fatal outcome of
    /// `EngineRegistry::acquire`.
    #[error("no backend in the chain could serve {kind:?}")]
    ChainExhausted { kind: NodeKind },
}

/// Uniform inference interface.
pub trait Engine {
    fn load(&mut self, model: &ModelRef) -> Result<(), EngineError>;
    fn run(&mut self, inputs: &[(&str, TensorRef<'_>)])
        -> Result<Vec<(String, TensorIo)>, EngineError>;
    fn backend(&self) -> Backend;
}

/// An engine that loads anything and echoes its inputs, reporting
/// `Backend::Cpu`. It makes the registry and the scheduler testable with no
/// runtime, no model and no accelerator present.
#[derive(Debug, Default)]
pub struct NullEngine {
    pool: TensorPool,
    loaded: Option<ModelRef>,
}

impl NullEngine {
    /// A fresh engine with nothing loaded.
    pub fn new() -> Self {
        NullEngine::default()
    }

    /// The model this engine last loaded.
    pub fn loaded(&self) -> Option<&ModelRef> {
        self.loaded.as_ref()
    }
}

impl Engine for NullEngine {
    fn load(&mut self, model: &ModelRef) -> Result<(), EngineError> {
        self.loaded = Some(model.clone());
        Ok(())
    }

    fn run(
        &mut self,
        inputs: &[(&str, TensorRef<'_>)],
    ) -> Result<Vec<(String, TensorIo)>, EngineError> {
        let mut outputs = Vec::with_capacity(inputs.len());
        for (name, tensor) in inputs {
            let mut data = self.pool.take(tensor.data.len());
            data.copy_from_slice(tensor.data);
            outputs.push((
                (*name).to_string(),
                TensorIo::new(tensor.shape.to_vec(), tensor.dtype, data),
            ));
        }
        Ok(outputs)
    }

    fn backend(&self) -> Backend {
        Backend::Cpu
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_null_engine_echoes_every_named_input() {
        let mut engine = NullEngine::new();
        let latents = [1u8, 2, 3, 4];
        let timestep = [9u8];
        let inputs = [
            ("latents", TensorRef::new(&[1, 4], DType::U8, &latents)),
            ("timestep", TensorRef::new(&[1], DType::U8, &timestep)),
        ];

        let outputs = engine.run(&inputs).expect("run");
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].0, "latents");
        assert_eq!(outputs[0].1.data, latents);
        assert_eq!(outputs[0].1.shape, vec![1, 4]);
        assert_eq!(outputs[1].0, "timestep");
        assert_eq!(outputs[1].1.data, timestep);
        assert_eq!(engine.backend(), Backend::Cpu);
    }

    #[test]
    fn a_returned_buffer_is_reused_rather_than_reallocated() {
        let mut pool = TensorPool::new();
        let first = pool.take(4096);
        let address = first.as_ptr();
        pool.give(first);

        let second = pool.take(1024);
        assert_eq!(second.len(), 1024);
        assert_eq!(
            second.as_ptr(),
            address,
            "a retired allocation large enough must be reused"
        );
        assert!(pool.buffers.is_empty());
    }

    #[test]
    fn a_pool_allocates_when_nothing_retired_is_large_enough() {
        let mut pool = TensorPool::new();
        pool.give(vec![0u8; 8]);
        let buffer = pool.take(4096);
        assert_eq!(buffer.len(), 4096);
        assert_eq!(pool.buffers.len(), 1, "the small buffer is still retired");
    }

    #[test]
    fn a_tensor_payload_is_checked_against_its_shape() {
        let data = [0u8; 16];
        assert!(TensorRef::new(&[2, 2], DType::F32, &data).is_consistent());
        assert!(!TensorRef::new(&[2, 3], DType::F32, &data).is_consistent());
        assert_eq!(TensorRef::new(&[2, 2], DType::F32, &data).elements(), 4);
    }
}
