use std::{
    collections::HashSet,
    ffi::OsStr,
    fs::File,
    path::{Path, PathBuf},
};

use anyhow::{Context, anyhow};
use log::{info, warn};
use serde::Serialize;

use crate::{
    contest::Contest,
    dir_guard::DirGuard,
    problem::{
        Problem,
        StatementType::{self},
    },
};

const PALETTE: [&str; 3] = ["#9486EA", "#E93603", "#9B630C"];

pub fn convert_contest(
    polygon_directory: &Path,
    dom_directory: &Path,
    contest: &Contest,
    preferred_type: StatementType,
) {
    let _guard = DirGuard::change_to(dom_directory);

    prepare_problemset_yaml(contest);
    prepare_problems_yaml(contest);
    prepare_contest_yaml(contest);

    for problem in &contest.problems {
        prepare_problem_directory(problem).expect("failed to create problem directory");

        prepare_problem_yaml(
            &PathBuf::from(&problem.short_name).join("problem.yaml"),
            dom_directory,
            problem,
        )
        .unwrap();

        info!("\tcopying test files");
        copy_test_files(polygon_directory, dom_directory, problem).unwrap();
        info!("\tcopying submission files");
        copy_submission_files(polygon_directory, dom_directory, problem).unwrap();
        info!("\tcopying problem statements");
        copy_problem_statements(polygon_directory, dom_directory, problem, preferred_type).unwrap();
        info!("\tcopying custom files");
        copy_custom_files(polygon_directory, dom_directory, problem).unwrap();
        info!("\tcreating DOMJudge-related files");

        write_domjudge_files(problem).unwrap();
        info!("prepared problem directory: {}", problem.short_name);
    }
}

pub fn write_domjudge_files(problem: &Problem) -> anyhow::Result<()> {
    use std::fs;
    let root = Path::new(&problem.short_name);
    fs::write(root.join(".timelimit"), format!("{:?}", problem.time_limit))?;
    fs::write(
        root.join("domjudge-problem.ini"),
        format!(
            "name={}\ntimelimit={:?}",
            problem.full_name, problem.time_limit
        ),
    )?;
    Ok(())
}

fn prepare_problemset_yaml(contest: &Contest) {
    #[derive(Serialize)]
    struct ProblemEntry<'a> {
        color: &'a str,
        rgb: &'a str,
        #[serde(rename = "short-name")]
        short_name: &'a str,
    }

    #[derive(Serialize)]
    struct ProblemsetYaml<'a> {
        problemset: Vec<ProblemEntry<'a>>,
    }

    let problemset: Vec<ProblemEntry> = contest
        .problems
        .iter()
        .zip(PALETTE.iter().cycle())
        .map(|(problem, &color)| ProblemEntry {
            color,
            short_name: &problem.short_name,
            rgb: color,
        })
        .collect();

    let file = File::create("problemset.yaml").unwrap();
    yaml_serde::to_writer(file, &ProblemsetYaml { problemset }).unwrap();
}

fn prepare_problems_yaml(contest: &Contest) {
    #[derive(Serialize)]
    struct ProblemEntry<'a> {
        id: &'a str,
        label: &'a str,
        ordinal: usize,
        rgb: &'a str,
        time_limit: f64,
    }

    let mut problems = Vec::new();

    for (problem, &color) in contest.problems.iter().zip(PALETTE.iter().cycle()) {
        problems.push(ProblemEntry {
            id: &problem.short_name,
            label: &problem.index,
            rgb: color,
            time_limit: problem.time_limit,
            ordinal: 0, // placeholder
        });
    }

    problems.sort_by_key(|p| p.label);

    for (k, problem) in problems.iter_mut().enumerate() {
        problem.ordinal = k + 1;
    }

    let file = File::create("problems.yaml").unwrap();
    yaml_serde::to_writer(file, &problems).unwrap();
}

#[derive(Serialize)]
struct ContestInfo<'a> {
    duration: &'a str,
    name: &'a str,
    #[serde(rename = "penalty-time")]
    penalty_time: usize,
    #[serde(rename = "scoreboard-freeze-length")]
    scoreboard_freeze_length: &'a str,
    #[serde(rename = "short-name")]
    short_name: &'a str,
    #[serde(rename = "start-time")]
    start_time: &'a str,
}

fn prepare_contest_yaml(contest: &Contest) {
    use chrono::{Utc, format::SecondsFormat};

    let time = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, false);

    let contest_info = ContestInfo {
        duration: "5:00:00",
        name: &contest.name,
        penalty_time: 20,
        scoreboard_freeze_length: "1:00:00",
        short_name: &format!("contest-{}", contest.index),
        start_time: &time,
    };

    let file = File::create("contest.yaml").unwrap();
    yaml_serde::to_writer(file, &contest_info).unwrap();
}

fn retag(tag: &str) -> String {
    if tag == "main" {
        return String::from("accepted");
    }
    tag.to_string()
}

/// Equivalent of Python's `shutil.copy(src, dst)`: if `dst` is an existing
/// directory the file is copied into it, otherwise `dst` is the target file.
/// Missing parent directories are created.
fn shutil_copy(src: &Path, dst: &Path) -> anyhow::Result<()> {
    use std::fs;

    let target = if dst.is_dir() {
        dst.join(
            src.file_name()
                .ok_or_else(|| anyhow!("{} has no file name", src.display()))?,
        )
    } else {
        dst.to_path_buf()
    };
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::copy(src, &target)
        .with_context(|| format!("copy {} -> {}", src.display(), target.display()))?;
    Ok(())
}

fn get_problem_dir(polygone_directory: &Path, problem: &Problem) -> std::path::PathBuf {
    polygone_directory
        .join("problems")
        .join(&problem.short_name)
}

pub fn copy_test_file(
    problem: &Problem,
    polygon_problem_dir: &Path,
    dom_problem_dir: &Path,
    test_input_file: &Path,
) -> anyhow::Result<()> {
    let test_input_file_name = test_input_file.file_name().unwrap();
    let test_index = test_input_file_name.to_str().unwrap().parse::<usize>()? - 1;

    let test_answer_file = test_input_file.with_extension("a");

    // make sure answer file exists
    if !test_answer_file.exists() {
        warn!(
            "test input file for problem {}: {} has no corresponding answers file",
            polygon_problem_dir.display(),
            test_input_file.display(),
        );

        return Err(anyhow!("missing test answer file"));
    }

    let dest_dir = if problem.samples.contains(&test_index) {
        "sample"
    } else {
        "secret"
    };

    let dest_dir = dom_problem_dir.join("data").join(dest_dir);

    shutil_copy(
        &polygon_problem_dir.join(test_input_file),
        &dest_dir.join(format!("{}.in", test_input_file_name.display())),
    )?;

    shutil_copy(
        &polygon_problem_dir.join(test_answer_file),
        &dest_dir.join(format!("{}.ans", test_input_file_name.display())),
    )?;

    Ok(())
}

pub fn copy_test_files(
    polygon_directory: &Path,
    dom_directory: &Path,
    problem: &Problem,
) -> anyhow::Result<()> {
    let polygon_problem_dir = get_problem_dir(polygon_directory, problem);
    let dom_problem_dir = dom_directory.join(&problem.short_name);

    let polygon_tests_dir = polygon_problem_dir.join("tests");

    for dir_entry in std::fs::read_dir(&polygon_tests_dir)? {
        let dir_entry = dir_entry?;

        // skip answers (file ending in .a)
        if dir_entry.path().extension() == Some(OsStr::new("a")) {
            continue;
        }

        // dir_entry should be a test input file
        let test_input_file = dir_entry.path();

        copy_test_file(
            problem,
            &polygon_problem_dir,
            &dom_problem_dir,
            &test_input_file,
        )?;
    }

    Ok(())
}

pub fn copy_submission_files(
    polygon_directory: &Path,
    _dom_directory: &Path,
    problem: &Problem,
) -> anyhow::Result<()> {
    for solution in &problem.solution_files {
        let path = Path::new(&problem.short_name)
            .join("submissions")
            .join(retag(&solution.tag));

        std::fs::create_dir_all(&path)?;

        shutil_copy(
            &get_problem_dir(polygon_directory, problem).join(&solution.path),
            &path,
        )?;
    }
    Ok(())
}

pub fn copy_custom_files(
    polygon_directory: &Path,
    dom_directory: &Path,
    problem: &Problem,
) -> anyhow::Result<()> {
    let src_dir = get_problem_dir(polygon_directory, problem);
    let name = Path::new(&problem.short_name);
    if let Some(f) = &problem.checker_file {
        shutil_copy(
            &src_dir.join(f),
            &dom_directory.join(name.join("output_validators").join("validator")),
        )?;
    }

    if let Some(f) = &problem.interactor_file {
        shutil_copy(
            &src_dir.join(f),
            &dom_directory.join(name.join("output_validators").join("validator")),
        )?;
    }

    if let Some(f) = &problem.validator_file {
        shutil_copy(
            &src_dir.join(f),
            &dom_directory.join(name.join("input_validators").join("validator")),
        )?;
    }

    Ok(())
}

pub fn copy_problem_statements(
    polygon_directory: &Path,
    dom_directory: &Path,
    problem: &Problem,
    preferred_type: StatementType,
) -> anyhow::Result<()> {
    let src_dir = get_problem_dir(polygon_directory, problem);

    // Python's `by_type` dict: later statements of the same type overwrite
    // earlier ones, and the fallback is the first type that was inserted.
    let last_of_type = |t: StatementType| {
        problem
            .statements
            .iter()
            .rev()
            .find(|s| s.statement_type == t)
    };

    let statement = last_of_type(preferred_type)
        .or_else(|| {
            problem
                .statements
                .first()
                .and_then(|first| last_of_type(first.statement_type))
        })
        .ok_or_else(|| anyhow!("problem {} has no statements", problem.short_name))?;
    shutil_copy(
        &src_dir.join(&statement.path),
        &dom_directory.join(Path::new(&problem.short_name)),
    )?;

    let dest = dom_directory.join(Path::new(&problem.short_name).join("problem_statement"));
    let mut statement_files: HashSet<String> = HashSet::new();
    for statement in &problem.statements {
        // For tex files, we only copy the main file
        if statement.statement_type == StatementType::Tex {
            shutil_copy(&src_dir.join(&statement.path), &dest)?;
        } else {
            let dirname = src_dir.join(&statement.path);
            let dirname = dirname.parent().unwrap_or_else(|| Path::new(""));
            for entry in std::fs::read_dir(dirname)? {
                let file = entry?.file_name().to_string_lossy().into_owned();
                // NOTE: like the original, this checks relative to the CWD, not `dirname`.
                if Path::new(&file).is_dir() {
                    warn!("File {file} is a directory. Ignoring");
                    continue;
                }

                if !statement_files.insert(file.clone()) {
                    warn!("File {file} already exists in statement directory. Ignoring");
                    continue;
                }
                shutil_copy(&dirname.join(&file), &dest)?;
            }
        }
    }
    Ok(())
}

pub fn prepare_problem_directory(problem: &Problem) -> anyhow::Result<()> {
    use std::fs;

    let sub_directories = [
        "",
        "problem_statement",
        "data/secret",
        "data/sample",
        "submissions",
        "attachments",
    ];

    let root = Path::new(&problem.short_name);
    for sub in sub_directories {
        fs::create_dir_all(root.join(sub))?;
    }
    if problem.checker_file.is_some() || problem.interactor_file.is_some() {
        fs::create_dir_all(root.join("output_validators").join("validator"))?;
    }
    if problem.validator_file.is_some() {
        fs::create_dir_all(root.join("input_validators").join("validator"))?;
    }
    Ok(())
}

// Field order is alphabetical because PyYAML's `yaml.dump` sorts keys by default.
#[derive(Serialize)]
struct Limits {
    memory: u64,
    time: f64,
}

#[derive(Serialize)]
struct ProblemYaml {
    author: String,
    limits: Limits,
    uuid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    validation: Option<&'static str>,
}

fn prepare_problem_yaml(
    path: &Path,
    _contest_directory: &Path,
    problem: &Problem,
) -> anyhow::Result<()> {
    let validation = if problem.interactor_file.is_some() {
        Some("custom interactive")
    } else if problem.checker_file.is_some() {
        Some("custom")
    } else {
        None
    };
    let info = ProblemYaml {
        author: problem.author.clone(),
        limits: Limits {
            memory: problem.memory_limit,
            time: problem.time_limit,
        },
        uuid: problem.index.clone(),
        validation,
    };
    std::fs::write(path, yaml_serde::to_string(&info)?)?;
    Ok(())
}
