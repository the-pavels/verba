import AppKit

struct PopupPlacement: Equatable {
    let size: NSSize
    let origin: NSPoint
}

enum PopupPositioner {
    private static let pointerGap: CGFloat = 12
    private static let screenMargin: CGFloat = 8

    static func placement(
        popupSize: NSSize,
        pointer: NSPoint,
        screens: [NSScreen]
    ) -> PopupPlacement {
        guard let screen = screens.first(where: { $0.frame.contains(pointer) })
            ?? NSScreen.main
            ?? screens.first
        else {
            return PopupPlacement(size: popupSize, origin: pointer)
        }

        return placement(
            popupSize: popupSize,
            pointer: pointer,
            visibleFrame: screen.visibleFrame
        )
    }

    static func placement(
        popupSize: NSSize,
        pointer: NSPoint,
        visibleFrame: NSRect
    ) -> PopupPlacement {
        let availableSize = NSSize(
            width: max(0, visibleFrame.width - 2 * screenMargin),
            height: max(0, visibleFrame.height - 2 * screenMargin)
        )
        let fittedSize = NSSize(
            width: min(popupSize.width, availableSize.width),
            height: min(popupSize.height, availableSize.height)
        )

        return PopupPlacement(
            size: fittedSize,
            origin: origin(
                popupSize: fittedSize,
                pointer: pointer,
                visibleFrame: visibleFrame
            )
        )
    }

    static func origin(
        popupSize: NSSize,
        pointer: NSPoint,
        screens: [NSScreen]
    ) -> NSPoint {
        guard let screen = screens.first(where: { $0.frame.contains(pointer) })
            ?? NSScreen.main
            ?? screens.first
        else {
            return pointer
        }

        return origin(
            popupSize: popupSize,
            pointer: pointer,
            visibleFrame: screen.visibleFrame
        )
    }

    static func origin(
        popupSize: NSSize,
        pointer: NSPoint,
        visibleFrame: NSRect
    ) -> NSPoint {
        let right = pointer.x + pointerGap
        let left = pointer.x - popupSize.width - pointerGap
        let below = pointer.y - popupSize.height - pointerGap
        let above = pointer.y + pointerGap

        let preferredX = right + popupSize.width <= visibleFrame.maxX - screenMargin
            ? right
            : left
        let preferredY = below >= visibleFrame.minY + screenMargin
            ? below
            : above

        let minimumX = visibleFrame.minX + screenMargin
        let maximumX = max(minimumX, visibleFrame.maxX - screenMargin - popupSize.width)
        let minimumY = visibleFrame.minY + screenMargin
        let maximumY = max(minimumY, visibleFrame.maxY - screenMargin - popupSize.height)

        return NSPoint(
            x: min(max(preferredX, minimumX), maximumX).rounded(),
            y: min(max(preferredY, minimumY), maximumY).rounded()
        )
    }
}
