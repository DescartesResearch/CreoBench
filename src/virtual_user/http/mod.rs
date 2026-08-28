//! This module provides the [`HttpExecutor`] for sending a HTTP request defined by a
//! [`HTTPStaticRequestSpec`][`crate::script::HTTPStaticRequestSpec`].

mod cookies;
mod error;
mod executor;

#[doc(inline)]
pub(super) use cookies::CookieJar;
#[doc(inline)]
pub(super) use error::HttpExecuteError;
#[doc(inline)]
pub(super) use error::UrlError;
#[doc(inline)]
pub(super) use executor::HttpExecutor;
