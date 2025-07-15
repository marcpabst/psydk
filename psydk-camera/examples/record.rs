use webcam_py::prelude::*;

fn main() {
    let caps = CameraCaps {
        width: 1280,
        height: 720,
        framerate_numerator: 60,
        framerate_denominator: 1,
        format: "NV12".to_string(),
    };
    // 8500 kbps
    let bitrate = None; // 8500 kbps
    let recorder = Recorder::new(caps, "output.mkv".to_string(), false, 0, bitrate)
        .expect("Failed to create recorder");
    println!("Starting recorder...");
    // // keep printing the last frame time
    // while recorder.is_recording() {
    //     let last_frame_time = recorder.last_frame_time();
    //     println!("Last frame time: {}", last_frame_time);
    //     // std::thread::sleep(std::time::Duration::from_secs(1));
    // }
    std::thread::sleep(std::time::Duration::from_secs(60));
    println!("Stopping recorder...");
    recorder.stop();
}
