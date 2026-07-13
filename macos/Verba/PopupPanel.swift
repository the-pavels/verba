import AppKit

final class PopupPanel: NSPanel {
    private static let escapeKeyCode: UInt16 = 53

    var onDismissRequest: (() -> Void)?

    init(contentSize: NSSize) {
        super.init(
            contentRect: NSRect(origin: .zero, size: contentSize),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )

        animationBehavior = .utilityWindow
        backgroundColor = .clear
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .transient]
        hasShadow = true
        hidesOnDeactivate = false
        isFloatingPanel = true
        isOpaque = false
        isReleasedWhenClosed = false
        level = .floating
    }

    override var canBecomeKey: Bool {
        true
    }

    override var canBecomeMain: Bool {
        false
    }

    override func sendEvent(_ event: NSEvent) {
        guard event.type == .keyDown, event.keyCode == Self.escapeKeyCode else {
            super.sendEvent(event)
            return
        }

        onDismissRequest?()
    }
}
