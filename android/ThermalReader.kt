package mba.robin.ondroidmediaforge

import android.content.Context
import android.os.PowerManager

/**
 * Thermal reader — subscribes to the device's thermal headroom and feeds the
 * Rust core's ThermalGovernor.
 *
 * Uses `PowerManager.getThermalHeadroom()` (API 30+) to read the current
 * thermal headroom as a float in [0.0, 1.0], where 1.0 means the device is
 * at maximum sustainable thermal load. The governor uses this to decide
 * whether to continue, derate, widen stride, or pause (AD-8).
 *
 * On devices where `getThermalHeadroom()` is unavailable, falls back to
 * `PowerManager.THERMAL_STATUS_*` constants mapped to approximate headroom
 * values.
 */
class ThermalReader {

    data class ThermalSnapshot(
        val headroom: Float,
        val status: Int,
        val timestampMs: Long,
    )

    private var powerManager: PowerManager? = null
    private var lastHeadroom: Float = 0.0f

    /**
     * Initialise with the application context. Caches the PowerManager.
     */
    fun init(context: Context) {
        powerManager = context.getSystemService(Context.POWER_SERVICE) as? PowerManager
    }

    /**
     * Read the current thermal headroom as a float in [0.0, 1.0].
     *
     * On API 30+ (minSdk 31), uses `getThermalHeadroom()` directly.
     * Falls back to mapping `THERMAL_STATUS_*` to approximate values.
     */
    fun readHeadroom(): Float {
        val pm = powerManager ?: return 0.0f

        // minSdk is 31, so getThermalHeadroom is always available.
        val headroom = try {
            pm.getThermalHeadroom()
        } catch (e: Exception) {
            mapStatusToHeadroom(pm.currentThermalStatus)
        }

        lastHeadroom = headroom
        return headroom
    }

    /**
     * Read a full thermal snapshot including the status constant.
     */
    fun readSnapshot(): ThermalSnapshot {
        val pm = powerManager ?: return ThermalSnapshot(0.0f, 0, System.currentTimeMillis())
        return ThermalSnapshot(
            headroom = readHeadroom(),
            status = pm.currentThermalStatus,
            timestampMs = System.currentTimeMillis(),
        )
    }

    /**
     * Map PowerManager thermal status constants to approximate headroom.
     * Used as a fallback when getThermalHeadroom() throws.
     */
    private fun mapStatusToHeadroom(status: Int): Float {
        return when (status) {
            PowerManager.THERMAL_STATUS_NONE -> 0.0f
            PowerManager.THERMAL_STATUS_LIGHT -> 0.2f
            PowerManager.THERMAL_STATUS_MODERATE -> 0.4f
            PowerManager.THERMAL_STATUS_SEVERE -> 0.6f
            PowerManager.THERMAL_STATUS_CRITICAL -> 0.8f
            PowerManager.THERMAL_STATUS_EMERGENCY -> 0.95f
            PowerManager.THERMAL_STATUS_SHUTDOWN -> 1.0f
            else -> 0.0f
        }
    }
}
