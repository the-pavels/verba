import AppKit

final class PopupPanel: NSPanel {
    var onDismissRequest: (() -> Void)?
    var onCopyRequest: (() -> Void)?

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
        becomesKeyOnlyIfNeeded = false
        setFixedContentSize(contentSize)
    }

    func setFixedContentSize(_ contentSize: NSSize) {
        contentMinSize = NSSize(
            width: min(contentMinSize.width, contentSize.width),
            height: min(contentMinSize.height, contentSize.height)
        )
        contentMaxSize = NSSize(
            width: max(contentMaxSize.width, contentSize.width),
            height: max(contentMaxSize.height, contentSize.height)
        )
        setContentSize(contentSize)
        contentMinSize = contentSize
        contentMaxSize = contentSize
    }

    override var canBecomeKey: Bool {
        true
    }

    override var canBecomeMain: Bool {
        false
    }

    override func sendEvent(_ event: NSEvent) {
        switch PopupKeyboardCommand.command(for: event) {
        case .dismiss:
            onDismissRequest?()
        case .copy:
            onCopyRequest?()
        case nil:
            super.sendEvent(event)
        }
    }
}

enum PopupKeyboardCommand: Equatable {
    private static let escapeKeyCode: UInt16 = 53

    case dismiss
    case copy

    static func command(for event: NSEvent) -> Self? {
        guard event.type == .keyDown else {
            return nil
        }
        if event.keyCode == escapeKeyCode {
            return .dismiss
        }

        let modifiers = event.modifierFlags.intersection([.command, .control, .option])
        guard modifiers == .command,
              event.charactersIgnoringModifiers?.lowercased() == "c" else {
            return nil
        }
        return .copy
    }
}
