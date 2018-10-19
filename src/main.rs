extern crate bytes;
extern crate tokio;
extern crate futures;
extern crate rand;
extern crate actix;
extern crate actix_web;

use std::time::{Duration, Instant};

use bytes::{Bytes};
use tokio::timer::Interval;
use futures::{Future, Async, Poll, Stream, sync::oneshot::*};
use actix::*;
use actix_web::{server, App, HttpRequest, HttpResponse, HttpContext};

mod chat_server;

struct SSEChatSessionState {
    addr: Addr<chat_server::ChatServer>,
}

struct SSE {
    /// unique session id
    id: usize,
}

impl Actor for SSE {
    type Context = HttpContext<Self, SSEChatSessionState>;
    fn started(&mut self, ctx: &mut Self::Context) {
        // register self in chat server. `AsyncContext::wait` register
        // future within context, but context waits until this future resolves
        // before processing any other events.
        // HttpContext::state() is instance of WsChatSessionState, state is shared
        // across all routes within application
        let addr = ctx.address();
        ctx.run_interval(Duration::new(5, 0), |act: &mut Self, ctx: &mut Self::Context| {
            println!("run interval");
           ctx.write("data: Online\r\n\r\n")
        });
        ctx.state()
            .addr
            .send(chat_server::Connect {
                addr: addr.recipient(),
            })
            .into_actor(self)
            .then(|res, act, ctx| {
                /*match res {
                    Ok(res) => act.id = res,
                    // something is wrong with chat server
                    //_ => ctx.stop(),
                }*/
                fut::ok(())
            })
            .wait(ctx);
    }

    fn stopping(&mut self, ctx: &mut Self::Context) -> Running {
        // notify chat server
        ctx.state().addr.do_send(chat_server::Disconnect { id: self.id });
        Running::Stop
    }
}

impl Handler<chat_server::Message> for SSE {
    type Result = ();
    fn handle(&mut self, msg: chat_server::Message, ctx: &mut Self::Context) {
        ctx.write(msg.0);
    }
}

fn handle_sse<'r>(req: &'r HttpRequest<SSEChatSessionState>) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/event-stream")
        .body(HttpContext::create(req.clone(), SSE{
            id: 1
        }))
}

fn main() {
    let sys = actix::System::new("sse-http-context");

    // Start chat server actor in separate thread
    let s = Arbiter::start(|_| chat_server::ChatServer::default());
    server::new(move|| {
        let state = SSEChatSessionState {
            addr: s.clone(),
        };

        App::with_state(state)
            .resource("/", |r| r.f(handle_sse))
    }).bind("127.0.0.1:8080")
        .unwrap()
        .start();
    let _ = sys.run();
}