use wayland_client::{protocol::wl_registry, Dispatch, Connection, QueueHandle};

// see https://github.com/Smithay/wayland-rs/blob/master/wayland-client/examples/list_globals.rs
// Yeah this code is not mine


// This struct represents the state of our app. This simple app does not
// need any state, by this type still supports the `Dispatch` implementations.
pub struct EmptyAppData;

// Implement `Dispatch<WlRegistry, ()> for out state. This provides the logic
// to be able to process events for the wl_registry interface.
//
// The second type parameter is the user-data of our implementation. It is a
// mechanism that allows you to associate a value to each particular Wayland
// object, and allow different dispatching logic depending on the type of the
// associated value.
//
// In this example, we just use () as we don't have any value to associate. See
// the `Dispatch` documentation for more details about this.
impl Dispatch<wl_registry::WlRegistry, ()> for EmptyAppData {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<EmptyAppData>,
    ) {
        // When receiving events from the wl_registry, we are only interested in the
        // `global` event, which signals a new available global.
        // When receiving this event, we just print its characteristics in this example.
        if let wl_registry::Event::Global { name, interface, version } = event {
            println!("[{}] {} (v{})", name, interface, version);
        }
    }
}