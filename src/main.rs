#[macro_use] extern crate rocket;

#[cfg(test)] mod tests;

use rocket::{State, Shutdown};
use rocket::fs::{relative, FileServer};
use rocket::form::Form;
use rocket::response::stream::{EventStream, Event};
use rocket::serde::{Serialize, Deserialize};
use rocket::tokio::sync::broadcast::{channel, Sender, error::RecvError};
use rocket::tokio::select;

#[derive(Debug, Clone, FromForm, Serialize, Deserialize)]
#[cfg_attr(test, derive(PartialEq, UriDisplayQuery))]
#[serde(crate = "rocket::serde")]
struct Message {
    #[field(validate = len(..30))]
    pub room: String,
    #[field(validate = len(..20))]
    pub username: String,
    pub message: String,
}

/// Returns an infinite stream of server-sent events. Each event is a message
/// pulled from a broadcast queue sent by the `post` handler.
#[get("/events")]
async fn events(queue: &State<Sender<Message>>, mut end: Shutdown) -> EventStream![] {
    let mut rx = queue.subscribe();
    EventStream! {
        loop {
            let msg = select! {
                // Receive a message from the broadcast queue
                msg = rx.recv() => match msg {
                    Ok(msg) => msg,
                    Err(RecvError::Closed) => break, // If the channel is closed, exit the loop
                    Err(RecvError::Lagged(_)) => continue, // If there is a backlog, continue to the next iteration
                },
                _ = &mut end => break, // If a shutdown signal is received, exit the loop
            };

            // Send the received message as a JSON event in the server-sent event stream
            yield Event::json(&msg);
        }
    }
}

/// Receive a message from a form submission and broadcast it to any receivers.
#[post("/message", data = "<form>")]
fn post(form: Form<Message>, queue: &State<Sender<Message>>) {
    // Broadcast the received message to all subscribers by sending it to the broadcast queue
    let _res = queue.send(form.into_inner());
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        // Create a broadcast channel to send messages between components of the application
        .manage(channel::<Message>(1024).0)
        // Mount the routes for handling form submissions and server-sent events
        .mount("/", routes![post, events])
        // Mount the file server to serve static files from the "static" directory
        .mount("/", FileServer::from(relative!("static")))
}
