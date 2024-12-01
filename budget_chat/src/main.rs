mod chat;
use chat::{budget_chat::BudgetChat, chat_member::ChatMember};

use std::{
    env,
    net::{TcpListener, TcpStream},
    thread,
};
use uuid::Uuid;

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
    // BudgetChat encapsulates an Arc + Mutex that powers handling multiple
    // connections on different threads
    let budget_chat = BudgetChat::new();

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

fn handle_connection(stream: TcpStream, mut budget_chat: BudgetChat) {
    // Define a unique session ID for logging and identification purposes
    let session_id = Uuid::new_v4().to_string();

    println!("{} - INFO - Opened a new session", session_id);

    // Handle registration for the new member
    let register_result = ChatMember::register_new_member(stream, session_id.clone());
    if register_result.is_err() {
        println!(
            "{} - ERROR - Failed to register new member with {:?}",
            session_id,
            register_result.err()
        );
        return;
    }

    // Unwrap the chat_member and save a reference to the name
    let chat_member = register_result.unwrap();
    let user_name = chat_member.name.clone();

    // Add the new member to the budget chat
    println!(
        "{} - INFO - Adding newly registered member to chat data structure...",
        session_id
    );
    budget_chat.add_new_member(chat_member, &session_id);
    println!(
        "{} - INFO - Added newly registered member to chat data structure!",
        session_id
    );

    // Send current membership to the new user
    let member_names = budget_chat.get_current_member_names(&session_id);
    let result = budget_chat.send_message_to_session(
        &session_id,
        &room_membership_message_builder(&user_name, member_names),
    );

    // If the result was already an error, we should terminate this connection
    if result.is_err() {
        // We can remove the last entry in the vector since we just pushed it and still hold the lock. This
        // way we don't try to broadcast a message to this client going forward as well
        budget_chat.remove_user_from_chat(&session_id);

        // Terminate the connection!
        return;
    }

    // Broadcast the new user to the other chat members. We specifically do this after confirming that we sent
    // the room membership to this user.
    budget_chat.broadcast_message_to_chat(&session_id, &user_joined_message_builder(&user_name));

    // Now we can listen for messages from the client and broadcast them to other users
    loop {
        // Read until newline
        let message_result = budget_chat.read_message_from_session(&session_id);

        if message_result.is_err() {
            budget_chat.remove_user_from_chat(&session_id);
            break;
        }

        let message = message_result.unwrap();

        // Now broadcast this message to the rest of the clients
        budget_chat.broadcast_message_to_chat(
            &session_id,
            &user_chat_message_builder(&user_name, message.to_string()),
        );
    }

    println!(
        "{} - INFO - Terminating connection with chat member {}",
        session_id, user_name
    );
}

// Message Builder Utilities

fn user_joined_message_builder(name: &String) -> String {
    format!("* {} has entered the room", name)
}

fn room_membership_message_builder(
    current_user_name: &String,
    chat_member_names: Vec<String>,
) -> String {
    // Get current members, filter out the current user, and
    let names = chat_member_names
        .iter()
        .filter(|name| *name != current_user_name)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");

    format!("* The room contains: {}", names)
}

fn user_chat_message_builder(current_user_name: &String, message: String) -> String {
    format!("[{}] {}", current_user_name, message)
}
