pub const AUTH: &str = "auth";

pub const ACCESS_TOKEN: &str = "access_token";
pub const ORDER_BY: &str = "orderBy";
pub const LIMIT_TO_FIRST: &str = "limitToFirst";
pub const LIMIT_TO_LAST: &str = "limitToLast";
pub const START_AT: &str = "startAt";
pub const END_AT: &str = "endAt";
pub const EQUAL_TO: &str = "equalTo";
pub const SHALLOW: &str = "shallow";
pub const FORMAT: &str = "format";
pub const EXPORT: &str = "export";

pub const SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/userinfo.email",
    "https://www.googleapis.com/auth/firebase.database",
];

pub const GCP_TOKEN_LENGTH: u64 = 3500;

#[derive(Debug)]
pub enum Method {
    GET,
    POST,
    PUT,
    DELETE,
    PATCH,
}

#[derive(Debug)]
pub struct Response {
    pub data: String,
}

impl Response {
    pub fn new() -> Self {
        Self {
            data: String::default(),
        }
    }
}
