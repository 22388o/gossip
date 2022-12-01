use crate::db::{DbEvent, DbEventSeen, DbEventTag, DbPerson, DbPersonRelay, DbRelay};
use crate::{BusMessage, Error, GLOBALS, Settings};
use super::JsEvent;
use futures::{SinkExt, StreamExt};
use http::Uri;
use nostr_proto::{
    ClientMessage, Event, EventKind, Filters, Metadata, PublicKeyHex,
    RelayMessage, SubscriptionId, Unixtime, Url,
};
use tokio::select;
use tokio::net::TcpStream;
use tokio::sync::broadcast::Receiver;
use tokio::sync::mpsc::UnboundedSender;
use tokio_tungstenite::{WebSocketStream, MaybeTlsStream};
use tungstenite::protocol::{Message as WsMessage, WebSocketConfig};

pub struct Minion {
    url: Url,
    pubkeys: Vec<PublicKeyHex>,
    to_overlord: UnboundedSender<BusMessage>,
    from_overlord: Receiver<BusMessage>,
    settings: Settings,
    dbrelay: Option<DbRelay>,
}

impl Minion {
    pub fn new(url: Url, pubkeys: Vec<PublicKeyHex>) -> Minion {
        let to_overlord = GLOBALS.to_overlord.clone();
        let from_overlord = GLOBALS.to_minions.subscribe();

        Minion {
            url, pubkeys, to_overlord, from_overlord,
            settings: Default::default(),
            dbrelay: None,
        }
    }
}

impl Minion {
    pub async fn handle(&mut self) {
        // Catch errors, Return nothing.
        if let Err(e) = self.handle_inner().await {
            log::error!("ERROR handling {}: {}", &self.url, e);
        }

        // Bump the failure count for the relay.
        if let Some(dbrelay) = &mut self.dbrelay {
            dbrelay.failure_count += 1;
            if let Err(e) = DbRelay::update(dbrelay.clone()).await {
                log::error!("ERROR bumping relay failure count {}: {}", &self.url, e);
            }
        }

        // Should we signal that we are exiting?
    }

    async fn handle_inner(&mut self) -> Result<(), Error> {
        log::info!("Task started to handle relay at {}", &self.url);

        // Load settings
        self.settings.load().await?;

        // Compute how far to look back
        let (feed_since, special_since) = {

            // Find the oldest 'last_fetched' among the 'person_relay' table.
            // Null values will come through as 0.
            let mut special_since: i64 = DbPersonRelay::fetch_oldest_last_fetched(
                &self.pubkeys,
                &self.url.0
            ).await? as i64;

            // Subtract overlap to avoid gaps due to clock sync and event
            // propogation delay
            special_since -= self.settings.overlap as i64;

            // For feed related events, don't look back more than one feed_chunk ago
            let one_feedchunk_ago = Unixtime::now().unwrap().0 - self.settings.feed_chunk as i64;
            let feed_since = special_since.max(one_feedchunk_ago);

            (Unixtime(feed_since), Unixtime(special_since))
        };

        if self.pubkeys.len() == 0 {
            // Right now, we can't continue with no people to watch for.
            // Our filters require authors, or else they are asking for EVERYBODY.
            // FIXME better.
            return Ok(());
        }

        // Create the author filter
        let mut feed_filter: Filters = Filters::new();
        for pk in self.pubkeys.iter() {
            feed_filter.add_author(&pk, None);
        }
        feed_filter.add_event_kind(EventKind::TextNote);
        feed_filter.add_event_kind(EventKind::Reaction);
        feed_filter.add_event_kind(EventKind::EventDeletion);
        feed_filter.since = Some(feed_since);
        log::debug!(
            "Feed Filter {}: {}",
            &self.url,
            serde_json::to_string(&feed_filter)?
        );

        // Create the lookback filter
        let mut special_filter: Filters = Filters::new();
        for pk in self.pubkeys.iter() {
            special_filter.add_author(&pk, None);
        }
        special_filter.add_event_kind(EventKind::Metadata);
        //special_filter.add_event_kind(EventKind::RecommendRelay);
        //special_filter.add_event_kind(EventKind::ContactList);
        special_filter.since = Some(special_since);
        log::debug!(
            "Special Filter {}: {}",
            &self.url,
            serde_json::to_string(&special_filter)?
        );

        // Connect to the relay
        let mut websocket_stream = {
            let uri: http::Uri = self.url.0.parse::<Uri>()?;
            let authority = uri.authority().ok_or(Error::UrlHasNoHostname)?.as_str();
            let host = authority
                .find('@')
                .map(|idx| authority.split_at(idx + 1).1)
                .unwrap_or_else(|| authority);
            if host.is_empty() {
                return Err(Error::UrlHasEmptyHostname);
            }

            let key: [u8; 16] = rand::random();

            let req = http::request::Request::builder()
                .method("GET")
                .header("Host", host)
                .header("Connection", "Upgrade")
                .header("Upgrade", "websocket")
                .header("Sec-WebSocket-Version", "13")
                .header("Sec-WebSocket-Key", base64::encode(&key))
                .uri(uri)
                .body(())?;

            let config: WebSocketConfig = WebSocketConfig {
                max_send_queue: None,
                max_message_size: Some(1024*1024*16), // their default is 64 MiB, I choose 16 MiB
                max_frame_size: Some(1024*1024*16), // their default is 16 MiB.
                accept_unmasked_frames: true, // default is false which is the standard
            };

            let (websocket_stream, _response) =
                tokio_tungstenite::connect_async_with_config(req, Some(config)).await?;
            log::info!("Connected to {}", &self.url);

            websocket_stream
        };

        //let (mut write, mut read) = websocket_stream.split();

        // Subscribe to our filters
        // FIXME, get filters in response to an appropriate bus message
        let message = ClientMessage::Req(
            SubscriptionId(format!("gossip-main-{}", textnonce::TextNonce::new())),
            vec![feed_filter, special_filter],
        );
        let wire = serde_json::to_string(&message)?;
        websocket_stream.send(WsMessage::Text(wire.clone())).await?;
        //log::debug!("Sent {}", &wire);

        // Bump the success count for the relay
        {
            let maybe_dbrelay = DbRelay::fetch_one(&self.url).await?;
            if let Some(mut dbrelay) = maybe_dbrelay {
                dbrelay.success_count += 1;
                DbRelay::update(dbrelay.clone()).await?;
                self.dbrelay = Some(dbrelay);
            } else {
                log::error!("Could not load relay to update success count: {}", self.url);
            }
        }

        // Tell the overlord we are ready to receive commands
        self.tell_overlord_we_are_ready().await?;

        'relayloop:
        loop {
            match self.loop_handler(&mut websocket_stream).await {
                Ok(keepgoing) => {
                    if !keepgoing {
                        break 'relayloop;
                    }
                },
                Err(e) => {
                    // Log them and keep going
                    log::error!("{}", e);
                }
            }
        }

        Ok(())
    }

    async fn loop_handler(&mut self,
                          ws_stream: &mut WebSocketStream<MaybeTlsStream<TcpStream>>)
                          -> Result<bool, Error>
    {
        let mut keepgoing: bool = true;

        select! {
            ws_message = ws_stream.next() => {
                let ws_message = ws_message.unwrap()?;
                log::trace!("Handling message from {}", &self.url);
                match ws_message {
                    WsMessage::Text(t) => {
                        self.handle_nostr_message(t).await?;
                        // FIXME: some errors we should probably bail on.
                        // For now, try to continue.
                    },
                    WsMessage::Binary(_) => log::warn!("Unexpected binary message"),
                    WsMessage::Ping(x) => ws_stream.send(WsMessage::Pong(x)).await?,
                    WsMessage::Pong(_) => log::warn!("Unexpected pong message"),
                    WsMessage::Close(_) => keepgoing = false,
                    WsMessage::Frame(_) => log::warn!("Unexpected frame message"),
                }
            },
            bus_message = self.from_overlord.recv() => {
                let bus_message = match bus_message {
                    Ok(bm) => bm,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        return Ok(false);
                    },
                    Err(e) => return Err(e.into())
                };
                if bus_message.target == self.url.0 {
                    log::warn!("Websocket task got message, unimpmented: {}",
                               bus_message.payload);
                } else if &*bus_message.target == "all" {
                    if &*bus_message.kind == "shutdown" {
                        log::info!("Websocket listener {} shutting down", &self.url);
                        keepgoing = false;
                    } else if &*bus_message.kind == "settings_changed" {
                        self.settings = serde_json::from_str(&bus_message.payload)?;
                    }
                }
            },
        }

        Ok(keepgoing)
    }

    async fn handle_nostr_message(
        &self,
        ws_message: String
    ) -> Result<(), Error> {

        // TODO: pull out the raw event without any deserialization to be sure we don't mangle
        //       it.

        let relay_message: RelayMessage = serde_json::from_str(&ws_message)?;

        let mut maxtime = Unixtime::now()?;
        maxtime.0 += 60 * 15; // 15 minutes into the future

        match relay_message {
            RelayMessage::Event(_subid, event) => {
                if let Err(e) = event.verify(Some(maxtime)) {
                    log::error!("VERIFY ERROR: {}, {}", e, serde_json::to_string(&event)?)
                } else {
                    self.save_event_in_database(&event).await?;
                    self.process_event(*event).await?;
                }
            }
            RelayMessage::Notice(msg) => {
                log::info!("NOTICE: {} {}", &self.url, msg);
            }
            RelayMessage::Eose(subid) => {
                // We should update last_fetched
                let now = Unixtime::now().unwrap().0 as u64;
                DbPersonRelay::update_last_fetched(self.url.0.clone(), now).await?;

                // These don't have to be processed.
                log::info!("EOSE: {} {:?}", &self.url, subid);
            }
            RelayMessage::Ok(id, ok, ok_message) => {
                // These don't have to be processed.
                log::info!("OK: {} {:?} {} {}", &self.url, id, ok, ok_message);
            }
        }

        Ok(())
    }

    async fn save_event_in_database(
        &self,
        event: &Event
    ) -> Result<(), Error> {
        let db_event = DbEvent {
            id: event.id.into(),
            raw: serde_json::to_string(&event)?, // TODO: this is reserialized.
            pubkey: event.pubkey.into(),
            created_at: event.created_at.0,
            kind: event.kind.into(),
            content: event.content.clone(),
            ots: event.ots.clone()
        };
        DbEvent::insert(db_event).await?;

        let mut seq = 0;
        for tag in event.tags.iter() {
            // convert to vec of strings
            let v: Vec<String> = serde_json::from_str(&serde_json::to_string(&tag)?)?;

            let db_event_tag = DbEventTag {
                event: event.id.as_hex_string(),
                seq: seq,
                label: v.get(0).cloned(),
                field0: v.get(1).cloned(),
                field1: v.get(2).cloned(),
                field2: v.get(3).cloned(),
                field3: v.get(4).cloned(),
            };
            DbEventTag::insert(db_event_tag).await?;
            seq += 1;
        }

        let db_event_seen = DbEventSeen {
            event: event.id.as_hex_string(),
            relay: self.url.0.clone(),
            when_seen: Unixtime::now()?.0 as u64
        };
        DbEventSeen::replace(db_event_seen).await?;

        Ok(())
    }

    async fn tell_overlord_we_are_ready(
        &self,
    ) -> Result<(), Error> {
        self.to_overlord.send(BusMessage {
            relay_url: Some(self.url.0.clone()),
            target: "overlord".to_string(),
            kind: "minion_is_ready".to_string(),
            payload: "".to_owned(),
        })?;

        Ok(())
    }

    async fn send_overlord_newevent(
        &self,
        event: Event
    ) -> Result<(), Error> {
        let js_event: JsEvent = event.into();
        self.to_overlord.send(BusMessage {
            relay_url: Some(self.url.0.clone()),
            target: "overlord".to_string(),
            kind: "new_event".to_string(),
            payload: serde_json::to_string(&js_event)?,
        })?;
        Ok(())
    }


    async fn send_javascript_setpeople(
        &self,
        people: Vec<DbPerson>
    ) -> Result<(), Error> {
        self.to_overlord.send(BusMessage {
            relay_url: Some(self.url.0.clone()),
            target: "javascript".to_string(),
            kind: "setpeople".to_string(),
            payload: serde_json::to_string(&people)?,
        })?;

        Ok(())
    }

    async fn process_event(
        &self,
        event: Event
    ) -> Result<(), Error> {

        match event.kind {
            EventKind::Metadata => {
                log::debug!("Event(metadata) from {}", &self.url);
                let created_at: u64 = event.created_at.0 as u64;
                let metadata: Metadata = serde_json::from_str(&event.content)?;
                if let Some(mut person) = DbPerson::fetch_one(event.pubkey.into()).await? {
                    person.name = Some(metadata.name);
                    person.about = metadata.about;
                    person.picture = metadata.picture;
                    if person.dns_id != metadata.nip05 {
                        person.dns_id = metadata.nip05;
                        person.dns_id_valid = 0; // changed so starts invalid
                        person.dns_id_last_checked = match person.dns_id_last_checked {
                            None => Some(created_at),
                            Some(lc) => Some(created_at.max(lc)),
                        }
                    }
                    DbPerson::update(person.clone()).await?;
                    self.send_javascript_setpeople(vec![person]).await?;
                } else {
                    let person = DbPerson {
                        pubkey: event.pubkey.into(),
                        name: Some(metadata.name),
                        about: metadata.about,
                        picture: metadata.picture,
                        dns_id: metadata.nip05,
                        dns_id_valid: 0, // new so starts invalid
                        dns_id_last_checked: Some(created_at),
                        followed: 0
                    };
                    DbPerson::insert(person.clone()).await?;
                    self.send_javascript_setpeople(vec![person]).await?;
                }
            },
            EventKind::TextNote => {
                log::debug!("Event(textnote) from {}", &self.url);
                // Javascript needs to render this event on the feed:
                self.send_overlord_newevent(event).await?;
            },
            EventKind::RecommendRelay => {
                log::debug!("Event(recommend_relay) from {} [IGNORED]", &self.url);
                // TBD
            },
            EventKind::ContactList => {
                log::debug!("Event(contact_list) from {} [IGNORED]", &self.url);
                // TBD
            },
            EventKind::EventDeletion => {
                log::debug!("Event(deletion) from {} [IGNORED]", &self.url);
                // TBD
            },
            EventKind::Reaction => {
                log::debug!("Event(reaction) from {} [IGNORED]", &self.url);
                // TBD
            },
            _ => { }
        }

        Ok(())
    }
}
