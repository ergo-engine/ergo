use std::path::PathBuf;

use crate::error_format::{render_cli_error, CliErrorInfo};

#[derive(Debug, Default)]
pub struct ReplayOptions {
    pub graph_path: Option<PathBuf>,
    pub adapter_path: Option<PathBuf>,
    pub cluster_paths: Vec<PathBuf>,
}

#[derive(Debug, Default)]
pub struct RunArtifactOptions {
    pub pretty_capture: bool,
    pub capture_output: Option<PathBuf>,
}

pub fn parse_run_artifact_options(
    args: &[String],
    target: &str,
) -> Result<RunArtifactOptions, String> {
    let mut options = RunArtifactOptions::default();
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "-p" | "--pretty-capture" => {
                options.pretty_capture = true;
                i += 1;
            }
            "-o" | "--capture" | "--capture-output" => {
                let value = args.get(i + 1).ok_or_else(|| {
                    render_cli_error(
                        &CliErrorInfo::new(
                            "cli.missing_option_value",
                            format!("{} requires a path", args[i]),
                        )
                        .with_where(format!("arg '{}'", args[i]))
                        .with_fix("provide -o <path>"),
                    )
                })?;
                options.capture_output = Some(PathBuf::from(value));
                i += 2;
            }
            other => {
                return Err(render_cli_error(
                    &CliErrorInfo::new(
                        "cli.invalid_option",
                        format!("unknown run {target} option '{other}'"),
                    )
                    .with_where(format!("arg '{other}'"))
                    .with_fix(format!(
                        "for 'ergo run {target}', use -p|--pretty-capture and -o|--capture|--capture-output"
                    )),
                ));
            }
        }
    }

    Ok(options)
}

pub fn parse_replay_options(args: &[String]) -> Result<ReplayOptions, String> {
    let mut options = ReplayOptions::default();
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "-g" | "--graph" => {
                let value = args.get(i + 1).ok_or_else(|| {
                    render_cli_error(
                        &CliErrorInfo::new(
                            "cli.missing_option_value",
                            format!("{} requires a path", args[i]),
                        )
                        .with_where(format!("arg '{}'", args[i]))
                        .with_fix("provide -g <graph.yaml>"),
                    )
                })?;
                options.graph_path = Some(PathBuf::from(value));
                i += 2;
            }
            "-a" | "--adapter" => {
                let value = args.get(i + 1).ok_or_else(|| {
                    render_cli_error(
                        &CliErrorInfo::new(
                            "cli.missing_option_value",
                            format!("{} requires a path", args[i]),
                        )
                        .with_where(format!("arg '{}'", args[i]))
                        .with_fix("provide -a <adapter.yaml>"),
                    )
                })?;
                options.adapter_path = Some(PathBuf::from(value));
                i += 2;
            }
            "--cluster-path" | "--search-path" => {
                let value = args.get(i + 1).ok_or_else(|| {
                    render_cli_error(
                        &CliErrorInfo::new(
                            "cli.missing_option_value",
                            format!("{} requires a path", args[i]),
                        )
                        .with_where(format!("arg '{}'", args[i]))
                        .with_fix("provide a directory path"),
                    )
                })?;
                options.cluster_paths.push(PathBuf::from(value));
                i += 2;
            }
            other => {
                return Err(render_cli_error(
                    &CliErrorInfo::new(
                        "cli.invalid_option",
                        format!("unknown replay option '{other}'"),
                    )
                    .with_where(format!("arg '{other}'"))
                    .with_fix(
                        "use -g|--graph, -a|--adapter, --cluster-path, or --search-path for replay",
                    ),
                ));
            }
        }
    }

    if options.graph_path.is_none() {
        return Err(render_cli_error(
            &CliErrorInfo::new(
                "cli.missing_required_option",
                "replay requires -g|--graph <graph.yaml>",
            )
            .with_where("replay command options")
            .with_fix("rerun with -g <graph.yaml>"),
        ));
    }

    Ok(options)
}
