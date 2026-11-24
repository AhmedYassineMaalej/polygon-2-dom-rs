#![warn(clippy::pedantic)]

mod contest;
mod converter;
mod dir_guard;
mod prepare_statements;
mod problem;
mod scripts;

use anyhow::Ok;
use clap::Parser;
use env_logger::{Builder, Env};
use log::{info, warn};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

use crate::{
    contest::Contest,
    problem::{Problem, StatementType},
    scripts::{doall, mark_scripts_as_executable},
};

#[derive(Parser, Debug)]
struct Args {
    /// The contest directory
    #[arg(long)]
    directory: PathBuf,

    /// The target file
    #[arg(long, default_value = None)]
    target: Option<PathBuf>,

    /// Generate test files
    #[arg(long)]
    tests: bool,

    /// Compile Tex Files
    #[arg(long)]
    tex: bool,

    /// Preferred statement type
    #[arg(long, default_value = "pdf")]
    preferred_statement: String,
}

use std::io::Write;

fn main() -> anyhow::Result<()> {
    Builder::from_env(Env::default().default_filter_or("trace"))
        .format(|buf, record| {
            let level_style = buf.default_level_style(record.level());
            writeln!(
                buf,
                "[{level_style}{}{level_style:#}] {}",
                record.level(),
                record.args()
            )
        })
        .init();

    rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build_global()
        .unwrap();

    let args = Args::parse();

    let tmp = tempfile::tempdir()?.keep();
    info!("created tmp directory: {}", tmp.display());

    copy_dir_all(&args.directory, &tmp).unwrap();

    // run everything in the tmp directory
    std::env::set_current_dir(&tmp)?;

    mark_scripts_as_executable(&tmp);
    info!("marked scripts as executable");

    let mut contest_xml = String::new();
    {
        let mut file = File::open("contest.xml").expect("failed to open contest.xml");
        file.read_to_string(&mut contest_xml).unwrap();
    }
    fs::remove_file("contest.xml").unwrap();

    // TODO: handle error when parsing contest
    let contest = Contest::from_xml(&contest_xml)?;
    info!("parsed contest: {:?}", contest.name);

    if args.tests {
        info!("generating test files");
        let problems_dir = tmp.join("problems");

        // TODO: better error handling
        contest.problems.par_iter().for_each(|problem| {
            info!(
                "Running tests generator for problem {}",
                problem.full_name()
            );

            doall(problem, &problems_dir.join(&problem.short_name));
        });

        custom_file_warning("checkers", &contest.get_problems_with_checker());
        custom_file_warning("validators", &contest.get_problems_with_validator());
        custom_file_warning("interactors", &contest.get_problems_with_interactor());
    }

    let target = args.target.as_ref().unwrap();
    fs::create_dir_all(target)?;
    info!("created target folder: {}", target.display());

    info!("converting files to DOMJudge format");
    converter::convert_contest(&tmp, target, &contest, StatementType::Pdf);

    Ok(())
}

fn custom_file_warning(file_type: &str, problems: &[&Problem]) {
    if problems.is_empty() {
        return;
    }

    warn!(
        "Some problems in this contest contain custom {file_type}. You have to edit them manually to adhere to DOMJudge format"
    );

    let problems = problems.iter().fold(String::new(), |mut acc, p| {
        if !acc.is_empty() {
            acc.push_str(", ");
        }
        acc.push_str(&p.full_name);
        acc
    });

    warn!("custom {file_type}: {problems}");
}

fn copy_dir_all(src: &Path, dst: &Path) -> anyhow::Result<()> {
    use std::fs;

    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let entry_type = entry.file_type()?;
        if entry_type.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }

    Ok(())
}
