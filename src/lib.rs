use std::{ sync::{ mpsc, Arc, Mutex }, thread };

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

type Job = Box<dyn FnOnce() + Send + 'static>;

impl ThreadPool {
    /// Create a new ThreadPool.
    ///
    /// The size is the number of threads in the pool.
    ///
    /// # Panics
    ///
    /// The `new` function will panic if the size is zero.
    pub fn new(size: usize) -> ThreadPool {
        assert!(size > 0);

        let (sender, receiver) = mpsc::channel();

        // Реализация канала, которую предоставляет Rust - несколько
        // производителей, один потребитель. Это означает, что мы не можем
        // просто клонировать принимающую сторону канала, чтобы исправить этот
        // код. Кроме этого, мы не хотим отправлять одно и то же сообщение
        // нескольким потребителям, поэтому нам нужен единый список сообщений
        // для множества обработчиков, чтобы каждое сообщение обрабатывалось
        // лишь один раз. Кроме того, удаление задачи из очереди канала включает
        // изменение receiver, поэтому потокам необходим безопасный способ
        // делиться и изменять receiver, в противном случае мы можем получить
        // условия гонки.
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        ThreadPool {
            workers,
            sender: Some(sender),
        }
    }

    pub fn execute<F>(&self, f: F) where F: FnOnce() + Send + 'static {
        let job = Box::new(f);

        self.sender.as_ref().unwrap().send(job).unwrap();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        // После этой строчки все вызовы recv, выполняемые рабочими процессами в
        // бесконечном цикле, вернут ошибку.
        drop(self.sender.take());

        for worker in &mut self.workers {
            println!("Shutting down worker {}", worker.id);

            // Метод take у типа Option забирает значение из варианта Some и
            // оставляет вариант None в этом месте.
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
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Worker {
        let thread = thread::spawn(move || {
            loop {
                let message = receiver.lock().unwrap().recv();

                match message {
                    Ok(job) => {
                        println!("Worker {id} got a job; executing.");

                        job();
                    }
                    Err(_) => {
                        println!("Worker {id} disconnected; shutting down.");
                        break;
                    }
                }
            }
        });

        Worker {
            id,
            thread: Some(thread),
        }
    }
}
