use std::{
    os::unix::net::UnixStream,
    io::{self, Read},
};
use calloop::{
    EventSource,
    Poll,
    PostAction,
    Readiness,
    Token,
    TokenFactory,
};
use log::{debug, error, warn};

pub type ClientId = u32;

#[derive(Debug)]
pub enum ClientEvent {
    MessageReceived { client_id: ClientId, data: Vec<u8> },
    Disconnected { client_id: ClientId },
}

pub struct ClientConnection {
    stream: UnixStream,
    client_id: ClientId,
}

impl ClientConnection {
    pub(crate) fn new(stream: UnixStream, client_id: ClientId) -> io::Result<Self> {
        // Set the stream to non-blocking mode
        stream.set_nonblocking(true)?;
        
        debug!("Created client connection with ID: {}", client_id);
        
        Ok(Self {
            stream,
            client_id,
        })
    }

    pub fn client_id(&self) -> ClientId {
        self.client_id
    }

    pub fn stream(&self) -> &UnixStream {
        &self.stream
    }

    pub fn stream_mut(&mut self) -> &mut UnixStream {
        &mut self.stream
    }

    fn read_data<F>(&mut self, mut callback: F) -> io::Result<()>
    where
        F: FnMut(ClientEvent, &mut ()),
    {
        let mut buffer = [0u8; 4096];
        
        loop {
            match self.stream.read(&mut buffer) {
                Ok(0) => {
                    // Client disconnected
                    debug!("Client {} disconnected", self.client_id);
                    callback(ClientEvent::Disconnected { 
                        client_id: self.client_id 
                    }, &mut ());
                    return Ok(());
                }
                Ok(bytes_read) => {
                    debug!("Received {} bytes from client {}", bytes_read, self.client_id);
                    let data = buffer[..bytes_read].to_vec();
                    callback(ClientEvent::MessageReceived { 
                        client_id: self.client_id, 
                        data 
                    }, &mut ());
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // No more data to read
                    break;
                }
                Err(e) => {
                    error!("Error reading from client {}: {}", self.client_id, e);
                    return Err(e);
                }
            }
        }
        
        Ok(())
    }
}

impl EventSource for ClientConnection {
    type Event = ClientEvent;
    type Metadata = ();
    type Ret = ();
    type Error = io::Error;

    fn process_events<F>(
        &mut self,
        readiness: Readiness,
        token: Token,
        mut callback: F,
    ) -> Result<PostAction, Self::Error>
    where
        F: FnMut(Self::Event, &mut Self::Metadata) -> Self::Ret,
    {
        if readiness.readable {
            self.read_data(callback)?;
        }
        
        Ok(PostAction::Continue)
    }

    fn register(
        &mut self,
        poll: &mut Poll,
        token_factory: &mut TokenFactory,
    ) -> calloop::Result<()> {
        poll.register(&self.stream, calloop::Interest::READ, calloop::Mode::Level, token_factory.token())?;
        Ok(())
    }

    fn reregister(
        &mut self,
        poll: &mut Poll,
        token_factory: &mut TokenFactory,
    ) -> calloop::Result<()> {
        poll.reregister(&self.stream, calloop::Interest::READ, calloop::Mode::Level, token_factory.token())?;
        Ok(())
    }

    fn unregister(&mut self, poll: &mut Poll) -> calloop::Result<()> {
        poll.unregister(&self.stream)?;
        Ok(())
    }
} 