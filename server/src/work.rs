use crossbeam_channel::{unbounded, Receiver, Sender};

use std::thread;
use std::{
    sync::{Arc, Mutex},
    usize,
};

trait FnBox {
    fn call_box(self: Box<Self>);
}

impl<F: FnOnce()> FnBox for F {
    fn call_box(self: Box<F>) {
        (*self)()
    }
}

type Job = Box<dyn FnBox + Send + 'static>;

enum Message {
    NewJob(Job, Arc<Mutex<usize>>),
    Terminate,
}

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Sender<Message>,
    receiver: Arc<Mutex<Receiver<Message>>>,
    active_jobs: Arc<Mutex<usize>>,
}

impl ThreadPool {
    pub fn new(size: usize) -> ThreadPool {
        assert!(size > 0);

        let mut workers = Vec::with_capacity(size);
        let (sender, receiver) = unbounded();
        let receiver = Arc::new(Mutex::new(receiver));
        let active_jobs = Arc::new(Mutex::new(0));

        for id in 0..size {
            workers.push(Worker::new(id, receiver.clone()));
        }

        println!("{} workers waiting for jobs", size);

        ThreadPool {
            workers,
            sender,
            receiver,
            active_jobs,
        }
    }

    pub fn execute<F>(&mut self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        // New worker if the queue is busy.
        let worker_id = self.workers.len();

        if *self.active_jobs.lock().unwrap() >= worker_id {
            let receiver = self.receiver.clone();
            self.workers.push(Worker::new(worker_id, receiver));

            println!("Worker {} has been created", worker_id);
        }

        // Job queue.
        let job = Box::new(f);

        self.sender
            .send(Message::NewJob(job, self.active_jobs.clone()))
            .unwrap();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        println!("Sending |Terminate| message to all workers");

        for _ in &mut self.workers {
            self.sender.send(Message::Terminate).unwrap();
        }

        println!("Shutting down all workers");

        for worker in &mut self.workers {
            println!("Shutting down worker {}", worker.id);

            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<Receiver<Message>>>) -> Worker {
        let thread = thread::spawn(move || loop {
            let message = receiver.lock().unwrap().recv().unwrap();

            match message {
                Message::NewJob(job, active_jobs) => {
                    *active_jobs.lock().unwrap() += 1;

                    println!("Worker {} executing a job", id);
                    job.call_box();

                    *active_jobs.lock().unwrap() -= 1;
                    println!("Worker {} just finish his job", id);
                }
                Message::Terminate => {
                    println!("Worker {} was told to terminate", id);

                    break;
                }
            }
        });

        Worker {
            id,
            thread: Some(thread),
        }
    }
}
