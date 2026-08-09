//! A ladder ATP that tries a sequence of backends in order, returning the
//! first definite verdict (`Sound` or `Unsound`).

use std::time::Duration;

use mrs_core::{Formula, SymbolTable};

use super::{Atp, AtpVerdict};

/// Try each backend in turn. Stops at the first `Sound` or `Unsound`.
/// Returns `Unknown` if all backends are inconclusive.
pub struct LadderAtp {
    pub backends: Vec<Box<dyn Atp + Sync + Send>>,
}

impl LadderAtp {
    pub fn new() -> Self {
        Self {
            backends: Vec::new(),
        }
    }

    pub fn push(mut self, b: Box<dyn Atp + Sync + Send>) -> Self {
        self.backends.push(b);
        self
    }
}

impl Default for LadderAtp {
    fn default() -> Self {
        Self::new()
    }
}

impl Atp for LadderAtp {
    fn name(&self) -> &'static str {
        "ladder"
    }
    fn check_step(
        &self,
        symbols: &SymbolTable,
        premises: &[Formula],
        conclusion: &Formula,
        budget: Duration,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> AtpVerdict {
        if self.backends.is_empty() {
            return AtpVerdict::Unknown;
        }

        // 1. Run MrsAtp sequentially first (fast, in-process, avoids subprocess spawn)
        let mut remaining_backends = Vec::new();
        for b in &self.backends {
            if b.name() == "mrs" {
                match b.check_step(symbols, premises, conclusion, budget, cancel) {
                    AtpVerdict::Sound => return AtpVerdict::Sound,
                    AtpVerdict::Unsound => return AtpVerdict::Unsound,
                    AtpVerdict::Unknown => {}
                }
            } else {
                remaining_backends.push(b);
            }
        }

        if remaining_backends.is_empty() {
            return AtpVerdict::Unknown;
        }

        // 2. Run remaining external ATPs in parallel
        let (tx, rx) = std::sync::mpsc::channel();
        let cancel_flag = std::sync::atomic::AtomicBool::new(false);
        let per = std::cmp::max(Duration::from_secs(1), budget);

        std::thread::scope(|scope| {
            let num_backends = remaining_backends.len();
            for b in &remaining_backends {
                let tx = tx.clone();
                let cancel_ref = &cancel_flag;
                scope.spawn(move || {
                    let res = b.check_step(symbols, premises, conclusion, per, cancel_ref);
                    if res == AtpVerdict::Sound || res == AtpVerdict::Unsound {
                        cancel_ref.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                    let _ = tx.send(res);
                });
            }
            drop(tx);

            let mut resolved = AtpVerdict::Unknown;
            let mut received = 0;
            while received < num_backends {
                // Propagate parent cancellation:
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    cancel_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                }

                if let Ok(res) = rx.recv() {
                    received += 1;
                    match res {
                        AtpVerdict::Sound => {
                            cancel_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                            resolved = AtpVerdict::Sound;
                            break;
                        }
                        AtpVerdict::Unsound => {
                            cancel_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                            resolved = AtpVerdict::Unsound;
                            break;
                        }
                        AtpVerdict::Unknown => {}
                    }
                } else {
                    break;
                }
            }
            resolved
        })
    }

    fn search_reports(&self) -> Vec<mrs_search::ScheduleReport> {
        self.backends
            .iter()
            .flat_map(|backend| backend.search_reports())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct ReportingBackend {
        reports: Mutex<Vec<mrs_search::ScheduleReport>>,
    }

    impl ReportingBackend {
        fn new(worker_counts: &[usize]) -> Self {
            Self {
                reports: Mutex::new(
                    worker_counts
                        .iter()
                        .map(|&workers| mrs_search::ScheduleReport {
                            workers,
                            ..mrs_search::ScheduleReport::default()
                        })
                        .collect(),
                ),
            }
        }
    }

    impl Atp for ReportingBackend {
        fn name(&self) -> &'static str {
            "reporting"
        }

        fn check_step(
            &self,
            _symbols: &SymbolTable,
            _premises: &[Formula],
            _conclusion: &Formula,
            _budget: Duration,
            _cancel: &std::sync::atomic::AtomicBool,
        ) -> AtpVerdict {
            AtpVerdict::Unknown
        }

        fn search_reports(&self) -> Vec<mrs_search::ScheduleReport> {
            self.reports
                .lock()
                .map(|mut reports| std::mem::take(&mut *reports))
                .unwrap_or_default()
        }
    }

    #[test]
    fn search_reports_aggregates_and_drains_backends() {
        let ladder = LadderAtp::new()
            .push(Box::new(ReportingBackend::new(&[1, 1])))
            .push(Box::new(ReportingBackend::new(&[2])));

        let reports = ladder.search_reports();
        assert_eq!(
            reports
                .iter()
                .map(|report| report.workers)
                .collect::<Vec<_>>(),
            vec![1, 1, 2]
        );
        assert!(ladder.search_reports().is_empty());
    }
}
