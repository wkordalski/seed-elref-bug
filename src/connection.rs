use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    rc::Rc,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

use seed::prelude::*;

#[derive(Clone, Debug)]
pub(crate) enum Msg {
    Opened,
    Closed,
    Failed,
    Received(String),
    Reconnect,
}

#[derive(Clone)]
pub(crate) struct Connection {
    data: Arc<Mutex<ConnectionData>>,
}

pub(crate) struct ConnectionData {
    url: String,
    websocket: WebSocket,
    reconnector: Option<StreamHandle>,

    next_free_id: u64,
    requests: HashMap<u64, RequestEntry>,
}

impl Connection {
    pub(crate) fn new(url: &str, orders: &mut impl Orders<Msg>) -> Self {
        Self {
            data: Arc::new(Mutex::new(ConnectionData {
                url: url.to_owned(),
                websocket: create_websocket(url, orders),
                reconnector: None,

                next_free_id: 0,
                requests: HashMap::new(),
            })),
        }
    }

    pub(crate) fn update(msg: Msg, model: &mut Self, orders: &mut impl Orders<Msg>) {
        let mut data = model.data.lock().unwrap();
        match msg {
            Msg::Failed | Msg::Closed => {
                if data.reconnector.is_none() {
                    data.reconnector = Some(
                        orders.stream_with_handle(streams::backoff(Some(16), |_| Msg::Reconnect)),
                    );
                }
            }
            Msg::Reconnect => {
                data.websocket = create_websocket(&data.url, orders);
            }
            Msg::Opened => {
                data.reconnector = None;
                for entry in data.requests.values() {
                    let _ = send_message(&entry.request, &data.websocket);
                }
            }
            Msg::Received(packet) => {
                seed::log!(packet);
                let (rid, content) = packet.split_once('|').unwrap();
                let rid: u64 = rid.parse().unwrap();
                let entry = data.requests.remove(&rid);
                if let Some(entry) = entry {
                    entry.set_response(content.to_string());
                }
            }
        }
    }

    pub(crate) fn request(&self, message: &str) -> impl Future<Output = String> {
        let state = Arc::new(Mutex::new(ResponseFutureState {
            response_message: None,
            waker: None,
        }));

        let data = &mut *self.data.lock().unwrap();

        let id = data.next_free_id;
        data.next_free_id = data.next_free_id.wrapping_add(1);

        let request = format!("{id}|{message}");

        let _ = data.websocket.send_text(&request);

        data.requests.insert(
            id,
            RequestEntry {
                request,
                future_state: state.clone(),
            },
        );

        ResponseFuture { state }
    }
}

//------------------------------------------------------------------------------
// Operations on raw websockets
//------------------------------------------------------------------------------

fn create_websocket(url: &str, orders: &mut impl Orders<Msg>) -> WebSocket {
    let msg_sender = orders.msg_sender();

    WebSocket::builder(url, orders)
        .on_open(|| Msg::Opened)
        .on_message(move |msg| decode_message(msg, msg_sender))
        .on_close(|_| Msg::Closed)
        .on_error(|| Msg::Failed)
        .build_and_open()
        .unwrap()
}

fn decode_message(message: WebSocketMessage, msg_sender: Rc<dyn Fn(Option<Msg>)>) {
    if message.contains_text() {
        msg_sender(Some(Msg::Received(message.text().unwrap())));
    } else {
        panic!("Unsupported message type");
    }
}

fn send_message(message: impl AsRef<str>, websocket: &WebSocket) -> Result<(), WebSocketError> {
    websocket.send_text(message)
}

//------------------------------------------------------------------------------
// Tracking requests
//------------------------------------------------------------------------------

struct RequestEntry {
    request: String,
    future_state: Arc<Mutex<ResponseFutureState>>,
}

impl RequestEntry {
    fn set_response(self, message: String) {
        let mut state = self.future_state.lock().unwrap();

        state.response_message = Some(message);
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
    }
}

struct ResponseFuture {
    state: Arc<Mutex<ResponseFutureState>>,
}

struct ResponseFutureState {
    response_message: Option<String>,
    waker: Option<Waker>,
}

impl Future for ResponseFuture {
    type Output = String;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.state.lock().unwrap();

        if let Some(message) = state.response_message.take() {
            Poll::Ready(message)
        } else {
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}
