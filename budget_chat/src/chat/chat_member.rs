use std::io::{BufRead, BufReader, Error, Write};
use std::net::TcpStream;

pub struct ChatMember {
    // The name of the ChatMember
    pub name: String,

    // The Stream opened by the client
    pub source_stream: TcpStream,

    // The session that "owns" the ChatMember relationship
    pub owning_session_id: String,

    // Represents if the member is fully registered and can
    // receive broadcasted messages.
    pub registered: bool,
}

impl ChatMember {
    pub fn register_new_member(
        source_stream: TcpStream,
        owning_session_id: String,
    ) -> Result<Self, Error> {
        println!(
            "{} - INFO - Requesting name from client: {:?}",
            owning_session_id, source_stream
        );

        // Create the new ChatMember in an "unregistered" state.
        // If everything succeeds below we'll return it instead of an error.
        let mut new_member = Self {
            name: "UNREGISTERED".to_owned(),
            source_stream,
            owning_session_id: owning_session_id.clone(),
            registered: false,
        };

        // Send a constant string requesting the name of the new client
        let request_result =
            new_member.send_message(&"Welcome to budgetchat! What shall I call you?".to_owned());
        if request_result.is_err() {
            println!(
                "{} - ERROR - Failed to write name request",
                new_member.owning_session_id
            );
            return Err(request_result.err().unwrap());
        }

        let name_result = new_member.read_message();
        if name_result.is_err() {
            println!(
                "{} - ERROR - Terminating session due to error reading from client",
                new_member.owning_session_id
            );
            return Err(name_result.err().unwrap());
        }

        let name = name_result.unwrap();

        // Trim any newlines off of the string
        let name = name.trim().to_owned();

        // There's prob a better way to do this but w/e
        // Scan for any non-alphanumeric characters in the given name
        for c in name.chars() {
            if !c.is_alphanumeric() {
                println!(
                    "{} - ERROR - Received a name with non-alphanumeric characters: {}",
                    &owning_session_id, name
                );
                return Err(Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Received an empty message from client",
                ));
            }
        }

        println!(
            "{} - INFO - Received name from client: {}",
            new_member.owning_session_id, &name
        );

        // Set the name to what was received and registered to true.
        new_member.name = name;
        new_member.registered = true;

        Ok(new_member)
    }

    pub fn try_clone(&mut self) -> Result<ChatMember, Error> {
        let stream_clone = self.source_stream.try_clone();
        if stream_clone.is_err() {
            return Err(Error::new(
                std::io::ErrorKind::Other,
                "Could not clone the stream of the ChatMember",
            ));
        }

        Ok(Self {
            name: self.name.clone(),
            source_stream: stream_clone.unwrap(),
            owning_session_id: self.owning_session_id.clone(),
            registered: self.registered,
        })
    }

    pub fn read_message(&mut self) -> Result<String, Error> {
        let mut response_buffer = BufReader::new(&self.source_stream);
        let mut message = String::new();
        let read_result = response_buffer.read_line(&mut message);
        let message = message.trim().to_owned();

        if read_result.is_err() {
            let error = read_result.err().unwrap();
            println!(
                "{} - ERROR - Received an error reading message from {}: {:?}",
                self.owning_session_id, self.name, &error
            );
            return Err(error);
        }

        if message.is_empty() {
            println!(
                "{} - INFO - Received an empty message from {}",
                self.owning_session_id, self.name
            );
            return Err(Error::new(
                std::io::ErrorKind::InvalidData,
                "Received an empty message from client",
            ));
        }

        Ok(message)
    }

    pub fn send_message(&mut self, message: &String) -> Result<(), Error> {
        println!(
            "{} - INFO - Sending message to {}: {}",
            self.owning_session_id, self.name, message
        );

        self.source_stream
            .write_all(format!("{}\n", message).as_bytes())
    }
}
