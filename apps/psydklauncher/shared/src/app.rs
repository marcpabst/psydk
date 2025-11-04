use std::{
    fs::read_dir,
    path::{Path, PathBuf},
};

use chrono::{serde::ts_milliseconds_option::deserialize as ts_milliseconds_option, DateTime, Utc};
use crux_core::{
    macros::effect,
    render::{render, RenderOperation},
    Command,
};

use rand::Rng;
use serde::{Deserialize, Serialize};

// ANCHOR: model
#[derive(Serialize)]
pub struct Model {
    pub experiments: Vec<Experiment>,
    pub selected_experiment: Option<Experiment>,
    pub selected_task: Option<Task>,
    pub selected_subject: Option<Subject>,
    pub selected_session: Option<Session>,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, Eq)]
pub struct Count {
    pub value: isize,
}
// ANCHOR_END: model

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ViewModel {
    pub experiments: Vec<Experiment>,
    pub subjects: Vec<Subject>,
    pub sessions: Vec<Session>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ListItem {
    pub id: u128,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq, Eq)]
pub struct Subject {
    id: u128,
    directory: PathBuf,
    name: String,
    sessions: Vec<Session>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq, Eq)]
pub struct Session {
    directory: PathBuf,
    id: u128,
    name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq, Eq)]
pub struct Task {
    id: u128,
    name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq, Eq)]
pub struct Experiment {
    id: u128,
    directory: PathBuf,
    name: String,
    icon_path: Option<PathBuf>,
    version: String,
    description: String,
    subjects: Vec<Subject>,
    tasks: Vec<Task>,
    default_task: Task,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Event {
    // events from the shell
    LoadExperiments(Vec<String>),
    AddNewSubject(Experiment, String),
    AddNewSession(Subject, String),
}

#[effect(typegen)]
#[derive(Debug)]
pub enum Effect {
    Render(RenderOperation),
}

#[derive(Default)]
pub struct App;

impl std::default::Default for Model {
    fn default() -> Self {
        Self {
            experiments: vec![],
            selected_experiment: None,
            selected_task: None,
            selected_subject: None,
            selected_session: None,
        }
    }
}

impl Model {
    pub fn new(paths: &[&Path]) -> Self {
        // We need to search iteratively through the path to find experiments.
        // This means looking into all subdirectories and checking for a pyproject.tom file.
        // If a subdirectory contains a pyproject.toml file with a [tool.psydk.experiment] section,
        // we consider it an experiment.
        // If a subdirectory does not contain a pyproject.toml file, we look into its subdirectories.

        let mut experiments = vec![];

        // recursively walk the directory
        fn walk_dir(path: &Path, experiments: &mut Vec<Experiment>, depth: usize) {
            println!("Walking directory: {:?}", path);
            // if directory is hidden, skip it
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with('.') {
                    println!("Skipping hidden directory.");
                    return;
                }
            }

            if depth > 10 {
                return; // prevent infinite recursion
            }

            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries {
                    if let Ok(entry) = entry {
                        let path = entry.path();
                        if path.is_dir() {
                            // Check if this directory contains a pyproject.toml file
                            let pyproject_path = path.join("pyproject.toml");
                            if pyproject_path.exists() {
                                println!("Found pyproject.toml in directory: {:?}", path);
                                // Try to load the experiment from this path

                                match Model::try_load_experiment_from_path(&path) {
                                    Ok(experiment) => {
                                        println!("Loaded experiment: {:?}", experiment.name);
                                        experiments.push(experiment);
                                    }
                                    Err(err) => {
                                        println!("Failed to load experiment from {:?}: {}", path, err);
                                    }
                                }
                            } else {
                                // Recurse into the subdirectory
                                walk_dir(&path, experiments, depth + 1);
                            }
                        }
                    }
                }
            }
        }
        for path in paths.iter() {
            walk_dir(path, &mut experiments, 0);
        }

        Self {
            experiments,
            selected_experiment: None,
            selected_task: None,
            selected_subject: None,
            selected_session: None,
        }
    }

    fn try_load_experiment_from_path(path: &Path) -> Result<Experiment, String> {
        // try to read the pyproject.toml file
        if let Ok(contents) = std::fs::read_to_string(path.join("pyproject.toml")) {
            // parse the toml
            if let Ok(value) = contents.parse::<toml::Table>() {
                // check for [tool.psydk.experiment] section
                if let (Some(project_section), Some(psydk_section)) =
                    (value.get("project"), value.get("tool").and_then(|t| t.get("psydk")))
                {
                    // extract experiment details
                    let name = project_section
                        .get("name")
                        .and_then(|n| n.as_str())
                        .ok_or("project.toml missing project name")?;

                    let version = project_section
                        .get("version")
                        .and_then(|v| v.as_str())
                        .ok_or("project.toml missing version")?;

                    let description = project_section
                        .get("description")
                        .and_then(|d| d.as_str())
                        .unwrap_or("")
                        .to_string();

                    let mut tasks = psydk_section
                        .get("tasks")
                        .and_then(|t| t.as_array())
                        .unwrap_or(&vec![])
                        .iter()
                        .filter_map(|task| {
                            task.as_str().map(|name| Task {
                                id: rand::thread_rng().gen(),
                                name: name.to_string(),
                            })
                        })
                        .collect::<Vec<Task>>();

                    if tasks.is_empty() {
                        tasks.push(Task {
                            id: rand::thread_rng().gen(),
                            name: "default".to_string(),
                        });
                    }

                    let mut data_dir = std::path::PathBuf::from(
                        psydk_section
                            .get("data_directory")
                            .and_then(|d| d.as_str())
                            .ok_or("psydk section missing data_directory")?,
                    );

                    // add the experiment path to the data directory if it's relative
                    if data_dir.is_relative() {
                        data_dir = path.join(data_dir);
                    }

                    // load the subjecs and sessions from the data directory
                    // a suject is a directory starting with "sub-"
                    // a session is a directory starting with "ses-" inside a subject directory

                    let subjects = read_dir(&data_dir)
                        .map_err(|e| format!("Failed to read data directory: {}", e))?
                        .filter_map(|entry| {
                            let entry = entry.ok()?;
                            let path = entry.path();
                            if path.is_dir() {
                                let name = path.file_name()?.to_str()?;
                                if name.starts_with("sub-") {
                                    // remove the sub- prefix for the subject label
                                    let subj_label = name.trim_start_matches("sub-");
                                    // load sessions
                                    let sessions = read_dir(&path)
                                        .ok()?
                                        .filter_map(|sess_entry| {
                                            let sess_entry = sess_entry.ok()?;
                                            let sess_path = sess_entry.path();
                                            if sess_path.is_dir() {
                                                let sess_name = sess_path.file_name()?.to_str()?;
                                                if sess_name.starts_with("ses-") {
                                                    // remove the ses- prefix for the session label
                                                    let sess_label = sess_name.trim_start_matches("ses-");
                                                    Some(Session {
                                                        id: rand::thread_rng().gen(),
                                                        directory: sess_path.to_path_buf(),
                                                        name: sess_label.to_string(),
                                                    })
                                                } else {
                                                    None
                                                }
                                            } else {
                                                None
                                            }
                                        })
                                        .collect::<Vec<Session>>();

                                    Some(Subject {
                                        id: rand::thread_rng().gen(),
                                        directory: path.to_path_buf(),
                                        name: subj_label.to_string(),
                                        sessions,
                                    })
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<Subject>>();

                    // create Experiment struct
                    return Ok(Experiment {
                        id: rand::thread_rng().gen(),
                        directory: path.to_path_buf(),
                        name: name.to_string(),
                        icon_path: None,
                        version: version.to_string(),
                        description,
                        subjects: subjects,
                        tasks: tasks.clone(),
                        default_task: tasks[0].clone(),
                    });
                } else {
                    return Err("No [project] and/or [tools.psydk] sections in pyproject.toml found.".to_string());
                }
            } else {
                return Err("Failed to parse pyproject.toml".to_string());
            }
        } else {
            return Err("Failed to read pyproject.toml".to_string());
        }
    }
}

impl crux_core::App for App {
    type Model = Model;
    type Event = Event;
    type ViewModel = ViewModel;
    type Capabilities = ();
    type Effect = Effect;

    fn update(&self, msg: Event, model: &mut Model, _caps: &()) -> Command<Effect, Event> {
        match msg {
            Event::LoadExperiments(paths) => {
                // For simplicity, we are not loading from a file in this example.
                println!("LoadExperiments: paths={:#?}", paths);

                let _ = std::mem::replace(model, Model::new(&paths.iter().map(Path::new).collect::<Vec<&Path>>()));

                render()
            }
            Event::AddNewSubject(exp, name) => {
                println!("AddNewSubject: experiment={:?}, name={}", exp.name, name);
                // check if subject name is alphanumeric
                if !name.chars().all(|c| c.is_alphanumeric()) {
                    println!("Subject name must be alphanumeric.");
                    return render();
                }
                // find the experiment to add the subject to
                for experiment in model.experiments.iter_mut() {
                    if experiment.id == exp.id {
                        // try to create the subject directory on disk
                        let target_dir = experiment.directory.join("data").join(format!("sub-{}", name));

                        match std::fs::create_dir_all(&target_dir) {
                            Ok(_) => {
                                experiment.subjects.push(Subject {
                                    id: rand::thread_rng().gen(),
                                    directory: target_dir,
                                    name: name.clone(),
                                    sessions: vec![],
                                });
                            }
                            Err(err) => {
                                println!("Failed to create subject directory: {:?}", err);
                                return render();
                            }
                        }
                        break;
                    }
                }
                render()
            }
            Event::AddNewSession(subj, name) => {
                println!("AddNewSession: subject={:?}, name={}", subj.name, name);
                // check if session name is alphanumeric
                if !name.chars().all(|c| c.is_alphanumeric()) {
                    println!("Session name must be alphanumeric.");
                    return render();
                }
                // find the experiment and subject to add the session to
                for experiment in model.experiments.iter_mut() {
                    if let Some(existing_subj) = experiment.subjects.iter_mut().find(|s| s.id == subj.id) {
                        // try to create the session directory on disk
                        let target_dir = experiment
                            .directory
                            .join("data")
                            .join(format!("sub-{}", existing_subj.name))
                            .join(format!("ses-{}", name));

                        match std::fs::create_dir_all(&target_dir) {
                            Ok(_) => {
                                existing_subj.sessions.push(Session {
                                    id: rand::thread_rng().gen(),
                                    directory: target_dir,
                                    name: name.clone(),
                                });
                                return render();
                            }
                            Err(err) => {
                                println!("Failed to create session directory: {:?}", err);
                                return render();
                            }
                        }
                    }
                }
                render()
            }
        }
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        println!(
            "View called: selected_experiment={:?}, selected_subject={:?}, selected_session={:?}",
            model.selected_experiment, model.selected_subject, model.selected_session
        );
        Self::ViewModel {
            experiments: model.experiments.clone(),
            subjects: model
                .selected_experiment
                .as_ref()
                .map(|exp| exp.subjects.clone())
                .unwrap_or_default(),
            sessions: model
                .selected_subject
                .as_ref()
                .map(|subj| subj.sessions.clone())
                .unwrap_or_default(),
        }
    }
}
