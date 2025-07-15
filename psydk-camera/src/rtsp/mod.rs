pub mod rtsp_msg_handler;
pub mod rtsp_session;

use rtsp_msg_handler::{RtspCommand, RtspMessage, RtspParsable, RtspResponse};
pub use rtsp_session::{ClientPorts, RtspSession};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::net::{TcpListener, TcpStream};
use std::str;
use std::thread;

fn respond_to_client(req: RtspMessage, stream: &TcpStream, session: Option<RtspSession>) {
    match req.response(session) {
        Some(resp) => {
            println!("Response {:?}\n", resp);
            let mut writer = BufWriter::new(stream);
            match writer.write_all(resp.as_bytes()) {
                Ok(_) => (),
                Err(e) => (println!("Error writing bytes: {}", e)),
            }
        }
        None => {
            println!("No response found!");
        }
    }
}

fn handle_client(stream: TcpStream, start_stream_callback: impl Fn(RtspSession, String) + Send + Sync + 'static) {
    let client_ip = stream.peer_addr().unwrap().ip().to_string();
    println!("Client connected: {}", client_ip.to_owned());
    let mut reader = BufReader::new(&stream);
    let mut tcp_msg_buf = String::new();
    let mut session: Option<RtspSession> = None;

    loop {
        match reader.read_line(&mut tcp_msg_buf) {
            Ok(size) => {
                if size <= 0 {
                    break;
                }
                if tcp_msg_buf.contains("\r\n\r\n") {
                    let _string = str::from_utf8(&tcp_msg_buf.as_bytes()).unwrap();
                    println!("Request {:?}", _string);

                    match RtspMessage::parse_as_rtsp(tcp_msg_buf.to_owned()) {
                        Some(req) => match req.command {
                            Some(RtspCommand::Setup) => {
                                session = Some(RtspSession::setup(req.clone()));
                                respond_to_client(req.clone(), &stream, session.clone());
                            }
                            Some(RtspCommand::Play) => match session.clone() {
                                Some(sess) => {
                                    let c_ip = client_ip.clone();
                                    start_stream_callback(sess, c_ip);
                                    respond_to_client(req.clone(), &stream, session.clone());
                                    println!("Playing!");
                                }
                                None => {
                                    println!("No Session Found!");
                                    break;
                                }
                            },
                            Some(_) => {
                                respond_to_client(req.clone(), &stream, session.clone());
                            }
                            None => {
                                println!("Could not determine the Rtsp Command!");
                                break;
                            }
                        },
                        None => {
                            println!("Could not parse RtspMessage!");
                            break;
                        }
                    }
                    tcp_msg_buf.clear();
                }
            }
            Err(e) => {
                println!("Error reading TcpStream: {:?}", e);
                break;
            }
        }
    }
    println!("Client handled");
}

pub fn run_rtsp_server(port: u16, start_stream_callback: impl Fn(RtspSession, String) + Clone + Send + Sync + 'static) {
    let bind_str = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&bind_str).unwrap();
    println!("RTSP Server listening on port {}", &bind_str);

    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let start_stream_callback_clone = start_stream_callback.clone();
                    thread::spawn(move || handle_client(stream, start_stream_callback_clone));
                }
                Err(e) => {
                    println!("Error: {}", e);
                }
            }
        }
    });
}
