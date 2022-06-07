use std::{
    cell::RefCell,
    fmt,
    future::Future,
    pin::Pin,
    rc::{Rc, Weak},
    task::{Context, Poll, Waker},
};

use seed::div;
use seed::prelude::*;
use web_sys::{Element, HtmlElement};

/// Allows for rendering DOM in an invisible space and taking measurements on it then.
#[derive(Clone)]
pub(crate) struct Measurer {
    data: Rc<RefCell<MeasurerData>>,
}

struct MeasurerData {
    /// All measurements that should be rendered
    measurements: Vec<WeakMeasurement>,
    /// Futures' states of measurements that have not been rendered and woken up yet
    futures: Vec<Weak<RefCell<FutureState>>>,
    /// Maps message to application message type and sends to update.
    /// Use it only within `async` blocks.
    msg_sender: Rc<dyn Fn(Msg)>,
}

/// Stores reference to rendered DOM element.
///
/// When the DOM element is not needed no more (referencing Measurement has been dropped),
/// it is not rendered any more.
///
/// Cloning this struct is cheap as it stores [`Rc<_>`] under the hood.
#[derive(Clone, Debug)]
pub(crate) struct Measurement(Rc<MeasurementData>);

struct WeakMeasurement(Weak<MeasurementData>);

struct MeasurementData {
    text: String,
    div: ElRef<HtmlElement>,
    /// This is only to prove that some node was rendered, but without el_ref attached
    rendered: RefCell<bool>,
}

struct FutureState {
    measurement: Measurement,
    waker: Option<Waker>,
}

pub(crate) struct MeasureFuture {
    state: Rc<RefCell<FutureState>>,
}

#[derive(Debug)]
pub enum Msg {
    WaitForRender,
    Measured,
    MeasuredElementMessage,
}

impl Measurer {
    pub(crate) fn new(msg_sender: Rc<dyn Fn(Msg)>) -> Self {
        let data = MeasurerData {
            futures: Vec::new(),
            measurements: Vec::new(),
            msg_sender,
        };
        Self {
            data: Rc::new(RefCell::new(data)),
        }
    }

    /// Gets node to display hiddenly and returns displayed element asynchronously
    /// for measurements.
    pub(crate) fn measure(&self, text: String) -> impl Future<Output = Measurement> {
        let measurement = Measurement::new(text);
        let state = Rc::new(RefCell::new(FutureState {
            measurement: measurement.clone(),
            waker: None,
        }));

        let mut guard = self.data.borrow_mut();
        let msg_sender = Rc::clone(&guard.msg_sender);
        guard.measurements.push(measurement.downgrade());
        guard.futures.push(Rc::downgrade(&state));
        drop(guard);

        async move {
            msg_sender(Msg::WaitForRender);
            MeasureFuture { state }.await
        }
    }

    pub(crate) fn view(&self) -> Node<Msg> {
        let mut guard = self.data.borrow_mut();

        // Filter-out disposed measurements
        let (filtered_measurements, measurements_to_render): (Vec<_>, Vec<_>) = guard
            .measurements
            .drain(..)
            .filter_map(|w| w.upgrade().map(move |m| (w, m)))
            .unzip();
        guard.measurements = filtered_measurements;

        // Mark that specific measurement is rendered
        for m in &measurements_to_render {
            *m.0.rendered.borrow_mut() = true;
        }

        div![measurements_to_render
            .iter()
            .map(|m| m.view().map_msg(|()| Msg::MeasuredElementMessage)),]
    }

    pub(crate) fn update(
        &mut self,
        msg: Msg,
        orders: &mut impl Orders<crate::Msg>,
        wrap_msg: fn(Msg) -> crate::Msg,
    ) {
        match msg {
            Msg::WaitForRender => {
                orders.after_next_render(move |_| wrap_msg(Msg::Measured));
                orders.render();
            }
            Msg::Measured => {
                let mut guard = self.data.borrow_mut();
                let mut wakers = Vec::new();
                let mut filtered_futures = Vec::new();
                for future_state_weak in guard.futures.drain(..) {
                    if let Some(future_state_ref) = future_state_weak.upgrade() {
                        let mut future_state = future_state_ref.borrow_mut();
                        assert_eq!(
                            future_state.measurement.0.div.get().is_some(),
                            *future_state.measurement.0.rendered.borrow(),
                            "Wrongly rendered text: {:?}", &future_state.measurement.0.text
                        );
                        if future_state.measurement.0.div.get().is_some() {
                            if let Some(waker) = future_state.waker.take() {
                                wakers.push(waker);
                            }
                        } else {
                            filtered_futures.push(future_state_weak);
                        }
                    }
                }
                let wakeup_needed = !filtered_futures.is_empty();
                guard.futures = filtered_futures;
                drop(guard);

                if !wakers.is_empty() {
                    orders.send_msg(crate::Msg::Wake(wakers));
                }

                if wakeup_needed {
                    orders.render();
                } else {
                    orders.skip();
                }
            }
            Msg::MeasuredElementMessage => {
                panic!("Measured elements should not generate messages")
            }
        }
    }
}

impl Measurement {
    fn new(text: String) -> Self {
        Self(Rc::new(MeasurementData {
            text,
            div: ElRef::new(),
            rendered: RefCell::new(false),
        }))
    }

    fn downgrade(&self) -> WeakMeasurement {
        WeakMeasurement(Rc::downgrade(&self.0))
    }

    fn view(&self) -> Node<()> {
        div![el_ref(&self.0.div), div![&self.0.text]]
    }

    /// Returns rendered node
    pub(crate) fn get(&self) -> Element {
        let container = self.0.div.get().expect(
            "Called `Measurement::get()` before future completion (i.e. node was rendered).",
        );
        container.first_element_child().unwrap()
    }
}

impl WeakMeasurement {
    fn upgrade(&self) -> Option<Measurement> {
        self.0.upgrade().map(Measurement)
    }
}

impl Future for MeasureFuture {
    type Output = Measurement;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.state.borrow_mut();

        if state.measurement.0.div.get().is_some() {
            Poll::Ready(state.measurement.clone())
        } else {
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl fmt::Debug for MeasurementData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MeasurementData")
            .field("text", &self.text)
            .field("div", &self.div)
            .finish()
    }
}
