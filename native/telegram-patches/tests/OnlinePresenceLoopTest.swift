import Foundation

@main struct OnlinePresenceLoopTest {
    static func main() {
        var starts = 0
        var stops = 0
        var timerActive = false
        let loop = SlopogramOnlinePresenceLoop(start: { starts += 1; timerActive = true }, stop: { stops += 1; timerActive = false })
        loop.update(online: true, suppressed: true)
        for _ in 0..<100 { loop.update(online: true, suppressed: true) }
        precondition(starts == 0 && stops == 0 && !timerActive)
        loop.update(online: true, suppressed: false)
        precondition(starts == 1 && timerActive)
        loop.update(online: true, suppressed: true)
        precondition(stops == 1 && !timerActive)
        loop.update(online: true, suppressed: false)
        precondition(starts == 2 && timerActive)
        loop.update(online: false, suppressed: false)
        precondition(stops == 2 && !timerActive)
        loop.update(online: false, suppressed: true)
        precondition(starts == 2 && stops == 2)
    }
}
