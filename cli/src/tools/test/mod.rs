pub mod discovery;
pub mod reporters;
pub mod types;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Context;
use anyhow::Error;
use base::runtime::DenoRuntime;
use base::server::ServerFlags;
use base::worker::WorkerBuilder;
use deno_core::url::Url;
use ext_workers::context::MainWorkerRuntimeOpts;
use ext_workers::context::UserWorkerRuntimeOpts;
use ext_workers::context::WorkerContextInitOpts;
use ext_workers::context::WorkerRuntimeOpts;
use tokio::sync::mpsc;

use crate::flags::TestFlags;
use crate::flags::TestReporterKind;
use crate::flags::TestWorkerKind;
use self::discovery::discover_test_files;
use self::reporters::JunitTestReporter;
use self::reporters::PrettyTestReporter;
use self::reporters::TestReporter;
use self::types::{
    JsTestSummary, TestFilter, TestPlan, TestResult, TestStepDescription, TestStepResult,
    TestSummary,
};

/// A step event collected from recursive step processing.
struct StepEvent {
    description: TestStepDescription,
    result: TestStepResult,
    elapsed: u64,
}

// --- Test orchestration ---

pub async fn run_tests(flags: TestFlags) -> Result<bool, Error> {
    let start = Instant::now();

    // 1. Discover test files
    let test_files = discover_test_files(&flags.paths);

    if test_files.is_empty() {
        eprintln!("No test files found");
        return Ok(false);
    }

    eprintln!(
        "Found {} test file{}",
        test_files.len(),
        if test_files.len() == 1 { "" } else { "s" }
    );

    // 2. Determine the service path from the user-provided paths (not discovered files).
    //    This ensures the worker's base directory matches the project root, so relative
    //    path resolution (e.g., for .env files, imports) works correctly.
    let service_path = resolve_service_path(&flags.paths);

    // 3. Create a temporary entrypoint file in the service path that imports all test files
    let entrypoint_path = service_path.join(".__edge_test_entrypoint.ts");
    let mut import_lines = Vec::new();
    for test_file in &test_files {
        let abs_path = test_file
            .canonicalize()
            .unwrap_or_else(|_| test_file.clone());
        let rel_path = pathdiff::diff_paths(&abs_path, &service_path)
            .unwrap_or_else(|| abs_path.clone());
        let rel_str = rel_path.to_string_lossy().replace('\\', "/");
        let rel_str = if rel_str.starts_with("./") || rel_str.starts_with("../") {
            rel_str.to_string()
        } else {
            format!("./{}", rel_str)
        };
        import_lines.push(format!("import \"{}\";", rel_str));
    }
    std::fs::write(&entrypoint_path, import_lines.join("\n"))
        .context("failed to write test entrypoint file")?;

    let _cleanup = scopeguard::guard(entrypoint_path.clone(), |path| {
        let _ = std::fs::remove_file(&path);
    });

    let entrypoint_url = Url::from_file_path(&entrypoint_path)
        .map_err(|_| anyhow::anyhow!("failed to convert entrypoint path to URL"))?;

    // 4. Create worker init opts
    let (worker_pool_tx, _worker_pool_rx) =
        mpsc::unbounded_channel::<ext_workers::context::UserWorkerMsgs>();

    let conf = match flags.worker_kind {
        TestWorkerKind::Main => {
            WorkerRuntimeOpts::MainWorker(MainWorkerRuntimeOpts {
                worker_pool_tx,
                shared_metric_src: None,
                event_worker_metric_src: None,
                context: None,
            })
        }
        TestWorkerKind::User => {
            WorkerRuntimeOpts::UserWorker(UserWorkerRuntimeOpts {
                service_path: None,
                key: None,
                pool_msg_tx: None,
                events_msg_tx: None,
                cancel: None,
                memory_limit_mb: 512,
                low_memory_multiplier: 5,
                worker_timeout_ms: flags.timeout_ms,
                cpu_time_max_budget_per_task_ms: None,
                cpu_time_soft_limit_ms: flags.timeout_ms,
                cpu_time_hard_limit_ms: flags.timeout_ms * 2,
                beforeunload_wall_clock_pct: None,
                beforeunload_cpu_pct: None,
                beforeunload_memory_pct: None,
                force_create: false,
                allow_remote_modules: true,
                custom_module_root: None,
                permissions: None,
                context: None,
            })
        }
    };

    let init_opts = WorkerContextInitOpts {
        service_path: service_path.clone(),
        no_module_cache: flags.no_module_cache,
        no_npm: None,
        env_vars: std::env::vars().collect(),
        conf,
        static_patterns: vec![],
        timing: None,
        maybe_eszip: None,
        maybe_module_code: None,
        maybe_entrypoint: Some(entrypoint_url.to_string()),
        maybe_s3_fs_config: None,
        maybe_tmp_fs_config: None,
        maybe_otel_config: None,
        test_mode: true,
    };

    let server_flags = Arc::new(ServerFlags::default());

    // 5. Build worker and create runtime
    let worker = WorkerBuilder::new(init_opts, server_flags)
        .build()
        .context("failed to build test worker")?;

    let mut runtime = DenoRuntime::<()>::new(worker)
        .await
        .context("failed to create test runtime")?;

    // 6. Load and evaluate the main module
    runtime
        .init_main_module()
        .await
        .context("failed to initialize main module")?;

    runtime
        .evaluate_main_module()
        .await
        .context("failed to evaluate test modules")?;

    // 7. Run the collected tests via __test_runner.run()
    let test_filter = flags.filter.as_ref().map(|f| TestFilter::from_flag(f));
    let filter_js = match &test_filter {
        Some(f) => f.to_js_arg(),
        None => "null".to_string(),
    };
    let fail_fast_js = if flags.fail_fast { "true" } else { "false" };

    let run_script = format!(
        r#"globalThis.__test_result = null;
globalThis.__test_runner.run({}, {}).then(r => {{ globalThis.__test_result = r; }})"#,
        filter_js, fail_fast_js
    );

    runtime
        .execute_script_with_locker("[test_runner_start]", run_script, true)
        .await
        .context("failed to run test suite")?;

    let result_global = runtime
        .execute_script_with_locker(
            "[test_runner_result]",
            "globalThis.__test_result".to_string(),
            false,
        )
        .await
        .context("failed to read test results")?;

    let result_str = runtime.read_global_as_string(result_global).await?;

    // 8. Parse JSON results
    let js_summary: JsTestSummary =
        serde_json::from_str(&result_str).context("failed to parse test results JSON")?;

    let total_duration = start.elapsed();
    let origin = "test suite".to_string();

    let plan = TestPlan {
        origin: origin.clone(),
        total: js_summary.results.len(),
        filtered_out: js_summary.filtered_out as usize,
        used_only: false,
    };

    // 9. Set up reporters
    let use_pretty = !matches!(flags.reporter, TestReporterKind::Junit)
        || flags.junit_path.is_some();
    let use_junit = flags.junit_path.is_some()
        || matches!(flags.reporter, TestReporterKind::Junit);

    let mut pretty: Option<PrettyTestReporter<std::io::Stderr>> =
        if use_pretty { Some(PrettyTestReporter::new()) } else { None };
    let mut junit: Option<JunitTestReporter> =
        if use_junit { Some(JunitTestReporter::new()) } else { None };

    // Report plan
    if let Some(ref mut r) = pretty { r.report_plan(&plan); }
    if let Some(ref mut r) = junit { r.report_plan(&plan); }

    // 10. Process results through event-method pattern
    let mut summary = TestSummary::new();
    summary.total = js_summary.results.len();
    summary.filtered_out = js_summary.filtered_out as usize;

    let mut next_id = 0usize;

    for js_result in &js_summary.results {
        let id = next_id;
        next_id += 1;

        let description = js_result.to_description(id, &origin);
        let result = js_result.to_test_result();

        // Report wait
        if let Some(ref mut r) = pretty { r.report_wait(&description); }
        if let Some(ref mut r) = junit { r.report_wait(&description); }

        // Collect step events (avoids recursive borrow issues with reporters)
        let mut step_events = Vec::new();
        collect_step_events(
            &js_result.steps,
            &mut step_events,
            &mut next_id,
            1,
            id,
            id,
            &description.name,
        );

        // Report step events
        for event in &step_events {
            if let Some(ref mut r) = pretty {
                r.report_step_wait(&event.description);
                r.report_step_result(&event.description, &event.result, event.elapsed);
            }
            if let Some(ref mut r) = junit {
                r.report_step_wait(&event.description);
                r.report_step_result(&event.description, &event.result, event.elapsed);
            }

            match &event.result {
                TestStepResult::Ok => summary.passed_steps += 1,
                TestStepResult::Ignored => summary.ignored_steps += 1,
                TestStepResult::Failed(_) => summary.failed_steps += 1,
            }
        }

        // Report result
        if let Some(ref mut r) = pretty { r.report_result(&description, &result, js_result.duration); }
        if let Some(ref mut r) = junit { r.report_result(&description, &result, js_result.duration); }

        // Update summary
        match &result {
            TestResult::Ok => summary.passed += 1,
            TestResult::Ignored => summary.ignored += 1,
            TestResult::Failed(failure) => {
                summary.failed += 1;
                summary.failures.push((description, failure.clone()));
            }
            TestResult::Cancelled => summary.ignored += 1,
        }
    }

    // Report summary
    if let Some(ref mut r) = pretty { r.report_summary(&summary, &total_duration); }
    if let Some(ref mut r) = junit { r.report_summary(&summary, &total_duration); }

    // 11. Handle JUnit XML output
    if let Some(ref junit_reporter) = junit {
        let xml = junit_reporter.generate_xml(total_duration);

        if let Some(ref junit_path) = flags.junit_path {
            std::fs::write(junit_path, &xml)
                .with_context(|| format!("failed to write JUnit XML to {}", junit_path))?;
        } else {
            print!("{}", xml);
        }
    }

    // 12. Return success/failure
    Ok(summary.failed == 0)
}

/// Recursively collect step events into a flat list for reporting.
fn collect_step_events(
    steps: &[types::JsStepResult],
    events: &mut Vec<StepEvent>,
    next_id: &mut usize,
    level: usize,
    parent_id: usize,
    root_id: usize,
    root_name: &str,
) {
    for js_step in steps {
        let id = *next_id;
        *next_id += 1;

        // Collect nested steps first (depth-first)
        collect_step_events(
            &js_step.steps,
            events,
            next_id,
            level + 1,
            id,
            root_id,
            root_name,
        );

        let description = js_step.to_description(id, level, parent_id, root_id, root_name);
        let result = js_step.to_step_result();

        events.push(StepEvent {
            description,
            result,
            elapsed: js_step.duration,
        });
    }
}

/// Derive the service path (project root) from user-provided CLI paths.
///
/// - If any path is a directory, use the first directory path.
/// - If all paths are files, use the parent of the first file.
/// - Falls back to cwd.
fn resolve_service_path(paths: &[String]) -> PathBuf {
    for path_str in paths {
        let path = PathBuf::from(path_str);
        if path.is_dir() {
            return path
                .canonicalize()
                .unwrap_or(path);
        }
    }

    // All paths are files — use the parent of the first one
    if let Some(first) = paths.first() {
        let path = PathBuf::from(first);
        if let Some(parent) = path.canonicalize().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())) {
            return parent;
        }
    }

    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}
