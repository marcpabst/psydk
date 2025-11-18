use atomic_float::AtomicF64;
use gstreamer_video::VideoOrientationMethod;
use pyo3::prelude::*;
extern crate gstreamer as gst;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize},
    Arc,
};

use gst::prelude::*;

mod rtsp;

pub mod prelude {
    pub use crate::CameraCaps;
    pub use crate::Recorder;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rotation {
    Rotate0, // No rotation
    Rotate90,
    Rotate180,
    Rotate270,
}

pub fn create_usb_camera_source() -> Result<gst::Element, gst::glib::Error> {
    // Initialize GStreamer if not already done
    gst::init()?;

    #[cfg(target_os = "linux")]
    {
        gst::ElementFactory::make("v4l2src").name("usb-camera-source").build()
    }

    #[cfg(target_os = "windows")]
    {
        // Try mfvideosrc first (modern Windows)
        gst::ElementFactory::make("mfvideosrc")
            .name("usb-camera-source")
            .build()
            .or_else(|_| {
                // Fallback to wasapi2src if mfvideosrc is not available
                // Note: wasapi2src is primarily for audio, this is just a last resort
                eprintln!("Warning: mfvideosrc not available, camera support may be limited");
                Err(gst::glib::Error::new(
                    gst::CoreError::MissingPlugin,
                    "mfvideosrc plugin not available",
                ))
            })
        // // use ksvideosrc  for lower latency
        // gst::ElementFactory::make("ksvideosrc")
        //     .name("usb-camera-source")
        //     .build()
        //     .or_else(|_| {
        //         // Fallback to ksvideosrc if mfvideosrc is not available
        //         eprintln!("Warning: ksvideosrc not available, camera support may be limited");
        //         Err(gst::glib::Error::new(
        //             gst::CoreError::MissingPlugin,
        //             "ksvideosrc plugin not available",
        //         ))
        //     })
    }

    #[cfg(target_os = "macos")]
    {
        gst::ElementFactory::make("avfvideosrc")
            .name("usb-camera-source")
            .build()
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        Err(gst::glib::Error::new(
            gst::CoreError::Failed,
            "Unsupported operating system",
        ))
    }
}

fn record(
    filename: &str,
    caps: &CameraCaps,
    is_recording: Arc<AtomicBool>,
    stop_flag: Arc<AtomicBool>,
    last_frame_time: Arc<AtomicF64>,
    last_frame_count: Arc<AtomicUsize>,
    display: bool,
    utp_port: Option<u16>,
    rotate: Option<Rotation>,
    bitrate: Option<u32>,
) {
    // Initialize GStreamer
    gst::init().unwrap();

    let rotation_angle = match rotate {
        None => VideoOrientationMethod::Identity,
        Some(Rotation::Rotate0) => VideoOrientationMethod::Identity,
        Some(Rotation::Rotate90) => VideoOrientationMethod::_90r,
        Some(Rotation::Rotate180) => VideoOrientationMethod::_180,
        Some(Rotation::Rotate270) => VideoOrientationMethod::_90l,
    };

    // Create the elements
    let source = create_usb_camera_source().unwrap_or_else(|_| {
        panic!("Failed to create USB camera source. Ensure the appropriate GStreamer plugins are installed.");
    });
    let caps_filter = gst::ElementFactory::make("capsfilter").build().unwrap();
    let rotate = gst::ElementFactory::make("videoflip")
        .property("video-direction", &rotation_angle)
        .build()
        .unwrap();
    let tee = gst::ElementFactory::make("tee").build().unwrap();
    let tee2 = gst::ElementFactory::make("tee").build().unwrap();

    let display_queue = gst::ElementFactory::make("queue").build().unwrap();
    let videoconvert = gst::ElementFactory::make("videoconvert").build().unwrap();
    let autovideosink = gst::ElementFactory::make("autovideosink").build().unwrap();

    let recorder_queue = gst::ElementFactory::make("queue").build().unwrap();
    recorder_queue.set_property("max-size-buffers", &1u32.to_value());
    let encoder = gst::ElementFactory::make("x264enc")
        .property_from_str("tune", "zerolatency")
        .property_from_str("speed-preset", "ultrafast")
        .build()
        .unwrap();
    let parser = gst::ElementFactory::make("h264parse").build().unwrap();
    let muxer = gst::ElementFactory::make("matroskamux").build().unwrap();
    let sink = gst::ElementFactory::make("filesink").build().unwrap();

    // Add a probe to the source to capture frame timestamps
    source
        .static_pad("src")
        .unwrap()
        .add_probe(
            gst::PadProbeType::BUFFER | gst::PadProbeType::PUSH,
            move |_, probe_info| {
                let time = probe_info.buffer().and_then(|buf| buf.pts());
                if let Some(time) = time {
                    // Convert to seconds and store in the atomic variable
                    let seconds = time.seconds_f64();
                    last_frame_time.store(seconds, std::sync::atomic::Ordering::Relaxed);
                    // Increment the frame count
                    last_frame_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }

                gst::PadProbeReturn::Ok
            },
        )
        .unwrap();

    // set num-buffers=1 for the source
    // source.set_property("num-buffers", &1i32.to_value());
    // source.set_property("enable-quirks", true);

    // Set properties
    caps_filter.set_property(
        "caps",
        &gst::Caps::builder("video/x-raw")
            // .field("format", &caps.format)
            .field("width", caps.width)
            .field("height", caps.height)
            .field(
                "framerate",
                &gst::Fraction::new(caps.framerate_numerator, caps.framerate_denominator),
            )
            .build(),
    );

    sink.set_property("location", filename.to_value());

    // set bitrate for encoder to 8500 kbps
    if let Some(bitrate) = bitrate {
        encoder.set_property("bitrate", &bitrate);
    }

    // Create the empty pipeline
    let pipeline = gst::Pipeline::default();

    // Build the pipeline
    pipeline
        .add_many(&[
            &source,
            &caps_filter,
            &rotate,
            &tee,
            &videoconvert,
            &recorder_queue,
            &encoder,
            &tee2,
            &parser,
            &muxer,
            &sink,
        ])
        .unwrap();

    // link the source to the caps filter and the tee
    gst::Element::link_many(&[&source, &caps_filter, &rotate, &tee]).unwrap();

    // add and link the display pipeline
    if display {
        pipeline.add_many(&[&display_queue, &autovideosink]).unwrap();
        gst::Element::link_many(&[&tee, &display_queue, &autovideosink]).unwrap();
    }

    // link the recording pipeline
    gst::Element::link_many(&[
        &tee,
        &recorder_queue,
        &videoconvert,
        &encoder,
        &tee2,
        &parser,
        &muxer,
        &sink,
    ])
    .unwrap();

    // if utp_port is specified, add utp server
    if let Some(port) = utp_port {
        // let rtph264pay = gst::ElementFactory::make("rtph264pay")
        //     .property("config-interval", 10i32)
        //     .property("pt", 96u32)
        //     .property_from_str("aggregate-mode", "zero-latency")
        //     .build()
        //     .unwrap();
        // let queue = gst::ElementFactory::make("queue")
        //     .build()
        //     .unwrap();
        // let udpsink = gst::ElementFactory::make("udpsink")
        //     .property("host", "127.0.0.1")
        //     .property("port", port as i32)
        //     .property("sync", false)
        //     .build()
        //     .unwrap();
        // pipeline.add_many(&[&rtph264pay, &queue, &udpsink])
        //     .unwrap();
        // gst::Element::link_many(&[&tee2, &rtph264pay, &queue, &udpsink])
        //     .unwrap();
        //
        //

        // Clone the pipeline to move into the closure
        let pipeline_clone = pipeline.clone();

        // start rtsp server
        rtsp::run_rtsp_server(port, move |session: rtsp::RtspSession, client_ip: String| {
            // 1. Create elements
            let rtpbin = gst::ElementFactory::make("rtpbin")
                .build()
                .expect("Failed to create rtpbin");

            let rtph264pay = gst::ElementFactory::make("rtph264pay")
                .property("config-interval", 10i32)
                .property("pt", 96u32)
                .property_from_str("aggregate-mode", "zero-latency")
                .build()
                .expect("Failed to create rtph264pay");

            let queue = gst::ElementFactory::make("queue")
                .build()
                .expect("Failed to create queue");

            let udpsink = gst::ElementFactory::make("udpsink")
                .property("host", &client_ip)
                .property("port", &session.client_rtp.parse::<i32>().unwrap())
                .property("sync", false)
                .build()
                .expect("Failed to create udpsink");

            // 2. Add elements to pipeline
            pipeline_clone
                .add_many(&[&rtpbin, &rtph264pay, &queue, &udpsink])
                .expect("Failed to add elements to pipeline");

            // 3. Link source to payloader
            // Get a request pad from the tee
            let tee_pad = tee2.request_pad_simple("src_%u").expect("Failed to get tee src pad");
            let queue_sink_pad = queue.static_pad("sink").expect("Failed to get queue sink pad");
            tee_pad.link(&queue_sink_pad).expect("Failed to link tee to queue");

            // Link queue -> payloader
            gst::Element::link_many(&[&queue, &rtph264pay]).expect("Failed to link queue to payloader");

            // 4. Link payloader to rtpbin
            let pay_src_pad = rtph264pay.static_pad("src").expect("Failed to get payloader src pad");
            let rtpbin_sink_pad = rtpbin
                .request_pad_simple("send_rtp_sink_0")
                .expect("Failed to get rtpbin send_rtp_sink_0 pad");
            pay_src_pad
                .link(&rtpbin_sink_pad)
                .expect("Failed to link payloader to rtpbin");

            // 5. Link rtpbin src to udpsink
            let rtpbin_src_pad = rtpbin
                .static_pad("send_rtp_src_0")
                .expect("Failed to get rtpbin send_rtp_src_0 pad");
            let udpsink_sink_pad = udpsink.static_pad("sink").expect("Failed to get udpsink sink pad");
            rtpbin_src_pad
                .link(&udpsink_sink_pad)
                .expect("Failed to link rtpbin to udpsink");

            // 6. Sync state with parent
            for element in [&rtpbin, &rtph264pay, &queue, &udpsink] {
                element.sync_state_with_parent().unwrap();
            }
        })
    }

    // Start playing
    pipeline.set_state(gst::State::Playing).unwrap();

    // Wait until error or EOS
    let bus = pipeline.bus().unwrap();
    loop {
        for msg in bus.iter() {
            match msg.view() {
                gst::MessageView::Eos(..) => break,
                gst::MessageView::Error(err) => {
                    eprintln!("Error from {:?}: {}", err.src().map(|s| s.path_string()), err.error());
                    break;
                }
                gst::MessageView::StateChanged(s) => {
                    match s.current() {
                        gst::State::Playing => {
                            // chexk if state change pertains to whole pipeline
                            if s.src().map(|s| s.path_string()).unwrap() == "/GstPipeline:pipeline0" {
                                is_recording.store(true, std::sync::atomic::Ordering::Relaxed);
                            }
                        }
                        _ => {
                            // if the pipeline is not playing, set is_recording to false
                            if s.src().map(|s| s.path_string()).unwrap() == "/GstPipeline:pipeline0" {
                                is_recording.store(false, std::sync::atomic::Ordering::Relaxed);
                            }
                        }
                    }
                }
                _ => {
                    // check if recording is still required
                }
            }
        }
        if stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
    }

    // Send end-of-stream (EOS)
    pipeline.send_event(gst::event::Eos::new());

    // Wait for the pipeline to finish
    for msg in bus.iter_timed(gst::ClockTime::NONE) {
        match msg.view() {
            gst::MessageView::Eos(..) => break,
            gst::MessageView::Error(err) => {
                eprintln!("Error from {:?}: {}", err.src().map(|s| s.path_string()), err.error());
                break;
            }
            _ => {}
        }
    }

    // Set the pipeline to null state
    pipeline.set_state(gst::State::Null).unwrap();

    // Set is_recording to false
    is_recording.store(false, std::sync::atomic::Ordering::Relaxed);
}

#[pyclass(name = "Recorder")]
pub struct Recorder {
    is_recording: Arc<AtomicBool>,
    last_frame_time: Arc<AtomicF64>,
    last_frame_count: Arc<AtomicUsize>,
    stop_flag: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
#[pyclass(name = "CameraCaps")]
pub struct CameraCaps {
    pub width: i32,
    pub height: i32,
    pub framerate_numerator: i32,
    pub framerate_denominator: i32,
    pub format: String,
}

#[pymethods]
impl CameraCaps {
    #[new]
    fn __new__(width: i32, height: i32, framerate_numerator: i32, framerate_denominator: i32, format: String) -> Self {
        CameraCaps {
            width,
            height,
            framerate_numerator,
            framerate_denominator,
            format,
        }
    }
}

#[pymethods]
impl Recorder {
    #[new]
    #[pyo3(signature = (caps, filename, display = false, utp_port = None, rotate = 0, bitrate = None))]
    pub fn new(
        caps: CameraCaps,
        filename: String,
        display: bool,
        utp_port: Option<u16>,
        rotate: u32,
        bitrate: Option<u32>,
    ) -> PyResult<Self> {
        // run record in a new thread
        let stop_flag = Arc::new(AtomicBool::new(false));
        let is_recording = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();
        let is_recording_clone = is_recording.clone();
        let last_frame_time = Arc::new(AtomicF64::new(-1.0));
        let last_frame_time_clone = last_frame_time.clone();
        let last_frame_count = Arc::new(AtomicUsize::new(0));
        let last_frame_count_clone = last_frame_count.clone();

        let r = match rotate {
            0 => None,
            90 => Some(Rotation::Rotate90),
            180 => Some(Rotation::Rotate180),
            270 => Some(Rotation::Rotate270),
            _ => return Err(pyo3::exceptions::PyValueError::new_err("Invalid rotation value")),
        };

        std::thread::spawn(move || {
            let caps = caps.clone();
            record(
                &filename,
                &caps,
                is_recording_clone,
                stop_flag_clone,
                last_frame_time_clone,
                last_frame_count_clone,
                display,
                utp_port,
                r,
                bitrate,
            );
        });

        // wait for recording to start
        while !is_recording.load(std::sync::atomic::Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        Ok(Recorder {
            is_recording,
            stop_flag,
            last_frame_time,
            last_frame_count,
        })
    }

    pub fn stop(&self) {
        self.stop_flag.store(true, std::sync::atomic::Ordering::Relaxed);
        // Wait for the recording to stop
        while self.is_recording.load(std::sync::atomic::Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    pub fn last_frame_time(&self) -> f64 {
        self.last_frame_time.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn last_frame_count(&self) -> usize {
        self.last_frame_count.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn is_recording(&self) -> bool {
        self.is_recording.load(std::sync::atomic::Ordering::Relaxed)
    }

    // enable usgae as a context manager
    fn __enter__(slf: PyRef<Self>) -> PyResult<Py<Self>> {
        // return self
        Ok(slf.into())
    }

    pub fn __exit__(
        &self,
        _exc_type: Option<&pyo3::PyAny>,
        _exc_value: Option<&pyo3::PyAny>,
        _traceback: Option<&pyo3::PyAny>,
    ) -> PyResult<()> {
        self.stop();
        Ok(())
    }
}

/// A Python module implemented in Rust.
#[pymodule]
fn psydk_camera(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<Recorder>()?;
    m.add_class::<CameraCaps>()?;
    Ok(())
}
