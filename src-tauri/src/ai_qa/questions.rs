//! The questions a QA run asks: the built-in ten, or a JSON list from `--questions`.

/// The questions asked when `--questions` is not given.
pub const BUILT_IN_QUESTIONS: [&str; 10] = [
    "What did I do yesterday?",
    "What did I get done last week?",
    "What's overdue?",
    "What did I complete yesterday versus what did I only create?",
    "What was on my mind in my journal this week?",
    "What happened on Wednesday 21 October?",
    "Hi",
    "What exactly did I write in the Entry about the dog barking?",
    "Any updates on the passport?",
    "How many tasks did I finish this week?",
];

/// The questions in a JSON list of strings, which `--questions` reads.
pub fn load_questions(text: &str) -> Result<Vec<String>, String> {
    serde_json::from_str::<Vec<String>>(text)
        .map_err(|e| format!("the questions file must be a JSON list of strings: {e}"))
}
