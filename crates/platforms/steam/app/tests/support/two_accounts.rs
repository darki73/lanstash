//! Two scripted Steam accounts that each own app 100 under a package of their own: `player_one`'s
//! (37) grants depots 101 and 103, `player_two`'s (38) depot 102 alone. Each connection learns
//! its account from the logon and notes every manifest request code it is asked for.

use std::sync::{Arc, Mutex};

use lanstash_ports::secrets::Secrets;
use lanstash_steam_domain::cm::content::method;
use lanstash_steam_domain::cm::message::{Header, Message, emsg};
use lanstash_steam_domain::cm::pics::emsg as pics;
use lanstash_steam_domain::cm::proto::{Writer, fields};

use super::account::{APP_INFO, CDN, STEAMID, cdn, package_of, reply};
use super::{Script, answer, multi};

/// Each request code asked: (the account signed in on that connection, the depot).
pub type Asked = Arc<Mutex<Vec<(String, u32)>>>;

/// The package an account's license names, and the depots it grants.
fn package(account: &str) -> (u32, &'static [i32]) {
    if account == "player_two" { (38, &[102]) } else { (37, &[101, 103]) }
}

fn field(body: &[u8], n: u32) -> Option<lanstash_steam_domain::cm::proto::Wire<'_>> {
    fields(body).filter_map(Result::ok).find(|(f, _)| *f == n).map(|(_, v)| v)
}

/// One connection's Steam: whoever logs on, as one of the two accounts.
pub fn script(asked: Asked) -> Script {
    let mut account = String::new();
    Box::new(move |m: &Message| match m.emsg {
        emsg::CLIENT_LOGON => {
            account = field(&m.body, 50).and_then(|v| v.str()).unwrap_or_default().to_owned();
            let h = Header { steamid: STEAMID, session: 5, ..Header::default() };
            let ok = Writer::new().varint(1, 1).varint(3, 9).varint(7, 127).finish();
            let license = Writer::new().varint(1, u64::from(package(&account).0)).varint(17, 5).finish();
            vec![multi(&[
                Message::new(emsg::CLIENT_LOG_ON_RESPONSE, h, ok).encode(),
                Message::new(pics::CLIENT_LICENSE_LIST, Header::default(), Writer::new().bytes(2, &license).finish())
                    .encode(),
            ])]
        }
        pics::PICS_ACCESS_TOKEN_REQUEST => vec![reply(
            m,
            pics::PICS_ACCESS_TOKEN_RESPONSE,
            Writer::new().bytes(3, &Writer::new().varint(1, 100).varint(2, 7).finish()).finish(),
        )],
        pics::PICS_PRODUCT_INFO_REQUEST => {
            if fields(&m.body).filter_map(Result::ok).any(|(n, _)| n == 1) {
                let (id, depots) = package(&account);
                let p = Writer::new().varint(1, u64::from(id)).bytes(5, &package_of(id, &[100], depots)).finish();
                vec![reply(m, pics::PICS_PRODUCT_INFO_RESPONSE, Writer::new().bytes(3, &p).finish())]
            } else {
                let a = Writer::new().varint(1, 100).bytes(5, APP_INFO.as_bytes()).finish();
                vec![reply(m, pics::PICS_PRODUCT_INFO_RESPONSE, Writer::new().bytes(1, &a).finish())]
            }
        }
        emsg::SERVICE_METHOD_CALL_FROM_CLIENT => match m.header.target_job_name.as_deref() {
            Some(method::GET_SERVERS_FOR_STEAM_PIPE) => {
                let s = Writer::new().string(1, "CDN").string(8, CDN).string(9, CDN).finish();
                vec![answer(m, 1, Writer::new().bytes(1, &s).finish())]
            }
            Some(method::GET_MANIFEST_REQUEST_CODE) => {
                let depot = field(&m.body, 2).and_then(|v| v.int()).unwrap_or(0);
                asked.lock().unwrap().push((account.clone(), u32::try_from(depot).unwrap_or(0)));
                // Steam answers a code only to an account the depot is granted to.
                if package(&account).1.contains(&i32::try_from(depot).unwrap_or(0)) {
                    vec![answer(m, 1, Writer::new().varint(1, 1234).finish())]
                } else {
                    vec![answer(m, 15, vec![])]
                }
            }
            _ => vec![answer(m, 2, vec![])],
        },
        _ => Vec::new(),
    })
}

/// Dials the two accounts' Steam.
pub struct Dialer(pub Asked);

impl lanstash_ports::link::Dialer for Dialer {
    fn dial(&self, _: &str, _: u16, _: &str) -> Result<Box<dyn lanstash_ports::link::Link>, String> {
        Ok(super::steam(script(Arc::clone(&self.0))).0)
    }

    fn get(&self, _: &str, _: &str) -> Result<Vec<u8>, String> {
        Ok(br#"{"response":{"serverlist":[{"endpoint":"cm.example:443","type":"websockets"}]}}"#.to_vec())
    }
}

/// The services with both accounts signed in, and the fake CDN.
pub fn services(asked: Asked) -> lanstash_steam_app::session::Services {
    let secrets = super::Memory::default();
    for (name, token) in [("player_one", "A"), ("player_two", "B")] {
        let tokens = lanstash_steam_domain::cm::auth::Tokens {
            refresh_token: token.into(),
            access_token: String::new(),
            account_name: name.into(),
        };
        lanstash_steam_app::login::keep(&secrets as &dyn Secrets, &tokens, "").unwrap();
    }
    lanstash_steam_app::session::Services::new(
        Arc::new(Dialer(asked)),
        Arc::new(secrets),
        Arc::new(cdn()),
        Arc::new(lanstash_ports::clock::FixedClock(0)),
    )
}
