use std::io::Write;
use std::time::Duration;

use super::TestReporter;
use crate::tools::test::types::{
    TestDescription, TestPlan, TestResult, TestStepDescription, TestStepResult,
    TestSummary,
};

// --- ANSI colors ---

const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";

// --- Pretty Test Reporter ---

pub struct PrettyTestReporter<W: Write> {
    writer: W,
}

impl PrettyTestReporter<std::io::Stderr> {
    pub fn new() -> Self {
        Self {
            writer: std::io::stderr(),
        }
    }
}

impl<W: Write> TestReporter for PrettyTestReporter<W> {
    fn report_plan(&mut self, plan: &TestPlan) {
        let _ = writeln!(
            self.writer,
            "running {} test{} from {}",
            plan.total,
            if plan.total == 1 { "" } else { "s" },
            plan.origin
        );
    }

    fn report_wait(&mut self, _description: &TestDescription) {
        // In Deno, this prints "test <name> ..." before the test runs.
        // We defer the line to report_result for simpler output.
    }

    fn report_result(
        &mut self,
        description: &TestDescription,
        result: &TestResult,
        elapsed: u64,
    ) {
        let (color, status_text) = match result {
            TestResult::Ok => (GREEN, "ok"),
            TestResult::Ignored => (YELLOW, "ignored"),
            TestResult::Failed(_) => (RED, "FAILED"),
            TestResult::Cancelled => (YELLOW, "cancelled"),
        };
        let _ = writeln!(
            self.writer,
            "  {} ... {}{}{} ({}ms)",
            description.name, color, status_text, RESET, elapsed
        );
    }

    fn report_step_wait(&mut self, _description: &TestStepDescription) {
        // Deferred to report_step_result.
    }

    fn report_step_result(
        &mut self,
        description: &TestStepDescription,
        result: &TestStepResult,
        elapsed: u64,
    ) {
        let indent = "  ".repeat(description.level + 1);
        let (color, status_text) = match result {
            TestStepResult::Ok => (GREEN, "ok"),
            TestStepResult::Ignored => (YELLOW, "ignored"),
            TestStepResult::Failed(_) => (RED, "FAILED"),
        };
        let _ = writeln!(
            self.writer,
            "{}  {} ... {}{}{} ({}ms)",
            indent, description.name, color, status_text, RESET, elapsed
        );
    }

    fn report_summary(&mut self, summary: &TestSummary, elapsed: &Duration) {
        // Print failures section
        if !summary.failures.is_empty() {
            let _ = writeln!(self.writer);
            let _ = writeln!(self.writer, "{}{}FAILURES{}", BOLD, RED, RESET);
            for (desc, failure) in &summary.failures {
                let _ = writeln!(self.writer);
                let _ = writeln!(self.writer, "--- {} ---", desc.name);
                let _ = writeln!(self.writer, "{}", failure);
            }
        }

        // Print summary line
        let _ = writeln!(self.writer);

        let status = if summary.failed > 0 {
            format!("{}{}FAILED{}", BOLD, RED, RESET)
        } else {
            format!("{}{}ok{}", BOLD, GREEN, RESET)
        };

        let _ = writeln!(
            self.writer,
            "test result: {}. {} passed; {} failed; {} ignored ({:.2}s)",
            status,
            summary.passed,
            summary.failed,
            summary.ignored,
            elapsed.as_secs_f64()
        );
    }

    fn flush_report(&mut self, _elapsed: &Duration) -> anyhow::Result<()> {
        Ok(())
    }
}
