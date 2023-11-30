use crate::constants::{ACCESS_TOKEN, GCP_TOKEN_LENGTH, SCOPES};
use crate::sse::ServerEvents;
use anyhow::{anyhow, Error};
use constants::{Method, Response, AUTH};
use errors::{RequestError, RequestResult, UrlParseError, UrlParseResult};
use gcp_auth::{AuthenticationManager, CustomServiceAccount};
use params::Params;
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use std::fmt::Debug;
use std::sync::Arc;
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_retry::RetryTransientMiddleware;
use reqwest_middleware::ClientBuilder;
use tokio::sync::RwLock;
use tokio::time::Instant;
use url::Url;
use utils::check_uri;

mod constants;
mod errors;
mod params;
mod sse;
mod utils;

pub struct Firebase {
    uri: Url,
    auth_token: RwLock<(String, Instant)>,
    gcp_manager: Option<AuthenticationManager>,
}

#[derive(Clone)]
pub struct FirebaseSession {
    db: Arc<Firebase>,
    uri: Url,
}

impl Firebase {
    /// ```rust
    /// use firebase_rs::Firebase;
    ///
    /// let firebase = Firebase::new("https://myfirebase.firebaseio.com").unwrap();
    /// ```
    pub fn new(uri: &str) -> UrlParseResult<Self>
    where
        Self: Sized,
    {
        match check_uri(&uri) {
            Ok(uri) => Ok(Self {
                uri,
                auth_token: RwLock::from(("".to_string(), Instant::now())),
                gcp_manager: None,
            }),
            Err(err) => Err(err),
        }
    }

    /// ```rust
    /// const URI: &str = "...";
    /// const AUTH_KEY: &str = "...";
    ///
    /// use firebase_rs::Firebase;
    ///
    /// let firebase = Firebase::auth("https://myfirebase.firebaseio.com", AUTH_KEY).unwrap();
    /// ```
    pub fn auth(uri: &str, auth_key: &str) -> UrlParseResult<Self>
    where
        Self: Sized,
    {
        match check_uri(&uri) {
            Ok(uri) => Ok(Self {
                uri,
                auth_token: RwLock::from((auth_key.to_string(), Instant::now())),
                gcp_manager: None,
            }),
            Err(err) => Err(err),
        }
    }

    pub async fn auth_from_json(uri: &str, file_json: &str) -> Result<Self, Error> {
        // `credentials_path` variable is the path for the credentials `.json` file.
        let service_account = CustomServiceAccount::from_json(file_json)?;
        let authentication_manager = AuthenticationManager::from(service_account);

        let firebase = match check_uri(&uri) {
            Ok(uri) => {
                let auth_key = authentication_manager.get_token(SCOPES).await?;
                Ok(Self {
                    uri,
                    auth_token: RwLock::from((auth_key.as_str().to_string(), Instant::now())),
                    gcp_manager: Some(authentication_manager),
                })
            }
            Err(err) => Err(err),
        }
        .map_err(|err| anyhow!(err.to_string()))?;

        Ok(firebase)
    }

    async fn get_token(&self) -> Result<String, Error> {
        let at = self.auth_token.read().await;
        match self.gcp_manager.as_ref() {
            None => Ok(at.0.clone()),
            Some(manager) => {
                if at.1.elapsed().as_secs() >= GCP_TOKEN_LENGTH {
                    let token_result = manager.get_token(SCOPES).await?;
                    drop(at); //We need write access
                    let mut at = self.auth_token.write().await;
                    *at = (token_result.as_str().to_string(), Instant::now());
                    Ok(token_result.as_str().to_string())
                } else {
                    Ok(at.0.clone())
                }
            }
        }
    }
    pub fn new_session(db: Arc<Firebase>) -> FirebaseSession {
        FirebaseSession {
            uri: db.uri.clone(),
            db,
        }
    }
}

impl FirebaseSession {
    /// ```rust
    /// use std::sync::Arc;
    /// use firebase_rs::Firebase;
    ///
    /// # async fn run() {
    /// let firebase = Arc::new(Firebase::new("https://myfirebase.firebaseio.com").unwrap());
    /// let firebase_session = Firebase::new_session(firebase).at("users").with_params().start_at(1).order_by("name").equal_to(5).finish();
    /// let result = firebase_session.get::<String>().await;
    /// # }
    /// ```
    pub fn with_params(&self) -> Params {
        Params::new(self.clone())
    }

    /// To use simple interface with synchronous callbacks, pair with `.listen()`:
    /// ```rust
    /// use std::sync::Arc;
    /// use firebase_rs::Firebase;
    /// # async fn run() {
    /// let firebase = Arc::new(Firebase::new("https://myfirebase.firebaseio.com").unwrap());
    /// let firebase_session = Firebase::new_session(firebase).at("users");
    /// let stream = firebase_session.with_realtime_events().unwrap();
    /// stream.listen(|event_type, data| {
    ///                     println!("{:?} {:?}" ,event_type, data);
    ///                 }, |err| println!("{:?}" ,err), false).await;
    /// # }
    /// ```
    ///
    /// To use streaming interface for async code, pair with `.stream()`:
    /// ```rust
    /// use std::sync::Arc;
    /// use firebase_rs::Firebase;
    /// use futures_util::StreamExt;
    ///
    /// # async fn run() {
    /// let firebase = Arc::new(Firebase::new("https://myfirebase.firebaseio.com").unwrap());
    /// let firebase_session = Firebase::new_session(firebase).at("users");
    /// let stream = firebase_session.with_realtime_events()
    ///              .unwrap()
    ///              .stream(true);
    /// stream.for_each(|event| {
    ///           match event {
    ///               Ok((event_type, maybe_data)) => println!("{:?} {:?}" ,event_type, maybe_data),
    ///               Err(err) => println!("{:?}" ,err),
    ///           }
    ///           futures_util::future::ready(())
    ///        }).await;
    /// # }
    /// ```
    pub fn with_realtime_events(&self) -> Option<ServerEvents> {
        ServerEvents::new(self.uri.as_str())
    }

    /// ```rust
    /// use std::sync::Arc;
    /// use firebase_rs::Firebase;
    /// let firebase = Arc::new(Firebase::new("https://myfirebase.firebaseio.com").unwrap());
    /// let firebase_session = Firebase::new_session(firebase).at("users").at("USER_ID").at("f69111a8a5258c15286d3d0bd4688c55");
    /// ```
    pub fn at(&self, path: &str) -> Self {
        let mut new_path = String::default();

        let paths = self.uri.path_segments().map(|p| p.collect::<Vec<_>>());
        for mut path in paths.unwrap() {
            if path.find(".json").is_some() {
                path = path.trim_end_matches(".json");
            }
            new_path += format!("{}/", path).as_str();
        }

        new_path += path;

        if new_path.find(".json").is_some() {
            new_path = new_path.trim_end_matches(".json").to_string();
        }

        let mut uri = self.uri.clone();
        uri.set_path(&format!("{}.json", new_path));

        Self {
            db: self.db.clone(),
            uri,
        }
    }

    /// ```rust
    /// use std::sync::Arc;
    /// use firebase_rs::Firebase;
    ///
    /// let firebase = Arc::new(Firebase::new("https://myfirebase.firebaseio.com").unwrap());
    /// let firebase_session = Firebase::new_session(firebase).at("users");
    /// let uri = firebase_session.get_uri();
    /// ```
    pub async fn get_uri(&self) -> String {
        let mut uri = self.uri.clone();
        if !self.db.auth_token.read().await.0.is_empty() {
            uri.set_query(Some(&format!(
                "{}={}",
                if self.db.gcp_manager.is_some() {
                    ACCESS_TOKEN
                } else {
                    AUTH
                },
                self.db.get_token().await.unwrap_or_default()
            )));
        }
        uri.to_string()
    }

    async fn request(&self, method: Method, data: Option<Value>) -> RequestResult<Response> {
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(5);
        let client = ClientBuilder::new(reqwest::Client::new())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        let uri = self.get_uri().await;

        return match method {
            Method::GET => {
                let request = client.get(uri.to_string()).send().await;
                match request {
                    Ok(response) => {
                        if response.status() == StatusCode::from_u16(200).unwrap() {
                            return match response.text().await {
                                Ok(data) => {
                                    if data.as_str() == "null" {
                                        return Err(RequestError::NotFoundOrNullBody);
                                    }
                                    return Ok(Response { data });
                                }
                                Err(_) => Err(RequestError::NotJSON),
                            };
                        } else {
                            Err(RequestError::NetworkError)
                        }
                    }
                    Err(_) => return Err(RequestError::NetworkError),
                }
            }
            Method::POST => {
                if !data.is_some() {
                    return Err(RequestError::SerializeError);
                }

                let request = client.post(uri.to_string()).json(&data).send().await;
                match request {
                    Ok(response) => {
                        let data = response.text().await.unwrap();
                        Ok(Response { data })
                    }
                    Err(_) => Err(RequestError::NetworkError),
                }
            }
            Method::PUT => {
                if !data.is_some() {
                    return Err(RequestError::SerializeError);
                }

                let request = client.put(uri.to_string()).json(&data).send().await;
                match request {
                    Ok(response) => {
                        let data = response.text().await.unwrap();
                        Ok(Response { data })
                    }
                    Err(_) => Err(RequestError::NetworkError),
                }
            }
            Method::PATCH => {
                if !data.is_some() {
                    return Err(RequestError::SerializeError);
                }

                let request = client.patch(uri.to_string()).json(&data).send().await;
                match request {
                    Ok(response) => {
                        let data = response.text().await.unwrap();
                        Ok(Response { data })
                    }
                    Err(_) => Err(RequestError::NetworkError),
                }
            }
            Method::DELETE => {
                let request = client.delete(uri.to_string()).send().await;
                match request {
                    Ok(_) => Ok(Response {
                        data: String::default(),
                    }),
                    Err(_) => Err(RequestError::NetworkError),
                }
            }
        };
    }

    async fn request_generic<T>(&self, method: Method) -> RequestResult<T>
    where
        T: Serialize + DeserializeOwned + Debug,
    {
        let request = self.request(method, None).await;

        match request {
            Ok(response) => {
                let data: T = serde_json::from_str(response.data.as_str()).unwrap();

                Ok(data)
            }
            Err(err) => Err(err),
        }
    }

    /// ```rust
    /// use std::sync::Arc;
    /// use firebase_rs::Firebase;
    /// use serde::{Serialize, Deserialize};
    ///
    /// #[derive(Serialize, Deserialize, Debug)]
    /// struct User {
    ///    name: String
    /// }
    ///
    /// # async fn run() {
    /// let user = User { name: String::default() };
    /// let firebase = Arc::new(Firebase::new("https://myfirebase.firebaseio.com").unwrap());
    /// let firebase_session = Firebase::new_session(firebase).at("users");
    /// let users = firebase_session.set(&user).await;
    /// # }
    /// ```
    pub async fn set<T>(&self, data: &T) -> RequestResult<Response>
    where
        T: Serialize + DeserializeOwned + Debug,
    {
        let data = serde_json::to_value(&data).unwrap();
        self.request(Method::PUT, Some(data)).await
    }

    /// ```
    /// use std::sync::Arc;
    /// use firebase_rs::Firebase;
    /// use serde::{Serialize, Deserialize};
    ///
    /// #[derive(Serialize, Deserialize, Debug)]
    /// struct User {
    ///    name: String
    /// }
    ///
    /// # async fn run() {
    /// let user = User { name: String::default() };
    /// let firebase = Arc::new(Firebase::new("https://myfirebase.firebaseio.com").unwrap());
    /// let firebase_session = Firebase::new_session(firebase).at("users");
    /// let users = firebase_session.insert(&user).await;
    /// # }
    /// ```
    pub async fn insert<T>(&self, data: &T) -> RequestResult<Response>
    where
        T: Serialize + DeserializeOwned + Debug,
    {
        let data = serde_json::to_value(&data).unwrap();
        self.request(Method::POST, Some(data)).await
    }

    /// ```rust
    /// use std::collections::HashMap;
    /// use std::sync::Arc;
    /// use firebase_rs::{Firebase, FirebaseSession};
    /// use serde::{Serialize, Deserialize};
    ///
    /// #[derive(Serialize, Deserialize, Debug)]
    /// struct User {
    ///    name: String
    /// }
    ///
    /// # async fn run() {
    /// let firebase = Arc::new(Firebase::new("https://myfirebase.firebaseio.com").unwrap());
    /// let firebase_session = Firebase::new_session(firebase).at("users");
    /// let users = firebase_session.get::<HashMap<String, User>>().await;
    /// # }
    /// ```
    pub async fn get_as_string(&self) -> RequestResult<Response> {
        self.request(Method::GET, None).await
    }

    /// ```rust
    /// use std::collections::HashMap;
    /// use std::sync::Arc;
    /// use firebase_rs::Firebase;
    /// use serde::{Serialize, Deserialize};
    ///
    /// #[derive(Serialize, Deserialize, Debug)]
    /// struct User {
    ///     name: String
    /// }
    ///
    /// # async fn run() {
    /// let firebase = Arc::new(Firebase::new("https://myfirebase.firebaseio.com").unwrap());
    /// let firebase_session = Firebase::new_session(firebase).at("users");
    /// let user = firebase_session.get::<User>().await;
    ///
    ///  // OR
    ///
    /// let firebase = Arc::new(Firebase::new("https://myfirebase.firebaseio.com").unwrap());
    /// let firebase_session = Firebase::new_session(firebase).at("users");
    /// let user = firebase_session.get::<HashMap<String, User>>().await;
    /// # }
    /// ```
    pub async fn get<T>(&self) -> RequestResult<T>
    where
        T: Serialize + DeserializeOwned + Debug,
    {
        self.request_generic::<T>(Method::GET).await
    }

    /// ```rust
    /// use std::sync::Arc;
    /// use firebase_rs::Firebase;
    ///
    /// # async fn run() {
    /// let firebase = Arc::new(Firebase::new("https://myfirebase.firebaseio.com").unwrap());
    /// let firebase_session = Firebase::new_session(firebase).at("users").at("USER_ID");
    /// firebase_session.delete().await;
    /// # }
    /// ```
    pub async fn delete(&self) -> RequestResult<Response> {
        self.request(Method::DELETE, None).await
    }

    /// ```rust
    /// use std::sync::Arc;
    /// use firebase_rs::Firebase;
    /// use serde::{Serialize, Deserialize};
    ///
    /// #[derive(Serialize, Deserialize, Debug)]
    /// struct User {
    ///     name: String
    /// }
    ///
    /// # async fn run() {
    /// let firebase = Arc::new(Firebase::new("https://myfirebase.firebaseio.com").unwrap());
    /// let firebase_session = Firebase::new_session(firebase).at("users").at("users").at("USER_ID");
    /// let users = firebase_session.update(&User{name: "".to_string()}).await;
    /// # }
    /// ```
    pub async fn update<T>(&self, data: &T) -> RequestResult<Response>
    where
        T: DeserializeOwned + Serialize + Debug,
    {
        let value = serde_json::to_value(&data).unwrap();
        self.request(Method::PATCH, Some(value)).await
    }
}

#[cfg(test)]
mod tests {
    use crate::{Firebase, UrlParseError};
    use std::sync::Arc;

    const URI: &str = "https://firebase_id.firebaseio.com";
    const URI_WITH_SLASH: &str = "https://firebase_id.firebaseio.com/";
    const URI_NON_HTTPS: &str = "http://firebase_id.firebaseio.com/";

    #[tokio::test]
    async fn simple() {
        let firebase = Arc::new(Firebase::new(URI).unwrap());

        assert_eq!(
            URI_WITH_SLASH.to_string(),
            Firebase::new_session(firebase).get_uri().await
        );
    }

    #[tokio::test]
    async fn non_https() {
        let firebase = Firebase::new(URI_NON_HTTPS).map_err(|e| e.to_string());
        assert_eq!(
            firebase.err(),
            Some(String::from(UrlParseError::NotHttps.to_string()))
        );
    }

    #[tokio::test]
    async fn with_auth() {
        let firebase = Arc::new(Firebase::auth(URI, "auth_key").unwrap());
        assert_eq!(
            format!("{}/?auth=auth_key", URI.to_string()),
            Firebase::new_session(firebase).get_uri().await
        );
    }

    #[tokio::test]
    async fn with_sse_events() {
        // TODO: SSE Events Test
    }
}
