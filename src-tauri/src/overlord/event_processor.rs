
use super::JsEvent;
use nostr_proto::{Event, EventKind, Id, Tag};
use std::collections::{HashMap, HashSet};

pub struct EventProcessor {
    events: HashMap<Id, Event>,
    js_events: HashMap<Id, JsEvent>,
}

macro_rules! get_js_event {
    ($self:ident, $id:expr) => {
        $self.js_events.entry($id)
            .or_insert(JsEvent::new($id.into()))
    }
}

impl EventProcessor {
    pub fn new() -> EventProcessor {
        EventProcessor {
            events: HashMap::new(),
            js_events: HashMap::new(),
        }
    }

    /// This adds events, creating a new JsEvent
    /// and possible alters existing JsEventMetadata as well.
    ///
    /// It returns all the `JsEventMetadata` records it created or modified.
    pub fn add_events(&mut self, events: &[Event]) -> Vec<JsEvent>
    {
        let mut changed_ids: HashSet<Id> = HashSet::new();

        for event in events {
            for id in self.add_event(event) {
                changed_ids.insert(id);
            }
        }

        // Turn changed_ids into js_event copies for sending to javascript
        let mut output: Vec<JsEvent> = Vec::with_capacity(changed_ids.len());
        for id in &changed_ids {
            if let Some(jse) = self.js_events.get(id) {
                output.push(jse.to_owned())
            }
        }
        output
    }

    pub fn get_js_events(&self) -> Vec<JsEvent> {
        self.js_events.iter().map(|(_,e)| e.clone()).collect()
    }

    pub fn get_feed(&self) -> Vec<String> {

        let mut feed: Vec<JsEvent> = self.js_events.iter()
            .map(|(_,e)| e)
            .filter(|e| e.created_at.is_some()) // only if we have the event
            .filter(|e| e.kind == Some(1)) // only text notes in feeds
            .filter(|e| e.in_reply_to.is_none()) // only root events
            .map(|e| e.clone())
            .collect();
        log::info!("New feed, length={} (of {} events)", feed.len(), self.js_events.len());
        feed.sort_unstable_by(|a,b| a.created_at.cmp(&b.created_at));
        feed.iter().map(|e| e.id.0.clone()).collect()
    }

    fn add_event(&mut self, event: &Event) -> Vec<Id>
    {
        // Insert the event
        self.events.insert(event.id, event.clone());

        // Set the main js_event event data
        {
            let js_event: JsEvent = From::from(event);
            get_js_event!(self, event.id).set_main_event_data(js_event);
        }

        // Keep IDs for what has changed
        let mut changed: Vec<Id> = Vec::new();

        // Some kinds seen in the wild:
        //    nonce, p, e, t, client, content-warning,
        //    subject, h, i, nostril, r, hashtag
        for tag in event.tags.iter() {

            // Get some metadata from tags that could apply to multiple
            // kinds of events
            match tag {
                Tag::Event { id, recommended_relay_url: _, marker } => {
                    if event.kind == EventKind::TextNote {
                        if let Some(m) = marker {
                            if m=="reply" {
                                get_js_event!(self, event.id).in_reply_to = Some((*id).into());
                                get_js_event!(self, *id).replies.push(event.id.into());
                                changed.push(event.id);
                                changed.push(*id);
                            }
                        }
                    }
                    else if event.kind == EventKind::EventDeletion {
                        // If we have the event this refers to
                        if let Some(other_event) = self.events.get(&id) {
                            // Make sure the authors match
                            if other_event.pubkey != event.pubkey {
                                // Invalid delete event
                                self.js_events.remove(&event.id);
                                self.events.remove(&event.id);
                                return vec![];
                            }
                            get_js_event!(self, *id).deleted_reason = Some(event.content.clone());
                            changed.push(*id);
                        } else {
                            // FIXME - currently we don't apply this deletion event
                            // if we don't have the event it refers to because we cannot
                            // check that the authors match.
                            // but if we get the event it refers to later, nothing will
                            // trigger us to reapply it.
                        }
                    }
                },
                Tag::Pubkey { .. } => {
                    // Maybe we can generally handle these?
                    // Maybe it is too specific to certain event types.
                    // For now we process these under specific event types.
                },
                Tag::Hashtag(s) => {
                    get_js_event!(self, event.id).hashtags.push(s.to_string());
                    changed.push(event.id);
                },
                Tag::Reference(r) => {
                    get_js_event!(self, event.id).urls.push(r.to_string());
                    changed.push(event.id);
                },
                Tag::Geohash(_) => { }, // not implemented
                Tag::Subject(s) => {
                    get_js_event!(self, event.id).subject = Some(s.to_string());
                    changed.push(event.id);
                },
                Tag::Nonce { .. } => { }, // not implemented
                Tag::Other { tag, data } => {
                    if tag=="client"  && data.len() > 0 {
                        get_js_event!(self, event.id).client = Some(data[0].to_string());
                        changed.push(event.id);
                    }
                },
                Tag::Empty => { }, // nothing to do
            }
        }

        changed
    }
}
