//! Backend selection with fallback.
//!
//! A delegate that will not load is an ordinary condition on this hardware, not
//! a pipeline failure (OV-2). `EngineRegistry::acquire` walks the chain and
//! takes the first backend that loads; only an exhausted chain is an error.

use std::collections::HashMap;

use forge_core::capability::Backend;
use forge_core::graph::NodeKind;

use crate::{Engine, EngineError, ModelRef};

/// Builds an engine for one backend. Boxed rather than a trait so the registry
/// can be populated with closures from any crate.
pub type EngineFactory = Box<dyn Fn() -> Box<dyn Engine>>;

/// Ordered fallback list per node, e.g. `[Npu, Gpu, Cpu]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendChain(pub Vec<Backend>);

impl BackendChain {
    /// A chain in preference order.
    pub fn new(backends: impl IntoIterator<Item = Backend>) -> Self {
        BackendChain(backends.into_iter().collect())
    }

    /// The chain every accelerated node prefers.
    pub fn accelerated() -> Self {
        BackendChain(vec![Backend::Npu, Backend::Gpu, Backend::Cpu])
    }

    /// The chain for a device with no accelerator.
    pub fn cpu_only() -> Self {
        BackendChain(vec![Backend::Cpu])
    }
}

/// Holds one live engine per node kind, and the factories that can build them.
#[derive(Default)]
pub struct EngineRegistry {
    factories: HashMap<Backend, EngineFactory>,
    models: HashMap<NodeKind, ModelRef>,
    engines: HashMap<NodeKind, Box<dyn Engine>>,
}

impl EngineRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        EngineRegistry::default()
    }

    /// Register how to build an engine for `backend`.
    pub fn register(&mut self, backend: Backend, factory: EngineFactory) {
        self.factories.insert(backend, factory);
    }

    /// Declare the model a node kind loads. A kind with no model registered is
    /// acquired without a load step.
    pub fn set_model(&mut self, kind: NodeKind, model: ModelRef) {
        self.models.insert(kind, model);
    }

    /// Whether an engine is already live for `kind`.
    pub fn is_live(&self, kind: NodeKind) -> bool {
        self.engines.contains_key(&kind)
    }

    /// Walk the chain and return the first engine that loads.
    ///
    /// A load failure falls back to the next backend rather than failing the
    /// pipeline. An engine already acquired for this kind is returned as is, so
    /// a per-segment call does not re-pay the compilation cost (AD-6).
    pub fn acquire(
        &mut self,
        kind: NodeKind,
        chain: &BackendChain,
    ) -> Result<&mut dyn Engine, EngineError> {
        if !self.engines.contains_key(&kind) {
            let model = self.models.get(&kind).cloned();
            let mut loaded: Option<Box<dyn Engine>> = None;

            for backend in &chain.0 {
                let Some(factory) = self.factories.get(backend) else {
                    continue;
                };
                let mut engine = factory();
                let outcome = match &model {
                    Some(model) => engine.load(model),
                    None => Ok(()),
                };
                if outcome.is_ok() {
                    loaded = Some(engine);
                    break;
                }
            }

            let Some(engine) = loaded else {
                return Err(EngineError::ChainExhausted { kind });
            };
            self.engines.insert(kind, engine);
        }

        Ok(self
            .engines
            .get_mut(&kind)
            .expect("an engine was just inserted for this kind")
            .as_mut())
    }

    /// Drop the live engine for a kind, releasing its weights. This is how the
    /// scheduler serialises mutually exclusive stage families on a device whose
    /// model budget cannot hold both.
    pub fn release(&mut self, kind: NodeKind) {
        self.engines.remove(&kind);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NullEngine, TensorIo, TensorRef};

    /// An engine that never loads, standing in for a QNN delegate that is
    /// present but refuses on this silicon.
    struct RefusingEngine {
        backend: Backend,
    }

    impl Engine for RefusingEngine {
        fn load(&mut self, model: &ModelRef) -> Result<(), EngineError> {
            Err(EngineError::Load {
                model: model.name.clone(),
                backend: self.backend,
                reason: "delegate refused".to_string(),
            })
        }
        fn run(
            &mut self,
            _inputs: &[(&str, TensorRef<'_>)],
        ) -> Result<Vec<(String, TensorIo)>, EngineError> {
            Err(EngineError::Run("this engine never loads".to_string()))
        }
        fn backend(&self) -> Backend {
            self.backend
        }
    }

    fn registry_with_refusing_npu() -> EngineRegistry {
        let mut registry = EngineRegistry::new();
        registry.register(
            Backend::Npu,
            Box::new(|| {
                Box::new(RefusingEngine {
                    backend: Backend::Npu,
                })
            }),
        );
        registry.register(Backend::Cpu, Box::new(|| Box::new(NullEngine::new())));
        registry.set_model(
            NodeKind::ImageUpscale,
            ModelRef::new("quicksrnet", "models/quicksrnet.onnx"),
        );
        registry
    }

    #[test]
    fn a_first_entry_that_fails_to_load_yields_the_second_entrys_engine() {
        let mut registry = registry_with_refusing_npu();
        let chain = BackendChain::new([Backend::Npu, Backend::Cpu]);

        let engine = registry
            .acquire(NodeKind::ImageUpscale, &chain)
            .expect("the chain must fall back rather than fail");

        assert_eq!(engine.backend(), Backend::Cpu);
    }

    #[test]
    fn an_exhausted_chain_is_the_only_error() {
        let mut registry = registry_with_refusing_npu();
        let chain = BackendChain::new([Backend::Npu]);

        let error = match registry.acquire(NodeKind::ImageUpscale, &chain) {
            Ok(_) => panic!("a chain of one refusing backend must be exhausted"),
            Err(error) => error,
        };

        assert_eq!(
            error,
            EngineError::ChainExhausted {
                kind: NodeKind::ImageUpscale
            }
        );
        assert!(!registry.is_live(NodeKind::ImageUpscale));
    }

    #[test]
    fn a_backend_missing_from_the_registry_is_skipped_not_fatal() {
        let mut registry = EngineRegistry::new();
        registry.register(Backend::Cpu, Box::new(|| Box::new(NullEngine::new())));
        registry.set_model(NodeKind::Transcribe, ModelRef::new("whisper", "w.onnx"));

        let engine = registry
            .acquire(NodeKind::Transcribe, &BackendChain::accelerated())
            .expect("an unregistered backend must not end the walk");
        assert_eq!(engine.backend(), Backend::Cpu);
    }

    #[test]
    fn acquiring_twice_reuses_the_live_engine() {
        let mut registry = registry_with_refusing_npu();
        let chain = BackendChain::cpu_only();

        registry
            .acquire(NodeKind::ImageUpscale, &chain)
            .expect("first acquire");
        assert!(registry.is_live(NodeKind::ImageUpscale));

        // Removing the factories proves the second acquire builds nothing.
        registry.factories.clear();
        let engine = registry
            .acquire(NodeKind::ImageUpscale, &chain)
            .expect("second acquire must reuse the live engine");
        assert_eq!(engine.backend(), Backend::Cpu);

        registry.release(NodeKind::ImageUpscale);
        assert!(!registry.is_live(NodeKind::ImageUpscale));
    }

    #[test]
    fn a_kind_with_no_model_acquires_without_loading() {
        let mut registry = EngineRegistry::new();
        registry.register(Backend::Cpu, Box::new(|| Box::new(NullEngine::new())));

        let engine = registry
            .acquire(NodeKind::AudioSplit, &BackendChain::cpu_only())
            .expect("a node that holds no weights still gets an engine");
        assert_eq!(engine.backend(), Backend::Cpu);
    }
}
