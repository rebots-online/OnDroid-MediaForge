//! ONNX Runtime, CPU execution provider.
//!
//! ORT is the workhorse for the deterministic media operations — the audio
//! stack, LaMa, super-resolution and matting (AD-2). This adapter runs the CPU
//! provider only; the QNN provider is a later task.
//!
//! Compilation caches are a product feature, not an optimisation (AD-6). The
//! first session for a model writes an EPContext binary into [`OrtEngine::ctx_cache`],
//! and every later load reads that binary instead of recompiling the graph.

use std::path::{Path, PathBuf};

use forge_core::capability::Backend;
use ort::session::Session;
use ort::value::{DynValue, Tensor};

use crate::{DType, Engine, EngineError, ModelRef, TensorIo, TensorRef};

/// Session config keys ONNX Runtime reads to produce and consume an EPContext
/// model. Enabling context generation makes the first commit write the compiled
/// graph beside the model; a later run loads that file directly.
const EP_CONTEXT_ENABLE: &str = "ep.context_enable";
const EP_CONTEXT_FILE_PATH: &str = "ep.context_file_path";
const EP_CONTEXT_EMBED_MODE: &str = "ep.context_embed_mode";

/// ONNX Runtime engine; owns its EPContext cache directory.
pub struct OrtEngine {
    session: Option<Session>,
    /// Directory holding one EPContext binary per model, per device.
    pub ctx_cache: PathBuf,
    loaded: Option<ModelRef>,
}

impl std::fmt::Debug for OrtEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OrtEngine")
            .field("ctx_cache", &self.ctx_cache)
            .field("loaded", &self.loaded)
            .field("session", &self.session.is_some())
            .finish()
    }
}

impl OrtEngine {
    /// An engine caching compiled contexts under `ctx_cache`.
    pub fn new(ctx_cache: impl Into<PathBuf>) -> Self {
        OrtEngine {
            session: None,
            ctx_cache: ctx_cache.into(),
            loaded: None,
        }
    }

    /// Where this model's compiled context binary lives.
    pub fn context_path(&self, model: &ModelRef) -> PathBuf {
        self.ctx_cache.join(format!("{}.onnx_ctx.onnx", model.name))
    }

    /// The model this engine currently holds.
    pub fn loaded(&self) -> Option<&ModelRef> {
        self.loaded.as_ref()
    }

    fn load_error(model: &ModelRef, reason: impl std::fmt::Display) -> EngineError {
        EngineError::Load {
            model: model.name.clone(),
            backend: Backend::Cpu,
            reason: reason.to_string(),
        }
    }
}

impl Engine for OrtEngine {
    /// Open a session for `model`, preferring an already-compiled context.
    ///
    /// When a context binary exists it is committed directly, which is the
    /// warm path. Otherwise the source model is committed with context
    /// generation enabled, so the compile is paid once and the artefact is
    /// left behind for every later launch.
    fn load(&mut self, model: &ModelRef) -> Result<(), EngineError> {
        std::fs::create_dir_all(&self.ctx_cache)
            .map_err(|e| Self::load_error(model, format!("creating the context cache: {e}")))?;

        let cached = self.context_path(model);
        let warm = cached.is_file();

        let mut builder =
            Session::builder().map_err(|e| Self::load_error(model, format!("session builder: {e}")))?;
        builder = builder
            .with_execution_providers([ort::ep::CPU::default().build()])
            .map_err(|e| Self::load_error(model, format!("cpu execution provider: {e}")))?;

        if !warm {
            builder = builder
                .with_config_entry(EP_CONTEXT_ENABLE, "1")
                .and_then(|b| b.with_config_entry(EP_CONTEXT_FILE_PATH, cached.to_string_lossy()))
                .and_then(|b| b.with_config_entry(EP_CONTEXT_EMBED_MODE, "0"))
                .map_err(|e| Self::load_error(model, format!("context cache options: {e}")))?;
        }

        let source: &Path = if warm { &cached } else { &model.path };
        let session = builder
            .commit_from_file(source)
            .map_err(|e| Self::load_error(model, format!("committing {}: {e}", source.display())))?;

        self.session = Some(session);
        self.loaded = Some(model.clone());
        Ok(())
    }

    fn run(
        &mut self,
        inputs: &[(&str, TensorRef<'_>)],
    ) -> Result<Vec<(String, TensorIo)>, EngineError> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| EngineError::Run("no model is loaded".to_string()))?;

        let mut values = Vec::with_capacity(inputs.len());
        for (name, tensor) in inputs {
            values.push(((*name).to_string(), to_value(name, tensor)?));
        }

        let outputs = session
            .run(values)
            .map_err(|e| EngineError::Run(e.to_string()))?;

        let mut collected = Vec::with_capacity(outputs.len());
        for (name, value) in outputs.iter() {
            collected.push((name.to_string(), from_value(name, &value)?));
        }
        Ok(collected)
    }

    fn backend(&self) -> Backend {
        Backend::Cpu
    }
}

/// Reinterpret a byte payload as a vector of `T`.
///
/// The payload is copied rather than cast in place: `TensorRef::data` is a
/// borrowed byte slice with no alignment guarantee, and ORT requires a properly
/// aligned buffer.
fn elements<T: Copy>(bytes: &[u8], from_ne: fn(&[u8]) -> T) -> Vec<T> {
    let width = std::mem::size_of::<T>();
    bytes.chunks_exact(width).map(from_ne).collect()
}

fn shape_of(tensor: &TensorRef<'_>) -> Vec<i64> {
    tensor.shape.iter().map(|d| *d as i64).collect()
}

fn to_value(name: &str, tensor: &TensorRef<'_>) -> Result<ort::value::DynValue, EngineError> {
    if !tensor.is_consistent() {
        return Err(EngineError::Run(format!(
            "input {name}: payload of {} bytes does not match shape {:?} of {:?}",
            tensor.data.len(),
            tensor.shape,
            tensor.dtype
        )));
    }
    let shape = shape_of(tensor);
    let bytes = tensor.data;
    let build = |e: ort::Error| EngineError::Run(format!("input {name}: {e}"));

    Ok(match tensor.dtype {
        DType::F32 => Tensor::from_array((
            shape,
            elements(bytes, |b| f32::from_ne_bytes([b[0], b[1], b[2], b[3]])),
        ))
        .map_err(build)?
        .into_dyn(),
        DType::I32 => Tensor::from_array((
            shape,
            elements(bytes, |b| i32::from_ne_bytes([b[0], b[1], b[2], b[3]])),
        ))
        .map_err(build)?
        .into_dyn(),
        DType::I64 => Tensor::from_array((
            shape,
            elements(bytes, |b| {
                i64::from_ne_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
            }),
        ))
        .map_err(build)?
        .into_dyn(),
        DType::I8 => Tensor::from_array((shape, elements(bytes, |b| b[0] as i8)))
            .map_err(build)?
            .into_dyn(),
        DType::U8 => Tensor::from_array((shape, bytes.to_vec()))
            .map_err(build)?
            .into_dyn(),
        // Half precision is a QNN concern; the CPU provider in this build has
        // no f16 kernels, so it is refused here rather than silently widened.
        DType::F16 => {
            return Err(EngineError::Run(format!(
                "input {name}: f16 is not supported on the CPU execution provider"
            )))
        }
    })
}

fn from_value(name: &str, value: &DynValue) -> Result<TensorIo, EngineError> {
    fn pack<T: Copy>(shape: &[i64], data: &[T], dtype: DType, to_ne: fn(&T) -> Vec<u8>) -> TensorIo {
        let mut bytes = Vec::with_capacity(data.len() * dtype.size());
        for element in data {
            bytes.extend_from_slice(&to_ne(element));
        }
        TensorIo::new(shape.iter().map(|d| *d as usize).collect(), dtype, bytes)
    }

    if let Ok((shape, data)) = value.try_extract_tensor::<f32>() {
        return Ok(pack(shape, data, DType::F32, |v| v.to_ne_bytes().to_vec()));
    }
    if let Ok((shape, data)) = value.try_extract_tensor::<i64>() {
        return Ok(pack(shape, data, DType::I64, |v| v.to_ne_bytes().to_vec()));
    }
    if let Ok((shape, data)) = value.try_extract_tensor::<i32>() {
        return Ok(pack(shape, data, DType::I32, |v| v.to_ne_bytes().to_vec()));
    }
    if let Ok((shape, data)) = value.try_extract_tensor::<i8>() {
        return Ok(pack(shape, data, DType::I8, |v| vec![*v as u8]));
    }
    if let Ok((shape, data)) = value.try_extract_tensor::<u8>() {
        return Ok(pack(shape, data, DType::U8, |v| vec![*v]));
    }
    Err(EngineError::Run(format!(
        "output {name} has an element type this adapter does not carry"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_context_path_lives_under_the_cache_directory() {
        let engine = OrtEngine::new("/models/cache");
        let path = engine.context_path(&ModelRef::new("quicksrnet", "/models/quicksrnet.onnx"));
        assert!(path.starts_with(&engine.ctx_cache));
        assert_eq!(
            path,
            PathBuf::from("/models/cache/quicksrnet.onnx_ctx.onnx")
        );
    }

    #[test]
    fn an_engine_with_no_session_reports_cpu_and_refuses_to_run() {
        let mut engine = OrtEngine::new("/models/cache");
        assert_eq!(engine.backend(), Backend::Cpu);
        assert!(engine.loaded().is_none());
        let data = [0u8; 4];
        let inputs = [("x", TensorRef::new(&[1], DType::F32, &data))];
        assert!(matches!(engine.run(&inputs), Err(EngineError::Run(_))));
    }

    #[test]
    fn an_inconsistent_input_is_rejected_before_reaching_the_runtime() {
        let data = [0u8; 6];
        let tensor = TensorRef::new(&[4], DType::F32, &data);
        assert!(!tensor.is_consistent());
        assert!(to_value("x", &tensor).is_err());
    }
}
