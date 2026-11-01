use zmq;
use anyhow::Result;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use crate::websocket::CreateTokenArgs;

#[derive(Serialize, Deserialize)]
pub struct TokenMessage {
    pub token: CreateTokenArgs,
    pub timestamp: DateTime<Utc>,
    pub sequence: u64,  // For tracking message order
}

// Add new message type for worker status
#[derive(Serialize, Deserialize)]
pub struct WorkerStatusMessage {
    pub is_idle: bool,
    pub worker_id: String,
}

pub struct TokenPublisher {
    socket: zmq::Socket,
    control_socket: zmq::Socket,  // New socket for worker status
    sequence: u64,
    pub idle_workers: u32,  // Add idle workers counter
}

impl TokenPublisher {
    pub fn new() -> Result<Self> {
        let context = zmq::Context::new();
        let socket = context.socket(zmq::PUSH)?;
        let control_socket = context.socket(zmq::PULL)?;  // For receiving worker status
        
        // Performance options
        socket.set_sndhwm(1000)?;  // Limit memory usage
        socket.set_linger(0)?;      // Don't wait on close
        socket.set_immediate(true)?; // Send immediately
        
        socket.bind("tcp://127.0.0.1:5555")?;
        control_socket.bind("tcp://127.0.0.1:5556")?;  // Control channel
        
        Ok(Self { 
            socket,
            control_socket,
            sequence: 0,
            idle_workers: 0,  // Initialize idle workers
        })
    }

    pub fn increment_idle_workers(&mut self) {
        self.idle_workers += 1;
        println!("Idle workers: {}", self.idle_workers);
    }

    pub fn decrement_idle_workers(&mut self) {
        if self.idle_workers > 0 {
            self.idle_workers -= 1;
        }
        println!("Idle workers: {}", self.idle_workers);
    }

    pub fn get_idle_workers(&self) -> u32 {
        self.idle_workers
    }

    pub fn publish(&mut self, token: CreateTokenArgs) -> Result<()> {
        let msg = TokenMessage {
            token,
            timestamp: Utc::now(),
            sequence: self.sequence,
        };
        self.sequence += 1;

        let data = serde_json::to_vec(&msg)?;
        match self.socket.send(&data, zmq::DONTWAIT) {
            Ok(_) => {
                Ok(())
            },
            Err(zmq::Error::EAGAIN) => {
                println!("Buffer full, message dropped");
                Err(anyhow::anyhow!("Buffer full"))
            },
            Err(e) => Err(e.into()),
        }
    }

    // Add method to check for worker status updates
    pub fn check_worker_status(&mut self) -> Result<()> {
        match self.control_socket.recv_bytes(zmq::DONTWAIT) {
            Ok(msg) => {
                let status: WorkerStatusMessage = serde_json::from_slice(&msg)?;
                if status.is_idle {
                    self.increment_idle_workers();
                } else {
                    self.decrement_idle_workers();
                }
                Ok(())
            },
            Err(zmq::Error::EAGAIN) => Ok(()), // No messages waiting
            Err(e) => Err(e.into()),
        }
    }
}


pub struct TokenSubscriber {
    socket: zmq::Socket,
    control_socket: zmq::Socket,  // New socket for worker status
    last_sequence: u64,
    worker_id: String,
}

impl TokenSubscriber {
    pub fn new(worker_id: String) -> Result<Self> {
        let context = zmq::Context::new();
        let socket = context.socket(zmq::PULL)?;
        let control_socket = context.socket(zmq::PUSH)?;  // For sending worker status
        
        // Performance options
        socket.set_rcvhwm(1000)?; // buffer size
        socket.set_linger(0)?;
        socket.set_immediate(true)?;
        
        socket.connect("tcp://127.0.0.1:5555")?;
        control_socket.connect("tcp://127.0.0.1:5556")?;  // Control channel
        
        Ok(Self { 
            socket,
            control_socket,
            last_sequence: 0,
            worker_id,
        })
    }

    pub fn set_idle(&mut self, idle: bool) -> Result<()> {
        // Notify publisher of status change
        let status = WorkerStatusMessage {
            is_idle: idle,
            worker_id: self.worker_id.clone(),
        };
        
        let data = serde_json::to_vec(&status)?;
        self.control_socket.send(&data, 0)?;
        Ok(())
    }

    pub fn receive(&mut self) -> Result<TokenMessage> {
        let msg = self.socket.recv_bytes(0)?;
        let token_msg: TokenMessage = serde_json::from_slice(&msg)?;
        
        self.last_sequence = token_msg.sequence;
        
        // Check latency
        let latency = Utc::now() - token_msg.timestamp;
        if latency.num_microseconds().unwrap() > 1000 {
            println!(
                "High latency: {}μs", 
                latency.num_microseconds().unwrap()
            );
        }
        
        Ok(token_msg)
    }
}