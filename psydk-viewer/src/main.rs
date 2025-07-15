use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints};
use lsl::{Pullable, StreamInfo, StreamInlet};
use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

const MAX_SAMPLES: usize = 1000;
const BUFFER_SIZE: i32 = 360;

#[derive(Clone)]
struct StreamData {
    name: String,
    channel_count: usize,
    sample_rate: f64,
}

#[derive(Clone)]
struct DataSample {
    timestamp: f64,
    values: Vec<f32>,
}

enum LslCommand {
    RefreshStreams,
    Connect(usize), // Index of stream to connect to
    Disconnect,
}

enum LslResponse {
    StreamsFound(Vec<StreamData>),
    Connected(String, usize), // name, channel_count
    Disconnected,
    Error(String),
    Data(DataSample),
}

#[derive(Default)]
struct LslViewer {
    // Connection state
    available_streams: Vec<StreamData>,
    selected_stream_index: Option<usize>,
    is_connected: bool,

    // Channel selection
    channel_count: usize,
    selected_channels: Vec<bool>,

    // Data storage
    data_buffer: Vec<VecDeque<f32>>,
    time_buffer: VecDeque<f64>,

    // Communication channels
    command_sender: Option<Sender<LslCommand>>,
    response_receiver: Option<Receiver<LslResponse>>,

    // UI state
    status_message: String,
    auto_refresh: bool,
}

impl LslViewer {
    fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<LslCommand>();
        let (resp_tx, resp_rx) = mpsc::channel::<LslResponse>();

        // Spawn LSL handler thread
        thread::spawn(move || {
            lsl_handler_thread(cmd_rx, resp_tx);
        });

        Self {
            command_sender: Some(cmd_tx),
            response_receiver: Some(resp_rx),
            ..Default::default()
        }
    }

    fn send_command(&self, command: LslCommand) {
        if let Some(sender) = &self.command_sender {
            let _ = sender.send(command);
        }
    }

    fn process_responses(&mut self) {
        if let Some(receiver) = &self.response_receiver {
            // Process all available responses
            while let Ok(response) = receiver.try_recv() {
                match response {
                    LslResponse::StreamsFound(streams) => {
                        self.available_streams = streams;
                        if self.available_streams.is_empty() {
                            self.status_message = "No streams found".to_string();
                        } else {
                            self.status_message =
                                format!("Found {} stream(s)", self.available_streams.len());
                        }
                    }
                    LslResponse::Connected(name, channel_count) => {
                        self.channel_count = channel_count;
                        self.selected_channels = vec![true; channel_count];
                        self.data_buffer =
                            vec![VecDeque::with_capacity(MAX_SAMPLES); channel_count];
                        self.time_buffer = VecDeque::with_capacity(MAX_SAMPLES);
                        self.is_connected = true;
                        self.status_message =
                            format!("Connected to: {} ({} channels)", name, channel_count);
                    }
                    LslResponse::Disconnected => {
                        self.is_connected = false;
                        self.selected_stream_index = None;
                        self.status_message = "Disconnected".to_string();
                    }
                    LslResponse::Error(msg) => {
                        self.status_message = format!("Error: {}", msg);
                    }
                    LslResponse::Data(sample) => {
                        // Add timestamp
                        self.time_buffer.push_back(sample.timestamp);
                        if self.time_buffer.len() > MAX_SAMPLES {
                            self.time_buffer.pop_front();
                        }

                        // Add data for each channel
                        for (i, &value) in sample.values.iter().enumerate() {
                            if let Some(channel_buffer) = self.data_buffer.get_mut(i) {
                                channel_buffer.push_back(value);
                                if channel_buffer.len() > MAX_SAMPLES {
                                    channel_buffer.pop_front();
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn lsl_handler_thread(cmd_rx: Receiver<LslCommand>, resp_tx: Sender<LslResponse>) {
    let mut available_streams: Vec<StreamInfo> = Vec::new();
    let mut inlet: Option<StreamInlet> = None;
    let mut channel_count = 0;

    loop {
        // Check for commands
        match cmd_rx.try_recv() {
            Ok(LslCommand::RefreshStreams) => match lsl::resolve_streams(1.0) {
                Ok(streams) => {
                    available_streams = streams;
                    let stream_data: Vec<StreamData> = available_streams
                        .iter()
                        .map(|s| StreamData {
                            name: s.stream_name().to_string(),
                            channel_count: s.channel_count() as usize,
                            sample_rate: s.nominal_srate(),
                        })
                        .collect();
                    let _ = resp_tx.send(LslResponse::StreamsFound(stream_data));
                }
                Err(e) => {
                    let _ = resp_tx.send(LslResponse::Error(format!(
                        "Failed to refresh streams: {}",
                        e
                    )));
                }
            },
            Ok(LslCommand::Connect(index)) => {
                if let Some(stream_info) = available_streams.get(index) {
                    channel_count = stream_info.channel_count() as usize;
                    match StreamInlet::new(stream_info, BUFFER_SIZE, 0, true) {
                        Ok(new_inlet) => {
                            inlet = Some(new_inlet);
                            let _ = resp_tx.send(LslResponse::Connected(
                                stream_info.stream_name().to_string(),
                                channel_count,
                            ));
                        }
                        Err(e) => {
                            let _ = resp_tx
                                .send(LslResponse::Error(format!("Failed to connect: {}", e)));
                        }
                    }
                } else {
                    let _ = resp_tx.send(LslResponse::Error("Invalid stream index".to_string()));
                }
            }
            Ok(LslCommand::Disconnect) => {
                inlet = None;
                let _ = resp_tx.send(LslResponse::Disconnected);
            }
            Err(mpsc::TryRecvError::Disconnected) => break,
            Err(mpsc::TryRecvError::Empty) => {}
        }

        // Pull data if connected
        if let Some(ref inlet) = inlet {
            let mut sample = vec![0.0f32; channel_count];
            match inlet.pull_sample_buf(&mut sample, 0.0) {
                Ok(timestamp) if timestamp > 0.0 => {
                    let data = DataSample {
                        timestamp,
                        values: sample,
                    };
                    if resp_tx.send(LslResponse::Data(data)).is_err() {
                        break; // Main thread has closed
                    }
                }
                _ => {
                    thread::sleep(Duration::from_millis(1));
                }
            }
        } else {
            thread::sleep(Duration::from_millis(10));
        }
    }
}

impl eframe::App for LslViewer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process responses from LSL thread
        self.process_responses();

        // Auto-refresh UI
        if self.auto_refresh {
            ctx.request_repaint_after(Duration::from_millis(16)); // ~60 FPS
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("LSL Data Viewer");

            ui.separator();

            // Connection controls
            ui.horizontal(|ui| {
                if ui.button("Refresh Streams").clicked() {
                    self.send_command(LslCommand::RefreshStreams);
                }

                ui.checkbox(&mut self.auto_refresh, "Auto-refresh display");
            });

            // Stream selection
            if !self.available_streams.is_empty() {
                ui.group(|ui| {
                    ui.label("Available Streams:");
                    for (i, stream) in self.available_streams.iter().enumerate() {
                        ui.horizontal(|ui| {
                            let is_selected = self.selected_stream_index == Some(i);
                            if ui
                                .selectable_label(
                                    is_selected,
                                    format!(
                                        "{} - {} channels @ {} Hz",
                                        stream.name, stream.channel_count, stream.sample_rate
                                    ),
                                )
                                .clicked()
                                && !self.is_connected
                            {
                                // self.selected_stream_index = Some(i);
                                // self.send_command(LslCommand::Connect(i));
                            }
                        });
                    }
                });
            }

            // Connection status and controls
            ui.horizontal(|ui| {
                ui.label("Status:");
                ui.label(&self.status_message);

                if self.is_connected {
                    if ui.button("Disconnect").clicked() {
                        self.send_command(LslCommand::Disconnect);
                    }
                }
            });

            ui.separator();

            // Channel selection
            if self.is_connected && self.channel_count > 0 {
                ui.group(|ui| {
                    ui.label("Channel Selection:");
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("All").clicked() {
                            self.selected_channels.fill(true);
                        }
                        if ui.button("None").clicked() {
                            self.selected_channels.fill(false);
                        }
                        ui.separator();

                        for i in 0..self.channel_count {
                            ui.checkbox(&mut self.selected_channels[i], format!("Ch {}", i));
                        }
                    });
                });

                ui.separator();

                // Data visualization
                if !self.time_buffer.is_empty() {
                    let plot = Plot::new("lsl_plot")
                        .view_aspect(2.0)
                        .allow_zoom(true)
                        .allow_drag(true)
                        .allow_scroll(true);

                    plot.show(ui, |plot_ui| {
                        for (ch_idx, channel_data) in self.data_buffer.iter().enumerate() {
                            if ch_idx < self.selected_channels.len()
                                && self.selected_channels[ch_idx]
                                && !channel_data.is_empty()
                            {
                                let points: PlotPoints = channel_data
                                    .iter()
                                    .enumerate()
                                    .map(|(i, &y)| {
                                        let x = i as f64;
                                        [x, y as f64]
                                    })
                                    .collect();

                                plot_ui.line(Line::new(points).name(format!("Channel {}", ch_idx)));
                            }
                        }
                    });

                    // Display some stats
                    ui.horizontal(|ui| {
                        ui.label(format!("Samples buffered: {}", self.time_buffer.len()));
                        if let Some(&last_time) = self.time_buffer.back() {
                            ui.label(format!("Last timestamp: {:.3}", last_time));
                        }
                    });
                } else {
                    ui.label("No data received yet...");
                }
            }
        });
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("LSL Data Viewer"),
        ..Default::default()
    };

    eframe::run_native(
        "LSL Data Viewer",
        options,
        Box::new(|_cc| Ok(Box::new(LslViewer::new()))),
    )
}
