use std::time::Duration;

use super::TestReporter;
use crate::tools::test::types::{
    TestDescription, TestPlan, TestResult, TestStepDescription, TestStepResult,
    TestSummary,
};

// --- JUnit XML Test Reporter ---

struct JunitTestCase {
    name: String,
    classname: String,
    time: f64,
    failure: Option<String>,
    skipped: bool,
}

pub struct JunitTestReporter {
    origin: String,
    cases: Vec<JunitTestCase>,
}

impl JunitTestReporter {
    pub fn new() -> Self {
        Self {
            origin: String::new(),
            cases: Vec::new(),
        }
    }

    pub fn generate_xml(&self, total_duration: Duration) -> String {
        let total_tests = self.cases.len();
        let total_failures = self.cases.iter().filter(|c| c.failure.is_some()).count();

        let mut suites = String::new();

        let suite_tests = self.cases.len();
        let suite_failures = self.cases.iter().filter(|c| c.failure.is_some()).count();
        let suite_time: f64 = self.cases.iter().map(|c| c.time).sum();

        suites.push_str(&format!(
            "  <testsuite name=\"{}\" tests=\"{}\" failures=\"{}\" time=\"{:.3}\">\n",
            xml_escape(&self.origin),
            suite_tests,
            suite_failures,
            suite_time
        ));

        for case in &self.cases {
            suites.push_str(&format!(
                "    <testcase name=\"{}\" classname=\"{}\" time=\"{:.3}\"",
                xml_escape(&case.name),
                xml_escape(&case.classname),
                case.time
            ));

            if let Some(ref failure_msg) = case.failure {
                suites.push_str(&format!(
                    ">\n      <failure message=\"test failed\">{}</failure>\n    </testcase>\n",
                    xml_escape(failure_msg)
                ));
            } else if case.skipped {
                suites.push_str(">\n      <skipped />\n    </testcase>\n");
            } else {
                suites.push_str(" />\n");
            }
        }

        suites.push_str("  </testsuite>\n");

        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuites tests=\"{}\" failures=\"{}\" time=\"{:.3}\">\n{}</testsuites>\n",
            total_tests,
            total_failures,
            total_duration.as_secs_f64(),
            suites
        )
    }
}

impl TestReporter for JunitTestReporter {
    fn report_plan(&mut self, plan: &TestPlan) {
        self.origin = plan.origin.clone();
    }

    fn report_wait(&mut self, _description: &TestDescription) {}

    fn report_result(
        &mut self,
        description: &TestDescription,
        result: &TestResult,
        elapsed: u64,
    ) {
        let failure = match result {
            TestResult::Failed(f) => Some(f.to_string()),
            _ => None,
        };
        let skipped = matches!(result, TestResult::Ignored | TestResult::Cancelled);

        self.cases.push(JunitTestCase {
            name: description.name.clone(),
            classname: self.origin.clone(),
            time: elapsed as f64 / 1000.0,
            failure,
            skipped,
        });
    }

    fn report_step_wait(&mut self, _description: &TestStepDescription) {}

    fn report_step_result(
        &mut self,
        _description: &TestStepDescription,
        _result: &TestStepResult,
        _elapsed: u64,
    ) {
        // JUnit doesn't support nested test steps natively
    }

    fn report_summary(&mut self, _summary: &TestSummary, _elapsed: &Duration) {}

    fn flush_report(&mut self, elapsed: &Duration) -> anyhow::Result<()> {
        // XML generation happens when explicitly called via generate_xml()
        let _ = elapsed;
        Ok(())
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
