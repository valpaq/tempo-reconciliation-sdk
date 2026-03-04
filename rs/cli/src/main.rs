mod cmd;
mod error;
mod io;

use clap::{Parser, Subcommand};

use cmd::memo::{MemoArgs, MemoCommand};
use cmd::run::RunArgs;
use cmd::watch::WatchArgs;
use error::{EXIT_ERR, EXIT_IO};

/// Command-line tool for TIP-20 payment reconciliation on Tempo.
#[derive(Parser)]
#[command(
    name = "tempo-reconcile",
    version,
    about = "TIP-20 payment reconciliation CLI",
    long_about = None,
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    /// Output as JSON instead of human-readable text.
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Encode, decode, and generate memo values.
    Memo(MemoArgs),
    /// Reconcile payment events against expected payments (file-based).
    Run(RunArgs),
    /// Stream TIP-20 transfer events to stdout or a file (runs until Ctrl+C).
    Watch(WatchArgs),
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match &cli.command {
        Commands::Memo(args) => dispatch_memo(args, cli.json),
        Commands::Run(args) => cmd::run::run_reconcile(args, cli.json),
        Commands::Watch(args) => cmd::watch::run_watch(args).await,
    };

    if let Err(e) = result {
        eprintln!("error: {e:#}");
        // Use the error chain to pick the right exit code.
        let code = if e
            .chain()
            .any(|cause| cause.downcast_ref::<std::io::Error>().is_some())
        {
            EXIT_IO
        } else {
            EXIT_ERR
        };
        std::process::exit(code);
    }
}

fn dispatch_memo(args: &MemoArgs, json: bool) -> anyhow::Result<()> {
    match &args.command {
        MemoCommand::Encode(a) => cmd::memo::run_encode(a, json),
        MemoCommand::Decode(a) => cmd::memo::run_decode(a, json),
        MemoCommand::Generate(a) => cmd::memo::run_generate(a, json),
        MemoCommand::IssuerTag(a) => cmd::memo::run_issuer_tag(a, json),
    }
}
