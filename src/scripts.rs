use std::{
    ffi::OsStr,
    fs::{self, File, read_dir},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Stdio,
    time::Instant,
};

use log::info;
use rayon::iter::{ParallelBridge, ParallelIterator};
use walkdir::WalkDir;

use crate::problem::Problem;

pub fn doall(problem: &Problem, problem_dir: &Path) {
    // TODO: check if validator exists, if so run it
    // TODO: check if checker exists, if so run it

    let solution_binary = compile_main_solution(problem, problem_dir);
    // for test case in test cases: run gen-answers
    let tests_dir = problem_dir.join("tests");

    // gen-answers: compute output from test case
    read_dir(tests_dir)
        .unwrap()
        .flatten()
        .par_bridge()
        .for_each(|test| {
            let input_path = test.path();

            if input_path.extension() == Some(OsStr::new("a")) {
                return;
            }

            let output_path = input_path.with_extension("a");

            let input_file = File::open(&input_path).unwrap();
            let output_file = File::create(&output_path).unwrap();

            std::process::Command::new(&solution_binary)
                .stdin(Stdio::from(input_file))
                .stdout(Stdio::from(output_file))
                .spawn()
                .unwrap()
                .wait()
                .unwrap();

            info!(
                "wrote output of {} in {}",
                input_path.display(),
                output_path.display()
            );
        });
}

pub fn compile_main_solution(problem: &Problem, problem_dir: &Path) -> PathBuf {
    let solution_path = &problem
        .solution_files
        .iter()
        .find(|file| file.tag == "main")
        .unwrap()
        .path;

    let solution_path = &problem_dir.join(solution_path);

    let mut binary_path = solution_path.clone();
    binary_path.pop();
    binary_path.push(solution_path.file_stem().unwrap());

    info!(
        "main solution file for problem {} is at {}",
        problem.short_name,
        problem_dir.join(solution_path).display(),
    );

    let mut compile_command = std::process::Command::new("g++");
    compile_command
        .arg(solution_path)
        .arg("-o")
        .arg(&binary_path)
        .arg("-O3");

    let start = Instant::now();
    let mut child = compile_command.spawn().unwrap();
    child.wait().unwrap();
    let duration = start.elapsed();

    info!(
        "compiled {} at {} in {duration:?}",
        solution_path.display(),
        binary_path.display()
    );

    binary_path.clone()
}

pub fn mark_scripts_as_executable(dir: &Path) {
    let perms = fs::Permissions::from_mode(0o754);
    for file in WalkDir::new(dir)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.file_type().is_file())
    {
        if file.path().extension() == Some(OsStr::new("sh")) {
            fs::set_permissions(file.path(), perms.clone())
                .expect("failed to make script executable");
        }
    }
}
