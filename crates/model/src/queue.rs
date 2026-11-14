//! Runs a model's writes one at a time, so two can't overlap.

use std::{cell::Cell, rc::Rc};

use gpui::{AsyncApp, Context, Task, WeakEntity};

/// A model's jobs, run one at a time in the order they were pushed. Dropping it cancels the job
/// running and every one waiting.
#[derive(Default)]
pub(crate) struct Queue {
    last: Option<Task<()>>,
    /// Jobs pushed that haven't finished.
    unfinished: Rc<Cell<usize>>,
}

impl Queue {
    /// Runs `job` once every job pushed before it has finished.
    pub(crate) fn push<M: 'static>(
        &mut self,
        job: impl AsyncFnOnce(WeakEntity<M>, &mut AsyncApp) + 'static,
        cx: &mut Context<M>,
    ) {
        let previous = self.last.take();
        let unfinished = self.unfinished.clone();
        unfinished.set(unfinished.get() + 1);
        self.last = Some(cx.spawn(async move |this, cx| {
            if let Some(previous) = previous {
                previous.await;
            }
            job(this, cx).await;
            unfinished.set(unfinished.get() - 1);
        }));
    }

    /// Whether a job is running or waiting.
    pub(crate) fn is_busy(&self) -> bool {
        self.unfinished.get() > 0
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use gpui::{AppContext as _, TestAppContext};

    use super::*;

    struct Model {
        writes: Queue,
    }

    #[gpui::test]
    fn jobs_run_one_at_a_time_in_the_order_they_were_pushed(cx: &mut TestAppContext) {
        let log = Rc::new(RefCell::new(Vec::new()));
        let model = cx.new(|_| Model {
            writes: Queue::default(),
        });
        model.update(cx, |model, cx| {
            for (job, delay) in [("slow", 30), ("fast", 0)] {
                let log = log.clone();
                model.writes.push(
                    async move |_, cx| {
                        log.borrow_mut().push(format!("{job} starts"));
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(delay))
                            .await;
                        log.borrow_mut().push(format!("{job} ends"));
                    },
                    cx,
                );
            }
            assert!(model.writes.is_busy());
        });
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(30));
        cx.run_until_parked();
        assert_eq!(
            *log.borrow(),
            ["slow starts", "slow ends", "fast starts", "fast ends"]
        );
        model.read_with(cx, |model, _| assert!(!model.writes.is_busy()));
    }
}
