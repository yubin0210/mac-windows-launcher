use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;

use mac_spec_extractor::extractor::DockMetricsExtractor;

fn print_usage() {
    eprintln!(
        "Usage: mac-spec-extractor [OPTIONS]\n\n\
         Options:\n  \
           -o, --output <PATH>    Output path for generated JSON spec (e.g. specs/dock-metrics.json)\n  \
           -p, --plist <PATH>     Path to Dock.app or plist file to extract metrics from\n  \
           -h, --help             Display this help message\n  \
           -v, --version          Display version"
    );
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut output_path: Option<PathBuf> = None;
    let mut plist_path: Option<PathBuf> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => {
                if i + 1 < args.len() {
                    output_path = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                } else {
                    eprintln!("Error: --output requires a file path argument");
                    process::exit(1);
                }
            }
            "-p" | "--plist" => {
                if i + 1 < args.len() {
                    plist_path = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                } else {
                    eprintln!("Error: --plist requires a file path argument");
                    process::exit(1);
                }
            }
            "-h" | "--help" => {
                print_usage();
                process::exit(0);
            }
            "-v" | "--version" => {
                println!("mac-spec-extractor {}", mac_spec_extractor::version());
                process::exit(0);
            }
            unknown => {
                eprintln!("Unknown option: {}", unknown);
                print_usage();
                process::exit(1);
            }
        }
    }

    let extractor = match plist_path {
        Some(p) => DockMetricsExtractor::new().with_plist_path(p),
        None => DockMetricsExtractor::new(),
    };

    let metrics = extractor.extract();
    let json_output = match serde_json::to_string_pretty(&metrics) {
        Ok(s) => s + "\n",
        Err(e) => {
            eprintln!("Error serializing DockMetrics: {}", e);
            process::exit(1);
        }
    };

    if let Some(dest) = output_path {
        if let Some(parent) = dest.parent() {
            if !parent.as_os_str().is_empty() {
                if let Err(e) = fs::create_dir_all(parent) {
                    eprintln!("Error creating directory {:?}: {}", parent, e);
                    process::exit(1);
                }
            }
        }
        if let Err(e) = fs::write(&dest, &json_output) {
            eprintln!("Error writing spec to {:?}: {}", dest, e);
            process::exit(1);
        }
        println!("Successfully extracted Dock metrics to {:?}", dest);
    } else {
        print!("{}", json_output);
    }
}
