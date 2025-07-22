use std::{borrow::Cow, collections::HashMap};

use eframe::egui;
use egui::Color32;

pub mod bids;
pub mod listview;
pub mod pyproject;
use crate::pyproject::PyProjectToml;

use listview::{ListItem, ListView};

pub enum ModalType {
    None,
    NewSubject,
    NewSession,
}

pub struct Subject {
    name: String,
    sessions: Vec<Session>,
}

pub struct Session {
    name: String,
}

pub struct Experiment {
    name: String,
    version: String,
    description: String,
}

impl ListItem for Experiment {
    fn title(&self) -> Cow<str> {
        (&self.name).into()
    }

    fn subtitle(&self) -> Option<Cow<str>> {
        Some(format!("v. {}", self.version).into())
    }
}

impl ListItem for Subject {
    fn title(&self) -> Cow<str> {
        (&self.name).into()
    }

    fn subtitle(&self) -> Option<Cow<str>> {
        Some(format!("{} sessions", self.sessions.len()).into())
    }
}

impl ListItem for Session {
    fn title(&self) -> Cow<str> {
        (&self.name).into()
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };

    // path to repo containing pyproject.toml
    let mut repo_path = r"C:\Users\CVL\Documents\experiments\newflicker26".to_owned();
    let mut storage_path = r"D:\data\newflicker26".to_owned();

    // load the pyproject.toml file
    let pyproject_path = format!("{}/pyproject.toml", repo_path);
    let pyproject_string =
        std::fs::read_to_string(&pyproject_path).expect("Failed to read pyproject.toml file");
    let pyproject =
        PyProjectToml::new(&pyproject_string).expect("Failed to parse pyproject.toml file");

    println!(
        "Loaded pyproject.toml for project: {} version: {}",
        pyproject.name().unwrap_or(&"Unknown".to_owned()),
        pyproject.version().unwrap_or("0.0.0".to_owned())
    );

    // create the BIDS Layout
    let bids_layout = bids::BIDSLayout::new(&storage_path);

    // list all subjects in the BIDS Layout
    let subjects = {
        let mut subject_ids = bids_layout.subjects();
        let mut subjects: Vec<Subject> = vec![];
        for subject_id in &subject_ids {
            // qyery the sessions for each subject
            let session_ids = bids_layout.sessions(subject_id);
            let sessions = session_ids
                .into_iter()
                .map(|s| Session { name: s.to_owned() })
                .collect();
            subjects.push(Subject {
                name: subject_id.to_owned(),
                sessions,
            });
        }
        subjects
    };

    // Our application state:
    let mut name = "Arthur".to_owned();
    let mut age = 42;
    let mut experiment_idx = 0;
    let mut subject_idx = 0;
    let mut session_idx = 0;

    let mut show_modal = ModalType::None;

    // generate a very long list
    let list = (1..1001).map(|i| format!("Item {i}")).collect::<Vec<_>>();

    let experiments = vec![Experiment {
        name: pyproject.name().unwrap_or(&"No Name".to_owned()).to_owned(),
        version: pyproject
            .version()
            .unwrap_or("No Version".to_owned())
            .to_owned(),
        description: pyproject
            .description()
            .unwrap_or(&"No Description".to_owned())
            .to_owned(),
    }];

    eframe::run_simple_native("My egui App", options, move |ctx, _frame| {
        egui::SidePanel::left("side_panel0").show(ctx, |ui| {
            // scroll container
            let mut list_view = ListView::new(&experiments, Some(&mut experiment_idx));
            list_view.show(ui);
        });
        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            // scroll container
            let mut list_view = ListView::new(&subjects, Some(&mut subject_idx));
            ui.add(
                egui::Button::new("New subject")
                    .fill(Color32::from_rgb(0, 140, 255))
                    .min_size([200.0, 30.0].into()),
            )
            .on_hover_text("Add a new subject")
            .clicked()
            .then(|| {
                // action on button click
                println!("Button clicked! Adding new subject.");
                show_modal = ModalType::NewSubject; // open modal
            });
            list_view.show(ui);
        });
        egui::SidePanel::left("side_panel2").show(ctx, |ui| {
            let mut list_view =
                ListView::new(&subjects[subject_idx].sessions, Some(&mut session_idx));
            ui.add(
                egui::Button::new("New session")
                    .fill(Color32::from_rgb(0, 140, 255))
                    .min_size([200.0, 30.0].into()),
            )
            .on_hover_text("Add a new session")
            .clicked()
            .then(|| {
                // action on button click
                println!("Button clicked! Adding new session.");
                show_modal = ModalType::NewSession; // open modal
            });
            list_view.show(ui);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let session_name = subjects[subject_idx]
                .sessions
                .get(session_idx)
                .map_or("No Session".to_owned(), |s| s.name.clone());

            let run_name = "02".to_owned(); // hardcoded for simplicity

            // big green Run button
            let button = egui::Button::new(
                egui::RichText::new(format!(
                    "Start run {} in session {}",
                    run_name, session_name
                ))
                .size(16.),
            )
            .fill(Color32::from_rgb(2, 140, 0))
            .corner_radius(4)
            .min_size([200.0, 50.0].into());
            ui.horizontal_centered(|ui| {
                ui.add_sized(egui::vec2(ui.available_size().x, 50.0), button)
                    .on_hover_text("Run the session")
                    .clicked()
                    .then(|| {
                        // action on button click
                        println!("Button clicked! Running session for {}.", name);
                    });
            });
            // show some mock settings for the experiment
            ui.label("Selected Experiment:");
            if let Some(experiment) = experiments.get(experiment_idx) {
                ui.label(format!("Name: {}", experiment.name));
                ui.label(format!("Version: {}", experiment.version));
                ui.label(format!("Description: {}", experiment.description));
            } else {
                ui.label("No experiment selected.");
            }
        });

        match show_modal {
            ModalType::NewSubject => {
                egui::Modal::new(egui::Id::new("New Subject")).show(ctx, |ui| {
                    ui.label("Enter new subject details:");
                    ui.text_edit_singleline(&mut name);
                    if ui.button("Save").clicked() {
                        println!("New subject saved: {}", name);
                        show_modal = ModalType::None; // close modal
                    }
                });
            }
            ModalType::NewSession => {
                egui::Modal::new(egui::Id::new("New Session")).show(ctx, |ui| {
                    ui.label("Enter new session designation:");
                    ui.text_edit_singleline(&mut name);
                    if ui.button("Create").clicked() {
                        show_modal = ModalType::None; // close modal
                    }
                    if ui.button("Cancel").clicked() {
                        show_modal = ModalType::None; // close modal
                    }
                });
            }
            _ => {}
        }
    })
}
