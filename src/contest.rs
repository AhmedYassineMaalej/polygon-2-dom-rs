use std::{collections::HashSet, fs, path::PathBuf};

use anyhow::Context;
use log::info;
use roxmltree::{Document, Node};

use crate::problem::Problem;

pub struct Contest {
    pub index: usize,
    pub name: String,
    pub problems: Vec<Problem>,
}

pub trait Find<'a, 'input> {
    fn find(&self, tag: &str) -> Option<Node<'a, 'input>>;
}

impl<'a, 'input> Find<'a, 'input> for Node<'a, 'input> {
    fn find(&self, tag: &str) -> Option<Node<'a, 'input>> {
        self.descendants().find(|n| n.tag_name() == tag.into())
    }
}

impl Contest {
    pub fn new(index: usize, name: String, problems: Vec<Problem>) -> Self {
        Self {
            index,
            name,
            problems,
        }
    }

    pub fn from_xml(xml: &str) -> anyhow::Result<Self> {
        let doc = Document::parse(xml).context("failed to parse xml")?;
        let root = doc.root_element();

        let contest_index: usize = root
            .attribute("url")
            .ok_or("missing `url` attribute on root")
            .unwrap()
            .rsplit('/')
            .next()
            .unwrap() // rsplit always yields at least one item
            .parse()
            .unwrap();

        let name = root
            .find("names")
            .and_then(|n| n.find("name"))
            .and_then(|n| n.attribute("value"))
            .ok_or("missing names/name[@value]")
            .unwrap()
            .to_string();

        let _directories: HashSet<String> = std::fs::read_dir("problems")
            .unwrap()
            .filter_map(std::result::Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();

        let problems_list = root.find("problems").ok_or("missing <problems>").unwrap();

        let mut problems = Vec::new();
        for problem in problems_list
            .children()
            .filter(|n| n.has_tag_name("problem"))
        {
            let url = problem.attribute("url").unwrap();
            let index = problem.attribute("index").unwrap().to_uppercase();
            let short_name = url.split('/').next_back().unwrap();
            let problem_desc = PathBuf::from("problems")
                .join(short_name)
                .join("problem.xml");

            let problem_xml = fs::read_to_string(problem_desc).unwrap();
            let problem = Problem::try_from_xml(&problem_xml, index.clone())?;
            info!("parsed problem {index}: {short_name:?}");

            problems.push(problem);
        }

        Ok(Self::new(contest_index, name, problems))
    }

    pub fn get_problems_with_validator(&self) -> Vec<&Problem> {
        self.problems.iter().filter(|p| p.has_validator()).collect()
    }

    pub fn get_problems_with_interactor(&self) -> Vec<&Problem> {
        self.problems
            .iter()
            .filter(|p| p.has_interactor())
            .collect()
    }

    pub fn get_problems_with_checker(&self) -> Vec<&Problem> {
        self.problems.iter().filter(|p| p.has_checker()).collect()
    }
}
