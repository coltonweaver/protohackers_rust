use std::{
    env,
    io::{self, BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
};
use uuid::Uuid;

// Define constant request strings to use
const REQUEST_NAME_STRING: &'static str = "Welcome to budgetchat! What shall I call you?\n";

struct ChatMember {
    name: String,
    source_stream: TcpStream,
    active: bool,
}

impl ChatMember {
    fn try_clone(&self) -> Self {
        let cloned_name = self.name.clone();
        let cloned_stream = self.source_stream.try_clone().unwrap();

        Self {
            name: cloned_name,
            source_stream: cloned_stream,
            active: self.active,
        }
    }
}

struct BudgetChat {
    // We need to use a vector of Arcs so that we know when it's
    // no longer referenced
    chat_members: Vec<ChatMember>,
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
    // Create an Arc with an inner Mutex around the BudgetChat data structure that powers the problem.
    let budget_chat: Arc<Mutex<BudgetChat>> = Arc::new(Mutex::new(BudgetChat {
        chat_members: Vec::new(),
    }));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                // Clone the budget_chat so that we can pass ownership of the clone to handle_connection
                let clone_of_budget_chat = budget_chat.clone();

                // Move ownership of the TcpStream and the budget_chat clone into handle_connection on another thread
                thread::spawn(move || handle_connection(stream, clone_of_budget_chat));
            }
            Err(err) => panic!("Failed while listening for incoming connections: {}", err),
        }
    }
}

fn handle_connection(mut new_stream: TcpStream, budget_chat: Arc<Mutex<BudgetChat>>) {
    // Define a unique session ID for logging purposes
    let session_id = Uuid::new_v4().to_string();

    println!("{} - INFO - Opened a new session", session_id);

    // Request the name from the new connection. We won't register the new stream until
    // we've received a valid response so other clients won't be aware of a bad client.
    let name = request_name(&mut new_stream, &session_id);
    if name.is_none() {
        return;
    }

    let name = name.unwrap();

    println!(
        "{} - INFO - Received name from client: {}",
        session_id, &name
    );

    // Create a ChatMember with the new name and move ownership of the TcpStream into it.
    let mut chat_member = ChatMember {
        name,
        source_stream: new_stream,
        active: true,
    };

    // Now register the newly created ChatMember with the other members in BudgetChat
    budget_chat
        .lock()
        .unwrap()
        .chat_members
        .push(chat_member.try_clone());

    // Broadcast the new user to the other chat members
    broadcast_message(
        user_joined_message_builder(&chat_member.name),
        &chat_member.name,
        budget_chat.clone(),
        &session_id,
    );

    // Write the room membership to the person who joined
    if send_message_to_member(
        &room_membership_message_builder(
            &chat_member.name,
            &budget_chat.lock().unwrap().chat_members,
        ),
        &mut chat_member,
        &session_id,
    )
    .is_err()
    {
        // Set the chat member to inactive so they won't receive messages
        chat_member.active = false;
        return; // Terminate the connection
    }

    // Responses are all individual strings terminated with a newline, so we'll use
    // a BufReader to access the `read_line` method.
    let mut response_buffer = BufReader::new(chat_member.source_stream);

    // Now we can listen for messages from the client and broadcast them to other users
    loop {
        let mut message = String::new();
        let read_result = response_buffer.read_line(&mut message);
        if read_result.is_err() {
            println!(
                "{} - ERROR - Received an error reading message from {}: {:?}",
                session_id,
                chat_member.name,
                read_result.err()
            );
            chat_member.active = false;
            return;
        }

        // Messages need to have some characters
        if message.is_empty() {
            continue;
        }

        // Now broadcast this message to the rest of the clients
        broadcast_message(
            user_chat_message_builder(&chat_member.name, message),
            &chat_member.name,
            budget_chat.clone(),
            &session_id,
        );
    }
}

fn request_name(stream: &mut TcpStream, session_id: &String) -> Option<String> {
    println!(
        "{} - INFO - Requesting name for newly connected session",
        session_id
    );

    // Send a constant string requesting the name of the new client
    if stream.write(REQUEST_NAME_STRING.as_bytes()).is_err() {
        println!("{} - ERROR - Failed to write name...", session_id);
        return None;
    }

    println!("{} - INFO - Requested name, awaiting response.", session_id);

    // Responses are all individual strings terminated with a newline, so we'll use
    // a BufReader to access the `read_line` method.
    let mut response_buffer = BufReader::new(stream);

    let mut name = String::new();
    if response_buffer.read_line(&mut name).is_err() {
        println!(
            "{} - ERROR - Received an error reading name response...",
            session_id
        );
        return None;
    }

    if name.is_empty() {
        println!(
            "{} - ERROR - Received an empty name from the client...",
            session_id
        );
        return None;
    }

    Some(name.replace("\n", ""))
}

fn broadcast_message(
    msg: String,
    current_user_name: &String,
    budget_chat: Arc<Mutex<BudgetChat>>,
    session_id: &String,
) {
    println!(
        "{} - INFO - Broadcasting message to all clients except {}: {}",
        session_id,
        current_user_name,
        msg.replace("\n", "")
    );

    // We'll use a scope to define a critical section for the app
    {
        let mut lock = budget_chat.lock().unwrap();
        let chat_members = &mut lock.chat_members;

        for i in 0..chat_members.len() {
            let other = &mut chat_members[i];
            if other.name == *current_user_name || !other.active {
                // Skip broadcasting messages to inactive or current user
                continue;
            }

            // Send a message to the targeted user
            if send_message_to_member(&msg, other, session_id).is_err() {
                // But if we error out sending the message to that user we should remove it from the list.
                other.active = false;
            }
        }
    }
}

fn send_message_to_member(
    message: &String,
    chat_member: &mut ChatMember,
    session_id: &String,
) -> io::Result<usize> {
    println!(
        "{} - INFO - Sending message to {}: {}",
        session_id,
        chat_member.name,
        message.replace("\n", "")
    );

    // Write the message to the chat_member's TcpStream, returning the `Result<usize>`
    chat_member
        .source_stream
        .write(format!("{}\n", message).as_bytes())
}

fn user_joined_message_builder(name: &String) -> String {
    format!("* {} has entered the room", name)
}

fn room_membership_message_builder(
    current_user_name: &String,
    chat_members: &Vec<ChatMember>,
) -> String {
    let names = chat_members
        .iter()
        .map(|member| member.name.clone())
        .filter(|name| name != current_user_name)
        .collect::<Vec<_>>()
        .join(", ");

    format!("* The room contains: {}", names)
}

fn user_chat_message_builder(current_user_name: &String, message: String) -> String {
    format!("[{}] {}", current_user_name, message)
}
