import AppKit
import SwiftUI

struct ShortcutRecorderView: NSViewRepresentable {
    let value: String
    let accessibilityName: String
    let onRecord: (RecordedShortcut) -> Void

    func makeNSView(context: Context) -> ShortcutRecorderButton {
        let button = ShortcutRecorderButton()
        button.bezelStyle = .rounded
        button.font = .monospacedSystemFont(ofSize: NSFont.systemFontSize, weight: .regular)
        button.setButtonType(.momentaryPushIn)
        button.configuredValue = value
        button.recordingHandler = onRecord
        button.setAccessibilityLabel(accessibilityName)
        button.setAccessibilityHelp("Press to record a new shortcut. Press Escape to cancel.")
        return button
    }

    func updateNSView(_ button: ShortcutRecorderButton, context: Context) {
        button.configuredValue = value
        button.recordingHandler = onRecord
        button.setAccessibilityLabel(accessibilityName)
    }
}

@MainActor
final class ShortcutRecorderButton: NSButton {
    var configuredValue = "" {
        didSet {
            if !isRecording {
                title = configuredValue
                setAccessibilityValue(configuredValue)
            }
        }
    }
    var recordingHandler: ((RecordedShortcut) -> Void)?

    private(set) var isRecording = false

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        target = self
        action = #selector(beginRecording)
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        target = self
        action = #selector(beginRecording)
    }

    override var acceptsFirstResponder: Bool { true }

    @objc private func beginRecording() {
        isRecording = true
        title = "Press shortcut…"
        setAccessibilityValue("Recording")
        window?.makeFirstResponder(self)
    }

    override func keyDown(with event: NSEvent) {
        guard isRecording else {
            super.keyDown(with: event)
            return
        }

        let modifiers = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        if event.keyCode == 53,
           !modifiers.contains(.command),
           !modifiers.contains(.control),
           !modifiers.contains(.option),
           !modifiers.contains(.shift) {
            finishRecording()
            return
        }

        guard let key = shortcutKey(for: event) else {
            NSSound.beep()
            return
        }

        let shortcut = RecordedShortcut(
            key: key,
            command: modifiers.contains(.command),
            control: modifiers.contains(.control),
            option: modifiers.contains(.option),
            shift: modifiers.contains(.shift)
        )
        finishRecording()
        recordingHandler?(shortcut)
    }

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        guard isRecording else {
            return super.performKeyEquivalent(with: event)
        }
        keyDown(with: event)
        return true
    }

    override func resignFirstResponder() -> Bool {
        finishRecording()
        return super.resignFirstResponder()
    }

    private func finishRecording() {
        isRecording = false
        title = configuredValue
        setAccessibilityValue(configuredValue)
    }

    private func shortcutKey(for event: NSEvent) -> String? {
        if let named = Self.namedKeys[event.keyCode] {
            return named
        }
        if let function = Self.functionKeys[event.keyCode] {
            return function
        }
        guard let characters = event.charactersIgnoringModifiers?.uppercased(),
              characters.count == 1 else {
            return nil
        }
        return characters
    }

    private static let namedKeys: [UInt16: String] = [
        36: "return",
        48: "tab",
        49: "space",
        51: "delete",
        53: "escape",
        76: "return",
        117: "delete",
        123: "arrow-left",
        124: "arrow-right",
        125: "arrow-down",
        126: "arrow-up",
    ]

    private static let functionKeys: [UInt16: String] = [
        122: "F1", 120: "F2", 99: "F3", 118: "F4", 96: "F5",
        97: "F6", 98: "F7", 100: "F8", 101: "F9", 109: "F10",
        103: "F11", 111: "F12", 105: "F13", 107: "F14", 113: "F15",
        106: "F16", 64: "F17", 79: "F18", 80: "F19", 90: "F20",
    ]
}
