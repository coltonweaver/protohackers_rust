use std::{
    collections::HashMap,
    env,
    io::Error,
    net::{SocketAddr, UdpSocket},
};

// We'll define a constant server version and handle that key explicitly.
const SERVER_VERSION: &'static str = "version=cbw's Key-Value Store 1.0";

fn main() -> Result<(), Error> {
    let args: Vec<String> = env::args().collect();
    let ipv4_address = args[1].clone();
    let port = args[2].clone();
    let addr = format!("{}:{}", ipv4_address, port);

    let udp_socket = UdpSocket::bind(addr).expect("Could not bind to given address.");
    serve(udp_socket);

    Ok(())
}

fn serve(udp_socket: UdpSocket) {
    // We'll use a simple hashmap as the kv store for our server
    let mut kv_store: HashMap<String, String> = HashMap::new();

    loop {
        let read_result = read_request_from_socket(&udp_socket)
            .expect("Failed to read request from the UdpSocket");
        let request = read_result.0;
        let source = read_result.1;

        println!("Received request {}", request);

        if request.contains("=") {
            handle_insert(&mut kv_store, request);
        } else if request == "version" {
            send_message_to_source(&udp_socket, SERVER_VERSION.to_string(), &source);
        } else {
            let result = handle_query(&mut kv_store, request);
            send_message_to_source(&udp_socket, result, &source);
        }
    }
}

// UDP Socket Utilities

fn read_request_from_socket(udp_socket: &UdpSocket) -> Result<(String, SocketAddr), Error> {
    let mut buf = [0u8; 1000];
    let (amt, src) = udp_socket
        .recv_from(&mut buf)
        .expect("Failed to read data from socket");

    Ok((String::from_utf8_lossy(&buf[..amt]).into_owned(), src))
}

fn send_message_to_source(udp_socket: &UdpSocket, message: String, source: &SocketAddr) {
    udp_socket
        .send_to(message.as_bytes(), source)
        .expect("Could not send server version to source.");
}

// Request Handling Utilities

fn handle_insert(kv_store: &mut HashMap<String, String>, request: String) {
    let (key, value) = request.split_once("=").expect("Could not split on =");
    kv_store.insert(key.to_string(), value.to_string());
}

fn handle_query(kv_store: &mut HashMap<String, String>, request: String) -> String {
    match kv_store.get(&request) {
        Some(value) => return format!("{}={}", request, value),
        None => return "".to_string(),
    }
}
