//! Verdict types and SZS status formatting.

use std::fmt;

/// Final verdict for a whole proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Every step had positive evidence of soundness.
    Verified,
    /// At least one step has positive evidence of unsoundness.
    FailedVerified(String),
    /// We could not establish either; the proof is inconclusive.
    NotVerified(String),
}

impl Verdict {
    /// Render as a `% SZS status …` line (without trailing newline).
    pub fn as_szs_line(&self) -> String {
        match self {
            Verdict::Verified => "% SZS status Verified".to_string(),
            Verdict::FailedVerified(reason) => {
                if reason.is_empty() {
                    "% SZS status FailedVerified".to_string()
                } else {
                    format!("% SZS status FailedVerified : {reason}")
                }
            }
            Verdict::NotVerified(reason) => {
                if reason.is_empty() {
                    "% SZS status NotVerified".to_string()
                } else {
                    format!("% SZS status NotVerified : {reason}")
                }
            }
        }
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_szs_line())
    }
}

/// Per-step outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepOutcome {
    /// The step has been confirmed sound.
    Sound,
    /// The step is unsound; carry a reason for the SZS line.
    Unsound(String),
    /// We could not decide; carry a reason.
    Unknown(String),
}

/// Aggregate per-step outcomes into a final verdict.
///
/// Policy:
/// - any `Unsound` → `FailedVerified` (first reason wins).
/// - else any `Unknown` → `NotVerified` (first reason wins).
/// - else `Verified`.
pub fn aggregate<'a, I>(outcomes: I) -> Verdict
where
    I: IntoIterator<Item = (&'a str, StepOutcome)>,
{
    let mut first_unknown: Option<String> = None;
    for (name, oc) in outcomes {
        match oc {
            StepOutcome::Unsound(why) => {
                return Verdict::FailedVerified(format!("step {name}: {why}"));
            }
            StepOutcome::Unknown(why) => {
                if first_unknown.is_none() {
                    first_unknown = Some(format!("step {name}: {why}"));
                }
            }
            StepOutcome::Sound => {}
        }
    }
    match first_unknown {
        Some(why) => Verdict::NotVerified(why),
        None => Verdict::Verified,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_all_sound() {
        let v = aggregate(vec![("s1", StepOutcome::Sound), ("s2", StepOutcome::Sound)]);
        assert_eq!(v, Verdict::Verified);
    }

    #[test]
    fn aggregate_unsound_dominates_unknown() {
        let v = aggregate(vec![
            ("s1", StepOutcome::Unknown("timeout".into())),
            ("s2", StepOutcome::Unsound("bad".into())),
        ]);
        assert!(matches!(v, Verdict::FailedVerified(_)));
    }

    #[test]
    fn aggregate_unknown_when_no_unsound() {
        let v = aggregate(vec![
            ("s1", StepOutcome::Sound),
            ("s2", StepOutcome::Unknown("timeout".into())),
        ]);
        assert!(matches!(v, Verdict::NotVerified(_)));
    }
}
