mod remote;
mod render;
mod snapshot;

use std::env;
use std::io::{self, Write};
use std::process;

use anyhow::{Result, bail};

use crate::snapshot::parse_snapshot;

fn main() {
    if let Err(error) = run() {
        eprintln!("ocu: {error:#}");
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let config = parse_args(env::args().skip(1))?;
    let payload = match &config.source {
        UsageSource::Remote(host) => remote::fetch_usage_payload(host)?,
        UsageSource::Localhost => remote::fetch_local_usage_payload()?,
    };
    let snapshot = parse_snapshot(&payload)?;
    match config.output_mode {
        OutputMode::Ratatui => render::render_snapshot(&snapshot)?,
        OutputMode::Plain => write_plain_snapshot(&mut io::stdout().lock(), &snapshot)?,
    }
    Ok(())
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct AppConfig {
    output_mode: OutputMode,
    source: UsageSource,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum OutputMode {
    Ratatui,
    Plain,
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum UsageSource {
    Remote(String),
    Localhost,
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<AppConfig> {
    let mut config = AppConfig {
        output_mode: OutputMode::Ratatui,
        source: UsageSource::Localhost,
    };
    let mut source_selected = false;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--plain" => config.output_mode = OutputMode::Plain,
            "--localhost" => {
                if source_selected {
                    bail!("choose only one usage source: --localhost or --host");
                }
                config.source = UsageSource::Localhost;
                source_selected = true;
            }
            "--host" => {
                if source_selected {
                    bail!("choose only one usage source: --localhost or --host");
                }
                let host = args
                    .next()
                    .filter(|host| !host.trim().is_empty())
                    .ok_or_else(|| anyhow::anyhow!("--host requires an SSH target"))?;
                config.source = UsageSource::Remote(host);
                source_selected = true;
            }
            _ if arg.starts_with("--host=") => {
                if source_selected {
                    bail!("choose only one usage source: --localhost or --host");
                }
                let host = arg
                    .strip_prefix("--host=")
                    .filter(|host| !host.trim().is_empty())
                    .ok_or_else(|| anyhow::anyhow!("--host requires an SSH target"))?;
                config.source = UsageSource::Remote(host.to_owned());
                source_selected = true;
            }
            _ => bail!(
                "unexpected argument `{arg}`; usage: ocu [--plain] [--localhost | --host <ssh-target>]"
            ),
        }
    }

    Ok(config)
}

fn write_plain_snapshot(writer: &mut impl Write, snapshot: &snapshot::UsageSnapshot) -> Result<()> {
    serde_json::to_writer_pretty(&mut *writer, snapshot)?;
    writeln!(writer)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_args_should_use_ratatui_output_by_default() {
        assert_eq!(
            parse_args(Vec::<String>::new()).expect("empty args are valid"),
            AppConfig {
                output_mode: OutputMode::Ratatui,
                source: UsageSource::Localhost,
            }
        );
    }

    #[test]
    fn parse_args_should_reject_unknown_arguments() {
        let err = parse_args(["--legacy".to_string()]).expect_err("unknown args are rejected");

        assert!(err.to_string().contains("unexpected argument"));
    }

    #[test]
    fn parse_args_should_accept_plain_output_mode() {
        assert_eq!(
            parse_args(["--plain".to_string()]).expect("plain mode is valid"),
            AppConfig {
                output_mode: OutputMode::Plain,
                source: UsageSource::Localhost,
            }
        );
    }

    #[test]
    fn parse_args_should_accept_localhost_with_plain_output() {
        assert_eq!(
            parse_args(["--localhost".to_string(), "--plain".to_string()])
                .expect("localhost plain mode is valid"),
            AppConfig {
                output_mode: OutputMode::Plain,
                source: UsageSource::Localhost,
            }
        );
    }

    #[test]
    fn parse_args_should_accept_an_explicit_ssh_host() {
        assert_eq!(
            parse_args(["--host".to_string(), "build-box".to_string()])
                .expect("host mode is valid"),
            AppConfig {
                output_mode: OutputMode::Ratatui,
                source: UsageSource::Remote("build-box".to_string()),
            }
        );
    }

    #[test]
    fn parse_args_should_reject_multiple_sources() {
        let error = parse_args(["--localhost".to_string(), "--host=build-box".to_string()])
            .expect_err("sources are mutually exclusive");

        assert!(error.to_string().contains("choose only one usage source"));
    }

    #[test]
    fn write_plain_snapshot_should_emit_pretty_json() {
        let snapshot = snapshot::UsageSnapshot {
            buckets: vec![snapshot::UsageBucket {
                title: "Main Codex bucket",
                windows: [
                    snapshot::WindowUsage {
                        label: "5h",
                        reset_at: Some(1781517330),
                        used_percent: 15,
                    },
                    snapshot::WindowUsage {
                        label: "weekly",
                        reset_at: None,
                        used_percent: 38,
                    },
                ],
            }],
        };
        let mut output = Vec::new();

        write_plain_snapshot(&mut output, &snapshot).expect("plain output should render");

        let output = String::from_utf8(output).expect("plain output is UTF-8");
        assert!(output.contains("\"title\": \"Main Codex bucket\""));
        assert!(output.contains("\"used_percent\": 15"));
        assert!(output.ends_with('\n'));
    }
}
