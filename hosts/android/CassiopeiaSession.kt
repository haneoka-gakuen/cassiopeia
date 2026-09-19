package org.haneoka.cassiopeia

/** One native session per instance. Dispatch from the audio/game thread, not the audio callback. */
class CassiopeiaSession : AutoCloseable {
    private var handle: Long = nativeCreate().also { check(it != 0L) }

    @Synchronized
    fun dispatch(request: String): String {
        check(handle != 0L) { "Cassiopeia session is closed" }
        return nativeDispatch(handle, request.toByteArray(Charsets.UTF_8)).toString(Charsets.UTF_8)
    }

    @Synchronized
    override fun close() {
        if (handle != 0L) {
            nativeDestroy(handle)
            handle = 0L
        }
    }

    private external fun nativeCreate(): Long
    private external fun nativeDispatch(handle: Long, request: ByteArray): ByteArray
    private external fun nativeDestroy(handle: Long)

    companion object {
        init { System.loadLibrary("cassiopeia_android") }
    }
}
