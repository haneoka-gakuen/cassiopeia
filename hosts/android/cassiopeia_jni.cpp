#include <jni.h>
#include <cstdint>
#include <limits>
#include "cassiopeia.h"

static CassiopeiaHost *host(jlong handle) {
    return reinterpret_cast<CassiopeiaHost *>(static_cast<std::uintptr_t>(handle));
}
static void fail(JNIEnv *env, const char *message) {
    const auto error = env->FindClass("java/lang/IllegalStateException");
    if (error) env->ThrowNew(error, message);
}
extern "C" JNIEXPORT jlong JNICALL
Java_org_haneoka_cassiopeia_CassiopeiaSession_nativeCreate(JNIEnv *, jobject) {
    return static_cast<jlong>(reinterpret_cast<std::uintptr_t>(cassiopeia_create()));
}
extern "C" JNIEXPORT void JNICALL
Java_org_haneoka_cassiopeia_CassiopeiaSession_nativeDestroy(JNIEnv *, jobject, jlong handle) {
    cassiopeia_destroy(host(handle));
}
extern "C" JNIEXPORT jbyteArray JNICALL
Java_org_haneoka_cassiopeia_CassiopeiaSession_nativeDispatch(JNIEnv *env, jobject, jlong handle, jbyteArray input) {
    if (!handle || !input) { fail(env, "Missing Cassiopeia handle or input"); return nullptr; }
    const auto size = env->GetArrayLength(input);
    const auto bytes = env->GetByteArrayElements(input, nullptr);
    if (!bytes) return nullptr; // JVM has raised OutOfMemoryError.
    const auto length = cassiopeia_dispatch(host(handle), reinterpret_cast<const uint8_t *>(bytes), size);
    env->ReleaseByteArrayElements(input, bytes, JNI_ABORT);
    if (!length || length > static_cast<size_t>(std::numeric_limits<jsize>::max())) {
        fail(env, "Invalid Cassiopeia request or reply size"); return nullptr;
    }
    const auto result = env->NewByteArray(static_cast<jsize>(length));
    if (!result) return nullptr;
    env->SetByteArrayRegion(result, 0, static_cast<jsize>(length),
        reinterpret_cast<const jbyte *>(cassiopeia_reply(host(handle))));
    return result;
}
