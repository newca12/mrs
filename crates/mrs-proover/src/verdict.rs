//! Verdict types and SZS status formatting.

use std::fmt;

/// Final verdict for a whole proof.
///
/// Uses the current ProoVer/SZS terminology (`VerifiedGood`/`VerifiedBad`/
/// `Unknown`), not the older `Verified`/`FailedVerified`/`NotVerified` names
/// used in early ProoVer preparation material.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Every step had positive evidence of soundness.
    VerifiedGood,
    /// At least one step has positive evidence of unsoundness.
    VerifiedBad(String),
    /// We could not establish either; the proof is inconclusive.
    Unknown(String),
}

impl Verdict {
    /// Render as a `% SZS status …` line (without trailing newline).
    pub fn as_szs_line(&self) -> String {
        match self {
            Verdict::VerifiedGood => "% SZS status VerifiedGood".to_string(),
            Verdict::VerifiedBad(reason) => {
                if reason.is_empty() {
                    "% SZS status VerifiedBad".to_string()
                } else {
                    format!("% SZS status VerifiedBad : {reason}")
                }
            }
            Verdict::Unknown(reason) => {
                if reason.is_empty() {
                    "% SZS status Unknown".to_string()
                } else {
                    format!("% SZS status Unknown : {reason}")
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
/// - any `Unsound` → `VerifiedBad` (first reason wins).
/// - else any `Unknown` → `Unknown` (first reason wins).
/// - else `VerifiedGood`.
pub fn aggregate<'a, I>(outcomes: I) -> Verdict
where
    I: IntoIterator<Item = (&'a str, StepOutcome)>,
{
    let mut first_unknown: Option<String> = None;
    for (name, oc) in outcomes {
        match oc {
            StepOutcome::Unsound(why) => {
                return Verdict::VerifiedBad(format!("step {name}: {why}"));
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
        Some(why) => Verdict::Unknown(why),
        None => Verdict::VerifiedGood,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_all_sound() {
        let v = aggregate(vec![("s1", StepOutcome::Sound), ("s2", StepOutcome::Sound)]);
        assert_eq!(v, Verdict::VerifiedGood);
    }

    #[test]
    fn aggregate_unsound_dominates_unknown() {
        let v = aggregate(vec![
            ("s1", StepOutcome::Unknown("timeout".into())),
            ("s2", StepOutcome::Unsound("bad".into())),
        ]);
        assert!(matches!(v, Verdict::VerifiedBad(_)));
    }

    #[test]
    fn aggregate_unknown_when_no_unsound() {
        let v = aggregate(vec![
            ("s1", StepOutcome::Sound),
            ("s2", StepOutcome::Unknown("timeout".into())),
        ]);
        assert!(matches!(v, Verdict::Unknown(_)));
    }
}
