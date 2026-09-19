import Foundation
import CassiopeiaNative

/// Synchronous native boundary; each response owns a copy of its bytes.
public final class CassiopeiaSession {
    public enum Failure: Error { case closed, invalidRequest }
    private var handle: OpaquePointer? = cassiopeia_create()
    private let lock = NSLock()

    public init() {}

    public func dispatch(_ request: Data) throws -> Data {
        lock.lock()
        defer { lock.unlock() }
        guard let handle else { throw Failure.closed }
        let length = request.withUnsafeBytes { bytes in
            cassiopeia_dispatch(handle, bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count)
        }
        guard length > 0, let reply = cassiopeia_reply(handle) else { throw Failure.invalidRequest }
        return Data(bytes: reply, count: length)
    }

    public func close() {
        lock.lock()
        defer { lock.unlock() }
        if let handle { cassiopeia_destroy(handle) }
        handle = nil
    }

    deinit { close() }
}
