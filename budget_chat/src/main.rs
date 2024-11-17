use parking_lot::{Mutex, MutexGuard};
use std::{
    env,
    io::{BufRead, BufReader, Error, Write},
    net::{TcpListener, TcpStream},
    sync::Arc,
    thread,
};
use uuid::Uuid;

struct ChatMember {
    name: String,
    source_stream: TcpStream,
}

struct BudgetChat {
    chat_members: Vec<ChatMember>,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let ipv4_address = args[1].clone();
    let port = args[2].clone();
    let addr = format!("{}:{}", ipv4_address, port);

    println!("INFO - Listening for incoming connections at {}", addr);

    let listener = TcpListener::bind(&addr).unwrap();
    serve(listener);
}

fn serve(listener: TcpListener) {
    // Create an Arc with an inner Mutex around the BudgetChat data structure that powers the problem.
    let budget_chat: Arc<Mutex<BudgetChat>> = Arc::new(Mutex::new(BudgetChat {
        chat_members: Vec::new(),
    }));

    // We'll track the threads we've spawned here.
    let mut thread_handles = Vec::new();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                // Clone the budget_chat so that we can pass ownership of the clone to handle_connection
                let clone_of_budget_chat = budget_chat.clone();

                // Move ownership of the TcpStream and the budget_chat clone into handle_connection on another thread
                let handle = thread::spawn(move || handle_connection(stream, clone_of_budget_chat));

                // Track the thread for later as needed.
                thread_handles.push(handle);
            }
            Err(err) => println!(
                "ERROR - Failure while listening to incoming connections: {}",
                err
            ),
        }
    }

    // Let's wait for the handles to terminate before leaving the scope
    for handle in thread_handles {
        let _ = handle.join();
    }

    println!("INFO - Server terminating...");
}

fn handle_connection(mut stream: TcpStream, budget_chat: Arc<Mutex<BudgetChat>>) {
    // Define a unique session ID for logging purposes
    let session_id = Uuid::new_v4().to_string();

    println!("{} - INFO - Opened a new session", session_id);

    let chat_member = register_new_chat_member(&mut stream, &session_id);

    if chat_member.is_none() {
        return;
    }

    let chat_member = chat_member.unwrap();
    let user_name = chat_member.name.clone();

    // Create a scope that represents new user registration. We'll take a lock
    // on the budget_chat so that we can register the new chat_member and broadcast
    // all necessary messages.
    {
        // Lock the budget_chat so we can register the new ChatMember and broadcast messages
        let mut budget_chat = budget_chat.lock();

        // Now register the newly created ChatMember with the other members in BudgetChat
        budget_chat.chat_members.push(chat_member);

        // Write the room membership to the person who joined
        let result = send_message_to_current_user(
            &room_membership_message_builder(&user_name, &budget_chat.chat_members),
            &user_name,
            &mut budget_chat,
            &session_id,
        );

        // If the result was already an error, we should terminate this connection
        if result.is_err() {
            // We can remove the last entry in the vector since we just pushed it and still hold the lock. This
            // way we don't try to broadcast a message to this client going forward as well
            let last_idx = budget_chat.chat_members.len() - 1;
            budget_chat.chat_members.remove(last_idx);

            // Terminate the connection!
            return;
        }

        // Broadcast the new user to the other chat members. We specifically do this after confirming that we sent
        // the room membership to this user.
        broadcast_message_to_other_users(
            &user_joined_message_builder(&user_name),
            &user_name,
            &mut budget_chat,
            &session_id,
        );
    };

    // Let's create a single BufReader to re-use across iterations
    let mut response_buffer = BufReader::new(&stream);

    // Now we can listen for messages from the client and broadcast them to other users
    loop {
        // Read until newline
        let mut message = String::new();
        let read_result = response_buffer.read_line(&mut message);
        let message = message.trim();

        let mut budget_chat = budget_chat.lock();

        // If there was an error reading the message, terminate the connection and remove the
        // current user from the list.
        if read_result.is_err() {
            println!(
                "{} - ERROR - Received an error reading message from {}: {:?}",
                session_id,
                user_name,
                read_result.err()
            );

            // Remove user and terminate connection
            remove_user_from_chat(&user_name, &mut budget_chat, &session_id);
            break;
        }

        if message.is_empty() {
            println!(
                "{} - INFO - Received an empty message from {}, terminating chat for them...",
                session_id, user_name
            );

            remove_user_from_chat(&user_name, &mut budget_chat, &session_id);
            break;
        }

        // If either the chat room is empty no need to try and broadcast
        if budget_chat.chat_members.is_empty() {
            continue;
        }

        // Now broadcast this message to the rest of the clients
        broadcast_message_to_other_users(
            &user_chat_message_builder(&user_name, message.to_string()),
            &user_name,
            &mut budget_chat,
            &session_id,
        );
    }

    println!(
        "{} - INFO - Terminating connection with chat member {}",
        session_id, user_name
    );
}

// User Registration Utilities

fn register_new_chat_member(stream: &mut TcpStream, session_id: &String) -> Option<ChatMember> {
    // Request the name from the new connection. We won't register the new stream until
    // we've received a valid response so other clients won't be aware of a bad client.
    let name = match request_name(stream, &session_id) {
        Some(name) => name,
        None => {
            println!(
                "{} - WARN - Terminating session due to invalid name",
                session_id
            );
            return None;
        }
    };

    println!(
        "{} - INFO - Received name from client: {}",
        session_id, &name
    );

    // Create a ChatMember struct with the new name and move ownership of the TcpStream into it.
    let chat_member = ChatMember {
        name: name.clone(),
        source_stream: stream.try_clone().unwrap(),
    };

    Some(chat_member)
}

fn request_name(stream: &mut TcpStream, session_id: &String) -> Option<String> {
    println!(
        "{} - INFO - Requesting name for newly connected session {:?}",
        session_id, stream
    );

    // Send a constant string requesting the name of the new client
    let request_result =
        stream.write_all("Welcome to budgetchat! What shall I call you?\n".as_bytes());
    if request_result.is_err() {
        println!(
            "{} - ERROR - Failed to write name with error: {:?}",
            session_id,
            request_result.err()
        );
        return None;
    }

    // Responses are all individual strings terminated with a newline, so we'll use
    // a BufReader to access the `read_line` method.
    let mut response_buffer = BufReader::new(stream);
    let mut name = String::new();
    let result = response_buffer.read_line(&mut name);
    if result.is_err() {
        println!(
            "{} - ERROR - Received an error reading name response: {:?}",
            session_id,
            result.err()
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

    // Trim any newlines off of the string
    let name = name.trim();

    // There's prob a better way to do this but w/e
    // Scan for any non-alphanumeric characters in the given name
    for c in name.chars() {
        if !c.is_alphanumeric() {
            println!(
                "{} - ERROR - Received a name with non-alphanumeric characters: {}",
                session_id, name
            );
            return None;
        }
    }

    Some(name.to_string())
}

// Message Sending Utilities

fn broadcast_message_to_other_users(
    message: &String,
    current_user_name: &String,
    budget_chat: &mut MutexGuard<'_, BudgetChat>,
    session_id: &String,
) {
    println!(
        "{} - INFO - Broadcasting message to all clients except {}: {}",
        session_id, current_user_name, message
    );

    // We'll use a scope to define a critical section for the app
    {
        let chat_members = &mut budget_chat.chat_members;

        for (_, other) in chat_members.iter_mut().enumerate() {
            if other.name == *current_user_name {
                // Skip broadcasting messages to current user
                continue;
            }

            // Write the message to the chat_member's TcpStream, returning the `Result<usize>`
            let result = other
                .source_stream
                .write_all(format!("{}\n", message).as_bytes());

            if result.is_err() {
                println!(
                    "{} - ERROR - Failed to broadcast message to {}: {:?}",
                    session_id,
                    other.name,
                    result.err()
                );
            }
        }
    }
}

fn send_message_to_current_user(
    message: &String,
    name: &String,
    budget_chat: &mut MutexGuard<'_, BudgetChat>,
    session_id: &String,
) -> Result<(), Error> {
    println!(
        "{} - INFO - Sending message to {}: {}",
        session_id, name, message
    );

    // Find a mutable reference to the current chat member.
    if let Some(chat_member) = budget_chat
        .chat_members
        .iter_mut()
        .find(|member| member.name == *name)
    {
        // Write the message to the chat_member's TcpStream, returning the `Result`
        let result = chat_member
            .source_stream
            .write_all(format!("{}\n", message).as_bytes());

        result
    } else {
        panic!("Could not find current user!");
    }
}

// Membership Removal Utility

fn remove_user_from_chat(
    name: &String,
    budget_chat: &mut MutexGuard<'_, BudgetChat>,
    session_id: &String,
) {
    if let Some(idx) = budget_chat
        .chat_members
        .iter()
        .position(|member| member.name == *name)
    {
        // Pop the member out of the list and explicitly drop it to terminate
        // the connection now.
        let member = budget_chat.chat_members.remove(idx);
        drop(member);

        // Let the other users know the user has left
        broadcast_message_to_other_users(
            &format!("* {} has left the room", name),
            name,
            budget_chat,
            session_id,
        );
    } else {
        panic!("{} - ERROR - Can't find given member!", session_id);
    }
}

// Message Builder Utilities

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
