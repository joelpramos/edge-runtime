use std::fmt;

use serde::Deserialize;

// --- Core types aligned with Deno's cli/tools/test/ conventions ---

/// Describes a registered test (Deno's `TestDescription`).
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TestDescription {
    pub id: usize,
    pub name: String,
    pub ignore: bool,
    pub only: bool,
    pub origin: String,
}

/// Describes a test step (Deno's `TestStepDescription`).
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TestStepDescription {
    pub id: usize,
    pub name: String,
    pub level: usize,
    pub parent_id: usize,
    pub root_id: usize,
    pub root_name: String,
}

/// Result of a single test (Deno's `TestResult`).
#[derive(Debug, Clone)]
pub enum TestResult {
    Ok,
    Ignored,
    Failed(TestFailure),
    Cancelled,
}

/// Result of a test step (Deno's `TestStepResult`).
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum TestStepResult {
    Ok,
    Ignored,
    Failed(TestFailure),
}

/// How a test failed (Deno's `TestFailure`).
#[derive(Debug, Clone)]
pub enum TestFailure {
    JsError(String),
    FailedSteps(usize),
}

impl fmt::Display for TestFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TestFailure::JsError(s) => write!(f, "{}", s),
            TestFailure::FailedSteps(n) => write!(f, "{} step(s) failed", n),
        }
    }
}

/// Plan communicated before test execution (Deno's `TestPlan`).
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TestPlan {
    pub origin: String,
    pub total: usize,
    pub filtered_out: usize,
    pub used_only: bool,
}

/// Aggregated test summary (Deno's `TestSummary`).
#[derive(Debug, Clone)]
pub struct TestSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub ignored: usize,
    pub passed_steps: usize,
    pub failed_steps: usize,
    pub ignored_steps: usize,
    pub filtered_out: usize,
    pub failures: Vec<(TestDescription, TestFailure)>,
}

impl TestSummary {
    pub fn new() -> Self {
        Self {
            total: 0,
            passed: 0,
            failed: 0,
            ignored: 0,
            passed_steps: 0,
            failed_steps: 0,
            ignored_steps: 0,
            filtered_out: 0,
            failures: Vec::new(),
        }
    }
}

// --- JSON deserialization types (from JS test runner) ---
// These intermediate types map the JSON blob to our Deno-aligned enums.

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct JsTestSummary {
    pub total: u32,
    pub passed: u32,
    pub failed: u32,
    pub ignored: u32,
    #[serde(default)]
    pub filtered_out: u32,
    #[serde(default)]
    pub passed_steps: u32,
    #[serde(default)]
    pub failed_steps: u32,
    #[serde(default)]
    pub ignored_steps: u32,
    pub results: Vec<JsTestResult>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct JsTestResult {
    pub name: String,
    pub status: String,
    pub duration: u64,
    pub error: Option<String>,
    #[serde(default)]
    pub steps: Vec<JsStepResult>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct JsStepResult {
    pub name: String,
    pub status: String,
    pub duration: u64,
    pub error: Option<String>,
    #[serde(default)]
    pub steps: Vec<JsStepResult>,
}

// --- Conversion from JS types to Deno-aligned types ---

impl JsTestResult {
    pub fn to_test_result(&self) -> TestResult {
        match self.status.as_str() {
            "passed" => TestResult::Ok,
            "ignored" => TestResult::Ignored,
            "failed" => {
                let failure = match &self.error {
                    Some(err) => TestFailure::JsError(err.clone()),
                    None => {
                        let failed_steps = count_failed_steps(&self.steps);
                        if failed_steps > 0 {
                            TestFailure::FailedSteps(failed_steps)
                        } else {
                            TestFailure::JsError("unknown error".to_string())
                        }
                    }
                };
                TestResult::Failed(failure)
            }
            "cancelled" => TestResult::Cancelled,
            _ => TestResult::Failed(TestFailure::JsError(format!(
                "unknown status: {}",
                self.status
            ))),
        }
    }

    pub fn to_description(&self, id: usize, origin: &str) -> TestDescription {
        TestDescription {
            id,
            name: self.name.clone(),
            ignore: self.status == "ignored",
            only: false,
            origin: origin.to_string(),
        }
    }
}

impl JsStepResult {
    pub fn to_step_result(&self) -> TestStepResult {
        match self.status.as_str() {
            "passed" => TestStepResult::Ok,
            "ignored" => TestStepResult::Ignored,
            "failed" => {
                let failure = match &self.error {
                    Some(err) => TestFailure::JsError(err.clone()),
                    None => TestFailure::JsError("unknown error".to_string()),
                };
                TestStepResult::Failed(failure)
            }
            _ => TestStepResult::Failed(TestFailure::JsError(format!(
                "unknown status: {}",
                self.status
            ))),
        }
    }

    pub fn to_description(
        &self,
        id: usize,
        level: usize,
        parent_id: usize,
        root_id: usize,
        root_name: &str,
    ) -> TestStepDescription {
        TestStepDescription {
            id,
            name: self.name.clone(),
            level,
            parent_id,
            root_id,
            root_name: root_name.to_string(),
        }
    }
}

fn count_failed_steps(steps: &[JsStepResult]) -> usize {
    let mut count = 0;
    for step in steps {
        if step.status == "failed" {
            count += 1;
        }
        count += count_failed_steps(&step.steps);
    }
    count
}

// --- Test filter ---

/// Filter for test names. Supports substring and regex matching (Deno parity).
/// If the `--filter` value is wrapped in `/slashes/`, it's compiled as a regex.
/// Otherwise, it's a substring match.
#[derive(Debug)]
pub struct TestFilter {
    pub substring: Option<String>,
    pub regex: Option<regex::Regex>,
}

impl TestFilter {
    /// Construct a TestFilter from the `--filter` flag value.
    pub fn from_flag(s: &str) -> Self {
        if s.starts_with('/') && s.ends_with('/') && s.len() > 2 {
            let pattern = &s[1..s.len() - 1];
            match regex::Regex::new(pattern) {
                Ok(re) => TestFilter {
                    substring: None,
                    regex: Some(re),
                },
                Err(err) => {
                    eprintln!(
                        "warning: invalid regex filter '{}': {}, falling back to substring",
                        pattern, err
                    );
                    TestFilter {
                        substring: Some(s.to_string()),
                        regex: None,
                    }
                }
            }
        } else {
            TestFilter {
                substring: Some(s.to_string()),
                regex: None,
            }
        }
    }

    /// Serialize to JSON for passing to the JS test runner.
    pub fn to_js_arg(&self) -> String {
        match (&self.substring, &self.regex) {
            (Some(s), _) => {
                let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
                format!("{{\"substring\":\"{}\"}}", escaped)
            }
            (_, Some(re)) => {
                let escaped = re.as_str().replace('\\', "\\\\").replace('"', "\\\"");
                format!("{{\"regex\":\"{}\"}}", escaped)
            }
            (None, None) => "null".to_string(),
        }
    }
}

