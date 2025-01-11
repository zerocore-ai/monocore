//! Server-side state management for NFSv4

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use crate::error::Error;
use crate::Result;

/// Client ID and associated information
#[derive(Debug)]
pub struct ClientInfo {
    /// Last time the client renewed its lease
    pub last_renewal: Instant,
    /// Client's machine name
    pub machinename: String,
}

/// Open file state
#[derive(Debug)]
pub struct OpenState {
    /// File handle
    pub filehandle: Vec<u8>,
    /// Owner ID
    pub owner: u32,
    /// Access mode (read/write)
    pub access: u32,
    /// Share mode
    pub share: u32,
}

/// Lock state
#[derive(Debug)]
pub struct LockState {
    /// File handle
    pub filehandle: Vec<u8>,
    /// Lock owner
    pub owner: u32,
    /// Lock type (read/write)
    pub lock_type: u32,
    /// Lock range start
    pub offset: u64,
    /// Lock range length
    pub length: u64,
}

/// Server state manager
#[derive(Debug)]
pub struct StateManager {
    /// Client information
    clients: RwLock<HashMap<u32, ClientInfo>>,
    /// Open file states
    open_states: RwLock<HashMap<u32, OpenState>>,
    /// Lock states
    lock_states: RwLock<HashMap<u32, LockState>>,
    /// Lease duration
    lease_duration: Duration,
}

impl StateManager {
    /// Create a new state manager
    pub fn new(lease_duration: Duration) -> Self {
        StateManager {
            clients: RwLock::new(HashMap::new()),
            open_states: RwLock::new(HashMap::new()),
            lock_states: RwLock::new(HashMap::new()),
            lease_duration,
        }
    }

    /// Register a new client
    pub fn register_client(&self, client_id: u32, machinename: String) -> Result<()> {
        let mut clients = self
            .clients
            .write()
            .map_err(|_| Error::State("Lock poisoned".into()))?;
        clients.insert(
            client_id,
            ClientInfo {
                last_renewal: Instant::now(),
                machinename,
            },
        );
        Ok(())
    }

    /// Renew a client's lease
    pub fn renew_lease(&self, client_id: u32) -> Result<()> {
        let mut clients = self
            .clients
            .write()
            .map_err(|_| Error::State("Lock poisoned".into()))?;
        if let Some(client) = clients.get_mut(&client_id) {
            client.last_renewal = Instant::now();
            Ok(())
        } else {
            Err(Error::State("Client not found".into()))
        }
    }

    /// Check if a client's lease is still valid
    pub fn is_lease_valid(&self, client_id: u32) -> Result<bool> {
        let clients = self
            .clients
            .read()
            .map_err(|_| Error::State("Lock poisoned".into()))?;
        if let Some(client) = clients.get(&client_id) {
            Ok(client.last_renewal.elapsed() <= self.lease_duration)
        } else {
            Err(Error::State("Client not found".into()))
        }
    }

    /// Record an open state
    pub fn record_open(
        &self,
        stateid: u32,
        filehandle: Vec<u8>,
        owner: u32,
        access: u32,
        share: u32,
    ) -> Result<()> {
        let mut open_states = self
            .open_states
            .write()
            .map_err(|_| Error::State("Lock poisoned".into()))?;
        open_states.insert(
            stateid,
            OpenState {
                filehandle,
                owner,
                access,
                share,
            },
        );
        Ok(())
    }

    /// Record a lock state
    pub fn record_lock(
        &self,
        stateid: u32,
        filehandle: Vec<u8>,
        owner: u32,
        lock_type: u32,
        offset: u64,
        length: u64,
    ) -> Result<()> {
        let mut lock_states = self
            .lock_states
            .write()
            .map_err(|_| Error::State("Lock poisoned".into()))?;
        lock_states.insert(
            stateid,
            LockState {
                filehandle,
                owner,
                lock_type,
                offset,
                length,
            },
        );
        Ok(())
    }

    /// Remove an open state
    pub fn remove_open(&self, stateid: u32) -> Result<()> {
        let mut open_states = self
            .open_states
            .write()
            .map_err(|_| Error::State("Lock poisoned".into()))?;
        open_states.remove(&stateid);
        Ok(())
    }

    /// Remove a lock state
    pub fn remove_lock(&self, stateid: u32) -> Result<()> {
        let mut lock_states = self
            .lock_states
            .write()
            .map_err(|_| Error::State("Lock poisoned".into()))?;
        lock_states.remove(&stateid);
        Ok(())
    }

    /// Clean up expired client states
    pub fn cleanup_expired(&self) -> Result<()> {
        let mut clients = self
            .clients
            .write()
            .map_err(|_| Error::State("Lock poisoned".into()))?;
        let _open_states = self
            .open_states
            .write()
            .map_err(|_| Error::State("Lock poisoned".into()))?;
        let _lock_states = self
            .lock_states
            .write()
            .map_err(|_| Error::State("Lock poisoned".into()))?;

        // Remove expired clients and their states
        clients.retain(|_, client| client.last_renewal.elapsed() <= self.lease_duration);

        // TODO: Remove associated open and lock states
        // This would require maintaining a mapping between client IDs and their states

        Ok(())
    }
}
