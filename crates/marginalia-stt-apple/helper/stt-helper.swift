import Foundation
import Speech
import AudioToolbox

let language = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "it-IT"
let cmdSilenceTimeout = CommandLine.arguments.count > 2
    ? (Double(CommandLine.arguments[2]) ?? 0.8)
    : 0.8
let dictSilenceTimeout = CommandLine.arguments.count > 3
    ? (Double(CommandLine.arguments[3]) ?? 1.5)
    : 1.5
let triggerWords: [String] = CommandLine.arguments.count > 4 && !CommandLine.arguments[4].isEmpty
    ? CommandLine.arguments[4].split(separator: "|").map { $0.lowercased() }
    : []
let sampleRate: Double = CommandLine.arguments.count > 5
    ? (Double(CommandLine.arguments[5]) ?? 24000)
    : 24000

setbuf(stdout, nil)

// Mode state. All mutations happen on the main queue.
enum HelperMode { case command; case dictation }
var currentMode: HelperMode = .command

// Dictation feedback sounds.
var dictStartSound: SystemSoundID = 0
var dictEndSound: SystemSoundID = 0
AudioServicesCreateSystemSoundID(
    URL(fileURLWithPath: "/System/Library/Sounds/Tink.aiff") as CFURL, &dictStartSound)
AudioServicesCreateSystemSoundID(
    URL(fileURLWithPath: "/System/Library/Sounds/Pop.aiff") as CFURL, &dictEndSound)

// Authorization
let semaphore = DispatchSemaphore(value: 0)
SFSpeechRecognizer.requestAuthorization { status in
    guard status == .authorized else {
        fputs("Speech recognition not authorized\n", stderr)
        exit(1)
    }
    semaphore.signal()
}
semaphore.wait()

let locale = Locale(identifier: language)
guard let recognizer = SFSpeechRecognizer(locale: locale), recognizer.isAvailable else {
    fputs("SFSpeechRecognizer not available for \(language)\n", stderr)
    exit(1)
}
if #available(macOS 13.0, *) {
    recognizer.supportsOnDeviceRecognition = true
}

// Audio format for buffers received from Rust.
let audioFormat = AVAudioFormat(standardFormatWithSampleRate: sampleRate, channels: 1)!
let frameSamples = UInt32(sampleRate / 100) // 10ms frame

var currentRequest: SFSpeechAudioBufferRecognitionRequest?
var isRestarting = false
var silenceTimer: DispatchWorkItem?

func containsTrigger(_ text: String) -> Bool {
    if triggerWords.isEmpty { return false }
    let lower = text.lowercased()
    return triggerWords.contains(where: { lower.contains($0) })
}

func emit(_ text: String, mode: HelperMode) {
    if text.isEmpty { return }
    print(mode == .command ? "CMD \(text)" : "DICT_END \(text)")
}

func scheduleRestart() {
    silenceTimer?.cancel()
    silenceTimer = nil
    guard !isRestarting else { return }
    isRestarting = true
    currentRequest?.endAudio()
    currentRequest = nil
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
        isRestarting = false
        startRecognitionTask()
    }
}

func startRecognitionTask() {
    let request = SFSpeechAudioBufferRecognitionRequest()
    request.shouldReportPartialResults = true
    if #available(macOS 13.0, *) {
        request.requiresOnDeviceRecognition = true
    }
    currentRequest = request

    var lastText = ""
    var emitted = false

    recognizer.recognitionTask(with: request) { result, error in
        silenceTimer?.cancel()
        silenceTimer = nil
        let mode = currentMode

        if let result = result {
            lastText = result.bestTranscription.formattedString

            if result.isFinal {
                if !emitted { emit(lastText, mode: mode) }
                lastText = ""
                emitted = false
                scheduleRestart()
                return
            }

            // Stream the live transcript as it grows so the host UI
            // can show what the user is dictating in real time. Only
            // in dictation mode — command mode wants the silence-gated
            // single shot to avoid premature trigger matches.
            if mode == .dictation && !lastText.isEmpty {
                print("DICT_PARTIAL \(lastText)")
            }

            let timeout = (mode == .command) ? cmdSilenceTimeout : dictSilenceTimeout
            let snap = lastText
            let timer = DispatchWorkItem {
                if !emitted {
                    emit(snap, mode: currentMode)
                    emitted = true
                }
                scheduleRestart()
            }
            silenceTimer = timer
            DispatchQueue.main.asyncAfter(deadline: .now() + timeout, execute: timer)
        }

        if error != nil && !isRestarting { scheduleRestart() }
    }
}

// Read exactly N bytes from stdin. Returns nil on EOF.
func readExact(_ count: Int) -> Data? {
    var buf = Data(capacity: count)
    while buf.count < count {
        let chunk = FileHandle.standardInput.readData(ofLength: count - buf.count)
        if chunk.isEmpty { return nil }
        buf.append(chunk)
    }
    return buf
}

// Stdin reader: binary TLV protocol. Processes audio frames and mode commands.
DispatchQueue.global().async {
    startRecognitionTask()

    while true {
        guard let header = readExact(3) else { break }
        let type = header[0]
        let length = Int(header[1]) << 8 | Int(header[2])
        guard let payload = readExact(length) else { break }

        if type == 0x41 { // 'A' — audio frame
            let sampleCount = length / 4
            guard sampleCount > 0 else { continue }
            let pcm = AVAudioPCMBuffer(pcmFormat: audioFormat,
                                       frameCapacity: UInt32(sampleCount))!
            pcm.frameLength = UInt32(sampleCount)
            payload.withUnsafeBytes { raw in
                let src = raw.bindMemory(to: Float.self)
                memcpy(pcm.floatChannelData![0], src.baseAddress!, length)
            }
            DispatchQueue.main.async {
                currentRequest?.append(pcm)
            }
        } else if type == 0x4D { // 'M' — mode command
            let text = String(data: payload, encoding: .utf8)?.trimmingCharacters(in: .whitespaces) ?? ""
            DispatchQueue.main.async {
                switch text {
                case "COMMAND":
                    if currentMode == .dictation && dictEndSound != 0 {
                        AudioServicesPlaySystemSound(dictEndSound)
                    }
                    currentMode = .command
                    scheduleRestart()
                case "DICTATION":
                    if currentMode == .command && dictStartSound != 0 {
                        AudioServicesPlaySystemSound(dictStartSound)
                    }
                    currentMode = .dictation
                    scheduleRestart()
                default: break
                }
            }
        }
    }
    exit(0)
}

dispatchMain()
