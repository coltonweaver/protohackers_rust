use std::{
    env,
    io::{BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    thread,
};
use uuid::Uuid;

#[derive(Debug)]
enum Request {
    Invalid,
    Insert(InsertRequest),
    Query(QueryRequest),
}

#[derive(Debug)]
struct InsertRequest {
    timestamp: i32,
    price: i32,
}

#[derive(Debug)]
struct QueryRequest {
    mintime: i32,
    maxtime: i32,
}

#[derive(Debug)]
struct Transaction {
    timestamp: i32,
    price: i32,
}

#[derive(Debug)]
struct SessionState {
    session_id: String,
    client_transactions: Vec<Transaction>,
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
    // Track the client transactions and randomly generated session ID
    let mut session_state = SessionState {
        session_id: Uuid::new_v4().to_string(),
        client_transactions: Vec::new(),
    };

    println!("{} - INFO - New session created", session_state.session_id);

    loop {
        let mut read_buffer = [0u8; 9];
        if stream.read_exact(&mut read_buffer).is_err() {
            println!(
                "{} - INFO - Session terminated by client",
                session_state.session_id
            );
            break;
        }

        match parse_request(read_buffer) {
            Request::Insert(insert_request) => {
                handle_insert(insert_request, &mut session_state);
            }
            Request::Query(query_request) => {
                let result = handle_query(query_request, &session_state);
                respond_success(&stream, &session_state, result);
            }
            Request::Invalid => {
                respond_failure(&stream, &session_state);
                // Break so we terminate the connection
                break;
            }
        }
    }

    println!(
        "{} - INFO - Terminating session...",
        session_state.session_id
    );
}

// Request Parsing

fn parse_request(raw_bytes: [u8; 9]) -> Request {
    // Use a BufReader to read specific sets of bytes from the raw_bytes
    let mut request_buf = BufReader::new(&raw_bytes[..]);

    // We'll grab the first byte to convert into a character
    let mut op_code_bytes = [0u8; 1];
    if request_buf.read_exact(&mut op_code_bytes).is_err() {
        return Request::Invalid;
    }

    // Convert the op_code_byte (first and only element) of op_code_bytes to a char
    let op_code = op_code_bytes[0] as char;

    // Handle the op code appropriately
    if op_code == 'I' {
        return parse_insert_request(request_buf);
    } else if op_code == 'Q' {
        return parse_query_request(request_buf);
    } else {
        Request::Invalid
    }
}

fn parse_insert_request(mut request_buf: BufReader<&[u8]>) -> Request {
    let mut timestamp_bytes = [0u8; 4];
    if request_buf.read_exact(&mut timestamp_bytes).is_err() {
        return Request::Invalid;
    }

    let mut price_bytes = [0u8; 4];
    if request_buf.read_exact(&mut price_bytes).is_err() {
        return Request::Invalid;
    }

    let timestamp = i32::from_be_bytes(timestamp_bytes);
    let price = i32::from_be_bytes(price_bytes);

    Request::Insert(InsertRequest { timestamp, price })
}

fn parse_query_request(mut request_buf: BufReader<&[u8]>) -> Request {
    let mut mintime_bytes = [0u8; 4];
    if request_buf.read_exact(&mut mintime_bytes).is_err() {
        return Request::Invalid;
    }

    let mut maxtime_bytes = [0u8; 4];
    if request_buf.read_exact(&mut maxtime_bytes).is_err() {
        return Request::Invalid;
    }

    let mintime = i32::from_be_bytes(mintime_bytes);
    let maxtime = i32::from_be_bytes(maxtime_bytes);

    Request::Query(QueryRequest { mintime, maxtime })
}

// Request Handlers

fn handle_insert(insert_request: InsertRequest, session_state: &mut SessionState) {
    println!(
        "{} - INFO - Handling insert request: {:?}",
        session_state.session_id, insert_request
    );

    // Just append the transaction to the ClientTransactions
    session_state.client_transactions.push(Transaction {
        timestamp: insert_request.timestamp,
        price: insert_request.price,
    });
}

fn handle_query(query_request: QueryRequest, session_state: &SessionState) -> [u8; 4] {
    println!(
        "{} - INFO - Handling query request: {:?}",
        session_state.session_id, query_request
    );

    let mut total: i64 = 0;
    let mut txn_count: i64 = 0;
    for i in 0..session_state.client_transactions.len() {
        let txn = &session_state.client_transactions[i];
        if txn.timestamp >= query_request.mintime && txn.timestamp <= query_request.maxtime {
            total += txn.price as i64;
            txn_count += 1;
        }
    }

    if txn_count == 0 {
        println!(
            "{} - INFO - Found zero txns, returning zero...",
            session_state.session_id
        );
        return 0_i32.to_be_bytes();
    }

    let bytes = (total / txn_count).to_be_bytes();
    [bytes[4], bytes[5], bytes[6], bytes[7]]
}

// TcpStream Utils

fn respond_success(mut stream: &TcpStream, session_state: &SessionState, response: [u8; 4]) {
    println!(
        "{} - INFO - Responding to session client with {:?}",
        session_state.session_id, response
    );
    stream.write_all(&response).unwrap();
}

fn respond_failure(mut stream: &TcpStream, session_state: &SessionState) {
    println!(
        "{} - INFO - Responding with failure...",
        session_state.session_id
    );
    stream.write_all("\n".as_bytes()).unwrap();
}
