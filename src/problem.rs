use std::{collections::HashSet, path::PathBuf, str::FromStr};

use anyhow::{Context, Ok, Result, anyhow};
use roxmltree::Node;

use crate::contest::Find;

#[derive(Debug, Clone)]
pub struct Problem {
    pub index: String,
    pub short_name: String,
    pub full_name: String,
    pub statements: Vec<Statement>,
    pub solution_files: Vec<SolutionFile>,
    pub checker_file: Option<PathBuf>,
    pub validator_file: Option<PathBuf>,
    pub interactor_file: Option<PathBuf>,
    pub samples: HashSet<usize>,
    pub time_limit: f64,
    pub memory_limit: u64,
    pub author: String,
}

impl Problem {
    const TIME_LIMIT_DEFAULT: f64 = 1.0;
    const MEMORY_LIMIT_DEFAULT: u64 = 512;

    fn new(
        index: String,
        short_name: String,
        full_name: String,
        statements: Vec<Statement>,
        solution_files: Option<Vec<SolutionFile>>,
        checker_file: Option<PathBuf>,
        validator_file: Option<PathBuf>,
        interactor_file: Option<PathBuf>,
        samples: Option<HashSet<usize>>,
        time_limit: f64,
        memory_limit: u64,
        author: String,
    ) -> Self {
        let samples = samples.unwrap_or_default();
        let solution_files = solution_files.unwrap_or_default();

        Self {
            index,
            short_name,
            full_name,
            statements,
            solution_files,
            checker_file,
            validator_file,
            interactor_file,
            samples,
            time_limit,
            memory_limit,
            author,
        }
    }

    pub fn full_name(&self) -> &str {
        &self.full_name
    }

    pub fn has_validator(&self) -> bool {
        self.validator_file.is_some()
    }

    pub fn has_checker(&self) -> bool {
        self.checker_file.is_some()
    }

    pub fn has_interactor(&self) -> bool {
        self.interactor_file.is_some()
    }

    fn parse_author<'b>(root: &'b Node<'_, 'b>) -> Result<&'b str> {
        root.attribute("url")
            .context("problem xml has no url attribute")?
            .split('/')
            .nth_back(1)
            .context("problem url has no author segment")
    }

    fn parse_short_name<'b>(root: &'b Node<'_, 'b>) -> Result<&'b str> {
        root.attribute("short-name")
            .context("missing short-name attribute")
    }

    fn parse_full_name<'b>(root: &'b Node<'_, 'b>) -> Result<&'b str> {
        root.find("names")
            .context("missing names tag")?
            .find("name")
            .context("missing name tag")?
            .attribute("value")
            .context("name tag has no value attribute")
    }

    fn parse_statements<'b>(root: &'b Node<'_, 'b>) -> Result<Vec<Statement>> {
        let mut statements = Vec::new();
        for statement in root
            .find("statements")
            .context("missing statements tag")?
            .children()
            .filter(|node| node.tag_name() == "statement".into())
        {
            let path = statement
                .attribute("path")
                .context("failed to parse statement path")?;

            let statement_type = statement
                .attribute("type")
                .context("failed to parse statement type")?;

            statements.push(Statement::new(
                PathBuf::from(path),
                StatementType::from_str(statement_type)?,
            ));
        }

        Ok(statements)
    }

    fn parse_checker_file<'b>(assets: &'b Node<'_, 'b>) -> Result<Option<PathBuf>> {
        let Some(file) = assets.find("checker") else {
            return Ok(None);
        };

        let file = file
            .find("source")
            .context("missing source tag")?
            .attribute("path")
            .context("missing path attribute")?;

        Ok(Some(PathBuf::from(file)))
    }

    fn parse_validator_file<'b>(assets: &'b Node<'_, 'b>) -> Result<Option<PathBuf>> {
        let Some(file) = assets.find("validator") else {
            return Ok(None);
        };

        let file = file
            .find("source")
            .context("missing source tag")?
            .attribute("path")
            .context("missing path attribute")?;

        Ok(Some(PathBuf::from(file)))
    }

    fn parse_interactor_file<'b>(assets: &'b Node<'_, 'b>) -> Result<Option<PathBuf>> {
        let Some(file) = assets.find("interactor") else {
            return Ok(None);
        };

        let file = file
            .find("source")
            .context("missing source tag")?
            .attribute("path")
            .context("missing path attribute")?;

        Ok(Some(PathBuf::from(file)))
    }

    fn parse_time_limit<'b>(tests: &'b Node<'_, 'b>) -> Result<f64> {
        tests
            .find("time-limit")
            .context("missing time-limit tag")?
            .text()
            .context("empty time-limit tag")?
            .parse()
            .context("failed to parse time-limit into float")
            .map(convert_to_seconds)
    }

    fn parse_memory_limit<'b>(tests: &'b Node<'_, 'b>) -> Result<u64> {
        tests
            .find("memory-limit")
            .context("missing memory-limit tag")?
            .text()
            .context("empty memory-limit tag")?
            .parse()
            .context("failed to parse memory-limit")
            .map(convert_to_mb)
    }

    fn parse_sample_tests<'b>(tests: &'b Node<'_, 'b>) -> Result<HashSet<usize>> {
        let mut samples = HashSet::new();

        for (idx, test) in tests
            .find("tests")
            .context("missing tests tag")?
            .children()
            .filter(|node| node.tag_name() == "test".into())
            .enumerate()
        {
            if test.has_attribute("sample") {
                samples.insert(idx);
            }
        }

        Ok(samples)
    }

    pub fn try_from_xml(xml: &str, index: String) -> Result<Self> {
        let parsed = roxmltree::Document::parse(xml).context("failed to parse xml")?;
        let root = parsed.root_element();

        let author = Self::parse_author(&root).context("failed to parse author")?;
        let short_name = Self::parse_short_name(&root).context("failed to parse short-name")?;
        let full_name = Self::parse_full_name(&root).context("failed to parse full-name")?;

        let statements = Self::parse_statements(&root).context("failed to parse statements")?;

        let assets = root.find("assets").context("not assets tag found")?;

        let checker_file =
            Self::parse_checker_file(&assets).context("failed to parse checker file")?;

        let validator_file =
            Self::parse_validator_file(&assets).context("failed to parse validator file")?;

        let interactor_file =
            Self::parse_interactor_file(&assets).context("failed to parse interactor file")?;

        let tests = root
            .find("judging")
            .context("no judging tag found")?
            .find("testset")
            .context("no testset tag found")?;

        let time_limit = Self::parse_time_limit(&tests).context("failed to parse time limit")?;

        let memory_limit =
            Self::parse_memory_limit(&tests).context("failed to parse memory limit")?;

        let samples = Self::parse_sample_tests(&tests)?;

        let mut solution_files = Vec::new();

        for solution in assets
            .find("solutions")
            .context("missing solutions tag")?
            .children()
            .filter(|node| node.tag_name() == "solution".into())
        {
            let tag = solution
                .attribute("tag")
                .expect("solution has no tag")
                .to_string();

            let file = solution
                .descendants()
                .find(|node| node.tag_name() == "source".into())
                .and_then(|node| node.attribute("path"))
                .expect("solution has no source file");

            let file = PathBuf::from(file);

            solution_files.push(SolutionFile::new(file, tag));
        }

        Ok(Problem::new(
            index,
            short_name.to_string(),
            full_name.to_string(),
            statements,
            Some(solution_files),
            checker_file,
            validator_file,
            interactor_file,
            Some(samples),
            time_limit,
            memory_limit,
            author.to_string(),
        ))
    }
}

#[derive(Debug, Clone)]
pub struct SolutionFile {
    pub path: PathBuf,
    pub tag: String,
}

impl SolutionFile {
    fn new(path: PathBuf, tag: String) -> Self {
        Self { path, tag }
    }
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum StatementType {
    Html,
    Pdf,
    Markdown,
    Tex,
}

impl FromStr for StatementType {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> std::prelude::v1::Result<Self, Self::Err> {
        if value.contains("html") {
            return Ok(Self::Html);
        }

        if value.contains("pdf") {
            return Ok(Self::Pdf);
        }

        if value.contains("markdown") {
            return Ok(Self::Markdown);
        }

        if value.contains("tex") {
            return Ok(Self::Tex);
        }

        Err(anyhow!("unknown statement format: {value}"))
    }
}

#[derive(Debug, Clone)]
pub struct Statement {
    pub path: PathBuf,
    pub statement_type: StatementType,
}

impl Statement {
    fn new(path: PathBuf, statement_type: StatementType) -> Self {
        Self {
            path,
            statement_type,
        }
    }
}

fn convert_to_mb(value: u64) -> u64 {
    value / (1024 * 1024)
}

fn convert_to_seconds(value: f64) -> f64 {
    value / 1000.0
}
