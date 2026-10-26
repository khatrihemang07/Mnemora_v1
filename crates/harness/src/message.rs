//! What a Run's branch says to the Model, in order.

/// One Tool call the Model asked for: its id, the Tool's name and its arguments as sent (JSON text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    /// The id the Provider gave the call; its result answers it.
    pub id: String,
    /// The Tool's name.
    pub name: String,
    /// The arguments, exactly as the Model sent them.
    pub arguments: String,
    /// Opaque reasoning token a Provider attached to this call (Gemini's `thought_signature`). It
    /// is sent back verbatim on the same call in later Steps, and no other Provider sets it.
    pub thought_signature: Option<String>,
}

/// One message of a Run's request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// The person's question.
    User(String),
    /// An answer or a Step that asked for Tools, as the Model wrote it.
    Assistant {
        /// The text the Model wrote in that Step.
        text: String,
        /// The Tool calls it made in that Step.
        calls: Vec<Call>,
    },
    /// The result of one Tool call, read back to the Model.
    Tool {
        /// The call this answers.
        call_id: String,
        /// The Tool's name.
        name: String,
        /// What the Model reads.
        text: String,
        /// Whether the Tool refused the call.
        is_error: bool,
    },
    /// A correction turn: the Model is told what to fix and answers again.
    Correction(String),
}
