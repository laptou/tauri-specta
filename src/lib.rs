//! Typesafe Tauri commands
//!
//! ## Install
//!
//! ```bash
//! cargo add specta
//! cargo add tauri-specta --features javascript,typescript
//! ```
//!
//! ## Adding Specta to custom types
//!
//! ```rust
//! use specta::Type;
//! use serde::{Deserialize, Serialize};
//!
//! // The `specta::Type` macro allows us to understand your types
//! // We implement `specta::Type` on primitive types for you.
//! // If you want to use a type from an external crate you may need to enable the feature on Specta.
//! #[derive(Serialize, Type)]
//! pub struct MyCustomReturnType {
//!     pub some_field: String,
//! }
//!
//! #[derive(Deserialize, Type)]
//! pub struct MyCustomArgumentType {
//!     pub foo: String,
//!     pub bar: i32,
//! }
//! ```
//!
//! ## Annotate your Tauri commands with Specta
//!
//! ```rust
//! # //! #[derive(Serialize, Type)]
//! # pub struct MyCustomReturnType {
//! #    pub some_field: String,
//! # }
//! #[tauri::command]
//! #[specta::specta] // <-- This bit here
//! fn greet3() -> MyCustomReturnType {
//!     MyCustomReturnType {
//!         some_field: "Hello World".into(),
//!     }
//! }
//!
//! #[tauri::command]
//! #[specta::specta] // <-- This bit here
//! fn greet(name: String) -> String {
//!   format!("Hello {name}!")
//! }
//! ```
//!
//! ## Export your bindings
//!
//! ```rust
//! # #[specta::specta]
//! # fn greet() {}
//! # #[specta::specta]
//! # fn greet2() {}
//! # #[specta::specta]
//! # fn greet3() {}
//! use specta::collect_types;
//! use tauri_specta::{ts, js};
//!
//! // this example exports your types on startup when in debug mode or in a unit test. You can do whatever.
//! fn main() {
//!     #[cfg(debug_assertions)]
//!     ts::export(collect_types![greet, greet2, greet3], "../src/bindings.ts").unwrap();
//!
//!     // or export to JS with JSDoc
//!     #[cfg(debug_assertions)]
//!     js::export(collect_types![greet, greet2, greet3], "../src/bindings.js").unwrap();
//! }
//!
//! #[test]
//! fn export_bindings() {
//!     ts::export(collect_types![greet, greet2, greet3], "../src/bindings.ts").unwrap();
//!     js::export(collect_types![greet, greet2, greet3], "../src/bindings.js").unwrap();
//! }
//! ```
//!
//! ## Usage on frontend
//!
//! ```ts
//! import * as commands from "./bindings"; // This should point to the file we export from Rust
//!
//! await commands.greet("Brendan");
//! ```
//!
#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::unwrap_used, clippy::panic
	// , missing_docs
)]
#![cfg_attr(docsrs, feature(doc_cfg))]

use std::{
    borrow::Cow,
    fs::{self, File},
    io::Write,
    marker::PhantomData,
    path::{Path, PathBuf},
};

use crate::ts::ExportConfig;
use specta::{functions::FunctionDataType, ts::TsExportError, ExportError, TypeMap};

use tauri::{Invoke, Manager, Runtime};
pub use tauri_specta_macros::Event;

/// The exporter for [Javascript](https://www.javascript.com).
#[cfg(feature = "javascript")]
#[cfg_attr(docsrs, doc(cfg(feature = "javascript")))]
pub mod js;

/// The exporter for [TypeScript](https://www.typescriptlang.org).
#[cfg(feature = "typescript")]
#[cfg_attr(docsrs, doc(cfg(feature = "typescript")))]
pub mod ts;

mod event;

pub use event::*;

pub type CollectCommandsTuple<TInvokeHandler> = (
    Result<(Vec<FunctionDataType>, TypeMap), ExportError>,
    TInvokeHandler,
);

#[macro_export]
macro_rules! collect_commands {
 	(type_map: $type_map:ident, $($command:path),*) => {
        (
        	specta::collect_functions![$type_map; $($command),*],
       		::tauri::generate_handler![$($command),*],
        )
    };
    ($($command:path),*) => {{
        let mut type_map = specta::TypeMap::default();
        $crate::collect_commands![type_map: type_map, $($command),*]
    }};
}

pub(crate) const DO_NOT_EDIT: &str = "// This file was generated by [tauri-specta](https://github.com/oscartbeaumont/tauri-specta). Do not edit this file manually.";

pub(crate) const CRINGE_ESLINT_DISABLE: &str = "/* eslint-disable */
";

// TODO
// #[cfg(doctest)]
// doc_comment::doctest!("../README.md");

/// A set of functions that produce language-specific code
pub trait ExportLanguage: 'static {
    /// Type definitions and constants that the generated functions rely on
    fn globals() -> String;

    fn render_events(
        events: &[EventDataType],
        type_map: &TypeMap,
        cfg: &ExportConfig,
    ) -> Result<String, TsExportError>;

    /// Renders a collection of [`FunctionDataType`] into a string.
    fn render_commands(
        commands: &[FunctionDataType],
        type_map: &TypeMap,
        cfg: &ExportConfig,
    ) -> Result<String, TsExportError>;

    /// Renders the output of [`globals`], [`render_functions`] and all dependant types into a TypeScript string.
    fn render(
        commands: &[FunctionDataType],
        events: &[EventDataType],
        type_map: &TypeMap,
        cfg: &ExportConfig,
    ) -> Result<String, TsExportError>;
}

pub trait CommandsTypeState: 'static {
    type Runtime: tauri::Runtime;
    type InvokeHandler: Fn(tauri::Invoke<Self::Runtime>) + Send + Sync + 'static;

    fn split(self) -> CollectCommandsTuple<Self::InvokeHandler>;
}

fn dummy_invoke_handler(_: Invoke<impl Runtime>) {}

pub struct NoCommands<TRuntime>(PhantomData<TRuntime>);

impl<TRuntime> CommandsTypeState for NoCommands<TRuntime>
where
    TRuntime: tauri::Runtime,
{
    type Runtime = TRuntime;
    type InvokeHandler = fn(Invoke<TRuntime>);

    fn split(self) -> CollectCommandsTuple<Self::InvokeHandler> {
        (Ok(Default::default()), dummy_invoke_handler)
    }
}

pub struct Commands<TRuntime, TInvokeHandler>(
    CollectCommandsTuple<TInvokeHandler>,
    PhantomData<TRuntime>,
);

impl<TRuntime, TInvokeHandler> CommandsTypeState for Commands<TRuntime, TInvokeHandler>
where
    TRuntime: tauri::Runtime,
    TInvokeHandler: Fn(tauri::Invoke<TRuntime>) + Send + Sync + 'static,
{
    type Runtime = TRuntime;
    type InvokeHandler = TInvokeHandler;

    fn split(self) -> CollectCommandsTuple<TInvokeHandler> {
        self.0
    }
}

pub trait EventsTypeState: 'static {
    fn get(self) -> CollectEventsTuple;
}

pub struct NoEvents;

impl EventsTypeState for NoEvents {
    fn get(self) -> CollectEventsTuple {
        (Default::default(), Ok(vec![]), Default::default())
    }
}

pub struct Events(CollectEventsTuple);

impl EventsTypeState for Events {
    fn get(self) -> CollectEventsTuple {
        self.0
    }
}

/// General exporter, takes a generic for the specific language that is being exported to.
pub struct Exporter<TLang, TCommands, TEvents> {
    export_path: PathBuf,
    lang: PhantomData<TLang>,
    commands: TCommands,
    events: TEvents,
    cfg: ExportConfig,
    header: Cow<'static, str>,
}

impl<TLang, TRuntime> Exporter<TLang, NoCommands<TRuntime>, NoEvents> {
    pub fn new(export_path: impl AsRef<Path>) -> Self {
        Self {
            export_path: export_path.as_ref().into(),
            lang: PhantomData,
            commands: NoCommands(Default::default()),
            events: NoEvents,
            cfg: ExportConfig::default(),
            header: CRINGE_ESLINT_DISABLE.into(),
        }
    }
}

impl<TLang, TEvents, TRuntime> Exporter<TLang, NoCommands<TRuntime>, TEvents>
where
    TRuntime: tauri::Runtime,
{
    pub fn commands<TInvokeHandler: Fn(tauri::Invoke<TRuntime>) + Send + Sync + 'static>(
        self,
        commands: CollectCommandsTuple<TInvokeHandler>,
    ) -> Exporter<TLang, Commands<TRuntime, TInvokeHandler>, TEvents> {
        Exporter {
            export_path: self.export_path,
            lang: self.lang,
            commands: Commands(commands, Default::default()),
            events: self.events,
            cfg: self.cfg,
            header: self.header,
        }
    }
}

impl<TLang, TCommands> Exporter<TLang, TCommands, NoEvents> {
    pub fn events(self, events: CollectEventsTuple) -> Exporter<TLang, TCommands, Events> {
        Exporter {
            export_path: self.export_path,
            lang: self.lang,
            events: Events(events),
            commands: self.commands,
            cfg: self.cfg,
            header: self.header,
        }
    }
}

impl<TLang, TCommands, TEvents> Exporter<TLang, TCommands, TEvents> {
    /// Allows for specifying a custom [`ExportConfiguration`](specta::ts::ExportConfiguration).
    pub fn cfg(mut self, cfg: specta::ts::ExportConfig) -> Self {
        self.cfg = ExportConfig::new(cfg);
        self
    }

    /// Allows for specifying a custom header to
    pub fn with_header(mut self, header: &'static str) -> Self {
        self.header = header.into();
        self
    }
}

pub struct PluginUtils<TCommands, TManager, TSetup>
where
    TCommands: CommandsTypeState,
    TManager: Manager<TCommands::Runtime>,
    TSetup: FnOnce(&TManager),
{
    pub invoke_handler: TCommands::InvokeHandler,
    pub setup: TSetup,
    phantom: PhantomData<TManager>,
}

const PLUGIN_NAME: &str = "tauri-specta";

impl<TLang, TCommands, TEvents> Exporter<TLang, TCommands, TEvents>
where
    TLang: ExportLanguage,
    TCommands: CommandsTypeState,
    TEvents: EventsTypeState,
{
    #[must_use]
    pub fn to_plugin(self) -> tauri::plugin::TauriPlugin<TCommands::Runtime> {
        let builder = tauri::plugin::Builder::new(PLUGIN_NAME);

        let plugin_utils = self.to_utils_for_plugin(PLUGIN_NAME);

        builder
            .invoke_handler(plugin_utils.invoke_handler)
            .setup(move |app| {
                (plugin_utils.setup)(app);

                Ok(())
            })
            .build()
    }

    #[must_use]
    pub fn to_utils_for_plugin<TManager>(
        mut self,
        plugin_name: &'static str,
    ) -> PluginUtils<TCommands, TManager, impl FnOnce(&TManager)>
    where
        TManager: Manager<TCommands::Runtime>,
    {
        let plugin_name = PluginName::new(plugin_name);

        self.cfg.plugin_name = plugin_name;

        let (invoke_handler, event_collection) = self.export_inner().unwrap();

        PluginUtils {
            invoke_handler,
            setup: move |app| {
                let registry = EventRegistry::get_or_manage(app);
                registry.register_collection(event_collection, plugin_name);
            },
            phantom: PhantomData,
        }
    }

    fn export_inner(self) -> Result<(TCommands::InvokeHandler, EventCollection), TsExportError> {
        let path = self.export_path.clone();
        let cfg = self.cfg.clone();

        let (rendered, ret) = self.render()?;

        if let Some(export_dir) = path.parent() {
            fs::create_dir_all(export_dir)?;
        }

        #[cfg(debug_assertions)]
        {
            let mut file = File::create(&path)?;

            write!(file, "{}", rendered)?;

            cfg.inner.run_format(path)?;
        }

        Ok(ret)
    }

    fn render(
        self,
    ) -> Result<(String, (TCommands::InvokeHandler, EventCollection)), TsExportError> {
        let Self {
            commands,
            cfg,
            header,
            events,
            ..
        } = self;

        let (macro_data, invoke_handler) = commands.split();
        let (commands, commands_type_map) = macro_data?;

        let (events_registry, events, events_type_map) = events.get();

        let rendered = TLang::render(
            &commands,
            &events?,
            &commands_type_map
                .into_iter()
                .chain(events_type_map)
                .collect(),
            &cfg,
        )?;

        Ok((
            format!("{header}{rendered}"),
            (invoke_handler, events_registry),
        ))
    }
}

type HardcodedRuntime = tauri::Wry;

impl<TLang, TCommands, TEvents> Exporter<TLang, TCommands, TEvents>
where
    TLang: ExportLanguage,
    TCommands: CommandsTypeState<Runtime = HardcodedRuntime>,
    TEvents: EventsTypeState,
{
    /// Exports the output of [`internal::render`] for a collection of [`FunctionDataType`] into a TypeScript file.
    pub fn export(self) -> Result<(), TsExportError> {
        self.export_for_plugin(PLUGIN_NAME)
    }

    pub fn export_for_plugin(mut self, plugin_name: &'static str) -> Result<(), TsExportError> {
        self.cfg.plugin_name = PluginName::new(plugin_name);

        self.export_inner().map(|_| ())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PluginName(&'static str);

pub(crate) enum ItemType {
    Event,
    Command,
}

impl Default for PluginName {
    fn default() -> Self {
        PluginName(PLUGIN_NAME)
    }
}

impl PluginName {
    pub fn new(plugin_name: &'static str) -> Self {
        Self(plugin_name)
    }

    pub fn apply_as_prefix(&self, s: &str, item_type: ItemType) -> String {
        format!(
            "plugin:{}{}{}",
            self.0,
            match item_type {
                ItemType::Event => ":",
                ItemType::Command => "|",
            },
            s,
        )
    }
}
