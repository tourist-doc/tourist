use dirs;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;
use structopt::StructOpt;
use tourist_types::Index;
use tourist_types::path::AbsolutePathBuf;

mod command;
mod error;
mod resolve;

use error::Result;
use tourist_serde::parse_tour;

fn get_default_config() -> Option<PathBuf> {
    dirs::home_dir().and_then(|mut path| {
        path.push(".tourist");
        if path.exists() {
            Some(path)
        } else {
            None
        }
    })
}

fn get_override_config() -> Option<PathBuf> {
    env::var("TOURIST_CONFIG").ok().and_then(|val| {
        let path = PathBuf::from(val);
        if path.exists() {
            Some(path)
        } else {
            None
        }
    })
}

fn get_index() -> Result<Index> {
    let path = get_override_config().or(get_default_config());
    Ok(match path {
        None => HashMap::new(),
        Some(path) => {
            let contents = &fs::read_to_string(path)?;
            let index: HashMap<String, PathBuf> = serde_json::from_str(contents)?;
            index
                .iter()
                .filter_map(|(k, v)| AbsolutePathBuf::new(v.clone()).map(|ap| (k.to_owned(), ap)))
                .collect::<HashMap<_, _>>()
        }
    })
}

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
}

fn run() -> Result<()> {
    let opts = TouristArgs::from_args();

    match opts {
        TouristArgs::Dump(args) => {
            let tour = parse_tour(&fs::read_to_string(args.tour_file)?)?;
            if args.context {
                let index = get_index()?;
                command::dump(
                    &Some((
                        &index,
                        args.around.or(args.above).unwrap_or(0),
                        args.around.or(args.below).unwrap_or(0),
                    )),
                    &tour,
                )
            } else {
                command::dump(&None, &tour)
            }?;
        }
        TouristArgs::Package(args) => {
            let tour_source = fs::read_to_string(args.tour_file)?;
            let tour = parse_tour(&tour_source)?;
            command::package(
                &PathBuf::from(args.out.unwrap_or(PathBuf::from("out.tour.pkg"))),
                get_index()?,
                tour,
                &tour_source,
            )?;
        }
    }

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{}", e);
        process::exit(1);
    }
}
