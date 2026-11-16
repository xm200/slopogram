import InAppSettings
@main struct SystemEnergyTest {
    static func main() {
        precondition(slopogramSystemLite(.video, lowPower: true, reduceMotion: false, reduceTransparency: false))
        precondition(slopogramSystemLite(.animations, lowPower: false, reduceMotion: true, reduceTransparency: false))
        precondition(!slopogramSystemLite(.blur, lowPower: false, reduceMotion: true, reduceTransparency: false))
        precondition(slopogramSystemLite(.blur, lowPower: false, reduceMotion: false, reduceTransparency: true))
        precondition(slopogramSystemLite(.liquidGlass, lowPower: false, reduceMotion: false, reduceTransparency: true))
        precondition(!slopogramSystemLite(.video, lowPower: false, reduceMotion: false, reduceTransparency: false))
    }
}
