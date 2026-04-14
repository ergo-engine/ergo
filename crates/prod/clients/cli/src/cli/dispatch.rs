//! dispatch.rs — CLI command dispatch and argument parsing
//!
//! Purpose:
//! - Defines the top-level CLI argument structure using clap and
//!   dispatches subcommands to their respective handlers.

use std::path::Path;

use crate::cli::args;
use crate::output;

pub enum DispatchOutput {
    Text(String),
    Json(String),
}

fn wants_json_output(args: &[String]) -> bool {
    for i in 0..args.len() {
        if args[i] == "--format" && args.get(i + 1).is_some_and(|v| v == "json") {
            return true;
        }
        if args[i] == "--format=json" {
            return true;
        }
    }
    false
}

pub fn run() -> Result<DispatchOutput, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    dispatch_with_args(&args)
}

pub(crate) fn dispatch_with_args(args: &[String]) -> Result<DispatchOutput, String> {
    let mut args_it = args.iter().cloned();
    match args_it.next() {
        None => Ok(DispatchOutput::Text(output::text::usage())),
        Some(command) => match command.as_str() {
            "help" => {
                let topic_parts: Vec<String> = args_it.collect();
                if topic_parts.is_empty() {
                    return Ok(DispatchOutput::Text(output::text::usage()));
                }
                let topic = topic_parts.join(" ").to_ascii_lowercase();
                if let Some(help_text) =
                    output::text::help_topic(&topic, &crate::fixture_ops::fixture_usage())
                {
                    Ok(DispatchOutput::Text(help_text))
                } else {
                    Err(output::errors::unknown_help_topic(&topic_parts.join(" ")))
                }
            }
            "fixture" => {
                let target = args_it
                    .next()
                    .ok_or_else(crate::fixture_ops::fixture_usage)?;
                match target.as_str() {
                    "run" => {
                        let path = args_it
                            .next()
                            .ok_or_else(crate::fixture_ops::fixture_usage)?;
                        let rest: Vec<String> = args_it.collect();
                        let run_opts = args::parse_run_artifact_options(&rest, "fixture run")?;
                        let summary = crate::cli::handlers::run_fixture(
                            Path::new(&path),
                            run_opts.capture_output.as_deref(),
                            run_opts.pretty_capture,
                        )?;
                        Ok(DispatchOutput::Text(
                            output::text::render_fixture_run_summary(
                                &summary.episode_event_counts,
                                &summary.capture_path,
                            ),
                        ))
                    }
                    "inspect" => {
                        let rest: Vec<String> = args_it.collect();
                        let out = crate::fixture_ops::fixture_inspect_command(&rest)?;
                        if wants_json_output(&rest) {
                            Ok(DispatchOutput::Json(out))
                        } else {
                            Ok(DispatchOutput::Text(out))
                        }
                    }
                    "validate" => {
                        let rest: Vec<String> = args_it.collect();
                        let out = crate::fixture_ops::fixture_validate_command(&rest)?;
                        if wants_json_output(&rest) {
                            Ok(DispatchOutput::Json(out))
                        } else {
                            Ok(DispatchOutput::Text(out))
                        }
                    }
                    _ => Err(output::errors::invalid_fixture_subcommand(
                        &target,
                        &crate::fixture_ops::fixture_usage(),
                    )),
                }
            }
            "gen-docs" => {
                let rest: Vec<String> = args_it.collect();
                let out = crate::gen_docs::gen_docs_command(&rest)?;
                Ok(DispatchOutput::Text(out))
            }
            "init" => {
                let rest: Vec<String> = args_it.collect();
                let out = crate::init_project::init_command(&rest)?;
                Ok(DispatchOutput::Text(out))
            }
            "validate" => {
                let rest: Vec<String> = args_it.collect();
                let out = crate::validate::validate_command(&rest)?;
                if wants_json_output(&rest) {
                    Ok(DispatchOutput::Json(out))
                } else {
                    Ok(DispatchOutput::Text(out))
                }
            }
            "csv-to-fixture" => {
                let rest: Vec<String> = args_it.collect();
                let out = crate::csv_fixture::csv_to_fixture_command(&rest)?;
                Ok(DispatchOutput::Text(out))
            }
            "check-compose" => {
                let rest: Vec<String> = args_it.collect();
                let out = crate::validate::check_compose_command(&rest)?;
                if wants_json_output(&rest) {
                    Ok(DispatchOutput::Json(out))
                } else {
                    Ok(DispatchOutput::Text(out))
                }
            }
            "graph-to-dot" => {
                let rest: Vec<String> = args_it.collect();
                let out = crate::graph_to_dot::graph_to_dot_command(&rest)?;
                Ok(DispatchOutput::Text(out))
            }
            "render" => {
                let target = args_it.next().ok_or_else(crate::usage)?;
                match target.as_str() {
                    "graph" => {
                        let rest: Vec<String> = args_it.collect();
                        let out = crate::render::render_graph_command(&rest)?;
                        Ok(DispatchOutput::Text(out))
                    }
                    _ => Err(crate::usage()),
                }
            }
            "run" => {
                let target = args_it.next().ok_or_else(crate::usage)?;
                match target.as_str() {
                    "fixture" => Err(output::errors::removed_run_fixture()),
                    _ => {
                        let rest: Vec<String> = args_it.collect();
                        let summary =
                            crate::graph_yaml::run_graph_command(Path::new(&target), &rest)?;
                        Ok(DispatchOutput::Text(
                            output::text::render_graph_run_summary(
                                summary.completion,
                                summary.episodes,
                                summary.events,
                                summary.invoked,
                                summary.deferred,
                                &summary.capture_path,
                            ),
                        ))
                    }
                }
            }
            "replay" => {
                let path = args_it.next().ok_or_else(crate::usage)?;
                let rest: Vec<String> = args_it.collect();
                let replay_opts = args::parse_replay_options(&rest)?;
                let summary = crate::cli::handlers::replay_graph(
                    Path::new(&path),
                    replay_opts
                        .graph_path
                        .as_ref()
                        .expect("replay options enforce graph path"),
                    &replay_opts.cluster_paths,
                    replay_opts.adapter_path.as_deref(),
                )?;
                Ok(DispatchOutput::Text(output::text::render_replay_summary(
                    &summary.graph_id,
                    summary.events,
                    summary.invoked,
                    summary.deferred,
                    summary.skipped,
                )))
            }
            _ => Err(output::errors::unknown_command(&command)),
        },
    }
}

#[cfg(test)]
mod tests;
