/// Pure EWMH payload; actual XCB map/restack/send and QWindow focus remain part2/5.
pub fn activation_message(timestamp: u32, focused_window: Option<u32>) -> [u32; 5] {
    [1, timestamp, focused_window.unwrap_or(0), 0, 0]
}

pub fn xcb_activate_window(
    hub: &std::rc::Rc<super::base_linux_xcb_utilities::EventHub>,
    window: u32,
    focused_window: Option<u32>,
) -> bool {
    let connection = hub.connection();
    let Some(root) = connection.root_window() else {
        return false;
    };
    let Some(time) = hub.timestamp() else {
        return false;
    };
    let Some(atom) = connection.atom("_NET_ACTIVE_WINDOW") else {
        return false;
    };
    connection.map_window(window);
    connection.raise_window(window);
    let mut event = super::base_linux_xcb_library::Event([0; 32]);
    event.0[0] = 33;
    event.0[1] = 32;
    event.0[4..8].copy_from_slice(&window.to_ne_bytes());
    event.0[8..12].copy_from_slice(&atom.to_ne_bytes());
    for (i, value) in activation_message(time, focused_window)
        .into_iter()
        .enumerate()
    {
        event.0[12 + i * 4..16 + i * 4].copy_from_slice(&value.to_ne_bytes());
    }
    connection.send_event(root, (1 << 20) | (1 << 19), &event)
}
