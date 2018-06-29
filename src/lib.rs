#![allow(dead_code)]

extern crate chrono;
extern crate hyper;
extern crate reqwest;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

mod constants;
mod response;

use chrono::prelude::*;
use reqwest::header;
use serde::{de::Visitor, Deserialize, Deserializer, Serialize, Serializer};
use std::default::Default;
use std::fmt::Display;
use std::str::FromStr;

#[derive(Debug, Clone)]
struct EpitechClientBuilder {
    autologin: String,
}

#[derive(Debug, Clone)]
struct EpitechClient {
    autologin: String,
    client: reqwest::Client,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub enum Location {
    Bordeaux,
    LaReunion,
    Lille,
    Lyon,
    Marseille,
    Montpellier,
    Nancy,
    Nantes,
    Nice,
    Paris,
    Rennes,
    Strasbourg,
    Toulouse,
    Berlin,
    Barcelone,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub enum Promo {
    Tek1,
    Tek2,
    Tek3,
}

#[derive(Debug, Clone)]
pub struct StudentListFetchBuilder {
    client: EpitechClient,
    location: Option<Location>,
    promo: Option<Promo>,
    year: u32,
    course: String,
    active: bool,
    offset: u32,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
enum Error {
    InvalidStatusCode(u16),
    CookieNotFound,
    UnreachableRemote,
    InternalError,
}

impl EpitechClientBuilder {
    fn new() -> EpitechClientBuilder {
        EpitechClientBuilder {
            autologin: String::default(),
        }
    }

    fn autologin<T: Into<String>>(&mut self, autologin: T) -> &mut EpitechClientBuilder {
        self.autologin = autologin.into();
        self
    }

    fn authenticate(&mut self) -> Result<EpitechClient, Error> {
        let client = match reqwest::Client::builder()
            .redirect(reqwest::RedirectPolicy::none())
            .build()
        {
            Ok(x) => x,
            Err(_) => return Err(Error::InternalError),
        };
        match client.get(&self.autologin).send() {
            Ok(resp) => match resp.headers().get::<header::SetCookie>() {
                Some(cookie) => {
                    let mut headers = header::Headers::new();
                    let mut new_cookie = header::Cookie::new();
                    for it in cookie.iter() {
                        if it.starts_with("user=") {
                            let whole = it.split(";").nth(0).unwrap();
                            let name = String::from(&whole[0..4]);
                            let value = String::from(&whole[5..]);
                            new_cookie.append(name, value);
                        }
                    }
                    headers.set(new_cookie);
                    Ok(EpitechClient {
                        autologin: self.autologin.clone(),
                        client: match reqwest::Client::builder().default_headers(headers).build() {
                            Ok(x) => x,
                            Err(_) => return Err(Error::InternalError),
                        },
                    })
                }
                None => Err(Error::CookieNotFound),
            },
            Err(err) => {
                let status = err.status();
                match status {
                    Some(status) => Err(Error::InvalidStatusCode(status.as_u16())),
                    None => Err(Error::UnreachableRemote),
                }
            }
        }
    }
}

impl EpitechClient {
    fn builder() -> EpitechClientBuilder {
        EpitechClientBuilder::new()
    }

    fn make_request<T: ToString>(&self, url: T) -> Option<String> {
        let mut string = url.to_string();
        if !string.contains("&format=json") && !string.contains("?format=json") {
            let b = string.contains("?");
            string.push(if b { '&' } else { '?' });
            string.push_str("format=json");
        }
        if !string.starts_with(constants::ENDPOINT) {
            string.insert_str(0, constants::ENDPOINT);
        }
        self.client
            .get(&string)
            .send()
            .and_then(|mut val| val.text())
            .ok()
    }

    fn fetch_student_list(&mut self) -> StudentListFetchBuilder {
        let mut builder = StudentListFetchBuilder::new();
        builder.client(self.clone());
        builder
    }
}

impl Default for EpitechClient {
    fn default() -> EpitechClient {
        EpitechClient {
            autologin: String::from("NotAssigned"),
            client: reqwest::Client::new(),
        }
    }
}

impl StudentListFetchBuilder {
    fn new() -> StudentListFetchBuilder {
        StudentListFetchBuilder {
            client: EpitechClient::default(),
            location: None,
            promo: None,
            active: true,
            offset: 0,
            year: Local::now().date().year() as u32,
            course: String::from("bachelor/classic"),
        }
    }

    fn send(&mut self) -> Option<Vec<response::UserEntry>> {
        let mut url = String::from(format!("/user/filter/user?offset={}", self.offset));
        match self.location {
            Some(ref location) => url.push_str(format!("&location={}", location).as_ref()),
            None => {}
        };
        match self.promo {
            Some(ref promo) => url.push_str(format!("&promo={}", promo).as_ref()),
            None => {}
        };
        url.push_str(format!("&year={}", self.year).as_ref());
        url.push_str(format!("&course={}", self.course).as_ref());
        url.push_str(format!("&active={}", self.active).as_ref());
        self.client
            .make_request(&url)
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .and_then(|val| {
                val.as_object()
                    .and_then(|object| object.get("items"))
                    .and_then(|item| {
                        serde_json::from_value::<Vec<response::UserEntry>>(item.clone()).ok()
                    })
            })
    }

    fn client(&mut self, client: EpitechClient) -> &mut StudentListFetchBuilder {
        self.client = client;
        self
    }

    fn location(&mut self, location: Location) -> &mut StudentListFetchBuilder {
        self.location = Some(location);
        self
    }

    fn active(&mut self, active: bool) -> &mut StudentListFetchBuilder {
        self.active = active;
        self
    }

    fn offset(&mut self, offset: u32) -> &mut StudentListFetchBuilder {
        self.offset = offset;
        self
    }

    fn year(&mut self, year: u32) -> &mut StudentListFetchBuilder {
        self.year = year;
        self
    }

    fn promo(&mut self, promo: Promo) -> &mut StudentListFetchBuilder {
        self.promo = Some(promo);
        self
    }

    fn course(&mut self, course: String) -> &mut StudentListFetchBuilder {
        self.course = course;
        self
    }
}

impl<'de> Deserialize<'de> for Location {
    fn deserialize<D>(deserializer: D) -> Result<Location, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StringVisitor;

        impl<'a> Visitor<'a> for StringVisitor {
            type Value = String;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(
                    formatter,
                    "a string formatted like '<Country>/<City>' (eg. 'FR/STG')"
                )
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(v.to_owned())
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(v)
            }
        }

        deserializer
            .deserialize_string(StringVisitor)
            .and_then(|val| {
                val.parse()
                    .map_err(|_| serde::de::Error::custom("Error deserializing Location."))
            })
    }
}

impl Serialize for Location {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

impl FromStr for Location {
    type Err = ();
    fn from_str(string: &str) -> Result<Self, Self::Err> {
        for it in constants::LOCATION_TABLE.iter() {
            if string == it.1 {
                return Ok(it.0.clone());
            }
        }
        Err(())
    }
}

impl Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for it in constants::LOCATION_TABLE.iter() {
            if *self == it.0 {
                return write!(f, "{}", it.1);
            }
        }
        write!(f, "Unknown")
    }
}

impl FromStr for Promo {
    type Err = ();
    fn from_str(string: &str) -> Result<Self, Self::Err> {
        for it in constants::PROMO_TABLE.iter() {
            if string == it.1 {
                return Ok(it.0.clone());
            }
        }
        Err(())
    }
}

impl Display for Promo {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for it in constants::PROMO_TABLE.iter() {
            if *self == it.0 {
                return write!(f, "{}", it.1);
            }
        }
        write!(f, "Unknown")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn get_client() -> Result<EpitechClient, Error> {
        let mut contents = String::default();
        std::fs::File::open("test-config.json")
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();
        let val = String::from(
            serde_json::from_str::<serde_json::Value>(&contents)
                .unwrap()
                .as_object()
                .unwrap()
                .get("autologin")
                .unwrap()
                .as_str()
                .unwrap(),
        );
        EpitechClient::builder().autologin(val).authenticate()
    }

    #[test]
    fn auth_unreachable_remote() {
        let ret = EpitechClient::builder().autologin("toto").authenticate();
        assert!(ret.is_err());
        let api = ret.unwrap_err();
        assert!(api == Error::UnreachableRemote);
    }

    #[test]
    fn auth_working_link() {
        let api = get_client();
        assert!(api.is_ok());
    }

    #[test]
    fn fetch_student_list() {
        let ret = get_client();
        assert!(ret.is_ok());
        let mut api = ret.unwrap();
        let list = api.fetch_student_list()
            .location(Location::Strasbourg)
            .promo(Promo::Tek2)
            .year(2017)
            .send();
        assert!(list.is_some());
    }

}
