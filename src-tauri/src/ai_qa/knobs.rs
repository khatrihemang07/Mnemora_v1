//! What a QA run switches off: the knobs (`Off`) and the variants that switch on a set of them. The
//! app's own behaviour is every knob off. `toolset_for` drops the change Tools before a Run;
//! `provider::strip` removes the final-Step note and the intent paragraph from each request.

use mnemora_ai::Toolset;

/// The change Tools the before variant leaves out. They are the only Tools that differ.
pub const BEFORE_ONLY_TOOLS: [&str; 2] = ["journal_changes", "todo_changes"];

/// The report's variant: which Tools and which prompt the Run is given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    /// Without `journal_changes` and `todo_changes`, and without the intent paragraph.
    Before,
    /// The app's own Tools and prompt.
    After,
}

impl Variant {
    /// The name used in the report's file name and headings.
    pub fn label(self) -> &'static str {
        match self {
            Self::Before => "before",
            Self::After => "after",
        }
    }

    /// What the variant changes, for the report's header.
    pub fn describe(self) -> &'static str {
        match self {
            Self::Before => {
                "Tools: every Tool the app offers except `journal_changes` and `todo_changes`. \
                 System prompt: without the intent paragraph (\"Work out what the person wants \
                 before you call a Tool\"). The final-Step note is kept."
            }
            Self::After => "The current code: every Tool the app offers, and the app's prompt.",
        }
    }

    /// The knobs the variant switches off.
    pub fn off(self) -> Off {
        match self {
            Self::After => Off::default(),
            Self::Before => Off {
                intent: true,
                changes: true,
                ..Off::default()
            },
        }
    }
}

/// What a QA run switches off, one knob each. All false is the app's own behaviour.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Off {
    /// The final-Step note at the end of the system prompt (`final`).
    pub final_note: bool,
    /// The intent paragraph of the system prompt (`intent`).
    pub intent: bool,
    /// `journal_changes` and `todo_changes` (`changes`).
    pub changes: bool,
}

impl Off {
    /// Reads a comma-separated list of knob names: `final`, `intent`, `changes`. Empty is the app's
    /// behaviour.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut off = Self::default();
        for name in text
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
        {
            match name {
                "final" => off.final_note = true,
                "intent" => off.intent = true,
                "changes" => off.changes = true,
                other => {
                    return Err(format!("--off takes final, intent or changes, not {other}"));
                }
            }
        }
        Ok(off)
    }

    /// The knobs on, for a report's header.
    pub fn describe(self) -> String {
        let on: Vec<&str> = [
            (self.final_note, "final-Step note"),
            (self.intent, "intent paragraph"),
            (self.changes, "journal_changes and todo_changes Tools"),
        ]
        .iter()
        .filter(|(set, _)| *set)
        .map(|(_, name)| *name)
        .collect();
        if on.is_empty() {
            "nothing (the app's own behaviour)".to_owned()
        } else {
            on.join(", ")
        }
    }

    /// Both sets of knobs together.
    #[must_use]
    pub fn with(self, other: Self) -> Self {
        Self {
            final_note: self.final_note || other.final_note,
            intent: self.intent || other.intent,
            changes: self.changes || other.changes,
        }
    }
}

/// The toolset with the change Tools removed when `off.changes` is set. Every other part is kept as
/// it is. The intent paragraph is not here: it is removed from each request (`provider::strip`).
pub fn toolset_for(off: Off, toolset: Toolset) -> Toolset {
    let tools = if off.changes {
        toolset
            .tools
            .into_iter()
            .filter(|tool| !BEFORE_ONLY_TOOLS.contains(&tool.spec().name))
            .collect()
    } else {
        toolset.tools
    };
    Toolset {
        tools,
        off: toolset.off,
    }
}
