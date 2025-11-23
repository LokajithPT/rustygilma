use std::collections::VecDeque;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{Mutex, Semaphore};
use tokio::time::{Duration, Instant};
use tracing::debug;

#[derive(Debug)]
struct Connection {
    stream: TcpStream,
    created_at: Instant,
    last_used: Instant,
}

impl Connection {
    fn new(stream: TcpStream) -> Self {
        let now = Instant::now();
        Self {
            stream,
            created_at: now,
            last_used: now,
        }
    }
    
    fn is_expired(&self, max_age: Duration) -> bool {
        self.created_at.elapsed() > max_age
    }
    
    fn is_idle(&self, max_idle: Duration) -> bool {
        self.last_used.elapsed() > max_idle
    }
}

#[derive(Debug)]
pub struct ConnectionPool {
    connections: Arc<Mutex<VecDeque<Connection>>>,
    max_size: usize,
    max_age: Duration,
    max_idle: Duration,
    semaphore: Semaphore,
    server_addr: String,
}

impl ConnectionPool {
    pub fn new(server_addr: String, max_size: usize) -> Self {
        Self {
            connections: Arc::new(Mutex::new(VecDeque::with_capacity(max_size))),
            max_size,
            max_age: Duration::from_secs(300), // 5 minutes
            max_idle: Duration::from_secs(60), // 1 minute
            semaphore: Semaphore::new(max_size),
            server_addr,
        }
    }
    
    pub async fn get_connection(&self) -> Result<PooledConnection, Box<dyn std::error::Error + Send + Sync>> {
        let _permit = self.semaphore.acquire().await?;
        
        // Try to get an existing connection
        {
            let mut connections = self.connections.lock().await;
            if let Some(mut conn) = connections.pop_front() {
                // Check if connection is still valid
                if !conn.is_expired(self.max_age) && !conn.is_idle(self.max_idle) {
                    conn.last_used = Instant::now();
                    debug!("Reusing existing connection");
                    return Ok(PooledConnection {
                        connection: Some(conn),
                        pool: self,
                    });
                }
            }
        }
        
        // Create new connection
        debug!("Creating new connection to {}", self.server_addr);
        let stream = TcpStream::connect(&self.server_addr).await?;
        let conn = Connection::new(stream);
        
        Ok(PooledConnection {
            connection: Some(conn),
            pool: self,
        })
    }
    
    async fn return_connection(&self, mut conn: Connection) {
        let mut connections = self.connections.lock().await;
        
        if connections.len() < self.max_size && 
           !conn.is_expired(self.max_age) && 
           !conn.is_idle(self.max_idle) {
            conn.last_used = Instant::now();
            connections.push_back(conn);
            debug!("Returned connection to pool");
        } else {
            debug!("Discarding expired or oversized connection");
        }
    }
}

pub struct PooledConnection<'a> {
    connection: Option<Connection>,
    pool: &'a ConnectionPool,
}

impl<'a> PooledConnection<'a> {
    pub async fn send_command(&mut self, command: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(conn) = &mut self.connection {
            use tokio::io::AsyncWriteExt;
            conn.stream.write_all(command.as_bytes()).await?;
            conn.last_used = Instant::now();
            Ok(())
        } else {
            Err("Connection not available".into())
        }
    }
    
    pub fn get_stream(&mut self) -> Option<&mut TcpStream> {
        self.connection.as_mut().map(|conn| &mut conn.stream)
    }
    
    pub fn split(&mut self) -> Option<(tokio::net::tcp::ReadHalf<'_>, tokio::net::tcp::WriteHalf<'_>)> {
        self.connection.as_mut().map(|conn| conn.stream.split())
    }
}

impl<'a> Drop for PooledConnection<'a> {
    fn drop(&mut self) {
        if let Some(conn) = self.connection.take() {
            // Just drop the connection for now - pool management needs redesign
            drop(conn);
        }
    }
}

// Background task to clean up expired connections
pub async fn cleanup_task(pool: Arc<ConnectionPool>) {
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    
    loop {
        interval.tick().await;
        
        let mut connections = pool.connections.lock().await;
        let initial_len = connections.len();
        
        connections.retain(|conn| {
            !conn.is_expired(pool.max_age) && !conn.is_idle(pool.max_idle)
        });
        
        let removed = initial_len - connections.len();
        if removed > 0 {
            debug!("Cleaned up {} expired connections", removed);
        }
    }
}