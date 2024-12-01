use crate::ChatMember;
use parking_lot::{Mutex, MutexGuard};
use std::collections::HashMap;
use std::io::Error;
use std::sync::Arc;

#[derive(Clone)]
pub struct BudgetChat {
    // All members of the chat room from session ID -> ChatMember storage
    pub chat_members: Arc<Mutex<HashMap<String, ChatMember>>>,
}

// Private Methods

impl BudgetChat {
    fn lock_chat(
        &mut self,
        current_session_id: &String,
    ) -> MutexGuard<'_, HashMap<String, ChatMember>> {
        println!(
            "{} - INFO - Locking chat. Is it already locked? {}",
            current_session_id,
            self.chat_members.is_locked()
        );
        self.chat_members.lock()
    }
}

// Public Methods

impl BudgetChat {
    pub fn new() -> Self {
        Self {
            chat_members: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn add_new_member(&mut self, chat_member: ChatMember, current_session_id: &String) {
        let mut chat_members = self.lock_chat(current_session_id);
        chat_members.insert(chat_member.owning_session_id.to_owned(), chat_member);
    }

    pub fn send_message_to_session(
        &mut self,
        current_session_id: &String,
        message: &String,
    ) -> Result<(), Error> {
        let mut chat_members = self.lock_chat(current_session_id);
        let chat_member = chat_members
            .get_mut(current_session_id)
            .expect("Could not find chat_member with given current_session_id");

        chat_member.send_message(message)
    }

    pub fn read_message_from_session(
        &mut self,
        current_session_id: &String,
    ) -> Result<String, Error> {
        let chat_member = {
            let mut chat_members = self.lock_chat(current_session_id);
            let chat_member = chat_members
                .get_mut(current_session_id)
                .expect("Could not find chat_member with given current_session_id");

            chat_member.try_clone()
        };

        if chat_member.is_err() {
            return Err(chat_member.err().unwrap());
        }

        chat_member.unwrap().read_message()
    }

    pub fn broadcast_message_to_chat(&mut self, current_session_id: &String, message: &String) {
        let mut chat_members = self.lock_chat(current_session_id);
        println!(
            "{} - INFO - Broadcasting message to all sessions except {}: {}",
            current_session_id, current_session_id, message
        );

        for (_, (session_id, other)) in chat_members.iter_mut().enumerate() {
            if current_session_id == session_id {
                // Skip broadcasting messages to current session
                continue;
            }

            if !other.registered {
                // If the other isn't registered we don't want to broadcast
                continue;
            }

            // Write the message to the chat_member's TcpStream
            let result = other.send_message(message);
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

    pub fn get_current_member_names(&mut self, current_session_id: &String) -> Vec<String> {
        let chat_members = self.lock_chat(current_session_id);
        chat_members
            .iter()
            .map(|(_session_id, member)| member.name.clone())
            .collect::<Vec<_>>()
    }

    pub fn remove_user_from_chat(&mut self, current_session_id: &String) {
        let member = {
            let mut chat_members = self.lock_chat(current_session_id);
            // Pop the member out of the list and explicitly drop it to terminate
            // the connection now.
            chat_members
                .remove(current_session_id)
                .expect("Couldn't find given member")
        };

        // Only broadcast messages to registered chat members
        if member.registered {
            self.broadcast_message_to_chat(
                &current_session_id,
                &format!("* {} has left the room", &member.name),
            );
        }
    }
}
