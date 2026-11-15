use super::{base_linux_dbus_utilities as bus, base_linux_library as gio};
use crate::platform::base_platform_url_scheme::UrlSchemeDescriptor;
use zbus::blocking::Connection;
/// KShell NoOptions subset: only literal space separates words; no expansion.
pub fn split_args(input: &str) -> Vec<String> {
    split(input).unwrap_or_default()
}
fn split(input: &str) -> Option<Vec<String>> {
    let chars = input.chars().collect::<Vec<_>>();
    let mut i = 0;
    let mut out = vec![];
    while i < chars.len() {
        while chars.get(i) == Some(&' ') {
            i += 1;
        }
        if i == chars.len() {
            break;
        }
        let mut word = String::new();
        while i < chars.len() && chars[i] != ' ' {
            let c = chars[i];
            i += 1;
            match c {
                '\'' => {
                    while *chars.get(i)? != '\'' {
                        word.push(chars[i]);
                        i += 1;
                    }
                    i += 1;
                }
                '"' => loop {
                    let c = *chars.get(i)?;
                    i += 1;
                    if c == '"' {
                        break;
                    }
                    if c == '\\' {
                        let c = *chars.get(i)?;
                        i += 1;
                        if c != '"' && c != '\\' {
                            word.push('\\');
                        }
                        word.push(c);
                    } else {
                        word.push(c);
                    }
                },
                '$' if chars.get(i) == Some(&'\'') => {
                    i += 1;
                    loop {
                        let c = *chars.get(i)?;
                        i += 1;
                        if c == '\'' {
                            break;
                        }
                        if c != '\\' {
                            word.push(c);
                            continue;
                        }
                        let c = *chars.get(i)?;
                        i += 1;
                        match c {
                            'a' => word.push('\x07'),
                            'b' => word.push('\x08'),
                            'e' => word.push('\x1b'),
                            'f' => word.push('\x0c'),
                            'n' => word.push('\n'),
                            'r' => word.push('\r'),
                            't' => word.push('\t'),
                            '\\' | '\'' => word.push(c),
                            'c' => {
                                let c = *chars.get(i)?;
                                i += 1;
                                word.push(char::from(
                                    (if c as u32 <= 255 { c as u8 } else { 0 }) & 31,
                                ));
                            }
                            'x' => {
                                let mut n = chars.get(i)?.to_digit(16)?;
                                i += 1;
                                if let Some(next) = chars.get(i) {
                                    if let Some(v) = next.to_digit(16).filter(|v| *v > 0) {
                                        n = n * 16 + v;
                                        i += 1;
                                    }
                                    word.push(char::from_u32(n)?);
                                }
                            }
                            '0'..='7' => {
                                let mut n = c.to_digit(8)?;
                                for _ in 0..2 {
                                    if let Some(v) = chars.get(i).and_then(|c| c.to_digit(8)) {
                                        n = n * 8 + v;
                                        i += 1;
                                    } else {
                                        break;
                                    }
                                }
                                word.push(char::from_u32(n)?);
                            }
                            _ => {
                                word.push('\\');
                                word.push(c);
                            }
                        }
                    }
                }
                '\\' => {
                    word.push(*chars.get(i)?);
                    i += 1;
                }
                _ => word.push(c),
            }
        }
        out.push(word);
    }
    Some(out)
}
pub fn quote_arg(arg: &str) -> String {
    let s = arg.chars().filter(|c| !c.is_control()).collect::<String>();
    if s.is_empty() {
        return "''".into();
    }
    if s.chars()
        .any(|c| c == ' ' || "\\'\"$`<>|;&(){}*?#!~[]".contains(c))
    {
        format!("'{}'", s.replace('\'', "'\\''"))
    } else {
        s
    }
}
pub fn join_args(args: &[String]) -> String {
    args.iter()
        .map(|s| quote_arg(s))
        .collect::<Vec<_>>()
        .join(" ")
}
pub fn commandline(descriptor: &UrlSchemeDescriptor, with_url: bool) -> String {
    let mut args = vec![descriptor.executable.clone()];
    args.extend(split_args(&descriptor.arguments));
    args.push("--".into());
    if with_url {
        args.push("%u".into());
    }
    join_args(&args)
}
pub fn handler_type(descriptor: &UrlSchemeDescriptor) -> String {
    format!("x-scheme-handler/{}", descriptor.protocol)
}
pub fn check_url_scheme(descriptor: &UrlSchemeDescriptor) -> bool {
    gio::gio_check_url(&handler_type(descriptor), &commandline(descriptor, false))
}
pub fn unregister_url_scheme(descriptor: &UrlSchemeDescriptor) {
    gio::gio_unregister_url(&handler_type(descriptor), &commandline(descriptor, true));
}
pub fn snap_default_handler(
    connection: &Connection,
    protocol: &str,
    executable_name: &str,
) -> bus::Result<()> {
    let interface = bus::proxy(
        connection,
        "io.snapcraft.Settings",
        "/io/snapcraft/Settings",
        "io.snapcraft.Settings",
    )?;
    let current: String = interface.call("GetSub", &("default-url-scheme-handler", protocol))?;
    let expected = format!("{executable_name}.desktop");
    if current != expected {
        interface.call::<_, _, ()>(
            "SetSub",
            &("default-url-scheme-handler", protocol, expected),
        )?;
    }
    Ok(())
}
#[derive(Clone)]
pub struct UrlSchemeContext {
    pub is_snap: bool,
    pub desktop_file_name: String,
    pub executable_name: String,
    pub connection: Option<Connection>,
}
/// Context supplies actual QGuiApplication/Integration facts; no global Qt stand-in.
pub fn register_url_scheme(descriptor: &UrlSchemeDescriptor, context: &UrlSchemeContext) {
    if context.is_snap {
        let connection = context.connection.clone();
        let name = context.executable_name.clone();
        let protocol = descriptor.protocol.clone();
        tg_runtime::async_(move || {
            if let Ok(connection) = connection.map_or_else(Connection::session, Ok) {
                let _ = snap_default_handler(&connection, &protocol, &name);
            }
        });
        return;
    }
    if check_url_scheme(descriptor) {
        return;
    }
    unregister_url_scheme(descriptor);
    gio::gio_register_url(
        &handler_type(descriptor),
        &commandline(descriptor, false),
        &context.desktop_file_name,
        &descriptor.display_app_name,
    );
}
