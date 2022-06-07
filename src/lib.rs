use std::rc::Rc;
use std::task::Waker;

use connection::Connection;
use measurer::Measurer;
use seed::prelude::*;

use seed::div;

mod connection;
mod measurer;

struct Model {
    connection: Connection,
    measurer: Measurer,
    counter: u64,
}

enum Msg {
    AddRenderable,
    Measurer(measurer::Msg),
    Connection(connection::Msg),
    Wake(Vec<Waker>),
}

fn init(_url: Url, orders: &mut impl Orders<Msg>) -> Model {
    let msg_sender = orders.msg_sender();
    let connection = Connection::new("wss://ws.postman-echo.com/raw", &mut orders.proxy(Msg::Connection));
    let measurer = Measurer::new(Rc::new({
        let outer = Rc::clone(&msg_sender);
        move |msg| outer(Some(Msg::Measurer(msg)))
    }));

    Model {
        counter: 0,
        connection,
        measurer,
    }
}

fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::AddRenderable => {
            let measurer = model.measurer.clone();
            let connection = model.connection.clone();
            let id = model.counter;
            model.counter = model.counter.wrapping_add(1);

            orders.perform_cmd(async move {
                for i in 0..4 {
                    let mr = measurer.clone();
                    let connection = connection.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        let text = connection.request(&format!("Message {id}/{i}")).await;
                        seed::log!("Got content: ", text);
                        let r = format!("Renderable: {text}");
                        let ms = mr.measure(r).await;
                        let _r = ms.get();
                        seed::log!("Measured: ", text);
                    });
                }
            });
        }
        Msg::Connection(msg) => Connection::update(msg, &mut model.connection, &mut orders.proxy(Msg::Connection)),
        Msg::Measurer(msg) => model.measurer.update(msg, orders, Msg::Measurer),
        Msg::Wake(wakers) => {
            for w in wakers {
                w.wake();
            }
        }
    }
}

fn view(model: &Model) -> Node<Msg> {
    div![
        div!["Add measurements", ev(Ev::Click, |_| Msg::AddRenderable)],
        model.measurer.view().map_msg(Msg::Measurer)
    ]
}

#[wasm_bindgen(start)]
pub fn run() {
    seed::App::start("app", init, update, view);
}
