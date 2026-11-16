//! Cloud theme value and service; Window/preview/pixel operations are explicit effects5/6.
use super::{
    data_peer_colors::Color,
    data_peer_id::UserId,
    data_star_gift::UniqueGift,
    data_store_keys::*,
    data_wall_paper::{color_from_string, qt_int, WallPaper},
};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::{Rc, Weak},
    sync::Arc,
};
use tg_base::timer::{Timer, TimerHost};
use tg_rpl::{EventStream, Producer};
use tg_scheme as tl;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CloudThemeType {
    Dark,
    Light,
}
#[derive(Clone, Debug, Default)]
pub struct Settings {
    pub paper: Option<WallPaper>,
    pub accent_color: Color,
    pub outgoing_accent_color: Option<Color>,
    pub outgoing_messages_colors: Vec<Color>,
}
#[derive(Clone, Debug, Default)]
pub struct CloudTheme {
    pub id: u64,
    pub access_hash: u64,
    pub slug: String,
    pub title: String,
    pub document_id: u64,
    pub created_by: UserId,
    pub users_count: i32,
    pub emoticon: String,
    pub unique: Option<Rc<UniqueGift>>,
    pub settings: BTreeMap<CloudThemeType, Settings>,
}
/// Actual Session/Document/Api::FromTL and emoji index facts. No defaults.
pub trait ThemeContext {
    fn user_id(&self) -> UserId;
    fn process_document(&self, document: &tl::Document) -> u64;
    fn wall_paper(&self, wire: &tl::WallPaper) -> Option<WallPaper>;
    fn unique_gift(&self, wire: &tl::StarGift) -> Option<Rc<UniqueGift>>;
    fn emoji_key(&self, text: &str) -> Option<Vec<u16>>;
}
fn parse_settings(
    ctx: &dyn ThemeContext,
    v: &[tl::ThemeSettings],
) -> BTreeMap<CloudThemeType, Settings> {
    let mut map = BTreeMap::new();
    for tl::ThemeSettings::ThemeSettings(d) in v {
        let kind = if matches!(
            d.base_theme,
            tl::BaseTheme::BaseThemeNight(_) | tl::BaseTheme::BaseThemeTinted(_)
        ) {
            CloudThemeType::Dark
        } else {
            CloudThemeType::Light
        };
        let settings = Settings {
            paper: d.vwallpaper().and_then(|p| ctx.wall_paper(p)),
            accent_color: Color::rgb(d.accent_color as u32),
            outgoing_accent_color: d.voutbox_accent_color().map(|c| Color::rgb(c as u32)),
            outgoing_messages_colors: d.vmessage_colors().map_or_else(Vec::new, |v| {
                v.iter().map(|c| Color::rgb(*c as u32)).collect()
            }),
        };
        map.entry(kind).or_insert(settings); // C++ emplace: first type wins; every input is still parsed.
    }
    map
}
impl CloudTheme {
    pub fn parse(ctx: &dyn ThemeContext, wire: &tl::Theme, settings: bool) -> Self {
        let tl::Theme::Theme(d) = wire;
        let document_id = d.vdocument().map_or(0, |v| ctx.process_document(v));
        Self {
            id: d.id,
            access_hash: d.access_hash,
            slug: String::from_utf8_lossy(&d.slug).into(),
            title: String::from_utf8_lossy(&d.title).into(),
            document_id,
            created_by: if d.is_creator() {
                ctx.user_id()
            } else {
                UserId::new(0)
            },
            users_count: d.vinstalls_count().unwrap_or_default(),
            emoticon: String::from_utf8_lossy(d.vemoticon().map_or(&[], Vec::as_slice)).into(),
            unique: None,
            settings: if settings {
                parse_settings(ctx, d.vsettings().map_or(&[], Vec::as_slice))
            } else {
                BTreeMap::new()
            },
        }
    }
    pub fn parse_gift(
        ctx: &dyn ThemeContext,
        d: &tl::ChatThemeUniqueGiftData,
        settings: bool,
    ) -> Self {
        let Some(g) = ctx.unique_gift(&d.gift) else {
            return Self::default();
        };
        Self {
            id: g.id,
            unique: Some(g),
            settings: if settings {
                parse_settings(ctx, &d.theme_settings)
            } else {
                BTreeMap::new()
            },
            ..Default::default()
        }
    }
}
pub fn same_paper(a: &Option<WallPaper>, b: &Option<WallPaper>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a.key() == b.key(),
        _ => false,
    }
}
pub fn same_theme_settings(
    a: &BTreeMap<CloudThemeType, Settings>,
    b: &BTreeMap<CloudThemeType, Settings>,
) -> bool {
    a.len() == b.len()
        && a.iter().all(|(k, v)| {
            b.get(k).is_some_and(|o| {
                v.accent_color == o.accent_color
                    && v.outgoing_accent_color == o.outgoing_accent_color
                    && v.outgoing_messages_colors == o.outgoing_messages_colors
                    && same_paper(&v.paper, &o.paper)
            })
        })
}
#[derive(Clone)]
pub enum Request {
    Themes(tl::AccountGetThemes),
    ChatThemes(tl::AccountGetChatThemes),
    GiftThemes(tl::AccountGetUniqueGiftChatThemes),
    Get(tl::AccountGetTheme),
    Install(tl::AccountInstallTheme),
    Save(tl::AccountSaveTheme),
}
#[derive(Clone)]
pub enum Effect {
    Send {
        id: i32,
        request: Request,
    },
    Cancel(i32),
    SetCurrentCloud(CloudTheme),
    CheckChatThemeWallPaper,
    ApplyDocument(CloudTheme),
    ApplyChatTheme {
        theme: CloudTheme,
        night: bool,
        owns_paper: bool,
    },
}
pub trait ThemeOwner: ThemeContext {
    fn current_cloud(&self) -> CloudTheme;
    fn night_mode(&self) -> bool;
    fn has_chat_window(&self) -> bool;
    fn chat_theme_owns_paper(&self, theme: &CloudTheme) -> bool;
    fn effect(&self, effect: Effect);
    fn process_users(&self, users: &[tl::User]);
    fn process_chats(&self, chats: &[tl::Chat]);
}
#[derive(Default)]
struct State {
    hash: u64,
    refresh: i32,
    list: Vec<CloudTheme>,
    chat_hash: u64,
    chat_request: i32,
    chat: Vec<CloudTheme>,
    gift_request: i32,
    gift_hash: u64,
    gift_offset: String,
    gift_loaded: bool,
    gifts: BTreeMap<u64, CloudTheme>,
    gift_tokens: Vec<String>,
    installed: [u64; 2],
}
thread_local! {static TESTING:Cell<bool>=const{Cell::new(false)};}
pub struct CloudThemes {
    scope: SessionScopeId,
    alive: Weak<ScopeState>,
    owner: Rc<dyn ThemeOwner>,
    state: RefCell<State>,
    updates: EventStream<()>,
    chat_updates: EventStream<()>,
    gift_updates: EventStream<()>,
    reload: Timer,
}
impl CloudThemes {
    pub fn new(scope: &SessionScope, owner: Rc<dyn ThemeOwner>, host: &Rc<TimerHost>) -> Rc<Self> {
        let s = Rc::new_cyclic(|w: &Weak<Self>| {
            let weak = w.clone();
            Self {
                scope: scope.id(),
                alive: scope.downgrade(),
                owner,
                state: RefCell::default(),
                updates: EventStream::new(),
                chat_updates: EventStream::new(),
                gift_updates: EventStream::new(),
                reload: Timer::in_host(
                    host,
                    Some(Rc::new(move || {
                        if let Some(s) = weak.upgrade() {
                            if s.alive.upgrade().is_some() {
                                s.reload_current()
                            }
                        }
                    })),
                ),
            }
        });
        s.setup_reload();
        s
    }
    pub fn session(&self) -> SessionScopeId {
        self.scope
    }
    pub fn format() -> &'static str {
        "tdesktop"
    }
    fn send(&self, r: Request) -> i32 {
        assert!(self.alive.upgrade().is_some(), "SessionScope ended");
        let id = tg_mtproto::mtp_instance::get_next_request_id();
        self.owner.effect(Effect::Send { id, request: r });
        id
    }
    pub fn testing_colors() -> bool {
        TESTING.with(Cell::get)
    }
    pub fn set_testing_colors(v: bool) {
        TESTING.with(|c| c.set(v))
    }
    pub fn updated(&self) -> Producer<()> {
        self.updates.events()
    }
    pub fn chat_themes_updated(&self) -> Producer<()> {
        self.chat_updates.events()
    }
    pub fn my_gift_themes_updated(&self) -> Producer<()> {
        self.gift_updates.events()
    }
    pub fn list(&self) -> Vec<CloudTheme> {
        self.state.borrow().list.clone()
    }
    pub fn chat_themes(&self) -> Vec<CloudTheme> {
        self.state.borrow().chat.clone()
    }
    pub fn my_gift_themes_tokens(&self) -> Vec<String> {
        self.state.borrow().gift_tokens.clone()
    }
    pub fn my_gift_themes_ready(&self) -> bool {
        let s = self.state.borrow();
        !s.gift_tokens.is_empty() || s.gift_loaded
    }
    pub fn need_reload(&self) -> bool {
        let c = self.owner.current_cloud();
        c.id != 0 && c.document_id != 0
    }
    pub fn setup_reload(&self) {
        if self.need_reload() || !self.owner.current_cloud().emoticon.is_empty() {
            self.reload.call_once(10_000)
        }
    }
    /// Owner binds precisely Window::Theme::BackgroundUpdate::ApplyingTheme.
    pub fn applying_theme(&self) {
        let need = self.need_reload();
        self.install();
        if need {
            self.schedule_reload()
        } else {
            self.reload.cancel()
        }
    }
    pub fn install(&self) {
        let c = self.owner.current_cloud();
        let night = self.owner.night_mode();
        let id = if c.document_id != 0 || !c.emoticon.is_empty() {
            c.id
        } else {
            0
        };
        {
            let mut s = self.state.borrow_mut();
            let slot = usize::from(night);
            if s.installed[slot] == id {
                return;
            }
            s.installed[slot] = id;
        }
        let mut flags = tl::AccountInstallTheme::F_FORMAT;
        if night {
            flags |= tl::AccountInstallTheme::F_DARK
        }
        if id != 0 {
            flags |= tl::AccountInstallTheme::F_THEME
        }
        self.send(Request::Install(tl::AccountInstallTheme::new(
            flags,
            (id != 0).then(|| tl::mtp_input_theme(id, c.access_hash)),
            Some(Self::format().as_bytes().to_vec()),
            None,
        )));
    }
    pub fn schedule_reload(&self) {
        if self.need_reload() {
            self.reload.call_once(3_600_000)
        } else {
            self.reload.cancel()
        }
    }
    pub fn reload_current(&self) {
        if !self.need_reload() {
            let c = self.owner.current_cloud();
            if !c.emoticon.is_empty() && !c.settings.is_empty() {
                self.refresh_chat_themes()
            }
            return;
        }
        let c = self.owner.current_cloud();
        self.send(Request::Get(tl::AccountGetTheme::new(
            Self::format().as_bytes().to_vec(),
            tl::mtp_input_theme(c.id, c.access_hash),
        )));
    }
    pub fn reload_failed(&self) {
        self.reload.call_once(3_600_000)
    }
    pub fn apply_update(&self, wire: &tl::Theme) {
        let c = CloudTheme::parse(self.owner.as_ref(), wire, true);
        let current = self.owner.current_cloud();
        if c.id == current.id {
            if c.document_id != 0 && c.document_id != current.document_id {
                self.owner.effect(Effect::ApplyDocument(c))
            } else if !c.settings.is_empty() {
                self.owner.effect(Effect::SetCurrentCloud(c));
                if self.owner.has_chat_window() {
                    self.owner.effect(Effect::CheckChatThemeWallPaper)
                }
            }
        }
        self.schedule_reload()
    }
    pub fn refresh(&self) {
        let hash = {
            let s = self.state.borrow();
            if s.refresh != 0 {
                return;
            }
            s.hash
        };
        let id = tg_mtproto::mtp_instance::get_next_request_id();
        self.state.borrow_mut().refresh = id;
        self.owner.effect(Effect::Send {
            id,
            request: Request::Themes(tl::AccountGetThemes::new(
                Self::format().as_bytes().to_vec(),
                hash,
            )),
        });
    }
    pub fn refresh_failed(&self) {
        self.state.borrow_mut().refresh = 0
    }
    pub fn themes_reply(&self, wire: &tl::AccountThemes) {
        self.state.borrow_mut().refresh = 0;
        if let tl::AccountThemes::AccountThemes(d) = wire {
            self.state.borrow_mut().hash = d.hash;
            self.state.borrow_mut().list.clear();
            for v in &d.themes {
                let c = CloudTheme::parse(self.owner.as_ref(), v, true);
                self.state.borrow_mut().list.push(c)
            }
            self.check_current_theme();
            self.updates.fire(())
        }
    }
    pub fn refresh_chat_themes(&self) {
        let hash = {
            let s = self.state.borrow();
            if s.chat_request != 0 {
                return;
            }
            s.chat_hash
        };
        let id = tg_mtproto::mtp_instance::get_next_request_id();
        self.state.borrow_mut().chat_request = id;
        self.owner.effect(Effect::Send {
            id,
            request: Request::ChatThemes(tl::AccountGetChatThemes::new(hash)),
        });
    }
    pub fn chat_themes_failed(&self) {
        self.state.borrow_mut().chat_request = 0
    }
    pub fn chat_themes_reply(&self, wire: &tl::AccountThemes) {
        self.state.borrow_mut().chat_request = 0;
        if let tl::AccountThemes::AccountThemes(d) = wire {
            self.state.borrow_mut().chat_hash = d.hash;
            self.state.borrow_mut().chat.clear();
            for v in &d.themes {
                let c = CloudTheme::parse(self.owner.as_ref(), v, true);
                self.state.borrow_mut().chat.push(c)
            }
            self.chat_updates.fire(());
            self.check_applied_chat_theme();
        }
    }
    pub fn saved_from_editor(&self, c: CloudTheme) {
        {
            let mut s = self.state.borrow_mut();
            if let Some(i) = s.list.iter().position(|v| v.id == c.id) {
                s.list[i] = c
            } else {
                s.list.insert(0, c)
            }
        }
        self.updates.fire(())
    }
    pub fn remove(&self, id: u64) {
        let c = self
            .state
            .borrow()
            .list
            .iter()
            .find(|c| c.id == id)
            .cloned();
        if let Some(c) = c {
            self.send(Request::Save(tl::AccountSaveTheme::new(
                tl::mtp_input_theme(c.id, c.access_hash),
                true,
            )));
            let mut s = self.state.borrow_mut();
            if let Some(i) = s.list.iter().position(|c| c.id == id) {
                s.list.remove(i);
            }
            drop(s);
            self.updates.fire(())
        }
    }
    pub fn theme_for_token(&self, token: &str) -> Option<CloudTheme> {
        if let Some(v) = token.strip_prefix("gift:") {
            let id = qt_u64(v).unwrap_or_default();
            return self.state.borrow().gifts.get(&id).cloned();
        }
        let emoji = self.owner.emoji_key(token)?;
        let list = self.state.borrow().chat.clone();
        list.into_iter()
            .find(|c| self.owner.emoji_key(&c.emoticon).as_ref() == Some(&emoji))
    }
    pub fn theme_for_token_value(self: &Rc<Self>, token: String) -> Producer<Option<CloudTheme>> {
        if token.starts_with("gift:") {
            return tg_rpl::single(self.theme_for_token(&token));
        }
        let testing = Self::testing_colors();
        if self.owner.emoji_key(&token).is_none() {
            return tg_rpl::single(None);
        }
        let found = self.theme_for_token(&token);
        if found.is_some() && !testing {
            return tg_rpl::single(found);
        }
        if found.is_none() {
            self.refresh_chat_themes()
        }
        let weak = Rc::downgrade(self);
        let watch = self
            .chat_themes_updated()
            .map(move |_| weak.upgrade().and_then(|s| s.theme_for_token(&token)))
            .filter(Option::is_some);
        if found.is_some() {
            tg_rpl::single(found).then(watch)
        } else {
            tg_rpl::single(None).then(watch.take(if testing { 1 << 20 } else { 1 }))
        }
    }
    pub fn refresh_chat_themes_for(&self, token: &str) {
        if !token.starts_with("gift:") {
            self.refresh_chat_themes()
        }
    }
    pub fn process_gift_theme_get_token(&self, d: &tl::ChatThemeUniqueGiftData) -> String {
        let c = CloudTheme::parse_gift(self.owner.as_ref(), d, true);
        if let Some(g) = &c.unique {
            let id = g.id;
            self.state.borrow_mut().gifts.insert(id, c);
            format!("gift:{id}")
        } else {
            String::new()
        }
    }
    pub fn my_gift_themes_load_more(&self, reload: bool) {
        let old = {
            let mut s = self.state.borrow_mut();
            if reload && !s.gift_tokens.is_empty() {
                std::mem::take(&mut s.gift_request)
            } else {
                0
            }
        };
        if old != 0 {
            self.owner.effect(Effect::Cancel(old))
        }
        let (hash, offset) = {
            let s = self.state.borrow();
            if s.gift_request != 0 || (!reload && s.gift_loaded) {
                return;
            }
            (
                s.gift_hash,
                if reload {
                    String::new()
                } else {
                    s.gift_offset.clone()
                },
            )
        };
        let id = tg_mtproto::mtp_instance::get_next_request_id();
        self.state.borrow_mut().gift_request = id;
        self.owner.effect(Effect::Send {
            id,
            request: Request::GiftThemes(tl::AccountGetUniqueGiftChatThemes::new(
                offset.into_bytes(),
                24,
                hash,
            )),
        });
    }
    pub fn gift_themes_failed(&self) {
        let mut s = self.state.borrow_mut();
        s.gift_request = 0;
        s.gift_loaded = true
    }
    pub fn gift_themes_reply(&self, reload: bool, wire: &tl::AccountChatThemes) {
        self.state.borrow_mut().gift_request = 0;
        match wire {
            tl::AccountChatThemes::AccountChatThemes(d) => {
                {
                    let mut s = self.state.borrow_mut();
                    if reload || s.gift_tokens.is_empty() {
                        s.gift_hash = d.hash;
                        s.gift_tokens.clear();
                        s.gift_loaded = false;
                    }
                }
                self.owner.process_users(&d.users);
                self.owner.process_chats(&d.chats);
                for c in &d.themes {
                    if let tl::ChatTheme::ChatThemeUniqueGift(g) = c {
                        let token = self.process_gift_theme_get_token(g);
                        self.state.borrow_mut().gift_tokens.push(token)
                    }
                }
                {
                    let mut s = self.state.borrow_mut();
                    if let Some(next) = d.vnext_offset() {
                        s.gift_offset = String::from_utf8_lossy(next).into()
                    } else {
                        s.gift_loaded = true
                    }
                }
                self.gift_updates.fire(())
            }
            tl::AccountChatThemes::AccountChatThemesNotModified(_) => {
                if !reload {
                    self.state.borrow_mut().gift_loaded = true;
                    self.gift_updates.fire(())
                }
            }
        }
    }
    pub fn check_current_theme(&self) {
        let c = self.owner.current_cloud();
        if c.id == 0 || c.document_id == 0 {
            return;
        }
        let found = self.state.borrow().list.iter().any(|v| v.id == c.id);
        if !found {
            self.install()
        }
    }
    pub fn check_applied_chat_theme(&self) {
        let c = self.owner.current_cloud();
        if c.emoticon.is_empty() || c.settings.is_empty() {
            return;
        }
        let Some(fresh) = self.theme_for_token(&c.emoticon) else {
            return;
        };
        if !self.owner.has_chat_window() {
            return;
        }
        if !same_theme_settings(&c.settings, &fresh.settings) {
            self.owner.effect(Effect::ApplyChatTheme {
                theme: fresh,
                night: self.owner.night_mode(),
                owns_paper: self.owner.chat_theme_owns_paper(&c),
            })
        } else {
            self.owner.effect(Effect::SetCurrentCloud(fresh));
            self.owner.effect(Effect::CheckChatThemeWallPaper)
        }
    }
}
pub fn qt_u64(s: &str) -> Option<u64> {
    let s = s.trim().strip_prefix('+').unwrap_or(s.trim());
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        None
    } else {
        s.parse().ok()
    }
}
pub fn prepare_testing_link(theme: &CloudTheme) -> String {
    let mut args = vec![];
    let colors = |c: &[Color]| {
        c.iter()
            .map(|c| format!("{:06x}", c.serialized()))
            .collect::<Vec<_>>()
            .join(",")
    };
    for (t, s) in &theme.settings {
        let pre = if *t == CloudThemeType::Dark {
            "dark_"
        } else {
            ""
        };
        args.push(format!("{pre}accent={:06x}", s.accent_color.serialized()));
        if let Some(p) = &s.paper {
            if !p.background_colors().is_empty() {
                args.push(format!("{pre}bg={}", colors(p.background_colors())))
            }
            args.push(format!("{pre}intensity={}", p.pattern_intensity()))
        }
        if let Some(c) = s.outgoing_accent_color {
            args.push(format!("{pre}out_accent{:06x}", c.serialized()))
        }
        if !s.outgoing_messages_colors.is_empty() {
            args.push(format!(
                "{pre}out_bg={}",
                colors(&s.outgoing_messages_colors)
            ))
        }
    }
    if args.is_empty() {
        String::new()
    } else {
        format!("tg://test_chat_theme?{}", args.join("&"))
    }
}
fn testing_color(s: &str) -> Option<Color> {
    let first = s.get(..6)?;
    color_from_string(first)
}
fn testing_colors(s: &str) -> Vec<Color> {
    let c: Option<Vec<_>> = s.split(',').map(testing_color).collect();
    c.filter(|c| c.len() <= 4).unwrap_or_default()
}
impl CloudThemes {
    pub fn update_theme_from_link(
        &self,
        emoticon: &str,
        p: &BTreeMap<String, String>,
    ) -> Option<CloudTheme> {
        if !Self::testing_colors() {
            return None;
        }
        let emoji = self.owner.emoji_key(emoticon)?;
        let list = self.state.borrow().chat.clone();
        let id = list
            .iter()
            .position(|c| self.owner.emoji_key(&c.emoticon).as_ref() == Some(&emoji))?;
        let get = |k: &str| p.get(k).map_or("", String::as_str);
        {
            let mut s = self.state.borrow_mut();
            let theme = &mut s.chat[id];
            let mut parse = |kind| {
                let Some(accent) = testing_color(get("accent")) else {
                    return;
                };
                let v = theme.settings.entry(kind).or_default();
                v.accent_color = accent;
                let bg = testing_colors(get("bg"));
                if !bg.is_empty() {
                    v.paper = v.paper.as_ref().map(|p| p.with_background_colors(bg))
                }
                let n = qt_int(get("intensity")).unwrap_or_default();
                if n != 0 {
                    v.paper = v.paper.as_ref().map(|p| p.with_pattern_intensity(n))
                }
                v.outgoing_accent_color = testing_color(get("out_accent"));
                v.outgoing_messages_colors = testing_colors(get("out_bg"));
            };
            if p.contains_key("dark_accent") {
                parse(CloudThemeType::Dark)
            }
            if p.contains_key("accent") {
                parse(if get("dark").is_empty() {
                    CloudThemeType::Light
                } else {
                    CloudThemeType::Dark
                })
            }
        }
        self.chat_updates.fire(());
        self.state.borrow().chat.get(id).cloned()
    }
}
/// Actual typed Sender bridge; route its reply back to the owning model queue.
pub fn send_request(
    sender: &tg_mtproto::sender::Sender,
    id: i32,
    r: Request,
    reply: Arc<dyn Fn(Result<Vec<i32>, String>) + Send + Sync>,
) {
    macro_rules! send {
        ($r:expr) => {{
            let fail = reply.clone();
            sender
                .request($r)
                .override_id(id)
                .done(move |v| reply(Ok(tl::tl::serialize(&v))))
                .fail(move |e| fail(Err(e.kind.clone())))
                .send();
        }};
    }
    match r {
        Request::Themes(r) => send!(r),
        Request::ChatThemes(r) => send!(r),
        Request::GiftThemes(r) => send!(r),
        Request::Get(r) => send!(r),
        Request::Install(r) => send!(r),
        Request::Save(r) => send!(r),
    }
}
