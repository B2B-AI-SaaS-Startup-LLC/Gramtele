use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    rc::Rc,
};
use tg_base::platform::linux::base_linux_xcb_library::{Connection, Event};
pub struct PrivateX11 {
    child: Child,
    pub display: String,
}
impl PrivateX11 {
    pub fn new() -> Self {
        let mut child = Command::new("Xvfb")
            .args([
                "-displayfd",
                "1",
                "-screen",
                "0",
                "640x480x24",
                "-nolisten",
                "tcp",
                "-ac",
            ])
            .env_remove("DISPLAY")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("isolated Xvfb required");
        let mut display = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut display)
            .unwrap();
        let number = display
            .trim()
            .parse::<u32>()
            .expect("Xvfb readiness/displayfd");
        Self {
            child,
            display: format!(":{number}"),
        }
    }
    pub fn terminate(&mut self) {
        self.child.kill().unwrap();
        self.child.wait().unwrap();
    }
    pub fn connect(&self) -> Rc<Connection> {
        Rc::new(Connection::connect(Some(&self.display)).expect("actual private XCB connection"))
    }
}
impl Drop for PrivateX11 {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
pub fn create_window(connection: &Connection) -> u32 {
    let window = connection.generate_id().unwrap();
    let root = connection.root_window().unwrap();
    let mut body = vec![0u8; 32];
    body[4..8].copy_from_slice(&window.to_ne_bytes());
    body[8..12].copy_from_slice(&root.to_ne_bytes());
    body[16..18].copy_from_slice(&100u16.to_ne_bytes());
    body[18..20].copy_from_slice(&100u16.to_ne_bytes());
    body[22..24].copy_from_slice(&1u16.to_ne_bytes());
    assert!(connection.send_raw_core(1, &body, true).is_some());
    window
}
pub fn set_selection(connection: &Connection, owner: u32, atom: u32) {
    let mut body = [0u8; 16];
    body[4..8].copy_from_slice(&owner.to_ne_bytes());
    body[8..12].copy_from_slice(&atom.to_ne_bytes());
    assert!(connection.send_raw_core(22, &body, true).is_some());
}
pub fn xtest_key(connection: &Connection, key: u8, down: bool) {
    let (opcode, _, _) = connection.extension("XTEST").unwrap();
    let mut body = [0u8; 36];
    body[1] = 2;
    body[4] = if down { 2 } else { 3 };
    body[5] = key;
    // Use the actual extension major opcode through a core raw request header.
    assert!(connection.send_raw_core(opcode, &body, true).is_some());
}
pub fn key_event(kind: u8, code: u8, state: u16) -> Event {
    let mut bytes = [0u8; 32];
    bytes[0] = kind;
    bytes[1] = code;
    bytes[28..30].copy_from_slice(&state.to_ne_bytes());
    Event(bytes)
}
