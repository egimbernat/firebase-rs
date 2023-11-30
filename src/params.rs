use crate::constants::{
    END_AT, EQUAL_TO, EXPORT, FORMAT, LIMIT_TO_FIRST, LIMIT_TO_LAST, ORDER_BY, SHALLOW, START_AT,
};
use crate::FirebaseSession;
use itertools::Itertools;
use std::collections::HashMap;


pub struct Params {
    pub fb: FirebaseSession,
    pub params: HashMap<String, String>,
}

impl Params {
    pub fn new(fb: FirebaseSession) -> Self {
        Self {
            fb,
            params: Default::default(),
        }
    }

    pub fn set_params(&mut self) -> () {
        for (k, v) in self.params.iter().sorted() {
            self.fb.uri.query_pairs_mut().append_pair(k, v);
        }
    }

    pub fn add_param<T>(&mut self, key: &str, value: T) -> &mut Self
    where
        T: ToString,
    {
        self.params.insert(key.to_string(), value.to_string());
        self.set_params();

        self
    }

    pub fn order_by(&mut self, key: &str) -> &mut Params {
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

    pub fn finish(&self) -> FirebaseSession {
        self.fb.clone()
    }
}

#[cfg(test)]
mod tests {
    use crate::params::Params;
    use std::collections::HashMap;
    use std::sync::Arc;
    use crate::Firebase;

    #[test]
    fn check_params() {
        let mut params: HashMap<String, String> = HashMap::new();
        params.insert("param_1".to_owned(), "value_1".to_owned());
        params.insert("param_2".to_owned(), "value_2".to_owned());

        let firebase = Arc::new(Firebase::new("https://github.com/emreyalvac").unwrap());

        let mut param = Params {
           fb: Firebase::new_session(firebase),
            params,
        };
        param.set_params();

        assert_eq!(
            param.fb.uri.as_str(),
            "https://github.com/emreyalvac?param_1=value_1&param_2=value_2"
        )
    }
}
