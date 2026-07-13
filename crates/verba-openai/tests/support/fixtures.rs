pub enum ExpectedOutcome {
    NoIssues,
    Corrected {
        text: &'static str,
        explanation: &'static str,
    },
}

pub struct PromptFixture {
    pub name: &'static str,
    pub input: &'static str,
    pub expected: ExpectedOutcome,
}

pub const FIXTURES: &[PromptFixture] = &[
    PromptFixture {
        name: "no changes",
        input: "This sentence is already correct.",
        expected: ExpectedOutcome::NoIssues,
    },
    PromptFixture {
        name: "multilingual text",
        input: "Das sind ein Test. Esta frase está bien.",
        expected: ExpectedOutcome::Corrected {
            text: "Das ist ein Test. Esta frase está bien.",
            explanation: "Corrected German subject-verb agreement.",
        },
    },
    PromptFixture {
        name: "punctuation",
        input: "Wait she said.",
        expected: ExpectedOutcome::Corrected {
            text: "Wait, she said.",
            explanation: "Added the missing comma.",
        },
    },
    PromptFixture {
        name: "paragraphs",
        input: "The first paragraph are here.\n\nThe second paragraph stays unchanged.",
        expected: ExpectedOutcome::Corrected {
            text: "The first paragraph is here.\n\nThe second paragraph stays unchanged.",
            explanation: "Corrected subject-verb agreement in the first paragraph.",
        },
    },
    PromptFixture {
        name: "lists",
        input: "- Apples are fresh\n- This orange taste sweet\n  - Nested item stays",
        expected: ExpectedOutcome::Corrected {
            text: "- Apples are fresh\n- This orange tastes sweet\n  - Nested item stays",
            explanation: "Corrected subject-verb agreement in the second list item.",
        },
    },
    PromptFixture {
        name: "quoted text",
        input: "She said, \"This are ready.\"",
        expected: ExpectedOutcome::Corrected {
            text: "She said, \"This is ready.\"",
            explanation: "Corrected subject-verb agreement inside the quotation.",
        },
    },
    PromptFixture {
        name: "formatting preservation",
        input: "  **This are bold.**  ",
        expected: ExpectedOutcome::Corrected {
            text: "  **This is bold.**  ",
            explanation: "Corrected subject-verb agreement while preserving formatting.",
        },
    },
];
