//! What a Conversation's branch says to the Model, in order.

/// One message of a Run's request. Tool messages join in when Tools do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// The person's question.
    User(String),
    /// An answer the Model gave earlier in the branch.
    Assistant(String),
}
