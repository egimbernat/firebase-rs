use std::collections::HashMap;
use url::Url;
use crate::{Firebase, UrlParseError};
use crate::constants::{END_AT, EQUAL_TO, EXPORT, FORMAT, LIMIT_TO_FIRST, LIMIT_TO_LAST, ORDER_BY, SHALLOW, START_AT};

pub struct Params {
    pub uri: Url,
    pub params: HashMap<&'static str, String>,
}

impl Params {
    pub fn new(uri: Url) -> Self {
        Self {
            uri,
            params: Default::default(),
        }
    }

    pub fn set_params(&mut self) -> () {
        for (k, v) in self.params.iter() {
            self.uri.set_query(Some(format!("{}={}", k, v).as_str()));
        }
    }

    pub fn add_param<T: ToString>(&mut self, key: &'static str, value: T) -> &mut Self {
        let value = value.to_string();
        self.params.insert(key, value);
        self.set_params();

        self
    }

    pub fn order_by(&mut self, key: &'static str) -> &mut Params {
        self.add_param(ORDER_BY, key)
    }

    pub fn limit_to_first(&mut self, count: u32) -> &mut Params {
        self.add_param(LIMIT_TO_FIRST, count)
    }

    pub fn limit_to_last(&mut self, count: u32) -> &mut Params {
        self.add_param(LIMIT_TO_LAST, count)
    }

    pub fn start_at(&mut self, index: u32) -> &mut Params {
        self.add_param(START_AT, index)
    }

    pub fn end_at(&mut self, index: u32) -> &mut Params {
        self.add_param(END_AT, index)
    }

    pub fn equal_to(&mut self, value: u32) -> &mut Params {
        self.add_param(EQUAL_TO, value)
    }

    pub fn shallow(&mut self, flag: bool) -> &mut Params {
        self.add_param(SHALLOW, flag)
    }

    pub fn format(&mut self) -> &mut Params {
        self.add_param(FORMAT, EXPORT)
    }

    pub fn finish(&self) -> Firebase {
        Firebase::new(self.uri.to_string()).unwrap()
    }
}