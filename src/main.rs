use zellij_tile::prelude::*;

use std::collections::BTreeMap;

const PIPE_NAME: &str = "switch_session";
const NEXT_PAYLOAD: &str = "next";
const PREV_PAYLOAD: &str = "prev";

macro_rules! log{
    ($($arg:tt)*) => {
        eprintln!(
            "[ZjSm {}:{}] {}",
            file!(),
            line!(),
            format_args!($($arg)*)
        )
    };
}

struct ZjSm {
    curr_session: String,
    sessions: Vec<String>,
    cached_pipe_msgs: Vec<PipeMessage>,
    pending_events: Vec<Event>,
    got_permission: bool,
}

impl Default for ZjSm {
    fn default() -> Self {
        log!("Launching Plugin");
        Self {
            curr_session: Default::default(),
            sessions: Default::default(),
            cached_pipe_msgs: Default::default(),
            pending_events: Default::default(),
            got_permission: Default::default(),
        }
    }
}

register_plugin!(ZjSm);

impl ZjSm {
    fn handle_pipe(&mut self, pipe_message: PipeMessage) {
        log!("{:?}", pipe_message);
        match pipe_message.source {
            PipeSource::Cli(_) | PipeSource::Keybind => {
                if pipe_message.name == PIPE_NAME {
                    if let Some(payload) = pipe_message.payload {
                        match payload.as_str() {
                            NEXT_PAYLOAD => match self.switch_session(true) {
                                Ok(_) => log!("Changed to next session"),
                                Err(e) => log!("{:?}", e),
                            },
                            PREV_PAYLOAD => match self.switch_session(false) {
                                Ok(_) => log!("Changed to prev session"),
                                Err(e) => log!("{:?}", e),
                            },
                            _ => {}
                        }
                    }
                } else {
                    log!("Unknown pipe!");
                }
            }
            PipeSource::Plugin(_) => {}
        }
    }

    fn handle_event(&mut self, event: Event) -> bool {
        match event {
            Event::SessionUpdate(session_infos, _) => {
                self.sessions = session_infos.iter().map(|s| s.name.clone()).collect();
                self.curr_session = session_infos
                    .into_iter()
                    .find(|s| s.is_current_session)
                    .map(|session_info| session_info.name)
                    .expect("Should be able to find current session");
            }
            Event::PermissionRequestResult(PermissionStatus::Granted) => {
                if !self.got_permission {
                    log!("Got permissions inside handler!");
                    self.got_permission = true;
                }
            }
            e => {
                log!("Not interested in event: {:?}", e);
            }
        }
        false
    }

    fn switch_session(&self, forward: bool) -> anyhow::Result<()> {
        log!("Switch session called");
        if self.sessions.len() < 2 {
            bail!("Not enough sessions: {:?}", self.sessions);
        }

        if let Some(curr_idx) = self
            .sessions
            .iter()
            .position(|sess| sess == &self.curr_session)
        {
            log!("Curr idx: {}", curr_idx);
            let len = self.sessions.len();
            let next_idx = if forward {
                (curr_idx + 1) % len
            } else {
                (curr_idx + len - 1) % len
            };
            log!("Next idx: {}", next_idx);
            let next_session_name = &self.sessions[next_idx];
            log!("Next Session: {}", next_session_name);
            switch_session(Some(next_session_name));
        } else {
            bail!("Cannot find current session index");
        }

        Ok(())
    }
}

impl ZellijPlugin for ZjSm {
    fn load(&mut self, _configuration: BTreeMap<String, String>) {
        let events = [EventType::SessionUpdate, EventType::PermissionRequestResult];
        let permissions = [
            PermissionType::ReadApplicationState,
            PermissionType::ReadCliPipes,
            PermissionType::ChangeApplicationState,
        ];
        log!("Requesting permissions for: {:?}", permissions);
        request_permission(&permissions);
        log!("Subcribing events: {:?}", events);
        subscribe(&events);
    }
    fn update(&mut self, event: Event) -> bool {
        if let Event::PermissionRequestResult(PermissionStatus::Granted) = event {
            log!("Got permissions!");
            self.got_permission = true;

            while !self.pending_events.is_empty() {
                if let Some(ev) = self.pending_events.pop() {
                    self.handle_event(ev);
                }
            }
        }
        if !self.got_permission {
            self.pending_events.push(event);
            log!(
                "Haven't gotten permission yet! Pending event: {}",
                self.pending_events.len()
            );
            return false;
        }
        self.handle_event(event)
    }
    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        log!("Recieved: {:?}", pipe_message);
        if !self.got_permission || self.sessions.is_empty() {
            log!(
                "Permissions: {}, SessionList empty: {}. Caching pipe messages",
                self.got_permission,
                self.sessions.is_empty()
            );
            self.cached_pipe_msgs.push(pipe_message);
            return false;
        } else {
            while !self.cached_pipe_msgs.is_empty() {
                if let Some(pipe_msg) = self.cached_pipe_msgs.pop() {
                    self.handle_pipe(pipe_msg);
                }
            }
        }
        self.handle_pipe(pipe_message);
        false
    }
    fn render(&mut self, _rows: usize, _cols: usize) {}
}
