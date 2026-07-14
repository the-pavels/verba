import AppKit

let outputDirectory = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
let outputs = [
    ("AppIcon-16.png", 16),
    ("AppIcon-16@2x.png", 32),
    ("AppIcon-32.png", 32),
    ("AppIcon-32@2x.png", 64),
    ("AppIcon-128.png", 128),
    ("AppIcon-128@2x.png", 256),
    ("AppIcon-256.png", 256),
    ("AppIcon-256@2x.png", 512),
    ("AppIcon-512.png", 512),
    ("AppIcon-512@2x.png", 1024),
]

func color(_ red: CGFloat, _ green: CGFloat, _ blue: CGFloat) -> NSColor {
    NSColor(calibratedRed: red / 255, green: green / 255, blue: blue / 255, alpha: 1)
}

func drawIcon(pixels: Int) throws -> Data {
    guard let bitmap = NSBitmapImageRep(
        bitmapDataPlanes: nil,
        pixelsWide: pixels,
        pixelsHigh: pixels,
        bitsPerSample: 8,
        samplesPerPixel: 4,
        hasAlpha: true,
        isPlanar: false,
        colorSpaceName: .deviceRGB,
        bytesPerRow: 0,
        bitsPerPixel: 0
    ), let context = NSGraphicsContext(bitmapImageRep: bitmap) else {
        throw CocoaError(.fileWriteUnknown)
    }

    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = context
    context.cgContext.scaleBy(x: CGFloat(pixels) / 1024, y: CGFloat(pixels) / 1024)
    context.cgContext.setShouldAntialias(true)

    let canvas = NSRect(x: 0, y: 0, width: 1024, height: 1024)
    NSGradient(starting: color(91, 92, 226), ending: color(27, 166, 166))?.draw(in: canvas, angle: -48)

    let primaryBubble = NSBezierPath(roundedRect: NSRect(x: 180, y: 400, width: 650, height: 430), xRadius: 90, yRadius: 90)
    color(244, 254, 255).setFill()
    primaryBubble.fill()

    let primaryTail = NSBezierPath()
    primaryTail.move(to: NSPoint(x: 345, y: 430))
    primaryTail.line(to: NSPoint(x: 505, y: 430))
    primaryTail.line(to: NSPoint(x: 345, y: 270))
    primaryTail.close()
    primaryTail.fill()

    color(66, 67, 184).setStroke()
    for line in [
        (NSPoint(x: 310, y: 685), NSPoint(x: 700, y: 685)),
        (NSPoint(x: 310, y: 555), NSPoint(x: 580, y: 555)),
    ] {
        let path = NSBezierPath()
        path.lineWidth = 50
        path.lineCapStyle = .round
        path.move(to: line.0)
        path.line(to: line.1)
        path.stroke()
    }

    let secondaryBubble = NSBezierPath(roundedRect: NSRect(x: 455, y: 150, width: 390, height: 285), xRadius: 75, yRadius: 75)
    NSColor.white.setFill()
    secondaryBubble.fill()

    let secondaryTail = NSBezierPath()
    secondaryTail.move(to: NSPoint(x: 675, y: 170))
    secondaryTail.line(to: NSPoint(x: 785, y: 170))
    secondaryTail.line(to: NSPoint(x: 785, y: 70))
    secondaryTail.close()
    secondaryTail.fill()

    NSGraphicsContext.restoreGraphicsState()

    guard let data = bitmap.representation(using: .png, properties: [:]) else {
        throw CocoaError(.fileWriteUnknown)
    }
    return data
}

for (filename, pixels) in outputs {
    let data = try drawIcon(pixels: pixels)
    try data.write(to: outputDirectory.appendingPathComponent(filename), options: .atomic)
}

print("Generated app icon assets from \(CommandLine.arguments[0])")
