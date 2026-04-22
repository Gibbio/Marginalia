import Foundation
import SwiftUI

/// Polls the runtime's event buffer on a timer and dispatches each event
/// to the host for state reconciliation. Kept as a standalone type so the
/// real `.app` can instantiate one in `@main` and the preview target can
/// skip it entirely (the mock host synthesizes its own state).
///
/// Cadence: 100 ms. See the plan ("Rischi noti → Polling rate vs CPU").
@MainActor
public final class EventPoller: ObservableObject {
    /// Callback the poller invokes on each tick with a batch of events.
    /// In practice the GUI wires this to `host.handle(event:)`.
    public typealias Sink = @MainActor (MarginaliaEvent) -> Void

    private var timer: Timer?
    private let interval: TimeInterval
    private let sink: Sink
    private let source: @MainActor () -> [MarginaliaEvent]

    public init(interval: TimeInterval = 0.1,
                source: @escaping @MainActor () -> [MarginaliaEvent],
                sink: @escaping Sink) {
        self.interval = interval
        self.source = source
        self.sink = sink
    }

    public func start() {
        stop()
        // Scheduled on the main runloop so sink runs on the main actor —
        // handlers can safely mutate @Published state.
        timer = Timer.scheduledTimer(withTimeInterval: interval, repeats: true) { [weak self] _ in
            // Timer fires on the main runloop, hop onto the main actor so
            // we can touch the main-actor-isolated source/sink closures.
            Task { @MainActor [weak self] in
                guard let self else { return }
                let batch = self.source()
                for event in batch { self.sink(event) }
            }
        }
    }

    public func stop() {
        timer?.invalidate()
        timer = nil
    }
}
