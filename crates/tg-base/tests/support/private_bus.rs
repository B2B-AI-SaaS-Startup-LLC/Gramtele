use std::{
    fs,
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};
use zbus::blocking::Connection;
pub struct PrivateBus {
    child: Child,
    pub address: String,
    pub root: PathBuf,
}
impl PrivateBus {
    pub fn new() -> Self {
        static IDS: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "tg-linux-bus-{}-{}",
            std::process::id(),
            IDS.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(root.join("services")).unwrap();
        fs::write(
            root.join("services/org.tg.Activatable.service"),
            "[D-BUS Service]\nName=org.tg.Activatable\nExec=/bin/false\n",
        )
        .unwrap();
        let config = root.join("bus.conf");
        fs::write(&config,format!(r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd"><busconfig><type>session</type><listen>unix:tmpdir={}</listen><auth>EXTERNAL</auth><servicedir>{}/services</servicedir><policy context="default"><allow own="*"/><allow send_destination="*"/><allow receive_sender="*"/></policy></busconfig>"#,root.display(),root.display())).unwrap();
        let mut child = Command::new("dbus-daemon")
            .arg(format!("--config-file={}", config.display()))
            .args(["--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("dbus-daemon required; never fall back to the user's session");
        let mut address = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        assert!(address.starts_with("unix:"));
        Self {
            child,
            address: address.trim().into(),
            root,
        }
    }
    pub fn connect(&self) -> Connection {
        zbus::blocking::connection::Builder::address(self.address.as_str())
            .unwrap()
            .build()
            .unwrap()
    }
}
impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.root);
    }
}
