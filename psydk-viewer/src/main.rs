// #![windows_subsystem = "windows"]
use eframe::egui;
use egui::Stroke;
use egui_plot::{AxisHints, GridInput, GridMark, Line, Plot, PlotPoints, Points, VLine};
use lsl::{Pullable, StreamInfo, StreamInlet, XMLElement};
use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;
use std::{f64, thread};

const DEFAULT_TIME_WINDOW_SECONDS: f64 = 2.0; // Show last 10 seconds of data
const BUFFER_SIZE: i32 = 360;
const DEFAULT_SCALE: f64 = 25.0; // Default scale for data visualization
const DEFAULT_DOWN_SAMPLE_FACTOR: usize = 1; // Default downsample factor
const DEFAULT_BASELINE_TIME_WINDOW: f32 = 0.100; // Default baseline time window in seconds

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
    Connected(String, Vec<String>), // Stream name and channel names
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

    // Channel data
    channel_names: Vec<String>,

    // Channel selection
    channel_count: usize,
    selected_channels: Vec<bool>,

    // Data visualization parameters
    data_scale: f64,
    time_window_seconds: f64,
    downsample_factor: usize,
    reference_channel: Option<usize>,

    // Data storage - now storing (timestamp, value) pairs
    //data_buffer: Vec<VecDeque<(f64, f32)>>,
    data_buffer: Vec<VecDeque<f32>>, // Buffer for each channel
    timestamp_buffer: VecDeque<f64>, // Separate buffer for timestamps
    channel_baselines: Vec<f64>,

    // Communication channels
    command_sender: Option<Sender<LslCommand>>,
    response_receiver: Option<Receiver<LslResponse>>,

    // UI state
    status_message: String,
    auto_refresh: bool,
    last_t: f64,
    channel_colors: Vec<egui::Color32>,
}

impl LslViewer {
    fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<LslCommand>();
        let (resp_tx, resp_rx) = mpsc::channel::<LslResponse>();

        // Spawn LSL handler thread
        thread::spawn(move || {
            lsl_handler_thread(cmd_rx, resp_tx);
        });

        let o = Self {
            command_sender: Some(cmd_tx),
            response_receiver: Some(resp_rx),
            auto_refresh: true,
            data_scale: DEFAULT_SCALE,
            time_window_seconds: DEFAULT_TIME_WINDOW_SECONDS,
            last_t: 0.0,
            downsample_factor: DEFAULT_DOWN_SAMPLE_FACTOR,
            reference_channel: None,

            ..Default::default()
        };

        // Initial command to refresh streams
        o.send_command(LslCommand::RefreshStreams);

        o
    }

    fn send_command(&self, command: LslCommand) {
        if let Some(sender) = &self.command_sender {
            let _ = sender.send(command);
        }
    }

    fn process_responses(&mut self) {
        if let Some(receiver) = &self.response_receiver {
            // use pastel colors for channels
            let colors = vec![
                egui::Color32::from_rgb(255, 105, 180), // Pink
                egui::Color32::from_rgb(135, 206, 235), // Sky Blue
                egui::Color32::from_rgb(255, 215, 0),   // Gold
                egui::Color32::from_rgb(144, 238, 144), // Light Green
                egui::Color32::from_rgb(255, 160, 122), // Light Salmon
                egui::Color32::from_rgb(255, 182, 193), // Light Pink
                egui::Color32::from_rgb(255, 228, 181), // Moccasin
                egui::Color32::from_rgb(173, 216, 230), // Light Blue
                egui::Color32::from_rgb(221, 160, 221), // Plum
                egui::Color32::from_rgb(255, 99, 71),   // Tomato
                egui::Color32::from_rgb(255, 140, 0),   // Dark Orange
                egui::Color32::from_rgb(255, 250, 205), // Lemon Chiffon
                egui::Color32::from_rgb(240, 230, 140), // Khaki
                egui::Color32::from_rgb(255, 218, 185), // Peach Puff
            ];
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
                    LslResponse::Connected(name, channels) => {
                        let channel_count = channels.len();
                        self.channel_count = channel_count;
                        self.selected_channels = vec![true; channel_count];
                        self.data_buffer = vec![VecDeque::new(); channel_count];
                        self.timestamp_buffer = VecDeque::new();
                        self.channel_baselines = vec![0.0; channel_count];
                        self.channel_names = channels;
                        self.is_connected = true;
                        self.status_message =
                            format!("Connected to: {} ({} channels)", name, channel_count);
                        // asign channel colors
                        self.channel_colors = (0..channel_count)
                            .map(|i| {
                                let color_index = i % colors.len();
                                colors[color_index].to_owned()
                            })
                            .collect();
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
                        // Add the timestamp to the timestamp buffer
                        self.timestamp_buffer.push_back(sample.timestamp);
                        // Add data for each channel
                        for (ch, &value) in sample.values.iter().enumerate() {
                            if let Some(channel_data_buffer) = self.data_buffer.get_mut(ch) {
                                channel_data_buffer.push_back(value);
                            }
                        }

                        // Remove old data (older than TIME_WINDOW_SECONDS)
                        let cutoff_time = sample.timestamp - self.time_window_seconds;
                        let cuttoff_index = self
                            .timestamp_buffer
                            .iter()
                            .rev()
                            .position(|&t| t <= cutoff_time);

                        if let Some(index) = cuttoff_index {
                            // Remove old timestamps
                            while self.timestamp_buffer.len() > index + 1 {
                                self.timestamp_buffer.pop_front();
                            }
                            // Remove old data for each channel
                            for channel_data in self.data_buffer.iter_mut() {
                                while channel_data.len() > index + 1 {
                                    channel_data.pop_front();
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn baseline_correct(&mut self) {
        // Calculate baseline for each channel
        for (i, channel_data) in self.data_buffer.iter_mut().enumerate() {
            if !channel_data.is_empty() {
                let mut sum = 0.0;
                let mut count = 0;
                let oldest_timestamp_to_inlcude = self.timestamp_buffer.back().unwrap_or(&0.0)
                    - DEFAULT_BASELINE_TIME_WINDOW as f64;
                for (_, &value) in self
                    .timestamp_buffer
                    .iter()
                    .zip(channel_data.iter())
                    .rev()
                    .take_while(|&(t, _)| *t >= oldest_timestamp_to_inlcude)
                {
                    sum += value as f64;
                    count += 1;
                }

                let baseline = sum / count as f64;
                self.channel_baselines[i] = baseline;
            }
        }
    }
}

fn extract_channel_names(info: &mut StreamInfo, expected_count: usize) -> Vec<String> {
    let mut channel_names = vec![];

    let mut cursor = info.desc().child("channels").child("channel");
    while cursor.is_valid() {
        channel_names.push(cursor.child_value_named("label"));
        cursor = cursor.next_sibling();
    }

    if channel_names.len() != expected_count {
        (0..info.channel_count())
            .map(|i| "Ch ".to_string() + &i.to_string())
            .collect()
    } else {
        channel_names
    }
}

fn lsl_handler_thread(cmd_rx: Receiver<LslCommand>, resp_tx: Sender<LslResponse>) {
    let mut available_streams: Vec<StreamInfo> = Vec::new();
    let mut inlet: Option<StreamInlet> = None;
    let mut channel_count = 0;

    loop {
        // Check for commands
        match cmd_rx.try_recv() {
            Ok(LslCommand::RefreshStreams) => match lsl::resolve_streams(3.0) {
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
                            new_inlet
                                .set_postprocessing(&[
                                    lsl::ProcessingOption::ClockSync,
                                    lsl::ProcessingOption::Dejitter,
                                ])
                                .expect("Failed to set postprocessing");

                            //
                            //
                            let mut info = new_inlet.info(5.0).expect("Failed to get stream info");

                            let channel_names = extract_channel_names(&mut info, channel_count);
                            inlet = Some(new_inlet);
                            let _ = resp_tx.send(LslResponse::Connected(
                                stream_info.stream_name().to_string(),
                                channel_names.clone(),
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
            if let Ok((chunk, timestamps)) = inlet.pull_chunk() {
                if !chunk.is_empty() {
                    for (i, &timestamp) in timestamps.iter().enumerate() {
                        let data = DataSample {
                            timestamp,
                            values: chunk[i].to_vec(),
                        };

                        if resp_tx.send(LslResponse::Data(data)).is_err() {
                            panic!("Failed to send data response");
                        }
                    }
                }
                thread::sleep(Duration::from_millis(25));
            } else {
                panic!("Failed to pull data from LSL inlet");
            }
        } else {
            thread::sleep(Duration::from_millis(25));
        }
    }
}

impl eframe::App for LslViewer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process responses from LSL thread
        self.process_responses();

        // Auto-refresh UI
        if self.auto_refresh {
            ctx.request_repaint_after(Duration::from_millis(32)); // ~60 FPS
        }

        // left panel for stream selection and controls
        egui::SidePanel::right("right_panel")
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    // Connection controls
                    // only show the refresh button if not connected
                    if !self.is_connected {
                        ui.horizontal(|ui| {
                            if ui.button("Refresh Streams").clicked() {
                                self.send_command(LslCommand::RefreshStreams);
                            }
                        });

                        // Stream selection
                        if !self.available_streams.is_empty() {
                            ui.group(|ui| {
                                ui.label("Available Streams:");
                                let streams = self.available_streams.clone();
                                for (i, stream) in streams.iter().enumerate() {
                                    ui.horizontal(|ui| {
                                        let is_selected = self.selected_stream_index == Some(i);
                                        if ui
                                            .selectable_label(
                                                is_selected,
                                                format!(
                                                    "{} - {} channels @ {} Hz",
                                                    stream.name,
                                                    stream.channel_count,
                                                    stream.sample_rate
                                                ),
                                            )
                                            .clicked()
                                            && !self.is_connected
                                        {
                                            self.selected_stream_index = Some(i);
                                            self.send_command(LslCommand::Connect(i));
                                        }
                                    });
                                }
                            });
                        }
                    }

                    // Connection status and controls

                    if self.is_connected && self.channel_count > 0 {
                        ui.group(|ui| {
                            ui.horizontal_wrapped(|ui| {
                                if ui.button("All").clicked() {
                                    self.selected_channels.fill(true);
                                }
                                if ui.button("None").clicked() {
                                    self.selected_channels.fill(false);
                                }
                                ui.separator();

                                for (i, name) in self.channel_names.iter().enumerate() {
                                    ui.checkbox(&mut self.selected_channels[i], name);
                                }
                            });
                        });

                        // Scale control via slider
                        ui.group(|ui| {
                            ui.label("Scale");
                            if ui
                                .add(
                                    egui::Slider::new(&mut self.data_scale, 1.0..=100.0)
                                        .text("mV")
                                        .clamp_to_range(true),
                                )
                                .changed()
                            {
                                // Update scale immediately
                                self.baseline_correct();
                            }
                        });

                        // Time window control via drop-down
                        ui.group(|ui| {
                            egui::ComboBox::from_id_source("time_window")
                                .selected_text(format!("{} seconds", self.time_window_seconds))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut self.time_window_seconds,
                                        1.0,
                                        "1 second",
                                    );
                                    ui.selectable_value(
                                        &mut self.time_window_seconds,
                                        2.0,
                                        "2 seconds",
                                    );
                                    ui.selectable_value(
                                        &mut self.time_window_seconds,
                                        5.0,
                                        "5 seconds",
                                    );
                                    ui.selectable_value(
                                        &mut self.time_window_seconds,
                                        10.0,
                                        "10 seconds",
                                    );
                                });
                        });

                        // Ad-hoc baseline correction
                        ui.group(|ui| {
                            ui.label("Baseline Correction");
                            if ui.button("Correct Now").clicked() {
                                self.baseline_correct();
                            }
                        });

                        // Allow resampling for plotting using an integer divsior (dropdown)
                        ui.group(|ui| {
                            egui::ComboBox::from_id_source("resample")
                                .selected_text(if self.downsample_factor == 1 {
                                    "No Resampling".to_string()
                                } else {
                                    format!("{}x Resampling", self.downsample_factor)
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.downsample_factor, 1, "Disable");
                                    ui.selectable_value(&mut self.downsample_factor, 2, "2x");
                                    ui.selectable_value(&mut self.downsample_factor, 3, "3x");
                                    ui.selectable_value(&mut self.downsample_factor, 4, "4x");
                                    ui.selectable_value(&mut self.downsample_factor, 5, "5x");
                                    ui.selectable_value(&mut self.downsample_factor, 10, "10x");
                                });
                        });

                        // Allow re-referencing to a specific channel
                        ui.group(|ui| {
                            egui::ComboBox::from_id_source("re_reference")
                                .selected_text(if let Some(ref_idx) = self.reference_channel {
                                    format!("Referenced to {}", self.channel_names[ref_idx])
                                } else {
                                    "Unreferenced".to_string()
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.reference_channel, None, "None");
                                    for (i, name) in self.channel_names.iter().enumerate() {
                                        ui.selectable_value(
                                            &mut self.reference_channel,
                                            Some(i),
                                            name,
                                        );
                                    }
                                });
                        });

                        // Stream information
                        ui.group(|ui| {
                            ui.label("Connected Stream Info:");
                            if let Some(index) = self.selected_stream_index {
                                if let Some(stream) = self.available_streams.get(index) {
                                    ui.label(format!("Name: {}", stream.name));
                                    ui.label(format!("Channels: {}", stream.channel_count));
                                    ui.label(format!("Sample Rate: {:.2} Hz", stream.sample_rate));
                                }
                            } else {
                                ui.label("No stream selected");
                            }
                        });
                    }
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical(|ui| {
                if self.is_connected && self.channel_count > 0 {
                    // Data visualization
                    if !self.data_buffer.is_empty() && self.data_buffer[0].len() > 0 {
                        let selected_channel_count =
                            self.selected_channels.iter().filter(|&&x| x).count();
                        let selected_channel_labels: Vec<String> = self
                            .selected_channels
                            .iter()
                            .enumerate()
                            .filter_map(|(i, &selected)| {
                                if selected {
                                    Some(self.channel_names[i].clone())
                                } else {
                                    None
                                }
                            })
                            .collect();

                        let y_formatter =
                            |grid_mark: GridMark, _range: &std::ops::RangeInclusive<f64>| {
                                let index = (-1.0 * grid_mark.value) as usize;

                                if index >= selected_channel_labels.len() {
                                    return "??".to_string();
                                }
                                selected_channel_labels[index].to_string()
                            };

                        let y_grid_spacer = |_grid_input: GridInput| {
                            (0..selected_channel_count)
                                .map(|i| GridMark {
                                    value: -1.0 * (i as f64),
                                    step_size: 1.0,
                                })
                                .collect::<Vec<_>>()
                        };

                        let plot = Plot::new("lsl_plot")
                            .default_x_bounds(0.0, self.time_window_seconds)
                            .default_y_bounds((selected_channel_count as f64 * -1.0) + 0.5, 0.5)
                            .allow_zoom(false)
                            .allow_drag(false)
                            .allow_scroll(false)
                            .x_axis_label("Time (seconds)")
                            .y_axis_label("Value")
                            .y_axis_formatter(y_formatter)
                            .y_grid_spacer(y_grid_spacer);

                        plot.show(ui, |plot_ui| {
                            // Find the most recent timestamp to use as reference
                            let latest_timestamp =
                                self.timestamp_buffer.back().cloned().unwrap_or(0.0);

                            // decide on the current time window to be shown (always n * TIME_WINDOW_SECONDS, where n is an integer)
                            let t0 =
                                latest_timestamp - (latest_timestamp % self.time_window_seconds);

                            // if we're re-referencing, prepare the reference channel
                            let ref_channel: Option<(Vec<f32>, f64)> =
                                if let Some(ref_idx) = self.reference_channel {
                                    if ref_idx < self.data_buffer.len() {
                                        Some((
                                            self.data_buffer[ref_idx].iter().cloned().collect(),
                                            self.channel_baselines[ref_idx],
                                        ))
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                };

                            let mut plot_idx = 0;
                            let mut t_last = 0.0;
                            for (ch_idx, channel_data) in self.data_buffer.iter().enumerate() {
                                if ch_idx < self.selected_channels.len()
                                    && self.selected_channels[ch_idx]
                                    && !channel_data.is_empty()
                                {
                                    let mut points_vec_a = Vec::new();
                                    let mut points_vec_b = Vec::new();

                                    // baseline-correct the data
                                    let baseline = self.channel_baselines[ch_idx];

                                    let n = self.downsample_factor.max(1);

                                    for (i, (value, timestamp)) in channel_data
                                        .iter()
                                        .step_by(n)
                                        .zip(self.timestamp_buffer.iter().step_by(n))
                                        .enumerate()
                                    {
                                        // We show a rolling window of data, so that new data is drawn from left to right
                                        let mut t = (timestamp - t0) % self.time_window_seconds;

                                        let v = if let Some(ref ref_data) = ref_channel {
                                            (*value as f64 - baseline)
                                                - (ref_data.0[i] as f64 - ref_data.1)
                                        } else {
                                            (*value as f64 - baseline)
                                        };

                                        let v = v * self.data_scale / 10000.0;

                                        let val = v + -1.0 * plot_idx as f64;

                                        if t > 0.0 {
                                            points_vec_a.push([t, val]);
                                        } else {
                                            t += self.time_window_seconds;
                                            points_vec_b.push([t, val]);
                                        }
                                    }

                                    t_last = points_vec_a.last().map_or(0.0, |p| p[0]);

                                    let points_a: PlotPoints = points_vec_a.into_iter().collect();
                                    let points_b: PlotPoints = points_vec_b.into_iter().collect();

                                    let line_a = Line::new(format!("Channel {}", ch_idx), points_a)
                                        .stroke(Stroke::new(1.0, self.channel_colors[ch_idx]));
                                    let line_b = Line::new(format!("Channel {}", ch_idx), points_b)
                                        .stroke(Stroke::new(1.0, egui::Color32::from_gray(150)));

                                    plot_ui.line(line_a);
                                    plot_ui.line(line_b);

                                    // add a vertical line at t_last
                                    plot_ui.vline(
                                        VLine::new("Time Window Start", t_last)
                                            .stroke(Stroke::new(
                                                1.0,
                                                egui::Color32::from_rgb(255, 10, 10),
                                            ))
                                            .name("Time Window Start"),
                                    );
                                    plot_idx += 1;
                                }
                            }
                            // check if we moved to a new time window
                            if t_last < self.last_t {
                                // request baseline correction
                                self.baseline_correct();
                            }
                            self.last_t = t_last;
                        });

                        // Display some stats
                        ui.horizontal(|ui| {
                            let total_samples: usize =
                                self.data_buffer.iter().map(|b| b.len()).sum();
                            ui.label(format!("Total samples buffered: {}", total_samples));

                            if let Some(channel_data) = self.data_buffer.first() {
                                if let Some(&last_time) = channel_data.back() {
                                    ui.label(format!("Last timestamp: {:.3}", last_time));
                                }
                            }
                        });
                    } else {
                        ui.label("No data received yet...");
                    }
                }
            });
        });

        // Status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.horizontal(|ui| {
                    ui.label("Status:");
                    ui.label(&self.status_message);

                    if self.is_connected {
                        if ui.button("Disconnect").clicked() {
                            self.send_command(LslCommand::Disconnect);
                        }
                    }
                });
            });
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
