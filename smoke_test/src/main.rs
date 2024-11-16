use std::{
    env,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread,
};

fn main() {
    let args: Vec<String> = env::args().collect();
    let ipv4_address = args[1].clone();
    let port = args[2].clone();
    let addr = format!("{}:{}", ipv4_address, port);

    let listener = TcpListener::bind(&addr).unwrap();
    println!("Listening for connections on {}...", addr);

    serve(listener);
}

fn serve(listener: TcpListener) {
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(|| handle_connection(stream));
            }
            Err(err) => panic!("Failed to connect with error {}", err),
        }
    }
}

fn handle_connection(mut stream: TcpStream) {
    println!("Handling incoming client connection...");

    loop {
        let mut read_buffer = [0; 1024];
        match stream.read(&mut read_buffer) {
            Ok(bytes_read) => {
                // Client has disconnected...
                if bytes_read == 0 {
                    break;
                }

                // Write back exactly what was read from the stream
                stream.write_all(&read_buffer[..bytes_read]).unwrap();
            }
            Err(error) => {
                panic!(
                    "Received unexpeceted error while reading from client stream: {}",
                    error
                );
            }
        }
    }

    println!("Client has disconnected...");
}
