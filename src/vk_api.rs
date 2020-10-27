use serde::{Deserialize, Deserializer, de::{self, DeserializeOwned}};
use serde_json::{de::from_slice, Value};
use reqwest::{Client, Response};
use thiserror::Error;
use std::{num::NonZeroU32, result::Result as StdResult};

macro_rules! generic_request {
    ($prefix:expr, $address:expr$(, $arg:tt)*$(,)?) => {
        {
            let mut uri = String::from($prefix);
            uri.push_str($address);
            uri.push('?');
            $(
                uri.push_str(stringify!($arg));
                uri.push('=');
                uri.push_str($arg.to_string().as_str());
                uri.push('&');
            )*
            if let Some('&') = uri.chars().rev().peekable().peek() {
                uri.pop();
            }
            uri
        }
    };
}

macro_rules! server_request {
    ($address:expr$(, $arg:tt)*$(,)?) => {
        generic_request!("https://", $address, $($arg, )*)
    };
}

macro_rules! api_request {
    ($method:expr, ($($arg:tt),*), $access_token:expr, $api_version:expr$(,)?) => {
        {
            let access_token = $access_token.as_str();
            let v = $api_version;
            generic_request!("https://api.vk.com/method/", $method,$( $arg,)* access_token, v)
        }
    };
    ($method:expr, ($($arg:tt),*), $access_token:expr$(,)?) => {
        compile_error!("Expected 4 arguments, found 3");
    };
    ($method:expr, ($($arg:tt),*)$(,)?) => {
        compile_error!("Expected 4 arguments, found 2");
    };
    ($method:expr$(,)?) => {
        compile_error!("Expected 4 arguments, found 1");
    };
    () => {
        compile_error!("Expected 4 arguments, found 0");
    };
    ($method:expr$(, $arg:expr)*$(,)?) => {
        compile_error!("Arguments for api request must be parenthesized");
    };
}

// macro_rules! ok {
//     ($result:expr) => {
//         {
//             match $result {
//                 Ok(s) => return Ok(s),
//                 _ => {}
//             }
//         }
//     }
// }

pub type Result<T> = StdResult<T, Error>;

#[derive(Debug, Deserialize, Error)]
pub enum Error {
    #[serde(rename(deserialize = "error"))]
    #[error("{0:?}")]
    VkError(VkError),
    #[serde(skip)]
    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),
    #[serde(deserialize_with = "into_lpsf")]
    #[error("{0:?}")]
    LPServerFailure(LongPollServerFailure),
    #[serde(skip)]
    #[error("Unknown error")]
    UnknownError,
}

pub use Error::*;

pub struct SessionInfo {
    client: Client,
    access_token: String,
    api_version: &'static str,
}

impl SessionInfo {
    pub fn new(access_token: String, api_version: &'static str) -> Self {
        Self {
            access_token,
            api_version,
            client: Client::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct VkResponse<T> {
    response: T,
}

impl<T> VkResponse<T> {
    fn unwrap(self) -> T { self.response }
}

#[derive(Debug, Deserialize)]
pub struct VkError {
    error_code: u32,
    error_msg: String,
}
#[derive(Debug, Deserialize)]
struct LongPollServerInfo {
    key: String,
    server: String,
    ts: u32,
    #[serde(default)]
    pts: u32,
}

#[derive(Debug)]
pub struct LongPollServer {
    info: LongPollServerInfo,
    wait: u8,
    mode: u8,
    group_id: Option<NonZeroU32>,
    version: u16,
}

#[derive(Debug, Deserialize)]
pub struct LongPollServerResponse {
    ts: u32,
    pub updates: Vec<Vec<Value>>,
}

#[derive(Debug, Deserialize)]
pub struct Stub {}

#[derive(Debug)]
pub enum LongPollServerFailure {
    EventHistoryIsObsolete { new_ts: u32 },
    KeyExpired,
    UserInfoLost,
    InvalidVersion { min_version: u16, max_version: u16 },
}

fn into_lpsf<'de, D: Deserializer<'de>>(deserializer: D) -> StdResult<LongPollServerFailure, D::Error> {
    use LongPollServerFailure::*;
    use serde_json::Map;
    let unknown_err = || de::Error::custom(UnknownError);
    let json_value: Value = VkResponse::deserialize(deserializer)?.response;
    let obj = json_value.as_object().ok_or_else(unknown_err)?;
    let fail_code = obj.get("failed").and_then(Value::as_u64).ok_or_else(unknown_err)?;
    let ehio = |obj: &Map<_, _>| {
        let new_ts = obj.get("new_ts")?.as_u64()? as u32;
        Some(EventHistoryIsObsolete { new_ts })
    };
    let iv = |obj: &Map<_, _>| {
        let min_version = obj.get("min_version")?.as_u64()? as u16;
        let max_version = obj.get("max_version")?.as_u64()? as u16;
        Some(InvalidVersion { min_version, max_version })
    };
    let lpsf_opt = match fail_code {
        1 => ehio(obj),
        2 => Some(KeyExpired),
        3 => Some(UserInfoLost),
        4 => iv(obj),
        _ => None
    };
    lpsf_opt.ok_or_else(unknown_err)
}


impl SessionInfo {
    pub async fn get_long_poll_server(&self, need_pts: bool, group_id: u32, lp_version: u16) -> Result<LongPollServer> {
        let group_id = NonZeroU32::new(group_id);
        let server_info = self.get_long_poll_server_info(need_pts, group_id, lp_version).await?;
        Ok(LongPollServer { info: server_info, wait: 25, mode: 2 | 8 | if need_pts { 32 } else { 0 }, group_id, version: lp_version } )
    }

    async fn get_long_poll_server_info(&self, need_pts: bool, group_id: Option<NonZeroU32>, lp_version: u16) -> Result<LongPollServerInfo> {
        let need_pts = need_pts as u8;
        let api_request =
            match group_id {
                Some(gid) => {
                    let group_id = gid.get();
                    api_request!("messages.getLongPollServer", (need_pts, group_id, lp_version), self.access_token, self.api_version)
                }
                None => api_request!("messages.getLongPollServer", (need_pts, lp_version), self.access_token, self.api_version),
            };
        self.converget(api_request).await.map(VkResponse::unwrap)
    }

    pub async fn delete_messages(&self, message_ids: impl AsRef<str>, spam: bool, group_id: u32, delete_for_all: bool) -> Result<Stub> {
        let message_ids = message_ids.as_ref();
        let spam = spam as u8;
        let group_id = NonZeroU32::new(group_id);
        let delete_for_all = delete_for_all as u8;
        let api_request =
            match group_id {
                Some(gid) => {
                    let group_id = gid.get();
                    api_request!("messages.delete", (message_ids, spam, group_id, delete_for_all), self.access_token, self.api_version)
                }
                None => api_request!("messages.delete", (message_ids, spam, delete_for_all), self.access_token, self.api_version),
            };
        self.converget(api_request).await.map(VkResponse::unwrap)
    }

    async fn get(&self, request: impl AsRef<str>) -> StdResult<Response, reqwest::Error> {
        self.client.get(request.as_ref()).send().await
    }

    async fn converget<T: DeserializeOwned>(&self, request: impl AsRef<str>) -> Result<T> {
        let response = self.get(request).await?;
        let bytes: Vec<u8> = response.bytes().await?.into_iter().collect();
        let bytes = bytes.as_slice();
        from_slice(bytes).map_err(|_| from_slice(bytes).unwrap_or(UnknownError))
    }
}

impl LongPollServer {
    pub async fn wait_for_updates(&self, s_info: &SessionInfo) -> Result<LongPollServerResponse> {
        let act = "a_check";
        let Self { info, wait, mode, version, .. } = self;
        let LongPollServerInfo { key, server, ts, .. } = info;
        let server_request = server_request!(server, act, key, ts, wait, mode, version);
        s_info.converget(server_request).await
    }

    pub fn into_async_iter<'a>(self, s_info: &'a SessionInfo) -> LongPollServerIterator<'a> {
        LongPollServerIterator { lps: self, s_info }
    }
}

pub struct LongPollServerIterator<'a> {
    lps: LongPollServer,
    s_info: &'a SessionInfo,
}

impl<'a> LongPollServerIterator<'a> {
    pub async fn next(&mut self) -> Option<Vec<Vec<Value>>> {
        use LongPollServerFailure::*;
        let Self { lps, s_info } = self;
        let &mut LongPollServer { mode, group_id, version, .. } = lps;
        loop {
            match lps.wait_for_updates(s_info).await {
                Ok(lpsr) => {
                    lps.info.ts = lpsr.ts;
                    break Some(lpsr.updates);
                },
                Err(LPServerFailure(lpsf)) => {
                    match lpsf {
                        EventHistoryIsObsolete { new_ts } => lps.info.ts = new_ts,
                        KeyExpired => {
                            if let Ok(new_info) = s_info
                            .get_long_poll_server_info(mode & 32 != 0, group_id, version)
                            .await {
                                lps.info.key = new_info.key;
                            }
                        }
                        UserInfoLost => {
                            if let Ok(new_info) = s_info
                            .get_long_poll_server_info(mode & 32 != 0, group_id, version)
                            .await {
                                lps.info.key = new_info.key;
                                lps.info.ts = new_info.ts;
                            }
                        }
                        InvalidVersion {..} => break None,
                    }
                }
                _ => break None,
            }
        }
    }
}