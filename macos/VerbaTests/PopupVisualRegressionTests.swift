import AppKit
import SwiftUI
import XCTest
@testable import Verba

@MainActor
final class PopupVisualRegressionTests: XCTestCase {
    func testTranslationPopupMatchesBaseline() throws {
        try assertSnapshot(
            .translation(
                originalText: "Guten Morgen! Können Sie mir bitte helfen?",
                languagePair: LanguagePairViewModel(source: "de", target: "en"),
                translatedText: "Good morning! Could you please help me?"
            ),
            named: "translation-light"
        )
    }

    func testProofreadingPopupMatchesBaseline() throws {
        let originalText = "This sentence now grammatically correct"
        let correctedText = "This sentence is now grammatically correct."
        let diff = ProofreadingDiff(original: originalText, corrected: correctedText)

        try assertSnapshot(
            VStack(alignment: .leading, spacing: 10) {
                PopupResultSection(title: "Original") {
                    ProofreadingDiffText(
                        segments: diff.original,
                        accessibilityLabel: "Original: \(originalText)"
                    )
                }

                PopupResultSection(title: "Corrected text", isEmphasized: true) {
                    ProofreadingDiffText(
                        segments: diff.corrected,
                        accessibilityLabel: "Corrected text: \(correctedText)"
                    )
                }
            }
            .padding(20),
            size: NSSize(width: 420, height: 190),
            named: "proofreading-light"
        )
    }

    func testErrorPopupMatchesBaseline() throws {
        try assertSnapshot(
            .error(
                action: .translate,
                title: "Translation failed",
                message: "Check your internet connection and try again.",
                recovery: .retry,
                diagnosticCode: "translation.failed"
            ),
            named: "error-light"
        )
    }

    private func assertSnapshot(
        _ presentation: PresentationViewModel,
        named name: String,
        file: StaticString = #filePath,
        line: UInt = #line
    ) throws {
        let size = PopupSizePolicy.size(for: presentation, textScale: 1)
        let content = PopupContentView(
            presentation: presentation,
            copyText: { _ in },
            continueProofreading: {},
            cancelProofreading: {},
            recover: { _, _ in }
        )
        try assertSnapshot(content, size: size, named: name, file: file, line: line)
    }

    private func assertSnapshot<Content: View>(
        _ content: Content,
        size: NSSize,
        named name: String,
        file: StaticString = #filePath,
        line: UInt = #line
    ) throws {
        let imageData = try render(content: content, size: size)

        if FileManager.default.fileExists(atPath: snapshotRecordMarkerURL.path) {
            try writeSnapshot(imageData, named: name)
            return
        }

        let baselineURL = try XCTUnwrap(
            Bundle(for: Self.self).url(
                forResource: name,
                withExtension: "png",
                subdirectory: "__Snapshots__"
            ),
            "Missing baseline for \(name).",
            file: file,
            line: line
        )
        let baselineData = try Data(contentsOf: baselineURL)
        let comparison = try compare(actual: imageData, expected: baselineData, size: size)

        guard comparison.changedPixelRatio <= 0.04,
              comparison.meanChannelError <= 0.015 else {
            addAttachment(data: baselineData, name: "\(name)-expected")
            addAttachment(data: imageData, name: "\(name)-actual")
            XCTFail(
                "Snapshot \(name) changed: "
                    + "\(comparison.changedPixelRatio.formatted(.percent.precision(.fractionLength(2)))) "
                    + "of pixels differ; mean channel error is "
                    + comparison.meanChannelError.formatted(.number.precision(.fractionLength(4))),
                file: file,
                line: line
            )
            return
        }
    }

    private func render<Content: View>(
        content: Content,
        size: NSSize
    ) throws -> Data {
        let renderedContent = content
        .frame(width: size.width, height: size.height)
        .background(Color(nsColor: .windowBackgroundColor))
        .environment(\.colorScheme, .light)
        .environment(\.locale, Locale(identifier: "en_US"))
        .tint(Color(nsColor: .systemBlue))

        let hostingView = NSHostingView(rootView: renderedContent)
        hostingView.appearance = NSAppearance(named: .aqua)
        hostingView.frame = NSRect(origin: .zero, size: size)

        let window = NSWindow(
            contentRect: hostingView.frame,
            styleMask: [.borderless],
            backing: .buffered,
            defer: false
        )
        window.appearance = NSAppearance(named: .aqua)
        window.backgroundColor = NSColor.windowBackgroundColor
        window.contentView = hostingView
        window.orderFrontRegardless()
        defer { window.orderOut(nil) }

        hostingView.layoutSubtreeIfNeeded()
        hostingView.displayIfNeeded()
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.05))
        hostingView.layoutSubtreeIfNeeded()
        hostingView.displayIfNeeded()

        let bitmap = try XCTUnwrap(
            hostingView.bitmapImageRepForCachingDisplay(in: hostingView.bounds)
        )
        hostingView.cacheDisplay(in: hostingView.bounds, to: bitmap)
        return try XCTUnwrap(bitmap.representation(using: .png, properties: [:]))
    }

    private func compare(
        actual: Data,
        expected: Data,
        size: NSSize
    ) throws -> SnapshotComparison {
        let width = Int(size.width.rounded())
        let height = Int(size.height.rounded())
        let actualPixels = try rgbaPixels(from: actual, width: width, height: height)
        let expectedPixels = try rgbaPixels(from: expected, width: width, height: height)
        var changedPixels = 0
        var channelError = 0

        for pixelOffset in stride(from: 0, to: actualPixels.count, by: 4) {
            var maximumDifference = 0
            for channelOffset in 0 ..< 4 {
                let difference = abs(
                    Int(actualPixels[pixelOffset + channelOffset])
                        - Int(expectedPixels[pixelOffset + channelOffset])
                )
                maximumDifference = max(maximumDifference, difference)
                channelError += difference
            }
            if maximumDifference > 16 {
                changedPixels += 1
            }
        }

        let pixelCount = width * height
        return SnapshotComparison(
            changedPixelRatio: Double(changedPixels) / Double(pixelCount),
            meanChannelError: Double(channelError) / Double(pixelCount * 4 * 255)
        )
    }

    private func rgbaPixels(from data: Data, width: Int, height: Int) throws -> [UInt8] {
        let image = try XCTUnwrap(NSImage(data: data))
        var proposedRect = NSRect(x: 0, y: 0, width: width, height: height)
        let source = try XCTUnwrap(
            image.cgImage(forProposedRect: &proposedRect, context: nil, hints: nil)
        )
        var pixels = [UInt8](repeating: 0, count: width * height * 4)
        let context = try XCTUnwrap(
            CGContext(
                data: &pixels,
                width: width,
                height: height,
                bitsPerComponent: 8,
                bytesPerRow: width * 4,
                space: CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
            )
        )
        context.draw(source, in: CGRect(x: 0, y: 0, width: width, height: height))
        return pixels
    }

    private func writeSnapshot(_ data: Data, named name: String) throws {
        try FileManager.default.createDirectory(
            at: snapshotDirectoryURL,
            withIntermediateDirectories: true
        )
        try data.write(to: snapshotDirectoryURL.appendingPathComponent("\(name).png"))
    }

    private var snapshotDirectoryURL: URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .appendingPathComponent("__Snapshots__", isDirectory: true)
    }

    private var snapshotRecordMarkerURL: URL {
        snapshotDirectoryURL.appendingPathComponent(".record")
    }

    private func addAttachment(data: Data, name: String) {
        let attachment = XCTAttachment(data: data, uniformTypeIdentifier: "public.png")
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}

private struct SnapshotComparison {
    let changedPixelRatio: Double
    let meanChannelError: Double
}
