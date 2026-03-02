mod pretty;
mod junit;

pub use pretty::PrettyTestReporter;
pub use junit::JunitTestReporter;

use std::time::Duration;

use super::types::{
    TestDescription, TestPlan, TestResult, TestStepDescription, TestStepResult,
    TestSummary,
};

/// Test reporter trait aligned with Deno's event-method pattern.
///
/// The orchestrator calls these methods per-event rather than in batch:
/// 1. `report_plan()` — once, before any tests run
/// 2. `report_wait()` / `report_result()` — per test
/// 3. `report_step_wait()` / `report_step_result()` — per test step
/// 4. `report_summary()` — once, after all tests complete
/// 5. `flush_report()` — final output (e.g., write XML file)
pub trait TestReporter {
    fn report_plan(&mut self, plan: &TestPlan);
    fn report_wait(&mut self, description: &TestDescription);
    fn report_result(
        &mut self,
        description: &TestDescription,
        result: &TestResult,
        elapsed: u64,
    );
    fn report_step_wait(&mut self, description: &TestStepDescription);
    fn report_step_result(
        &mut self,
        description: &TestStepDescription,
        result: &TestStepResult,
        elapsed: u64,
    );
    fn report_summary(&mut self, summary: &TestSummary, elapsed: &Duration);
    fn flush_report(&mut self, elapsed: &Duration) -> anyhow::Result<()>;
}
