use failure::ResultExt;
use std::fs;
use std::path::PathBuf;
use std::process;
use structopt::StructOpt;

mod command;
mod config;
mod error;
mod index;
mod serialize;
mod types;
mod vcs;

use command::{Dump, Package, Serve};
pub use command::{StopMetadata, StopReferenceView, StopView, TourMetadata, TourView, TouristRpc};
use error::{ErrorKind, Result};
use index::FileIndex;
use serialize::parse_tour;
use vcs::Git;

#[derive(StructOpt)]
struct DumpArgs {
    #[structopt(
        short = "c",
        long = "context",
        help = "Look in git for the source code referenced by each stop."
    )]
    context: bool,
    #[structopt(short = "A", help = "Lines to be shown above the target line.")]
    above: Option<usize>,
    #[structopt(short = "B", help = "Lines to be shown below the target line.")]
    below: Option<usize>,
    #[structopt(short = "C", help = "Lines to be shown around the target line.")]
    around: Option<usize>,
    #[structopt(name = "TOURFILE", parse(from_os_str))]
    tour_file: PathBuf,
}

#[derive(StructOpt)]
struct PackageArgs {
    #[structopt(
        short = "o",
        long = "out",
        help = "The name of the output file. By convention, the file should end with \
                \".tour.pkg\".",
        parse(from_os_str)
    )]
    out: Option<PathBuf>,
    #[structopt(name = "TOURFILE", parse(from_os_str))]
    tour_file: PathBuf,
}

#[derive(StructOpt)]
struct ServeArgs {}

#[derive(StructOpt)]
#[structopt(
    name = "tourist",
    about = "A CLI tool for the tourist documentation system."
)]
enum TouristArgs {
    #[structopt(name = "dump", about = "Dump a .tour file as a markdown document.")]
    Dump(DumpArgs),
    #[structopt(
        name = "package",
        about = "Package a tour file for viewing on the web."
    )]
    Package(PackageArgs),
    #[structopt(
        name = "serve",
        about = "Start a JSON-RPC 2.0 that implements the tourist protocol."
    )]
    Serve(ServeArgs),
}

fn run(opts: TouristArgs) -> Result<()> {
    match opts {
        TouristArgs::Dump(args) => {
            let tour = parse_tour(
                &fs::read_to_string(args.tour_file).context(ErrorKind::FailedToReadTour)?,
            )
            .context(ErrorKind::FailedToParseTour)?;
            if args.context {
                Dump::with_context(
                    Git,
                    FileIndex,
                    args.around.or(args.above).unwrap_or(0),
                    args.around.or(args.below).unwrap_or(0),
                )
            } else {
                Dump::new()
            }
            .process(&tour)?;
        }
        TouristArgs::Package(args) => {
            let tour_source =
                fs::read_to_string(args.tour_file).context(ErrorKind::FailedToReadTour)?;
            let tour = parse_tour(&tour_source).context(ErrorKind::FailedToParseTour)?;
            Package::new(Git, FileIndex).process(
                &args.out.unwrap_or_else(|| PathBuf::from("out.tour.pkg")),
                tour,
                &tour_source,
            )?;
        }
        TouristArgs::Serve(_) => {
            Serve::new(Git, FileIndex).process(config::get_default_tours()?);
        }
    }

    Ok(())
}

fn main() {
    if let Err(e) = run(TouristArgs::from_args()) {
        eprintln!("{}", e);
        process::exit(1);
    }
}
