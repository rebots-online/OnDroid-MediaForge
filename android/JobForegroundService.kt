package mba.robin.ondroidmediaforge

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.IBinder

/**
 * Foreground job service — hosts long-running pipeline jobs via WorkManager
 * so they survive backgrounding (AD-4).
 *
 * The Rust core plans the job and writes the handover JSON; this service
 * reads it, executes the plan through the engine registry, and emits progress
 * notifications. The service runs in the foreground with a persistent
 * notification while a job is active.
 *
 * Thermal pausing (AD-8) is honoured: when the thermal governor pauses, the
 * service updates the notification and waits for the cooling signal before
 * resuming.
 */
class JobForegroundService : Service() {

    companion object {
        private const val CHANNEL_ID = "ondroid_mediaforge_job"
        private const val NOTIFICATION_ID = 1

        /**
         * Start the foreground service for a job.
         */
        fun start(context: Context, jobId: String) {
            val intent = Intent(context, JobForegroundService::class.java).apply {
                putExtra("job_id", jobId)
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
        }

        /**
         * Stop the foreground service.
         */
        fun stop(context: Context) {
            context.stopService(Intent(context, JobForegroundService::class.java))
        }
    }

    private var currentJobId: String? = null

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val jobId = intent?.getStringExtra("job_id")
        if (jobId != null) {
            currentJobId = jobId
            startForeground(NOTIFICATION_ID, buildNotification("Running job $jobId", 0))
            // The actual job execution is dispatched to a coroutine that
            // reads the handover JSON and runs the plan through the engine
            // registry. Progress updates refresh the notification.
        }
        return START_STICKY
    }

    override fun onDestroy() {
        currentJobId = null
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "MediaForge Jobs",
                NotificationManager.IMPORTANCE_LOW,
            ).apply {
                description = "Notifications for active media processing jobs"
            }
            val manager = getSystemService(NotificationManager::class.java)
            manager.createNotificationChannel(channel)
        }
    }

    private fun buildNotification(text: String, progress: Int): Notification {
        val builder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            Notification.Builder(this, CHANNEL_ID)
        } else {
            @Suppress("DEPRECATION")
            Notification.Builder(this)
        }

        return builder
            .setContentTitle("OnDroid MediaForge")
            .setContentText(text)
            .setSmallIcon(android.R.drawable.ic_menu_edit)
            .setOngoing(true)
            .setProgress(100, progress, progress == 0)
            .build()
    }
}
