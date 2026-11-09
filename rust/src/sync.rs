//! Two-way sync with a CalDAV task list: Nextcloud Tasks, which the iPhone Reminders app
//! shows as a list once the Nextcloud account is added on the phone.
//!
//! Runs on a background thread. The UI sends "sync now" nudges after local edits; otherwise
//! the server is checked every 30 seconds (cheaply, via the list's ctag).

use crate::database::{self, Database, RemoteFields, SyncRow};
use reqwest::{header, Method, StatusCode, Url};
use std::collections::{HashMap, HashSet};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

pub const POLL_INTERVAL: Duration = Duration::from_secs(30);
const AUTH_ERROR: &str = "The server rejected the username or app password";
const XML: &str = "application/xml; charset=utf-8";
const ICS: &str = "text/calendar; charset=utf-8";

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Config {
    pub server: String,
    pub username: String,
    pub password: String,
    pub list_name: String,
}

impl Config {
    pub fn is_complete(&self) -> bool {
        !self.server.trim().is_empty()
            && !self.username.trim().is_empty()
            && !self.password.is_empty()
            && !self.list_name.trim().is_empty()
    }
}

#[derive(Default, Clone)]
pub struct Status {
    pub running: bool,
    pub last_success: Option<Instant>,
    pub error: Option<String>,
    /// Bumped whenever a sync changed local tasks; the UI reloads when it moves.
    pub generation: u64,
}

enum Cmd {
    Now,
    Stop,
}

/// A running sync thread. Dropping it stops the thread.
pub struct Handle {
    tx: mpsc::Sender<Cmd>,
    status: Arc<Mutex<Status>>,
}

impl Handle {
    /// Sync soon (after a short pause so bursts of edits go up together).
    pub fn nudge(&self) {
        let _ = self.tx.send(Cmd::Now);
    }

    pub fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        let _ = self.tx.send(Cmd::Stop);
    }
}

pub fn spawn(db: Arc<Database>, cfg: Config, on_change: impl Fn() + Send + 'static) -> Handle {
    let (tx, rx) = mpsc::channel();
    let status = Arc::new(Mutex::new(Status::default()));
    let shared = status.clone();
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            Ok(rt) => rt,
            Err(e) => {
                shared.lock().unwrap().error = Some(e.to_string());
                on_change();
                return;
            }
        };
        let mut engine = Engine::new(cfg);
        loop {
            shared.lock().unwrap().running = true;
            on_change();
            let result = rt.block_on(engine.sync_once(&db));
            {
                let mut s = shared.lock().unwrap();
                s.running = false;
                match result {
                    Ok(changed) => {
                        s.last_success = Some(Instant::now());
                        s.error = None;
                        if changed {
                            s.generation += 1;
                        }
                    }
                    Err(e) => {
                        eprintln!("⚠️ Sync failed: {e}");
                        s.error = Some(e);
                    }
                }
            }
            on_change();

            match rx.recv_timeout(POLL_INTERVAL) {
                Ok(Cmd::Now) => {
                    std::thread::sleep(Duration::from_millis(800));
                    loop {
                        match rx.try_recv() {
                            Ok(Cmd::Now) => continue,
                            Ok(Cmd::Stop) | Err(mpsc::TryRecvError::Disconnected) => return,
                            Err(mpsc::TryRecvError::Empty) => break,
                        }
                    }
                }
                Ok(Cmd::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => return,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
    });
    Handle { tx, status }
}

// ---------------------------------------------------------------------------- engine

struct RemoteTodo {
    href: String,
    etag: String,
    ical: String,
    fields: RemoteFields,
}

struct Engine {
    cfg: Config,
    dav: Dav,
    list_url: Option<Url>,
    last_ctag: Option<String>,
}

fn db_err(e: rusqlite::Error) -> String {
    format!("Database error: {e}")
}

impl Engine {
    fn new(cfg: Config) -> Self {
        let dav = Dav::new(&cfg);
        Self { cfg, dav, list_url: None, last_ctag: None }
    }

    /// One full sync. Returns whether local tasks changed.
    async fn sync_once(&mut self, db: &Database) -> Result<bool, String> {
        let list = match &self.list_url {
            Some(u) => u.clone(),
            None => {
                let u = self.find_or_create_list(db).await?;
                self.list_url = Some(u.clone());
                u
            }
        };
        let result = self.sync_list(db, &list).await;
        if result.is_err() {
            // Rediscover next time, in case the list moved or was deleted.
            self.list_url = None;
            self.last_ctag = None;
        }
        result
    }

    async fn find_or_create_list(&self, db: &Database) -> Result<Url, String> {
        let list = self.dav.find_or_create_list(&self.cfg.list_name).await?;
        // A different account or list: forget old links so nothing is deleted by mistake.
        if database::get_setting(db, "sync_list_url").as_deref() != Some(list.as_str()) {
            database::reset_sync_state(db).map_err(db_err)?;
            database::set_setting(db, "sync_list_url", list.as_str()).map_err(db_err)?;
        }
        Ok(list)
    }

    async fn sync_list(&mut self, db: &Database, list: &Url) -> Result<bool, String> {
        let rows = database::sync_rows(db).map_err(db_err)?;
        let tombs = database::tombstones(db).map_err(db_err)?;
        let local_pending = !tombs.is_empty()
            || rows
                .iter()
                .any(|r| (r.uid.is_some() && r.dirty > 0) || (r.uid.is_none() && !r.completed));

        let ctag = self.dav.ctag(list).await.ok().flatten();
        if !local_pending && ctag.is_some() && ctag == self.last_ctag {
            return Ok(false);
        }

        // 1. Deletions made here.
        for (href, _) in &tombs {
            let url = list.join(href).map_err(|e| e.to_string())?;
            self.dav.delete(&url).await?;
            database::remove_tombstone(db, href).map_err(db_err)?;
        }
        let deleted: HashSet<&str> = tombs.iter().map(|(h, _)| h.as_str()).collect();

        // 2. Merge the server's state.
        let remote = self.dav.fetch_todos(list).await?;
        let by_uid: HashMap<&str, &SyncRow> =
            rows.iter().filter_map(|r| r.uid.as_deref().map(|u| (u, r))).collect();
        let remote_uids: HashSet<&str> = remote.iter().map(|r| r.fields.uid.as_str()).collect();
        let mut changed = false;

        for item in &remote {
            if deleted.contains(item.href.as_str()) {
                continue;
            }
            match by_uid.get(item.fields.uid.as_str()) {
                None => {
                    database::insert_from_remote(db, &item.fields, &item.href, &item.etag, &item.ical)
                        .map_err(db_err)?;
                    changed = true;
                }
                // Edited here: local wins, on top of the server's latest copy.
                Some(local) if local.dirty > 0 => self.push(db, list, local, Some(item)).await?,
                Some(local) if local.etag.as_deref() != Some(item.etag.as_str()) => {
                    changed |= database::update_from_remote(db, local.id, &item.fields, &item.href, &item.etag, &item.ical)
                        .map_err(db_err)?;
                }
                Some(_) => {}
            }
        }

        // 3. Synced tasks that are gone from the server were deleted elsewhere.
        let gone: Vec<&SyncRow> = rows
            .iter()
            .filter(|r| r.uid.as_deref().is_some_and(|u| !remote_uids.contains(u)))
            .collect();
        let synced = rows.iter().filter(|r| r.uid.is_some()).count();
        if remote.is_empty() && synced >= 3 && gone.len() == synced {
            return Err(format!(
                "The \"{}\" list on the server is empty, so no local tasks were removed. \
                 Turn sync off and on again to upload them.",
                self.cfg.list_name.trim()
            ));
        }
        for local in gone {
            if local.dirty > 0 {
                self.push(db, list, local, None).await?;
            } else {
                changed |= database::delete_from_remote(db, local.id).map_err(db_err)?;
            }
        }

        // 4. Tasks created here.
        for local in rows.iter().filter(|r| r.uid.is_none() && !r.completed) {
            self.push(db, list, local, None).await?;
        }

        self.last_ctag = self.dav.ctag(list).await.ok().flatten();
        Ok(changed)
    }

    async fn push(&self, db: &Database, list: &Url, local: &SyncRow, remote: Option<&RemoteTodo>) -> Result<(), String> {
        let uid = local.uid.clone().unwrap_or_else(|| new_uid(local.id));
        let base = remote.map(|r| r.ical.as_str()).or(local.ical.as_deref());
        let ical = match base {
            Some(orig) => ical::patch(orig, local),
            None => ical::build(&uid, local),
        };
        let href = match (remote, &local.href) {
            (Some(r), _) => r.href.clone(),
            (None, Some(h)) => h.clone(),
            (None, None) => list.join(&format!("{uid}.ics")).map_err(|e| e.to_string())?.path().to_string(),
        };
        let url = list.join(&href).map_err(|e| e.to_string())?;
        let pre = match remote {
            Some(r) => Precondition::Match(r.etag.clone()),
            None => Precondition::NoneMatch,
        };
        match self.dav.put(&url, &ical, pre).await? {
            PutResult::Stored(etag) => {
                database::mark_pushed(db, local.id, local.dirty, &uid, &href, etag.as_deref(), &ical).map_err(db_err)
            }
            // Changed on the server meanwhile; the next round merges it.
            PutResult::Conflict => Ok(()),
        }
    }
}

fn new_uid(id: i64) -> String {
    use std::hash::{BuildHasher, Hasher};
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(nanos);
    h.write_i64(id);
    format!("flowstate-{:x}-{:016x}", nanos, h.finish())
}

// ---------------------------------------------------------------------------- WebDAV

enum Precondition {
    Match(String),
    NoneMatch,
}

enum PutResult {
    Stored(Option<String>),
    Conflict,
}

#[derive(Default, Debug)]
struct DavItem {
    href: String,
    etag: Option<String>,
    calendar_data: Option<String>,
    displayname: Option<String>,
    is_calendar: bool,
    components: Option<Vec<String>>,
    principal: Option<String>,
    home: Option<String>,
    ctag: Option<String>,
}

const PRINCIPAL_BODY: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:"><d:prop><d:current-user-principal/></d:prop></d:propfind>"#;

const HOME_BODY: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:prop><c:calendar-home-set/></d:prop></d:propfind>"#;

const LISTS_BODY: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
  <d:prop><d:resourcetype/><d:displayname/><c:supported-calendar-component-set/></d:prop>
</d:propfind>"#;

const CTAG_BODY: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:" xmlns:cs="http://calendarserver.org/ns/"><d:prop><cs:getctag/></d:prop></d:propfind>"#;

const TODO_QUERY: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<c:calendar-query xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
  <d:prop><d:getetag/><c:calendar-data/></d:prop>
  <c:filter><c:comp-filter name="VCALENDAR"><c:comp-filter name="VTODO"/></c:comp-filter></c:filter>
</c:calendar-query>"#;

fn mkcalendar_body(name: &str) -> String {
    let name = name.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<c:mkcalendar xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
  <d:set><d:prop>
    <d:displayname>{name}</d:displayname>
    <c:supported-calendar-component-set><c:comp name="VTODO"/></c:supported-calendar-component-set>
  </d:prop></d:set>
</c:mkcalendar>"#
    )
}

/// Accepts "cloud.example.com", "https://cloud.example.com/nextcloud", etc.
fn parse_server_url(s: &str) -> Result<Url, String> {
    let s = s.trim();
    let s = if s.contains("://") { s.to_string() } else { format!("https://{s}") };
    let url = Url::parse(&s).map_err(|_| format!("\"{s}\" isn't a valid server address"))?;
    Ok(with_slash(url))
}

fn with_slash(mut url: Url) -> Url {
    if !url.path().ends_with('/') {
        let p = format!("{}/", url.path());
        url.set_path(&p);
    }
    url
}

fn short(body: &str) -> String {
    let t: String = body.trim().chars().take(160).collect();
    if t.is_empty() { String::new() } else { format!(": {t}") }
}

struct Dav {
    client: reqwest::Client,
    server: String,
    user: String,
    pass: String,
}

impl Dav {
    fn new(cfg: &Config) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .user_agent(concat!("FlowState/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_default();
        Self {
            client,
            server: cfg.server.clone(),
            user: cfg.username.trim().to_string(),
            pass: cfg.password.clone(),
        }
    }

    async fn send(
        &self,
        method: &str,
        url: &Url,
        depth: Option<&str>,
        body: Option<(String, &'static str)>,
        extra: &[(&'static str, String)],
    ) -> Result<(StatusCode, header::HeaderMap, String), String> {
        let method = Method::from_bytes(method.as_bytes()).map_err(|e| e.to_string())?;
        let mut req = self
            .client
            .request(method, url.clone())
            .basic_auth(&self.user, Some(&self.pass));
        if let Some(d) = depth {
            req = req.header("Depth", d);
        }
        for (k, v) in extra {
            req = req.header(*k, v.as_str());
        }
        if let Some((body, content_type)) = body {
            req = req.header(header::CONTENT_TYPE, content_type).body(body);
        }
        let resp = req.send().await.map_err(|e| {
            if e.is_timeout() {
                "The server took too long to answer".to_string()
            } else if e.is_connect() {
                format!("Can't reach {}", url.host_str().unwrap_or("the server"))
            } else {
                e.to_string()
            }
        })?;
        let status = resp.status();
        if status == StatusCode::UNAUTHORIZED {
            return Err(AUTH_ERROR.into());
        }
        let headers = resp.headers().clone();
        let text = resp.text().await.map_err(|e| e.to_string())?;
        Ok((status, headers, text))
    }

    async fn propfind(&self, url: &Url, depth: &str, body: &str) -> Result<Vec<DavItem>, String> {
        let (status, _, text) = self.send("PROPFIND", url, Some(depth), Some((body.to_string(), XML)), &[]).await?;
        if status != StatusCode::MULTI_STATUS {
            return Err(format!("Unexpected answer from {url} ({status})"));
        }
        parse_multistatus(&text)
    }

    async fn propfind_one(&self, url: &Url, body: &str) -> Result<DavItem, String> {
        self.propfind(url, "0", body)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| format!("Empty answer from {url}"))
    }

    async fn calendar_home(&self) -> Result<Url, String> {
        let base = parse_server_url(&self.server)?;
        let mut candidates = vec![base.clone()];
        if !base.path().contains("/remote.php/") {
            // Nextcloud's DAV endpoint.
            candidates.push(base.join("remote.php/dav/").map_err(|e| e.to_string())?);
        }
        let mut last_err = format!("No CalDAV service found at {base}");
        for c in candidates {
            match self.propfind_one(&c, PRINCIPAL_BODY).await {
                Ok(DavItem { principal: Some(p), .. }) => {
                    let principal = c.join(&p).map_err(|e| e.to_string())?;
                    let item = self.propfind_one(&principal, HOME_BODY).await?;
                    match item.home {
                        Some(h) => return Ok(with_slash(principal.join(&h).map_err(|e| e.to_string())?)),
                        None => last_err = "The server didn't say where your task lists are".into(),
                    }
                }
                Ok(_) => {}
                Err(e) if e == AUTH_ERROR => return Err(e),
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
    }

    async fn find_or_create_list(&self, name: &str) -> Result<Url, String> {
        let name = name.trim();
        let home = self.calendar_home().await?;
        for item in self.propfind(&home, "1", LISTS_BODY).await? {
            let holds_tasks = item.components.as_ref().map_or(true, |c| c.iter().any(|c| c == "VTODO"));
            let same_name = item.displayname.as_deref().is_some_and(|d| d.trim().eq_ignore_ascii_case(name));
            if item.is_calendar && holds_tasks && same_name {
                return Ok(with_slash(home.join(&item.href).map_err(|e| e.to_string())?));
            }
        }

        // Not there yet: create it.
        let slug: String = name.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase();
        let slug = if slug.is_empty() { "flowstate".to_string() } else { slug };
        for n in 0..5 {
            let path = if n == 0 { format!("{slug}/") } else { format!("{slug}-{n}/") };
            let url = home.join(&path).map_err(|e| e.to_string())?;
            let (status, _, text) = self.send("MKCALENDAR", &url, None, Some((mkcalendar_body(name), XML)), &[]).await?;
            if status.is_success() {
                return Ok(url);
            }
            if status != StatusCode::METHOD_NOT_ALLOWED && status != StatusCode::CONFLICT {
                return Err(format!("Couldn't create the \"{name}\" list on the server ({status}){}", short(&text)));
            }
        }
        Err(format!("Couldn't create the \"{name}\" list on the server"))
    }

    async fn ctag(&self, list: &Url) -> Result<Option<String>, String> {
        Ok(self.propfind_one(list, CTAG_BODY).await?.ctag)
    }

    async fn fetch_todos(&self, list: &Url) -> Result<Vec<RemoteTodo>, String> {
        let (status, _, text) = self.send("REPORT", list, Some("1"), Some((TODO_QUERY.to_string(), XML)), &[]).await?;
        if status != StatusCode::MULTI_STATUS {
            return Err(format!("Couldn't read the task list ({status}){}", short(&text)));
        }
        let mut out = Vec::new();
        for item in parse_multistatus(&text)? {
            let (Some(etag), Some(data)) = (item.etag, item.calendar_data) else { continue };
            let Some(fields) = ical::parse(&data) else { continue };
            out.push(RemoteTodo { href: item.href, etag, ical: data, fields });
        }
        Ok(out)
    }

    async fn put(&self, url: &Url, ical: &str, pre: Precondition) -> Result<PutResult, String> {
        let h = match pre {
            Precondition::Match(etag) => ("If-Match", etag),
            Precondition::NoneMatch => ("If-None-Match", "*".to_string()),
        };
        let (status, headers, text) = self.send("PUT", url, None, Some((ical.to_string(), ICS)), &[h]).await?;
        if status == StatusCode::PRECONDITION_FAILED {
            return Ok(PutResult::Conflict);
        }
        if !status.is_success() {
            return Err(format!("The server refused a task ({status}){}", short(&text)));
        }
        let etag = headers.get(header::ETAG).and_then(|v| v.to_str().ok()).map(String::from);
        Ok(PutResult::Stored(etag))
    }

    async fn delete(&self, url: &Url) -> Result<(), String> {
        let (status, _, text) = self.send("DELETE", url, None, None, &[]).await?;
        if status.is_success() || status == StatusCode::NOT_FOUND || status == StatusCode::GONE {
            Ok(())
        } else {
            Err(format!("Couldn't delete a task on the server ({status}){}", short(&text)))
        }
    }
}

fn parse_multistatus(xml: &str) -> Result<Vec<DavItem>, String> {
    fn child<'a, 'i>(n: roxmltree::Node<'a, 'i>, name: &str) -> Option<roxmltree::Node<'a, 'i>> {
        n.children().find(|c| c.is_element() && c.tag_name().name() == name)
    }
    fn text_of(n: roxmltree::Node) -> Option<String> {
        let t: String = n.descendants().filter(|d| d.is_text()).filter_map(|d| d.text()).collect();
        let t = t.trim();
        (!t.is_empty()).then(|| t.to_string())
    }

    let doc = roxmltree::Document::parse(xml).map_err(|e| format!("Unexpected server reply: {e}"))?;
    let mut out = Vec::new();
    for resp in doc.descendants().filter(|n| n.is_element() && n.tag_name().name() == "response") {
        let mut item = DavItem {
            href: child(resp, "href").and_then(text_of).unwrap_or_default(),
            ..Default::default()
        };
        for ps in resp.children().filter(|n| n.is_element() && n.tag_name().name() == "propstat") {
            let ok = child(ps, "status").and_then(text_of).map_or(true, |s| s.contains(" 200"));
            let Some(prop) = child(ps, "prop") else { continue };
            if !ok {
                continue;
            }
            for p in prop.children().filter(|n| n.is_element()) {
                match p.tag_name().name() {
                    "getetag" => item.etag = text_of(p),
                    "calendar-data" => item.calendar_data = text_of(p),
                    "displayname" => item.displayname = text_of(p),
                    "resourcetype" => {
                        item.is_calendar = p.children().any(|c| c.is_element() && c.tag_name().name() == "calendar")
                    }
                    "supported-calendar-component-set" => {
                        item.components = Some(
                            p.children()
                                .filter(|c| c.is_element() && c.tag_name().name() == "comp")
                                .filter_map(|c| c.attribute("name"))
                                .map(|s| s.to_ascii_uppercase())
                                .collect(),
                        )
                    }
                    "current-user-principal" => item.principal = child(p, "href").and_then(text_of),
                    "calendar-home-set" => item.home = child(p, "href").and_then(text_of),
                    "getctag" => item.ctag = text_of(p),
                    _ => {}
                }
            }
        }
        out.push(item);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------- iCalendar

pub mod ical {
    use crate::database::{RemoteFields, SyncRow};

    /// Properties FlowState owns; everything else (notes, due dates, alarms…) is kept as is.
    const OWNED: &[&str] = &["SUMMARY", "STATUS", "COMPLETED", "PERCENT-COMPLETE", "PRIORITY", "LAST-MODIFIED", "DTSTAMP"];

    fn unfold(data: &str) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();
        for raw in data.split('\n') {
            let line = raw.strip_suffix('\r').unwrap_or(raw);
            if let Some(rest) = line.strip_prefix(' ').or_else(|| line.strip_prefix('\t')) {
                if let Some(last) = lines.last_mut() {
                    last.push_str(rest);
                    continue;
                }
            }
            if !line.is_empty() {
                lines.push(line.to_string());
            }
        }
        lines
    }

    /// Fold to 75-octet lines as RFC 5545 asks.
    fn fold(line: &str) -> String {
        let mut out = String::new();
        let mut width = 0;
        for c in line.chars() {
            let n = c.len_utf8();
            if width + n > 75 {
                out.push_str("\r\n ");
                width = 1;
            }
            out.push(c);
            width += n;
        }
        out
    }

    /// (uppercased name, value) of a content line; parameters are ignored.
    fn split(line: &str) -> (String, &str) {
        let mut in_quotes = false;
        for (i, c) in line.char_indices() {
            match c {
                '"' => in_quotes = !in_quotes,
                ':' if !in_quotes => {
                    let name = line[..i].split(';').next().unwrap_or("").to_ascii_uppercase();
                    return (name, &line[i + 1..]);
                }
                _ => {}
            }
        }
        (line.to_ascii_uppercase(), "")
    }

    pub fn escape(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                '\\' => out.push_str("\\\\"),
                ';' => out.push_str("\\;"),
                ',' => out.push_str("\\,"),
                '\n' => out.push_str("\\n"),
                '\r' => {}
                c => out.push(c),
            }
        }
        out
    }

    pub fn unescape(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut chars = s.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n') | Some('N') => out.push('\n'),
                    Some(other) => out.push(other),
                    None => {}
                }
            } else {
                out.push(c);
            }
        }
        out
    }

    /// "20261109T150000Z" / "20261109" → "2026-11-09 15:00:00" (time zones are approximated).
    pub fn to_sql(value: &str) -> Option<String> {
        let d: Vec<char> = value.chars().filter(|c| c.is_ascii_digit()).collect();
        if d.len() < 8 {
            return None;
        }
        let s = |a: usize, b: usize| d.get(a..b).map(|x| x.iter().collect::<String>());
        let time = if d.len() >= 14 {
            format!("{}:{}:{}", s(8, 10)?, s(10, 12)?, s(12, 14)?)
        } else {
            "00:00:00".into()
        };
        Some(format!("{}-{}-{} {}", s(0, 4)?, s(4, 6)?, s(6, 8)?, time))
    }

    /// "2026-11-09 15:00:00" (UTC) → "20261109T150000Z".
    pub fn from_sql(value: &str) -> String {
        let d: String = value.chars().filter(|c| c.is_ascii_digit()).collect();
        if d.len() >= 14 {
            format!("{}T{}Z", &d[..8], &d[8..14])
        } else {
            now()
        }
    }

    fn now() -> String {
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string()
    }

    fn is_high(priority: &str) -> bool {
        priority.trim().parse::<u8>().is_ok_and(|p| (1..=4).contains(&p))
    }

    pub fn parse(data: &str) -> Option<RemoteFields> {
        let mut f = RemoteFields {
            uid: String::new(),
            text: String::new(),
            completed: false,
            completed_at: None,
            created_at: None,
            priority: false,
        };
        let (mut in_todo, mut nested) = (false, 0);
        for line in unfold(data) {
            let (name, value) = split(&line);
            if !in_todo {
                in_todo = name == "BEGIN" && value.eq_ignore_ascii_case("VTODO");
                continue;
            }
            match name.as_str() {
                "BEGIN" => nested += 1,
                "END" if nested > 0 => nested -= 1,
                "END" => break,
                _ if nested > 0 => {}
                "UID" => f.uid = value.trim().to_string(),
                "SUMMARY" => f.text = unescape(value).replace(['\r', '\n'], " ").trim().to_string(),
                "STATUS" => f.completed = value.trim().eq_ignore_ascii_case("COMPLETED"),
                "COMPLETED" => f.completed_at = to_sql(value),
                "CREATED" => f.created_at = to_sql(value),
                "PRIORITY" => f.priority = is_high(value),
                _ => {}
            }
        }
        if !in_todo || f.uid.is_empty() {
            return None;
        }
        if f.text.is_empty() {
            f.text = "Untitled".into();
        }
        if f.completed && f.completed_at.is_none() {
            f.completed_at = Some(chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string());
        }
        if !f.completed {
            f.completed_at = None;
        }
        Some(f)
    }

    fn owned_props(t: &SyncRow, now: &str) -> Vec<String> {
        let mut p = vec![
            format!("DTSTAMP:{now}"),
            format!("LAST-MODIFIED:{now}"),
            format!("SUMMARY:{}", escape(&t.text)),
        ];
        if t.completed {
            p.push("STATUS:COMPLETED".into());
            let at = t.completed_at.as_deref().map(from_sql).unwrap_or_else(|| now.to_string());
            p.push(format!("COMPLETED:{at}"));
            p.push("PERCENT-COMPLETE:100".into());
        } else {
            p.push("STATUS:NEEDS-ACTION".into());
        }
        if t.priority {
            p.push("PRIORITY:1".into());
        }
        p
    }

    fn join(lines: &[String]) -> String {
        let mut out = String::new();
        for l in lines {
            out.push_str(&fold(l));
            out.push_str("\r\n");
        }
        out
    }

    pub fn build(uid: &str, t: &SyncRow) -> String {
        let now = now();
        let mut lines = vec![
            "BEGIN:VCALENDAR".to_string(),
            "VERSION:2.0".into(),
            "PRODID:-//FlowState//FlowState//EN".into(),
            "BEGIN:VTODO".into(),
            format!("UID:{uid}"),
            format!("CREATED:{}", from_sql(&t.created_at)),
        ];
        lines.extend(owned_props(t, &now));
        lines.push("END:VTODO".into());
        lines.push("END:VCALENDAR".into());
        join(&lines)
    }

    /// Rewrite the fields FlowState owns in an existing item, keeping everything else.
    pub fn patch(orig: &str, t: &SyncRow) -> String {
        let now = now();
        let mut out = Vec::new();
        let (mut in_todo, mut done, mut nested) = (false, false, 0);
        for line in unfold(orig) {
            let (name, value) = split(&line);
            if !in_todo || done {
                let starts_todo = !done && name == "BEGIN" && value.eq_ignore_ascii_case("VTODO");
                out.push(line.clone());
                if starts_todo {
                    in_todo = true;
                    // Properties must come before any VALARM, so add ours right away.
                    out.extend(owned_props(t, &now));
                }
                continue;
            }
            match name.as_str() {
                "BEGIN" => nested += 1,
                "END" if nested > 0 => nested -= 1,
                "END" => done = true,
                _ if nested > 0 => {}
                // Keep a medium/low priority set on the phone unless we changed it.
                "PRIORITY" if !t.priority && !is_high(value) => {}
                n if OWNED.contains(&n) => continue,
                _ => {}
            }
            out.push(line.clone());
        }
        join(&out)
    }
}

// ---------------------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    fn row(text: &str) -> SyncRow {
        SyncRow {
            id: 1,
            text: text.into(),
            completed: false,
            completed_at: None,
            created_at: "2026-11-09 12:00:00".into(),
            priority: false,
            uid: None,
            href: None,
            etag: None,
            ical: None,
            dirty: 1,
        }
    }

    const PHONE: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Apple Inc.//iOS 26//EN\r\n\
BEGIN:VTODO\r\nUID:ABC-123\r\nCREATED:20261109T120000Z\r\nDTSTAMP:20261109T120000Z\r\n\
SUMMARY:Buy filters\\, screws and a very long name that needs folding across lines to be valid\r\n  ok\r\n\
DESCRIPTION:aisle 7\r\nPRIORITY:1\r\nSTATUS:NEEDS-ACTION\r\n\
BEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:Reminder\r\nTRIGGER:-PT15M\r\nEND:VALARM\r\n\
END:VTODO\r\nEND:VCALENDAR\r\n";

    #[test]
    fn parses_phone_reminder() {
        let f = ical::parse(PHONE).unwrap();
        assert_eq!(f.uid, "ABC-123");
        assert!(f.text.starts_with("Buy filters, screws"));
        assert!(f.text.ends_with("valid ok"));
        assert!(f.priority);
        assert!(!f.completed);
        assert_eq!(f.created_at.as_deref(), Some("2026-11-09 12:00:00"));
    }

    #[test]
    fn patch_keeps_unknown_fields() {
        let mut t = row("Buy filters; and glue");
        t.completed = true;
        t.completed_at = Some("2026-11-10 08:30:00".into());
        let out = ical::patch(PHONE, &t);
        assert!(out.contains("DESCRIPTION:aisle 7"));
        assert!(out.contains("TRIGGER:-PT15M"));
        assert!(out.contains("SUMMARY:Buy filters\\; and glue"));
        assert!(out.contains("COMPLETED:20261110T083000Z"));
        assert!(!out.contains("PRIORITY:1"), "priority cleared locally");
        assert!(!out.contains("NEEDS-ACTION"));
        // Our properties sit before the alarm.
        assert!(out.find("SUMMARY:").unwrap() < out.find("BEGIN:VALARM").unwrap());
        let back = ical::parse(&out).unwrap();
        assert_eq!(back.text, "Buy filters; and glue");
        assert!(back.completed);
        assert_eq!(back.uid, "ABC-123");
    }

    #[test]
    fn build_round_trips() {
        let mut t = row("Call mom, about Sunday\nand bring cake");
        t.priority = true;
        let out = ical::build("u-1", &t);
        assert!(out.lines().all(|l| l.len() <= 76));
        let f = ical::parse(&out).unwrap();
        assert_eq!(f.uid, "u-1");
        assert_eq!(f.text, "Call mom, about Sunday and bring cake");
        assert!(f.priority);
        assert_eq!(f.created_at.as_deref(), Some("2026-11-09 12:00:00"));
    }

    #[test]
    fn reads_multistatus() {
        let xml = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:" xmlns:cal="urn:ietf:params:xml:ns:caldav">
 <d:response><d:href>/remote.php/dav/calendars/yuri/flowstate/a.ics</d:href>
  <d:propstat><d:prop><d:getetag>"e1"</d:getetag><cal:calendar-data>BEGIN:VCALENDAR&#13;
BEGIN:VTODO&#13;
UID:a&#13;
SUMMARY:x &amp; y&#13;
END:VTODO&#13;
END:VCALENDAR</cal:calendar-data></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
 </d:response>
 <d:response><d:href>/remote.php/dav/calendars/yuri/</d:href>
  <d:propstat><d:prop><d:resourcetype><d:collection/><cal:calendar/></d:resourcetype><d:displayname>FlowState</d:displayname></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
  <d:propstat><d:prop><d:getctag/></d:prop><d:status>HTTP/1.1 404 Not Found</d:status></d:propstat>
 </d:response>
</d:multistatus>"#;
        let items = parse_multistatus(xml).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].etag.as_deref(), Some("\"e1\""));
        assert_eq!(ical::parse(items[0].calendar_data.as_deref().unwrap()).unwrap().text, "x & y");
        assert!(items[1].is_calendar);
        assert_eq!(items[1].displayname.as_deref(), Some("FlowState"));
        assert!(items[1].ctag.is_none());
    }

    #[test]
    fn server_addresses() {
        assert_eq!(parse_server_url("cloud.example.com").unwrap().as_str(), "https://cloud.example.com/");
        assert_eq!(
            parse_server_url(" https://example.com/nextcloud ").unwrap().as_str(),
            "https://example.com/nextcloud/"
        );
    }

    /// Full round trip against a real CalDAV server (CI starts one). Set
    /// FLOWSTATE_CALDAV_URL to run it locally, e.g. against `python -m radicale`.
    #[test]
    fn caldav_round_trip() {
        let Ok(server) = std::env::var("FLOWSTATE_CALDAV_URL") else {
            eprintln!("FLOWSTATE_CALDAV_URL not set; skipping");
            return;
        };
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let cfg = Config {
            server,
            username: std::env::var("FLOWSTATE_CALDAV_USER").unwrap_or_else(|_| "alice".into()),
            password: std::env::var("FLOWSTATE_CALDAV_PASSWORD").unwrap_or_else(|_| "secret".into()),
            list_name: format!("FlowState test {nanos}"),
        };
        let db = Database::open_in_memory().unwrap();
        let mut engine = Engine::new(cfg.clone());
        let phone = Dav::new(&cfg);
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();

        rt.block_on(async {
            // A task typed on the computer goes up.
            database::add_task(&db, "buy milk").unwrap();
            engine.sync_once(&db).await.unwrap();
            let list = engine.list_url.clone().unwrap();
            let remote = phone.fetch_todos(&list).await.unwrap();
            assert_eq!(remote.len(), 1);
            assert_eq!(remote[0].fields.text, "buy milk");

            // A high-priority reminder with a note, added on the phone, comes down.
            let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Apple Inc.//iOS//EN\r\nBEGIN:VTODO\r\n\
UID:phone-1\r\nDTSTAMP:20261109T120000Z\r\nCREATED:20261109T120000Z\r\nSUMMARY:call mom\r\n\
DESCRIPTION:about the weekend\r\nPRIORITY:1\r\nSTATUS:NEEDS-ACTION\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";
            let url = list.join("phone-1.ics").unwrap();
            assert!(matches!(phone.put(&url, ics, Precondition::NoneMatch).await.unwrap(), PutResult::Stored(_)));
            assert!(engine.sync_once(&db).await.unwrap());
            let mom = database::find_by_text(&db, "call mom").expect("phone task arrived");
            assert!(mom.priority);

            // Completing it here completes it on the phone and keeps the note.
            database::toggle_task(&db, mom.id).unwrap();
            engine.sync_once(&db).await.unwrap();
            let remote = phone.fetch_todos(&list).await.unwrap();
            let r = remote.iter().find(|r| r.fields.uid == "phone-1").unwrap();
            assert!(r.fields.completed);
            assert!(r.ical.contains("about the weekend"));

            // Renamed on the phone.
            let milk = remote.iter().find(|r| r.fields.text == "buy milk").unwrap();
            let renamed = milk.ical.replace("SUMMARY:buy milk", "SUMMARY:buy oat milk");
            let milk_url = list.join(&milk.href).unwrap();
            phone.put(&milk_url, &renamed, Precondition::Match(milk.etag.clone())).await.unwrap();
            assert!(engine.sync_once(&db).await.unwrap());
            assert!(database::find_by_text(&db, "buy oat milk").is_some());

            // Deleted on the phone.
            phone.delete(&milk_url).await.unwrap();
            engine.sync_once(&db).await.unwrap();
            assert!(database::find_by_text(&db, "buy oat milk").is_none());

            // Deleted here.
            let mom = database::find_by_text(&db, "call mom").unwrap();
            database::delete_task(&db, mom.id).unwrap();
            engine.sync_once(&db).await.unwrap();
            assert!(phone.fetch_todos(&list).await.unwrap().is_empty());

            // Nothing changed: a quiet sync reports no local changes.
            assert!(!engine.sync_once(&db).await.unwrap());
        });
    }
}
