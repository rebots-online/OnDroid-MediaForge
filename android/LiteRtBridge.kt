package mba.robin.ondroidmediaforge

import android.content.Context

/**
 * LiteRT inference bridge — the Kotlin side of the generative/LLM pipeline.
 *
 * LiteRT carries generative and LLM stages (AD-2). This bridge loads a model,
 * selects the backend, and runs inference via the LiteRT API.
 *
 * AD-2a: The QNN delegate is attached to generative stages only, and this
 * bridge must select the CPU/GPU path on any of three losses:
 *   1. The silicon does not support the delegate.
 *   2. The delegate fails to load.
 *   3. An upstream artifact no longer resolves.
 *
 * No deterministic stage ever requests the delegate — the entire audio stack,
 * batch Whisper, LaMa, QuickSRNet, matting and RIFE all run on CPU/GPU at
 * Fold3 class. The QNN binary is an accelerator whose loss degrades generative
 * stages rather than breaking the app.
 *
 * Per AD-3, this bridge may consume the published Maven artifact as a scaffold;
 * the from-source substitution is T23/T24 and is not optional.
 */
class LiteRtBridge {

    private var delegateEnabled: Boolean = false
    private var delegateLost: Boolean = false

    data class InferRequest(
        val modelPath: String,
        val inputs: Map<String, FloatArray>,
        val isGenerative: Boolean,
    )

    data class InferResult(
        val outputs: Map<String, FloatArray>,
        val backendUsed: String,
        val delegateUsed: Boolean,
    )

    /**
     * Run inference. If the stage is generative and the delegate is available,
     * attempt the QNN delegate. On any of the three losses (AD-2a), fall back
     * to CPU/GPU without error.
     */
    fun infer(request: InferRequest): InferResult {
        val useDelegate = request.isGenerative && canUseDelegate()

        if (useDelegate) {
            try {
                return runWithDelegate(request)
            } catch (e: Exception) {
                // Loss 2: delegate failed to load. Fall through to CPU/GPU.
                delegateLost = true
            }
        }

        return runOnCpuGpu(request)
    }

    /**
     * Whether the QNN delegate can be attempted. Checks:
     * - The silicon supports it (not yet lost).
     * - The delegate has not previously failed to load.
     * - The stage is generative (caller enforces this).
     */
    private fun canUseDelegate(): Boolean {
        return !delegateLost && delegateEnabled
    }

    private fun runWithDelegate(request: InferRequest): InferResult {
        // Scaffold: delegates to the LiteRT API with the QNN delegate.
        // T24 substitutes the from-source-built LiteRT library here.
        throw UnsupportedOperationException("LiteRT delegate not yet wired — scaffold pending T24")
    }

    private fun runOnCpuGpu(request: InferRequest): InferResult {
        // Scaffold: delegates to the LiteRT API with CPU/GPU backend.
        // T24 substitutes the from-source-built LiteRT library here.
        return InferResult(
            outputs = emptyMap(),
            backendUsed = "CPU",
            delegateUsed = false,
        )
    }

    /**
     * Enable the QNN delegate for generative stages. Called once at app init
     * after confirming the device has an NPU.
     */
    fun enableDelegate() {
        delegateEnabled = true
    }
}
