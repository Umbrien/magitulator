use clap::{Parser, Subcommand};
use magitulator::{AnyResult, mirror::mirror};

#[derive(Parser, Debug)]
#[command(
    disable_help_subcommand = true,
    author,
    version,
    about,
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Perform a dry run without writing any changes to the repository.
    #[arg(long, global = true)]
    dry_run: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Rewrite commits to a new branch for inspection.
    Mirror {
        /// Starting object for the rewrite.
        base: String,
        /// Target object (branch name / commit hash) to rewrite.
        target: String,
    },
    /// Replace an original branch with its mirrored counterpart.
    Apply {
        /// The original target branch to replace
        target: String,
    },
    /// Rewrite commits and immediately update the target branch.
    Rewrite {
        /// Starting object for the rewrite.
        base: String,
        /// Target branch to rewrite in-place.
        target: String,
    },
}

fn main() -> AnyResult<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Mirror { base, target } => {
            mirror(&base, &target)?;
        }
        Commands::Apply { target } => {
            // Logic to delete original and rename mirrored branch
            println!("Applying changes to {}", target);
        }
        Commands::Rewrite { base, target } => {
            // Logic to mirror and then immediately apply
            println!("Rewriting from {} to {} in-place", base, target);
        }
    }

    Ok(())
}
