#if DEBUG
extension PresentationViewModel {
    static let translationPreview = Self.translation(
        originalText: "Guten Morgen! Können Sie mir bitte helfen?",
        languagePair: LanguagePairViewModel(source: "German", target: "English"),
        translatedText: "Good morning! Could you please help me?"
    )

    static let proofreadingPreview = Self.proofreading(
        originalText: "This sentence now grammatically correct",
        correctedText: "This sentence is now grammatically correct.",
        explanation: "Added the missing verb and adjusted the punctuation."
    )

    static let errorPreview = Self.error(
        action: .translate,
        title: "Translation failed",
        message: "Check your internet connection and try again.",
        recovery: .retry,
        diagnosticCode: "translation.preview"
    )
}
#endif
