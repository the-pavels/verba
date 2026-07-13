#if DEBUG
extension PresentationViewModel {
    static let translationPreview = Self.translation(
        originalText: "Guten Morgen! Können Sie mir bitte helfen?",
        languagePair: LanguagePairViewModel(source: "German", target: "English"),
        translatedText: "Good morning! Could you please help me?"
    )
}
#endif
