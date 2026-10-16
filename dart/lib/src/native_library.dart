import 'dart:ffi';
import 'dart:io';

/// Host override is authoritative; otherwise use the platform's bundled library.
DynamicLibrary openCassiopeiaNativeLibrary({String? path}) {
  if (path != null) return DynamicLibrary.open(path);
  if (Platform.isIOS || Platform.isMacOS) return DynamicLibrary.process();
  final executable = File(Platform.resolvedExecutable).parent;
  if (Platform.isLinux) {
    final bundled = File('${executable.path}/lib/libcassiopeia_native.so');
    return DynamicLibrary.open(
      bundled.existsSync() ? bundled.path : 'libcassiopeia_native.so',
    );
  }
  if (Platform.isWindows)
    return DynamicLibrary.open('${executable.path}/cassiopeia_native.dll');
  if (Platform.isAndroid) return DynamicLibrary.open('libcassiopeia_native.so');
  throw UnsupportedError('cassiopeia_native_platform');
}
