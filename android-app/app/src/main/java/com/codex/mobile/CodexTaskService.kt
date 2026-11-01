package com.codex.mobile

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.os.Build
import android.os.IBinder

/**
 * Keeps a user-initiated agent turn alive while the user changes apps.
 * No periodic or scheduled background task is created.
 */
class CodexTaskService : Service() {
    companion object {
        private const val CHANNEL = "codex-active-work"
        private const val ID = 1027
        private const val ACTION_STOP = "com.codex.mobile.STOP"
    }

    override fun onCreate() {
        super.onCreate()
        val notificationManager = getSystemService(NotificationManager::class.java)
        notificationManager.createNotificationChannel(
            NotificationChannel(CHANNEL, "Codex 正在执行的任务",
                NotificationManager.IMPORTANCE_LOW))
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val mobileState = (application as CodexMobileApp).state
        if (intent?.action == ACTION_STOP) {
            mobileState.interrupt()
            return START_NOT_STICKY
        }
        val launch = PendingIntent.getActivity(this, 0,
            Intent(this, MainActivity::class.java)
                .addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        val cancel = PendingIntent.getService(this, 1,
            Intent(this, CodexTaskService::class.java).setAction(ACTION_STOP),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        val notification = Notification.Builder(this, CHANNEL)
            .setSmallIcon(android.R.drawable.stat_notify_sync)
            .setContentTitle("Codex 正在处理任务")
            .setContentText("可返回应用查看进度、审批和执行结果")
            .setContentIntent(launch)
            .setOngoing(true)
            .addAction(Notification.Action.Builder(
                android.R.drawable.ic_media_pause, "停止任务", cancel).build())
            .build()
        startForeground(ID, notification)
        return START_NOT_STICKY
    }

    override fun onBind(intent: Intent?): IBinder? = null
}
