package mba.robin.ondroidmediaforge

import com.tauri.plugin.Plugin
import com.tauri.plugin.Invoke

/**
 * Tauri 2 plugin entry point for OnDroid MediaForge.
 *
 * Registers the Kotlin bridges that provide Android platform services to the
 * Rust core via JNI: LiteRT inference, MediaCodec hardware codecs, Play
 * Billing, the foreground job service, and the thermal reader.
 *
 * Every @Command dispatches to a coroutine — nothing long-running executes
 * on the main thread (AD-4).
 */
class MediaForgePlugin : Plugin() {

    private val liteRtBridge = LiteRtBridge()
    private val mediaCodecBridge = MediaCodecBridge()
    private val billingBridge = BillingBridge()
    private val thermalReader = ThermalReader()

    @Command
    fun liteRtInfer(invoke: Invoke) {
        launch {
            try {
                val request = invoke.parseArgs()
                val result = liteRtBridge.infer(request)
                invoke.resolve(result)
            } catch (e: Exception) {
                invoke.reject(e.message ?: "LiteRT inference failed")
            }
        }
    }

    @Command
    fun mediaCodecProcess(invoke: Invoke) {
        launch {
            try {
                val request = invoke.parseArgs()
                val result = mediaCodecBridge.process(request)
                invoke.resolve(result)
            } catch (e: Exception) {
                invoke.reject(e.message ?: "MediaCodec processing failed")
            }
        }
    }

    @Command
    fun queryEntitlement(invoke: Invoke) {
        launch {
            try {
                val result = billingBridge.queryEntitlement()
                invoke.resolve(result)
            } catch (e: Exception) {
                invoke.reject(e.message ?: "Entitlement query failed")
            }
        }
    }

    @Command
    fun readThermalHeadroom(invoke: Invoke) {
        launch {
            try {
                val headroom = thermalReader.readHeadroom()
                invoke.resolve(mapOf("headroom" to headroom))
            } catch (e: Exception) {
                invoke.reject(e.message ?: "Thermal read failed")
            }
        }
    }

    @Command
    fun startJobService(invoke: Invoke) {
        launch {
            try {
                val request = invoke.parseArgs()
                JobForegroundService.start(context, request.jobId)
                invoke.resolve(mapOf("started" to true))
            } catch (e: Exception) {
                invoke.reject(e.message ?: "Job service start failed")
            }
        }
    }

    @Command
    fun stopJobService(invoke: Invoke) {
        launch {
            try {
                JobForegroundService.stop(context)
                invoke.resolve(mapOf("stopped" to true))
            } catch (e: Exception) {
                invoke.reject(e.message ?: "Job service stop failed")
            }
        }
    }
}
