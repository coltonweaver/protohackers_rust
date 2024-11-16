use core::panic;
use serde::{Deserialize, Serialize};
use std::{
    env,
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    thread,
};

#[derive(Serialize, Deserialize, Debug)]
struct Request {
    method: String,
    number: f64,
}

#[derive(Serialize, Deserialize, Debug)]
struct Response {
    method: String,
    prime: bool,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let ipv4_address = args[1].clone();
    let port = args[2].clone();
    let addr = format!("{}:{}", ipv4_address, port);

    let listener = TcpListener::bind(&addr).unwrap();
    serve(listener);
}

fn serve(listener: TcpListener) {
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(|| handle_connection(stream));
            }
            Err(err) => panic!("Failed while listening for incoming connections: {}", err),
        }
    }
}

fn handle_connection(mut stream: TcpStream) {
    // Use a BufReader to enable reading until newlines
    let mut reader = BufReader::new(stream.try_clone().unwrap());

    loop {
        let mut read_buffer = String::new();
        reader.read_line(&mut read_buffer).unwrap();

        match parse_request(read_buffer.as_str()) {
            Ok(request) => {
                let response = handle_request(request);
                respond_success(&mut stream, response);
            }
            Err(_) => {
                respond_failure(&mut stream);
                // Break so we terminate the connection
                break;
            }
        }
    }
}

// Request Handling

fn parse_request(request_data: &str) -> Result<Request, serde_json::Error> {
    serde_json::from_str(request_data)
}

fn handle_request(request: Request) -> Response {
    Response {
        method: request.method.clone(),
        prime: is_prime(request.number as u64),
    }
}

// Send Response

fn respond_success(mut stream: &TcpStream, response: Response) {
    stream
        .write_all(format!("{}\n", serde_json::to_string(&response).unwrap()).as_bytes())
        .unwrap();
}

fn respond_failure(mut stream: &TcpStream) {
    // Write back a malformed response
    stream.write_all("\n".as_bytes()).unwrap();
}

// Helpers

fn is_prime(num: u64) -> bool {
    // Numbers less than or equal to 1 are not prime
    if num <= 1 {
        return false;
    }

    // Check if num is divisible by any number from 2 to the square root of num
    for i in 2..=(num as f64).sqrt() as u64 {
        if num % i == 0 {
            return false;
        }
    }

    // If num is not divisible by any number other than 1 and itself, it's prime
    true
}
