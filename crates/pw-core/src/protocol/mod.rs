// Copyright 2024 Paul Adamson
// Licensed under the Apache License, Version 2.0
//
// Protocol Objects - Rust representations of Playwright protocol objects
//
// This module contains the Rust implementations of all Playwright protocol objects.
// Each object corresponds to a type in the Playwright protocol (protocol.yml).
//
// Architecture:
// - All protocol objects implement the ChannelOwner trait
// - Objects are created by the object factory when server sends __create__ messages
// - Objects communicate with the server via their Channel

pub mod action_options;
pub mod artifact;
pub mod browser;
pub mod browser_context;
pub mod browser_type;
pub mod click;
pub mod cookie;
pub mod dialog;
pub mod download;
pub mod element_handle;
pub mod events;
pub mod file_payload;
pub mod frame;
pub mod keyboard;
pub mod locator;
pub mod mouse;
pub mod page;
pub mod playwright;
pub mod request;
pub mod response;
pub mod root;
pub mod route;
pub mod screenshot;
pub mod select_option;
pub mod tracing;

pub use action_options::{
    CheckOptions, FillOptions, HoverOptions, KeyboardOptions, MouseOptions, PressOptions,
    SelectOptions,
};
pub use browser::Browser;
pub use browser_context::{
    BrowserContext, BrowserContextOptions, BrowserContextOptionsBuilder, Geolocation, Viewport,
};
pub use browser_type::{BrowserType, ConnectOverCDPResult, LaunchedServer};
pub use click::{ClickOptions, KeyboardModifier, MouseButton, Position};
pub use cookie::{
    ClearCookiesOptions, Cookie, LocalStorageEntry, OriginState, SameSite, StorageState,
    StorageStateOptions,
};
pub use dialog::Dialog;
pub use download::Download;
pub use element_handle::ElementHandle;
pub use events::{ConsoleSubscription, EventStream, EventWaiter};
pub use file_payload::{FilePayload, FilePayloadBuilder};
pub use frame::Frame;
pub use keyboard::Keyboard;
pub use locator::Locator;
pub use mouse::Mouse;
pub use page::{
    ConsoleLocation, ConsoleMessage, ConsoleMessageKind, GotoOptions, Page, Response, Subscription,
    WaitUntil,
};
pub use playwright::Playwright;
pub use request::Request;
pub use response::ResponseObject;
pub use root::Root;
pub use route::{
    ContinueOptions, ContinueOptionsBuilder, FulfillOptions, FulfillOptionsBuilder, Route,
};
pub use screenshot::{ScreenshotClip, ScreenshotOptions, ScreenshotType};
pub use select_option::SelectOption;
pub use tracing::{
    Tracing, TracingStartChunkOptions, TracingStartOptions, TracingStartOptionsBuilder,
    TracingStopOptions,
};
