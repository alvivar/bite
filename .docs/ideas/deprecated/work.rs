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
    New(Job, Arc<Mutex<usize>>),
    Terminate,
}

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Sender<Message>,
    receiver: Arc<Mutex<Receiver<Message>>>,
    active: Arc<Mutex<usize>>,
}

impl ThreadPool {
    pub fn new(size: usize) -> ThreadPool {
        assert!(size > 0);

        let mut workers = Vec::with_capacity(size);
        let (sender, receiver) = unbounded();
        let receiver = Arc::new(Mutex::new(receiver));
        let count = Arc::new(Mutex::new(0));

        for id in 0..size {
            workers.push(Worker::new(id, receiver.clone()));
        }

        println!("{} worker threads waiting for jobs", size);

        ThreadPool {
            workers,
            sender,
            receiver,
            active: count,
        }
    }

    pub fn execute<F>(&mut self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        // New worker if the queue is busy.
        let worker_id = self.workers.len();

        if *self.active.lock().unwrap() >= worker_id {
            let receiver = self.receiver.clone();
            self.workers.push(Worker::new(worker_id, receiver));

            println!("Worker {} created", worker_id);
        }

        // Job queue.
        let job = Box::new(f);

        self.sender
            .send(Message::New(job, self.active.clone()))
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
                Message::New(job, count) => {
                    // @todo What about those drops?

                    let mut c = count.lock().unwrap();
                    *c += 1;
                    drop(c);

                    println!("Worker {} on a job!", id);
                    job.call_box();

                    let mut c = count.lock().unwrap();
                    *c -= 1;
                    drop(c);

                    println!("Worker {} released", id);
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
