use std::error::Error;
use std::path::PathBuf;

use tank_harness::calibration::CalibrationReport;
use tank_harness::validation_suite::run_calibration_suite;

struct Args {
    output_dir: PathBuf,
    parameter_set: String,
    compare_to: Option<PathBuf>,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("calibration_report: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args = parse_args()?;
    let report = run_calibration_suite(&args.parameter_set)?;
    let artifacts = report.write_bundle(&args.output_dir)?;

    println!("{}", report.render_text_summary());
    println!("JSON report: {}", artifacts.report_json.display());
    println!("Text summary: {}", artifacts.summary_txt.display());

    if let Some(compare_to) = args.compare_to {
        let before: CalibrationReport =
            serde_json::from_str(&std::fs::read_to_string(compare_to)?)?;
        let comparison = before.compare(&report);
        let comparison_artifacts = comparison.write_bundle(&args.output_dir)?;
        println!("{}", comparison.render_text_summary());
        println!(
            "Comparison JSON: {}",
            comparison_artifacts.comparison_json.display()
        );
        println!(
            "Comparison text: {}",
            comparison_artifacts.comparison_txt.display()
        );
    }

    Ok(())
}

fn parse_args() -> Result<Args, Box<dyn Error>> {
    let mut output_dir = PathBuf::from("artifacts/calibration/latest");
    let mut parameter_set = String::from("default");
    let mut compare_to = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output-dir" => {
                output_dir = PathBuf::from(
                    args.next()
                        .ok_or_else(|| usage_error("--output-dir requires a path"))?,
                );
            }
            "--parameter-set" => {
                parameter_set = args
                    .next()
                    .ok_or_else(|| usage_error("--parameter-set requires a value"))?;
            }
            "--compare-to" => {
                compare_to =
                    Some(PathBuf::from(args.next().ok_or_else(|| {
                        usage_error("--compare-to requires a path")
                    })?));
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            unknown => {
                return Err(usage_error(&format!("unknown argument '{unknown}'")).into());
            }
        }
    }

    Ok(Args {
        output_dir,
        parameter_set,
        compare_to,
    })
}

fn usage_error(message: &str) -> String {
    format!("{message}\n\n{}", usage_text())
}

fn print_usage() {
    println!("{}", usage_text());
}

fn usage_text() -> &'static str {
    "Usage: cargo run -p tank_harness --bin calibration_report -- [--output-dir PATH] [--parameter-set NAME] [--compare-to REPORT_JSON]"
}
