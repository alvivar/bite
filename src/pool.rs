use std::{
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
};

type Work = Box<dyn FnOnce() + Send + 'static>;

enum Job {
    Task(Work),
    Stop,
}

pub struct ThreadPool {
    tx: Sender<Job>,
    workers: Vec<Worker>,
}

impl ThreadPool {
    pub fn new(size: usize) -> ThreadPool {
        let (tx, rx) = channel();
        let rx = Arc::new(Mutex::new(rx));

        let mut workers = Vec::with_capacity(size);

        for _ in 0..size {
            let worker = Worker::new(Arc::clone(&rx));
            workers.push(worker);
        }

        ThreadPool { tx, workers }
    }

    pub fn submit<F>(&mut self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Job::Task(Box::new(f));
        self.tx.send(job).unwrap();
    }

    pub fn size(&self) -> usize {
        self.workers.len()
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        for _ in 0..self.workers.len() {
            self.tx.send(Job::Stop).unwrap();
        }

        for worker in &mut self.workers {
            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

struct Worker {
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    fn new(receiver: Arc<Mutex<Receiver<Job>>>) -> Worker {
        let thread = thread::spawn(move || loop {
            let job = receiver.lock().unwrap().recv();
            match job {
                Ok(Job::Task(f)) => f(),

                Ok(Job::Stop) => break,

                Err(_) => break,
            }
        });

        Worker {
            thread: Some(thread),
        }
    }
}
