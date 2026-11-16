//! Tone metadata/service. Example bodies stay wire-only until01/question1; no internal entities model.
use super::{
    data_download_manager::{send_request, Reply},
    data_peer_id::UserId,
};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use tg_base::timer::{Timer, TimerHost};
use tg_mtproto::mtproto_response::Error;
use tg_rpl::{EventStream, Producer};
use tg_scheme as tl;
#[derive(Clone, Default)]
pub struct AiComposeTone {
    pub id: u64,
    pub access_hash: u64,
    pub slug: String,
    pub title: String,
    pub emoji_id: u64,
    pub prompt: String,
    pub installs_count: i32,
    pub author_id: UserId,
    pub creator: bool,
    pub is_default: bool,
    pub default_type: String,
    pub first_example_wire: Option<tl::AiComposeToneExample>,
}
pub fn parse_tone(v: &tl::AiComposeTone) -> AiComposeTone {
    match v {
        tl::AiComposeTone::AiComposeTone(d) => AiComposeTone {
            id: d.id,
            access_hash: d.access_hash,
            slug: String::from_utf8_lossy(&d.slug).into(),
            title: String::from_utf8_lossy(&d.title).into(),
            emoji_id: d.vemoji_id().unwrap_or_default(),
            prompt: String::from_utf8_lossy(d.vprompt().map_or(&[], Vec::as_slice)).into(),
            installs_count: d.vinstalls_count().unwrap_or_default(),
            author_id: UserId::new(d.vauthor_id().unwrap_or_default()),
            creator: d.is_creator(),
            first_example_wire: d.vexample_english().cloned(),
            ..Default::default()
        },
        tl::AiComposeTone::AiComposeToneDefault(d) => AiComposeTone {
            title: String::from_utf8_lossy(&d.title).into(),
            emoji_id: d.emoji_id,
            is_default: true,
            default_type: String::from_utf8_lossy(&d.tone).into(),
            ..Default::default()
        },
    }
}
pub fn tone_to_mtp(t: &AiComposeTone) -> tl::InputAiComposeTone {
    if t.is_default {
        tl::mtp_input_ai_compose_tone_default(t.default_type.as_bytes().to_vec())
    } else {
        tl::mtp_input_ai_compose_tone_id(t.id, t.access_hash)
    }
}
#[derive(Clone)]
pub enum Request {
    Refresh(tl::AicomposeGetTones),
    Create(tl::AicomposeCreateTone),
    Update(tl::AicomposeUpdateTone),
    Save(tl::AicomposeSaveTone),
    Remove(tl::AicomposeDeleteTone),
    Resolve(tl::AicomposeGetTone),
    Example(tl::AicomposeGetToneExample),
}
#[derive(Clone)]
pub enum Effect {
    Send {
        id: i32,
        request: Request,
    },
    ProcessUsers(Vec<tl::User>),
    ToneDone {
        id: i32,
        tone: AiComposeTone,
    },
    Done(i32),
    Failed {
        id: i32,
        error: Error,
    },
    ExampleWireDone {
        id: i32,
        example: tl::AiComposeToneExample,
    },
}
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum PendingRefresh {
    #[default]
    None,
    Incremental,
    Full,
}
#[derive(Clone)]
enum Operation {
    Create,
    Update,
    Save(AiComposeTone, bool),
    Remove(AiComposeTone),
    Resolve,
    Example,
}
#[derive(Default)]
struct State {
    hash: u64,
    refresh: i32,
    pending: PendingRefresh,
    list: Vec<AiComposeTone>,
    recent: Vec<u64>,
    operations: BTreeMap<i32, Operation>,
}
pub struct AiComposeTones {
    state: RefCell<State>,
    effect: Rc<dyn Fn(Effect)>,
    updates: EventStream<()>,
    timer: Timer,
}
impl AiComposeTones {
    pub fn new(effect: Rc<dyn Fn(Effect)>, host: &Rc<TimerHost>) -> Rc<Self> {
        let s = Rc::new_cyclic(|w: &std::rc::Weak<Self>| {
            let w = w.clone();
            Self {
                state: RefCell::default(),
                effect,
                updates: EventStream::new(),
                timer: Timer::in_host(
                    host,
                    Some(Rc::new(move || {
                        if let Some(s) = w.upgrade() {
                            s.refresh()
                        }
                    })),
                ),
            }
        });
        s.refresh();
        s.timer.call_each(3600 * 1000);
        s
    }
    pub fn list(&self) -> Vec<AiComposeTone> {
        self.state.borrow().list.clone()
    }
    pub fn updated(&self) -> Producer<()> {
        self.updates.events()
    }
    pub fn refresh(&self) {
        let hash = self.state.borrow().hash;
        self.refresh_with_hash(hash)
    }
    pub fn apply_update(&self) {
        self.refresh()
    }
    pub fn refresh_with_hash(&self, hash: u64) {
        let id = {
            let mut s = self.state.borrow_mut();
            if s.refresh != 0 {
                if hash == 0 {
                    s.pending = PendingRefresh::Full
                } else if s.pending == PendingRefresh::None {
                    s.pending = PendingRefresh::Incremental
                }
                return;
            }
            let id = tg_mtproto::mtp_instance::get_next_request_id();
            s.refresh = id;
            id
        };
        (self.effect)(Effect::Send {
            id,
            request: Request::Refresh(tl::AicomposeGetTones::new(hash)),
        });
    }
    pub fn refresh_reply(&self, result: Result<tl::AicomposeTones, Error>) {
        self.state.borrow_mut().refresh = 0;
        if let Ok(tl::AicomposeTones::AicomposeTones(d)) = result {
            (self.effect)(Effect::ProcessUsers(d.users.clone()));
            {
                let mut s = self.state.borrow_mut();
                s.hash = d.hash;
                s.list = d.tones.iter().map(parse_tone).collect();
                Self::reapply(&mut s);
            }
            self.updates.fire(())
        }
        self.finish_refresh();
    }
    pub fn finish_refresh(&self) {
        let next = {
            let mut s = self.state.borrow_mut();
            let p = std::mem::take(&mut s.pending);
            match p {
                PendingRefresh::None => None,
                PendingRefresh::Full => Some(0),
                PendingRefresh::Incremental => Some(s.hash),
            }
        };
        if let Some(h) = next {
            self.refresh_with_hash(h)
        }
    }
    fn reapply(s: &mut State) {
        let mut list = vec![];
        let mut recent = vec![];
        for id in &s.recent {
            if let Some(t) = s.list.iter().find(|t| t.id == *id && !t.is_default) {
                list.push(t.clone());
                recent.push(*id)
            }
        }
        for t in &s.list {
            if t.is_default || !recent.contains(&t.id) {
                list.push(t.clone())
            }
        }
        s.recent = recent;
        s.list = list;
    }
    fn remove_custom(s: &mut State, id: u64) {
        if let Some(i) = s.list.iter().position(|t| t.id == id) {
            s.list.remove(i);
        }
    }
    fn forget(s: &mut State, id: u64) {
        if let Some(i) = s.recent.iter().position(|x| *x == id) {
            s.recent.remove(i);
        }
    }
    fn promote(s: &mut State, t: AiComposeTone) {
        if t.is_default {
            return;
        }
        Self::remove_custom(s, t.id);
        Self::forget(s, t.id);
        s.recent.insert(0, t.id);
        s.list.insert(0, t);
    }
    fn request(&self, op: Operation, request: Request) -> i32 {
        let id = tg_mtproto::mtp_instance::get_next_request_id();
        self.state.borrow_mut().operations.insert(id, op);
        (self.effect)(Effect::Send { id, request });
        id
    }
    pub fn create(
        &self,
        title: String,
        prompt: String,
        emoji_id: u64,
        display_author: bool,
    ) -> i32 {
        let flags = if display_author {
            tl::AicomposeCreateTone::F_DISPLAY_AUTHOR
        } else {
            Default::default()
        };
        self.request(
            Operation::Create,
            Request::Create(tl::AicomposeCreateTone::new(
                flags,
                emoji_id,
                title.into_bytes(),
                prompt.into_bytes(),
            )),
        )
    }
    pub fn update(
        &self,
        tone: &AiComposeTone,
        title: Option<String>,
        prompt: Option<String>,
        emoji_id: Option<u64>,
        display_author: Option<bool>,
    ) -> i32 {
        use tl::AicomposeUpdateTone as R;
        let mut flags = Default::default();
        if display_author.is_some() {
            flags |= R::F_DISPLAY_AUTHOR
        }
        if emoji_id.is_some() {
            flags |= R::F_EMOJI_ID
        }
        if title.is_some() {
            flags |= R::F_TITLE
        }
        if prompt.is_some() {
            flags |= R::F_PROMPT
        }
        self.request(
            Operation::Update,
            Request::Update(R::new(
                flags,
                tone_to_mtp(tone),
                display_author,
                emoji_id,
                title.map(String::into_bytes),
                prompt.map(String::into_bytes),
            )),
        )
    }
    pub fn save(&self, tone: &AiComposeTone, unsave: bool) -> i32 {
        self.request(
            Operation::Save(tone.clone(), unsave),
            Request::Save(tl::AicomposeSaveTone::new(tone_to_mtp(tone), unsave)),
        )
    }
    pub fn remove(&self, tone: &AiComposeTone) -> i32 {
        self.request(
            Operation::Remove(tone.clone()),
            Request::Remove(tl::AicomposeDeleteTone::new(tone_to_mtp(tone))),
        )
    }
    pub fn resolve(&self, slug: String) -> i32 {
        self.request(
            Operation::Resolve,
            Request::Resolve(tl::AicomposeGetTone::new(
                tl::mtp_input_ai_compose_tone_slug(slug.into_bytes()),
            )),
        )
    }
    pub fn get_tone_example_wire(&self, tone: &AiComposeTone, num: i32) -> i32 {
        self.request(
            Operation::Example,
            Request::Example(tl::AicomposeGetToneExample::new(tone_to_mtp(tone), num)),
        )
    }
    pub fn failed(&self, id: i32, error: Error) {
        let op = self.state.borrow_mut().operations.remove(&id);
        if op.is_some() && !matches!(op, Some(Operation::Remove(_))) {
            (self.effect)(Effect::Failed { id, error })
        }
    }
    pub fn tone_reply(&self, id: i32, wire: &tl::AiComposeTone) {
        let op = self.state.borrow_mut().operations.remove(&id);
        if !matches!(op, Some(Operation::Create | Operation::Update)) {
            return;
        }
        let t = parse_tone(wire);
        {
            let mut s = self.state.borrow_mut();
            Self::promote(&mut s, t.clone());
            s.hash = 0;
        }
        self.updates.fire(());
        (self.effect)(Effect::ToneDone { id, tone: t });
        self.refresh();
    }
    pub fn bool_reply(&self, id: i32) {
        let op = self.state.borrow_mut().operations.remove(&id);
        {
            let mut s = self.state.borrow_mut();
            match op {
                Some(Operation::Save(t, true)) => {
                    Self::remove_custom(&mut s, t.id);
                    Self::forget(&mut s, t.id)
                }
                Some(Operation::Save(t, false)) => Self::promote(&mut s, t),
                Some(Operation::Remove(t)) => {
                    if !t.is_default {
                        Self::remove_custom(&mut s, t.id);
                        Self::forget(&mut s, t.id)
                    }
                }
                _ => return,
            }
            s.hash = 0;
        }
        self.updates.fire(());
        (self.effect)(Effect::Done(id));
        self.refresh();
    }
    pub fn resolve_reply(&self, id: i32, result: &tl::AicomposeTones) {
        let op = self.state.borrow_mut().operations.remove(&id);
        if !matches!(op, Some(Operation::Resolve)) {
            return;
        }
        match result {
            tl::AicomposeTones::AicomposeTones(d) => {
                (self.effect)(Effect::ProcessUsers(d.users.clone()));
                if let Some(t) = d.tones.first() {
                    (self.effect)(Effect::ToneDone {
                        id,
                        tone: parse_tone(t),
                    })
                } else {
                    (self.effect)(Effect::Failed {
                        id,
                        error: Error::local("TONE_NOT_FOUND", "Tone not found."),
                    })
                }
            }
            _ => (self.effect)(Effect::Failed {
                id,
                error: Error::local("TONE_NOT_MODIFIED", "Tone not modified."),
            }),
        }
    }
    pub fn example_reply_wire(&self, id: i32, example: tl::AiComposeToneExample) {
        let op = self.state.borrow_mut().operations.remove(&id);
        if matches!(op, Some(Operation::Example)) {
            (self.effect)(Effect::ExampleWireDone { id, example })
        }
    }
}
pub fn send(sender: &tg_mtproto::sender::Sender, id: i32, r: Request, reply: Reply) {
    match r {
        Request::Refresh(r) => send_request(sender, id, r, reply),
        Request::Create(r) => send_request(sender, id, r, reply),
        Request::Update(r) => send_request(sender, id, r, reply),
        Request::Save(r) => send_request(sender, id, r, reply),
        Request::Remove(r) => send_request(sender, id, r, reply),
        Request::Resolve(r) => send_request(sender, id, r, reply),
        Request::Example(r) => send_request(sender, id, r, reply),
    }
}
