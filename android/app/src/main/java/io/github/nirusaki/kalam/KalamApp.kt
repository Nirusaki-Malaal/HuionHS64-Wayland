package io.github.nirusaki.kalam

import android.app.Application
import android.util.Log
import java.io.File
import java.io.PrintWriter
import java.io.StringWriter

class KalamApp : Application() {
    override fun onCreate() {
        super.onCreate()

        val defaultHandler = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { thread, throwable ->
            try {
                val sw = StringWriter()
                throwable.printStackTrace(PrintWriter(sw))
                val stackTrace = sw.toString()
                Log.e("KalamCrash", "FATAL EXCEPTION in thread ${thread.name}:\n$stackTrace")

                val logFile = File(filesDir, "crash.log")
                logFile.writeText("Thread: ${thread.name}\nTime: ${System.currentTimeMillis()}\n$stackTrace")
            } catch (t: Throwable) {
                t.printStackTrace()
            }
            defaultHandler?.uncaughtException(thread, throwable)
        }
    }
}
