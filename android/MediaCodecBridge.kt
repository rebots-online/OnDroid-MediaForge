package mba.robin.ondroidmediaforge

import android.media.MediaCodec
import android.media.MediaFormat
import android.media.MediaMuxer
import java.nio.ByteBuffer

/**
 * MediaCodec hardware bridge — provides hardware-accelerated video encode/decode
 * to the Rust core via JNI.
 *
 * The Rust core handles the pipeline graph and inference; this bridge handles
 * the container-level codec work that Android's MediaCodec API exposes more
 * efficiently than a cross-platform library. It is used for:
 * - Decoding source video frames for frame-by-frame processing.
 * - Encoding processed frames back to video.
 * - Muxing audio and video tracks for the final output (AvMux node).
 */
class MediaCodecBridge {

    data class ProcessRequest(
        val inputPath: String,
        val outputPath: String,
        val operation: String,
        val params: Map<String, String>,
    )

    data class ProcessResult(
        val outputPath: String,
        val framesProcessed: Int,
        val durationMs: Long,
    )

    /**
     * Decode a video file into individual frames for pipeline processing.
     * Returns the count of decoded frames.
     */
    fun decodeFrames(request: ProcessRequest): ProcessResult {
        val extractor = android.media.MediaExtractor()
        extractor.setDataSource(request.inputPath)

        var frameCount = 0
        for (i in 0 until extractor.trackCount) {
            val format = extractor.getTrackFormat(i)
            val mime = format.getString(MediaFormat.KEY_MIME) ?: continue
            if (mime.startsWith("video/")) {
                extractor.selectTrack(i)
                val codec = MediaCodec.createDecoderByType(mime)
                codec.configure(format, null, null, 0)
                codec.start()

                val bufferInfo = MediaCodec.BufferInfo()
                while (true) {
                    val inputIndex = codec.dequeueInputBuffer(10000)
                    if (inputIndex >= 0) {
                        val inputBuffer = codec.getInputBuffer(inputIndex)!!
                        val sampleSize = extractor.readSampleData(inputBuffer, 0)
                        if (sampleSize < 0) {
                            codec.queueInputBuffer(inputIndex, 0, 0, 0, MediaCodec.BUFFER_FLAG_END_OF_STREAM)
                        } else {
                            codec.queueInputBuffer(inputIndex, 0, sampleSize, extractor.sampleTime, 0)
                            extractor.advance()
                        }
                    }

                    val outputIndex = codec.dequeueOutputBuffer(bufferInfo, 10000)
                    if (outputIndex >= 0) {
                        frameCount++
                        codec.releaseOutputBuffer(outputIndex, false)
                        if (bufferInfo.flags and MediaCodec.BUFFER_FLAG_END_OF_STREAM != 0) {
                            break
                        }
                    }
                }
                codec.stop()
                codec.release()
                break
            }
        }
        extractor.release()

        return ProcessResult(
            outputPath = request.outputPath,
            framesProcessed = frameCount,
            durationMs = 0,
        )
    }

    /**
     * Encode processed frames back to a video file.
     */
    fun encodeFrames(request: ProcessRequest): ProcessResult {
        // Scaffold: encoding path uses MediaCodec encoder + MediaMuxer.
        // Full implementation depends on the frame pipeline output format.
        return ProcessResult(
            outputPath = request.outputPath,
            framesProcessed = 0,
            durationMs = 0,
        )
    }

    /**
     * Process a media file — dispatches to decode or encode based on the
     * operation parameter.
     */
    fun process(request: ProcessRequest): ProcessResult {
        return when (request.operation) {
            "decode" -> decodeFrames(request)
            "encode" -> encodeFrames(request)
            else -> throw IllegalArgumentException("Unknown operation: ${request.operation}")
        }
    }
}
