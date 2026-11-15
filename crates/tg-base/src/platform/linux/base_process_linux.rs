/// Pure EWMH payload; actual XCB map/restack/send and QWindow focus remain part2/5.
pub fn activation_message(timestamp: u32, focused_window: Option<u32>) -> [u32; 5] {
    [1, timestamp, focused_window.unwrap_or(0), 0, 0]
}
