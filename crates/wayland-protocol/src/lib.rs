use anyhow::{Context, Result};
use calloop::{EventSource, Poll, PostAction, Readiness, Token, TokenFactory};
use log::{debug, error, info};
use std::{fs, io, os::unix::net::UnixListener, path::Path};

mod client;
mod protocols;
pub use client::{ClientConnection, ClientEvent, ClientId};

#[repr(C)]
struct MessageHeader {
    object_id: ObjectId,
    opcode: u16,
    size: u16,
}

#[derive(Debug)]
pub enum WaylandEvent {
    ClientConnected(ClientConnection),
}

pub struct Wayland {
    socket_path: String,
    listener: Option<UnixListener>,
    next_client_id: ClientId,
}

impl Wayland {
    pub fn new(socket_path: impl Into<String>) -> Self {
        Self {
            socket_path: socket_path.into(),
            listener: None,
            next_client_id: 1,
        }
    }

    pub fn bind(&mut self) -> Result<()> {
        // Remove existing socket if it exists
        if Path::new(&self.socket_path).exists() {
            fs::remove_file(&self.socket_path).context("Failed to remove existing socket")?;
        }

        // Create Unix domain socket
        let listener = UnixListener::bind(&self.socket_path).context("Failed to bind to socket")?;

        // Set socket to non-blocking mode
        listener
            .set_nonblocking(true)
            .context("Failed to set socket to non-blocking mode")?;

        self.listener = Some(listener);
        info!("Wayland socket bound to {}", self.socket_path);
        Ok(())
    }

    fn handle_new_clients<F>(&mut self, mut callback: F) -> io::Result<()>
    where
        F: FnMut(WaylandEvent, &mut ()),
    {
        if let Some(ref listener) = self.listener {
            loop {
                match listener.accept() {
                    Ok((stream, _addr)) => {
                        let client_id = self.next_client_id;
                        self.next_client_id += 1;

                        match ClientConnection::new(stream, client_id) {
                            Ok(client) => {
                                debug!("New client connected with ID: {}", client_id);
                                callback(WaylandEvent::ClientConnected(client), &mut ());
                            }
                            Err(e) => {
                                error!("Failed to create client connection: {}", e);
                                return Err(e);
                            }
                        }
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        // No more clients to accept
                        break;
                    }
                    Err(e) => {
                        error!("Failed to accept client: {}", e);
                        return Err(e);
                    }
                }
            }
        }
        Ok(())
    }
}

impl EventSource for Wayland {
    type Event = WaylandEvent;
    type Metadata = ();
    type Ret = ();
    type Error = io::Error;

    fn process_events<F>(
        &mut self,
        readiness: Readiness,
        _token: Token,
        callback: F,
    ) -> Result<PostAction, Self::Error>
    where
        F: FnMut(Self::Event, &mut Self::Metadata) -> Self::Ret,
    {
        if readiness.readable {
            self.handle_new_clients(callback)?;
        }
        Ok(PostAction::Continue)
    }

    fn register(
        &mut self,
        poll: &mut Poll,
        token_factory: &mut TokenFactory,
    ) -> calloop::Result<()> {
        if let Some(ref listener) = self.listener {
            unsafe {
                // SAFETY: The listener is unregistered
                poll.register(
                    listener,
                    calloop::Interest::READ,
                    calloop::Mode::Level,
                    token_factory.token(),
                )?;
            }
        }
        Ok(())
    }

    fn reregister(
        &mut self,
        poll: &mut Poll,
        token_factory: &mut TokenFactory,
    ) -> calloop::Result<()> {
        if let Some(ref listener) = self.listener {
            poll.reregister(
                listener,
                calloop::Interest::READ,
                calloop::Mode::Level,
                token_factory.token(),
            )?;
        }
        Ok(())
    }

    fn unregister(&mut self, poll: &mut Poll) -> calloop::Result<()> {
        if let Some(listener) = &self.listener {
            poll.unregister(listener)?;
        }
        Ok(())
    }
}

impl Drop for Wayland {
    fn drop(&mut self) {
        // Clean up socket file when dropping
        if Path::new(&self.socket_path).exists() {
            if let Err(e) = fs::remove_file(&self.socket_path) {
                error!("Failed to remove socket file: {}", e);
            }
        }
    }
}
