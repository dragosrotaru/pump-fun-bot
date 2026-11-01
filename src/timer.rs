use std::time::Instant;
use chrono::{DateTime, Utc};

pub struct Timer {
    pub action: String,
    pub t_one: Option<Instant>,
    pub t_two: Option<Instant>,
    pub wall_time: Option<DateTime<Utc>>,
}

impl Timer {
    pub fn new(action: &str) -> Self {
        Self { 
            action: action.to_string(), 
            t_one: None, 
            t_two: None,
            wall_time: None,
        }
    }
    pub fn start(action: &str) -> Self {
        Self { 
            action: action.to_string(), 
            t_one: Some(Instant::now()), 
            t_two: None,
            wall_time: Some(Utc::now()),
        }
    }
    pub fn end(&mut self) {
        self.t_two = Some(Instant::now());
    }
    pub fn duration(&self) -> i64 {
        self.t_two.unwrap().duration_since(self.t_one.unwrap()).as_micros() as i64
    }
}

pub struct TimerSeq {
    pub timers: Vec<Timer>,
}

impl TimerSeq {
    pub fn new() -> Self {
        Self { timers: vec![] }
    }
    pub fn start(action: &str) -> Self {
        Self { timers: vec![Timer::start(action)] }
    }

    /**
     * Starts a new timer and ends the previous timer if it exists and it is not ended
     */
    pub fn next(&mut self, action: &str) {
        if let Some(timer) = self.timers.last_mut() {
            timer.end();
        }
        self.timers.push(Timer::start(action));
    }
    /**
     * Ends the last timer and returns the duration
     */     
    pub fn end(&mut self) -> () {
        let timer = self.timers.last_mut().unwrap();
        if timer.t_two.is_none() {
            timer.end();
        }
    }
}