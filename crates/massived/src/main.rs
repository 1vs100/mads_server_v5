use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use massive_proto::crypto::{InvalidMacAddress, MacAddress, MacKey};
use massive_proto::ids::{InvalidText, SkuName, SkuVersion, ZoneName};
use massived::config::{Config, ConfigFileError};
use massived::decode::{self, DecodeCommandError, Input};
use massived::handler::{Handler, SetupError};
use massived::logging::{self, Format};
use massived::probe::{self, ProbeError, ProbeOptions};
use massived::server::{ServeError, Server};
use thiserror::Error;
use tracing::{error, info};

#[derive(Debug, Error)]
enum CommandError {
    #[error(transparent)]
    Config(#[from] ConfigFileError),
    #[error(transparent)]
    Serve(#[from] ServeError),
    #[error(transparent)]
    Setup(#[from] SetupError),
    #[error(transparent)]
    Decode(#[from] DecodeCommandError),
    #[error(transparent)]
    Probe(#[from] ProbeError),
    #[error("--{argument}: {source}")]
    Text {
        argument: &'static str,
        source: InvalidText,
    },
    #[error("--mac-address: {0}")]
    MacAddress(#[from] InvalidMacAddress),
    #[error("one of --hex, --file or --capture is required")]
    MissingInput,
}

fn text_arg<T>(
    argument: &'static str,
    value: String,
    ctor: impl FnOnce(String) -> Result<T, InvalidText>,
) -> Result<T, CommandError> {
    ctor(value).map_err(|source| CommandError::Text { argument, source })
}

#[derive(Debug, Parser)]
#[command(
    version,
    about = "Field server for the Massive ad client protocol, revision 5"
)]
struct Cli {
    /// Log format on stderr.
    #[arg(long, value_enum, default_value_t = Format::Text, global = true)]
    log_format: Format,
    /// Log filter such as massived=debug. Takes precedence over the environment.
    #[arg(long, global = true)]
    log_filter: Option<String>,
    /// Also append JSON log lines to this file.
    #[arg(long, global = true)]
    log_file: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Serve the protocol.
    Serve {
        #[arg(short, long)]
        config: PathBuf,
    },
    /// Load and validate a configuration, including all asset files.
    CheckConfig {
        #[arg(short, long)]
        config: PathBuf,
    },
    /// Decode a message, an HTTP exchange or a capture file.
    Decode {
        #[arg(long, conflicts_with_all = ["file", "capture"])]
        hex: Option<String>,
        #[arg(long, conflicts_with = "capture")]
        file: Option<PathBuf>,
        #[arg(long)]
        capture: Option<PathBuf>,
        /// Key for MAC checks; defaults to the retail literal.
        #[arg(long)]
        key: Option<String>,
    },
    /// Run the retail client request sequence against a server.
    Probe {
        #[arg(long)]
        target: SocketAddr,
        #[arg(long, default_value = "Lobby_PT")]
        zone: String,
        #[arg(long, default_value = "00:11:22:33:44:55")]
        mac_address: String,
        #[arg(long, default_value = "mgs_1_vs_100_x360")]
        sku_name: String,
        #[arg(long, default_value = "edge")]
        sku_version: String,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    if let Err(e) = logging::init(
        cli.log_format,
        cli.log_filter.as_deref(),
        cli.log_file.as_deref(),
    ) {
        eprintln!("massived: {e}");
        return ExitCode::FAILURE;
    }
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            error!(error = %e, "runtime_start_failed");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(run(cli.command)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            error!(error = %e, "command_failed");
            ExitCode::FAILURE
        }
    }
}

async fn run(command: Command) -> Result<(), CommandError> {
    match command {
        Command::Serve { config } => {
            let config = Config::from_file(&config)?;
            let server = Server::bind(&config).await?;
            server
                .serve(async {
                    if let Err(e) = tokio::signal::ctrl_c().await {
                        error!(error = %e, "signal_handler_failed");
                    }
                })
                .await;
            Ok(())
        }
        Command::CheckConfig { config } => {
            let config = Config::from_file(&config)?;
            let handler = Handler::new(&config)?;
            info!(
                zones = handler.content().zone_count(),
                announced_zones = handler.content().announced_zones().len(),
                assets = handler.content().asset_count(),
                "configuration_valid"
            );
            Ok(())
        }
        Command::Decode {
            hex,
            file,
            capture,
            key,
        } => {
            let input = match (hex, file, capture) {
                (Some(h), _, _) => Input::Hex(h),
                (_, Some(f), _) => Input::File(f),
                (_, _, Some(c)) => Input::Capture(c),
                (None, None, None) => return Err(CommandError::MissingInput),
            };
            let key = key.map_or_else(MacKey::retail, |k| MacKey::new(k.into_bytes()));
            let stdout = std::io::stdout();
            decode::run(&input, &key, &mut stdout.lock())?;
            Ok(())
        }
        Command::Probe {
            target,
            zone,
            mac_address,
            sku_name,
            sku_version,
        } => {
            let options = ProbeOptions {
                target,
                zone: text_arg("zone", zone, ZoneName::new)?,
                mac_address: mac_address.parse::<MacAddress>()?,
                sku_name: text_arg("sku-name", sku_name, SkuName::new)?,
                sku_version: text_arg("sku-version", sku_version, SkuVersion::new)?,
            };
            let report = probe::run(&options).await?;
            info!(
                session_id = %report.session.session_id,
                maos = report.zone.maos.len(),
                orders = report.zone.orders.len(),
                downloads = report.downloads,
                "probe_passed"
            );
            Ok(())
        }
    }
}
